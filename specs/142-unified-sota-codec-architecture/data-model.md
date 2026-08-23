# Data Model: Unified SOTA Codec Engine & Multi-Core Architecture

**Feature**: `142-unified-sota-codec-architecture`  
**Date**: 2026-08-20  
**Phase**: Phase 1 Design

---

## 1. Domain Entities Architecture

This data model specifies the four-layer decoupled architecture:
1. **Layer 0 (`CodecVTableDescriptor`)**: Single-core C ABI function pointers.
2. **Layer 1 (`ParallelSchedulerConfig`)**: Multi-core scheduler and chunking configuration.
3. **Layer 2 (`ContainerWriterDescriptor`)**: Container metadata and entry writers.
4. **Layer 3 (`DualTrackWorkloadProfile`)**: Adaptive routing and topology context.

---

## 2. Entity Specifications

### 2.1 `CodecVTableDescriptor`
Represents the Layer 0 single-core C ABI interface for a compression engine.

| Field Name | Type | Required | Constraints / Enums | Description |
| :--- | :--- | :---: | :--- | :--- |
| `codecId` | `String` | Yes | Enum: `"deflate"`, `"zstd"`, `"lzma2"`, `"lz4"`, `"bzip2"`, `"brotli"`, `"lzfse"`, `"snappy"`, `"store"` | Unique codec identifier. |
| `engineName` | `String` | Yes | Non-empty string (e.g., `"libdeflate"`, `"fast-lzma2"`, `"libzstd"`) | Name of underlying SOTA library. |
| `license` | `String` | Yes | Enum: `"MIT"`, `"BSD-3-Clause"`, `"BSD-2-Clause"`, `"Apache-2.0"`, `"Public-Domain"` | Open source license. |
| `supportsDictionaryPriming`| `Boolean` | Yes | Boolean | Whether compressor accepts trailing history buffers. |
| `maxHistoryWindowBytes`| `Integer` | Yes | $\ge 0$ | Maximum history dictionary size in bytes. |
| `singleCoreThroughputMBps`| `Double` | Yes | $> 0.0$ | Measured single-core compression throughput. |

---

### 2.2 `ParallelSchedulerConfig`
Represents Layer 1 multi-core chunk dispatch and memory management configuration.

| Field Name | Type | Required | Constraints / Enums | Description |
| :--- | :--- | :---: | :--- | :--- |
| `schedulerId` | `String` | Yes | Non-empty string | Scheduler instance identifier. |
| `chunkSizeBytes` | `Integer` | Yes | Range: `[65536, 16777216]` (64KB to 16MB) | Default chunk size in bytes. |
| `overlapSizeBytes` | `Integer` | Yes | Range: `[0, 2097152]` (0 to 2MB) | Trailing dictionary overlap window size. |
| `numWorkerThreads` | `Integer` | Yes | Range: `[1, 128]` | Number of worker threads allocated. |
| `enableAsymmetricSizing`| `Boolean` | Yes | Boolean | Whether P-core vs E-core asymmetric chunking is active. |
| `maxResidentMemoryMB` | `Integer` | Yes | Range: `[16, 1024]` | Hard cap on memory buffer pool allocation. |

---

### 2.3 `ContainerWriterDescriptor`
Represents Layer 2 decoupled container format specifications.

| Field Name | Type | Required | Constraints / Enums | Description |
| :--- | :--- | :---: | :--- | :--- |
| `formatId` | `String` | Yes | Enum: `"sevenZip"`, `"zip"`, `"tar"`, `"tarZst"`, `"tarGz"`, `"tarBz2"`, `"tarXz"`, `"dmg"`, `"wim"`, `"iso"` | Archive container format identifier. |
| `containerParadigm` | `String` | Yes | Enum: `"randomAccessSeekable"`, `"sequentialStreaming"`, `"sectorIndexedDiskImage"` | Structural container layout paradigm. |
| `supportedCodecs` | `Array<String>` | Yes | Array of valid `codecId` strings | Allowed underlying codecs. |
| `bitstreamSequencerType`| `String` | Yes | Enum: `"rfc1951DeflateBfinal"`, `"lzma2ChunkReset"`, `"zstdFrameConcatenation"`, `"rawPassThrough"` | Bitstream closure and sequencing strategy. |

---

### 2.4 `DualTrackWorkloadProfile`
Represents Layer 3 workload classification and core dispatch parameters.

| Field Name | Type | Required | Constraints / Enums | Description |
| :--- | :--- | :---: | :--- | :--- |
| `workloadType` | `String` | Yes | Enum: `"smallFileBatch"`, `"largeContinuousStream"`, `"heterogeneousMix"` | Workload classification. |
| `totalFileCount` | `Integer` | Yes | $\ge 0$ | Total number of files in archive batch. |
| `totalSizeBytes` | `Integer` | Yes | $\ge 0$ | Total uncompressed input bytes. |
| `allocatedTrack` | `String` | Yes | Enum: `"fileLevelWorkerPool"`, `"chunkLevelWorkerPool"`, `"dualTrackCoordinated"` | Target scheduler execution track. |
| `pCoreChunkSizeBytes`| `Integer` | Yes | $\ge 65536$ | Chunk size assigned to Performance cores. |
| `eCoreChunkSizeBytes`| `Integer` | Yes | $\ge 65536$ | Chunk size assigned to Efficiency cores. |
