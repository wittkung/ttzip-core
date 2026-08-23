# Data Model: Blosc2 Cache-Aware Batch Compression Pipeline

**Feature**: `085-blosc2-cache-aware-batch-compression`
**Date**: 2026-08-18
**Status**: Ready

---

## 1. Domain Entities & Schemas

### 1.1 `BatchWorkUnit`
Represents an aggregated batch of small files scheduled as a single atomic unit to one CPU core for cache-hot sequential processing.

| Field Name | Type | Required | Description | Invariants & Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `batchId` | `integer` (uint32) | Yes | Sequential 0-based identifier of the batch unit | `batchId >= 0` |
| `itemIndices` | `array<integer>` | Yes | Array of 0-based indices pointing to items in the global archive item list | `1 <= count <= 64` |
| `totalUncompressedBytes` | `integer` (uint64) | Yes | Aggregated uncompressed payload size of all files in this batch | `1 <= totalUncompressedBytes <= 262144` (256KB) |
| `arenaOffset` | `integer` (uint64) | Yes | Byte offset where this batch's staging buffer starts in the aligned payload arena | Must be 128-byte aligned (`arenaOffset % 128 == 0`) |
| `arenaCapacity` | `integer` (uint64) | Yes | Bounded output capacity allocated in the arena for this batch | Must be 128-byte aligned (`arenaCapacity % 128 == 0`) |

---

### 1.2 `ArchiveItemMetadata`
Represents the complete POSIX and archive container metadata for a single file or directory item.

| Field Name | Type | Required | Description | Invariants & Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `itemIndex` | `integer` (uint32) | Yes | 0-based index in the global item collection | `itemIndex >= 0` |
| `sourcePath` | `string` | Yes | Absolute or relative filesystem path on disk | Max length 4096 bytes, non-empty |
| `relativePath` | `string` | Yes | Relative path stored in archive headers | Max length 4096 bytes, UTF-8 normalized |
| `isDirectory` | `boolean` | Yes | Flag indicating if this entry is a directory | If true, `uncompressedSize == 0` |
| `uncompressedSize` | `integer` (int64) | Yes | Original uncompressed file size in bytes | `>= 0` |
| `compressedSize` | `integer` (int64) | Yes | Actual compressed payload size in bytes | `>= 0` |
| `crc32Checksum` | `integer` (uint32) | Yes | Hardware-computed IEEE 802.3 CRC32 checksum | Exact 32-bit unsigned integer |
| `compressionMethod` | `integer` (uint16) | Yes | Desired compression method (0 = Store, 8 = Deflate) | `0` or `8` |
| `actualMethod` | `integer` (uint16) | Yes | Final method used after compression ratio evaluation | `0` or `8` |
| `tierClassification` | `string` (enum) | Yes | Size tier classification for execution routing | `"small"` (<64KB), `"medium"` (64KB~16MB), `"large"` (>16MB) |
| `headerOffset` | `integer` (uint64) | Yes | Byte offset of Local File Header in output file | `>= 0` |

---

### 1.3 `CacheAwarePayloadArena`
Represents the memory arena pre-allocated with 128-byte hardware cache-line alignment to hold both raw inputs and compressed outputs without per-file heap allocation.

| Field Name | Type | Required | Description | Invariants & Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `basePointerAddress` | `integer` (uint64) | Yes | Virtual memory address of the allocated arena buffer | Must satisfy `basePointerAddress % 128 == 0` |
| `totalAllocatedBytes` | `integer` (uint64) | Yes | Total physical byte size of the arena | Must be a multiple of 128 bytes |
| `alignmentBytes` | `integer` (uint32) | Yes | Hardware cache-line alignment boundary | Exact `128` on Apple Silicon |
| `totalBatchUnits` | `integer` (uint32) | Yes | Total number of `BatchWorkUnit` instances assigned to this arena | `>= 1` |
| `isLocked` | `boolean` | Yes | Atomic lifecycle state flag | `true` during compression, `false` upon deallocation |

---

### 1.4 `BatchCompressionExecutionResult`
Represents the structured result returned upon completing batch archive creation.

| Field Name | Type | Required | Description | Invariants & Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `outputArchivePath` | `string` | Yes | Final path of the created archive file | Non-empty, verified existing on disk |
| `format` | `string` (enum) | Yes | Archive container format | `"zip"`, `"7z"`, `"tar.zst"`, `"tar.gz"` |
| `totalFilesProcessed` | `integer` (uint32) | Yes | Total number of files successfully compressed | `>= 0` |
| `totalUncompressedBytes` | `integer` (uint64) | Yes | Total uncompressed payload volume | `>= 0` |
| `totalCompressedBytes` | `integer` (uint64) | Yes | Total compressed archive size on disk | `>= 0` |
| `executionDurationSeconds` | `number` (float64) | Yes | Total elapsed time in seconds | `> 0.0` |
| `sustainedThroughputMBs` | `number` (float64) | Yes | Calculated throughput in MB/s | `sustainedThroughputMBs == (totalUncompressedBytes / 1048576) / executionDurationSeconds` |
| `smallFileBatchCount` | `integer` (uint32) | Yes | Number of `BatchWorkUnit` batches executed | `>= 0` |

---

## 2. Memory Layout & Cacheline Alignment Rules

```
+---------------------------------------------------------------------------------------------------+
| CacheAwarePayloadArena (Allocated via posix_memalign, 128-byte aligned)                           |
+---------------------------------------------------------------------------------------------------+
| BatchUnit 0 Slot (128KB~256KB)  | BatchUnit 1 Slot (128KB~256KB)  | ... | BatchUnit N Slot        |
| [128-byte aligned offset]       | [128-byte aligned offset]       |     | [128-byte aligned]      |
+---------------------------------+---------------------------------+-----+-------------------------+
| [File 0: Output + 128B Pad]     | [File 32: Output + 128B Pad]    |     |                         |
| [File 1: Output + 128B Pad]     | [File 33: Output + 128B Pad]    |     |                         |
| ...                             | ...                             |     |                         |
| [File 31: Output + 128B Pad]    | [File 63: Output + 128B Pad]    |     |                         |
+---------------------------------+---------------------------------+-----+-------------------------+
```

### Invariant Equations
1. **Slot Offset Calculation**:
   $$\text{offset}_{i+1} = (\text{offset}_i + \text{bound}_i + 127) \ \& \ \sim 127$$
2. **Worker Core Cache Residency**:
   $$\text{Slot Size} \le \text{Apple Silicon P-Core L1D (128KB)} \lor \text{L2 Cluster (16MB)}$$
3. **Zero False Sharing**:
   $$\forall i \ne j, \quad \left\lfloor \frac{\text{offset}_i}{128} \right\rfloor \ne \left\lfloor \frac{\text{offset}_j}{128} \right\rfloor$$
