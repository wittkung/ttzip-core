# Data Model: zlib-ng Streaming Fallback Engine

**Feature**: `076-zlib-ng-streaming-fallback`  
**Created**: 2026-08-18  
**Status**: Stable  

---

## 1. Enums & Value Types

### 1.1 `DeflateTierMode`
Represents the architectural execution tier for Deflate operations.

| Value | Raw Value (`Int32`) | Description |
| :--- | :--- | :--- |
| `tier1Block` | `1` | Tier 1: Whole-Buffer fast-path powered by `libdeflate` with zero heap allocation. |
| `tier2Stream` | `2` | Tier 2: State-machine incremental streaming pipeline powered by `zlib-ng` SIMD. |

---

### 1.2 `DeflateWindowBits`
Specifies the sliding window size and container framing format according to RFC specifications.

| Value | Raw Value (`Int32`) | Format Specification |
| :--- | :--- | :--- |
| `raw` | `-15` | RFC 1951 Raw Deflate bitstream (no headers or footers, standard for ZIP entries). |
| `zlib` | `15` | RFC 1950 Zlib container format with 2-byte header and 4-byte Adler-32 checksum. |
| `gzip` | `31` | RFC 1952 GZIP container format with 10-byte header and 8-byte CRC-32/ISIZE footer. |

---

### 1.3 `DeflateStrategy`
Defines compression tuning heuristics for specific input entropy distributions.

| Value | Raw Value (`Int32`) | Optimization Strategy |
| :--- | :--- | :--- |
| `defaultStrategy` | `0` | Standard balance between match finding (LZ77) and dynamic Huffman coding. |
| `filtered` | `1` | Optimized for data filtered by predictor algorithms (e.g., PNG/audio deltas). |
| `huffmanOnly` | `2` | Bypasses match finder completely, performing only Huffman tree entropy encoding. |
| `rle` | `3` | Restricts match search to run-length repetitions (fastest for repetitive bitmaps). |
| `fixed` | `4` | Uses static pre-defined Huffman tables, saving dynamic tree construction time. |

---

### 1.4 `DeflateFlushMode`
Controls stream state synchronization and dictionary boundary emission.

| Value | Raw Value (`Int32`) | String Value | Flushing Behavior |
| :--- | :--- | :--- | :--- |
| `noFlush` | `0` | `"NO_FLUSH"` | Default streaming mode; emits bytes only when output buffer fills. |
| `syncFlush` | `2` | `"SYNC_FLUSH"` | Flushes all pending output to byte boundary and emits empty stored block. |
| `fullFlush` | `3` | `"FULL_FLUSH"` | Flushes all pending output and resets compression dictionary context. |
| `finish` | `4` | `"FINISH"` | Flushes all remaining stream data and terminates RFC stream bitstream. |

---

## 2. Configuration & Metric Structures

### 2.1 `DeflateStreamConfig`
Holds configuration parameters for initializing streaming compressor/decompressor sessions.

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `tierMode` | `DeflateTierMode` | Required | Specifies execution tier (`tier1Block` or `tier2Stream`). |
| `compressionLevel` | `Int` | `1` ... `9` (Default: `6`) | Deflate compression effort level. |
| `windowBits` | `Int` | `-15`, `15`, `31` | Container framing and window bit depth. |
| `memLevel` | `Int` | `1` ... `9` (Default: `8`) | Internal memory allocation scale for hash tables. |
| `strategy` | `DeflateStrategy` | Required (Default: `.defaultStrategy`) | Entropy encoding strategy. |

---

### 2.2 `DeflateStreamMetrics`
Provides an immutable snapshot of runtime throughput and hardware checksum progress.

| Field Name | Type | Description |
| :--- | :--- | :--- |
| `totalIn` | `UInt64` | Cumulative uncompressed input bytes consumed across all chunks. |
| `totalOut` | `UInt64` | Cumulative compressed output bytes produced across all chunks. |
| `adler32` | `UInt32` | Running Adler-32 checksum (computed in SIMD). |
| `crc32` | `UInt32` | Running CRC-32 checksum (computed via ARMv8/PCLMUL hardware instructions). |
| `isFinished` | `Bool` | `true` if `Z_STREAM_END` has been emitted and stream is cleanly terminated. |

---

## 3. C-Level State Structures (Native Bridge Plane)

### 3.1 `ttzip_deflate_stream_state_t`
Memory layout for opaque streaming state in `Sources/CTTZipBridge/include/CTTZipStreamCoder.h`.

```c
#define TTZIP_DEFLATE_STREAM_MAGIC 0x545A4453U // 'TZDS'

typedef struct ttzip_deflate_stream_state {
    uint32_t magic;              // Invariant verification (TTZIP_DEFLATE_STREAM_MAGIC)
    uint32_t tier_mode;          // TTZIP_DEFLATE_TIER_BLOCK (1) or TTZIP_DEFLATE_TIER_STREAM (2)
    uint64_t total_in;           // Total consumed bytes
    uint64_t total_out;          // Total produced bytes
    uint32_t adler32_checksum;   // Running Adler-32
    uint32_t crc32_checksum;     // Running CRC-32
    bool is_finished;            // Stream termination indicator
    int32_t last_status;         // zlib return code (Z_OK, Z_STREAM_END, Z_BUF_ERROR)
    void* internal_state;        // Opaque pointer to z_stream
} ttzip_deflate_stream_state_t;
```

---

### 3.2 `ttzip_hardware_capabilities_t`
Hardware feature detection flags populated by CPUID / sysctl routines.

```c
typedef struct {
    bool has_arm_neon;           // ARM NEON vector instructions available
    bool has_arm_crc32;          // ARMv8 hardware CRC32 instruction available
    bool has_x86_avx2;           // x86 AVX2 256-bit SIMD available
    bool has_x86_avx512;         // x86 AVX-512 foundation available
    bool has_x86_vpclmul;        // x86 PCLMULQDQ / VPCLMULQDQ available
} ttzip_hardware_capabilities_t;
```
