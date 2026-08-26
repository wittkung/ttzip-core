# Data Model: Libdeflate Architecture Integration & Unified Streaming

**Feature**: [062-libdeflate-architecture-integration](spec.md)

---

## 1. Core Structures and C ABI Models

### A. Chunk Streaming State (`ttzip_deflate_chunk_engine_t`)

Represents the internal engine state replacing the legacy `z_stream` pointer inside `ttzip_deflate_stream_state_t`.

| Field Name | Type | Description | Invariant / Constraints |
| :--- | :--- | :--- | :--- |
| `magic` | `uint32_t` | State integrity identifier (`TTZIP_DEFLATE_STREAM_MAGIC`) | Must equal `0x54545A53` when active, 0 after destruction |
| `window_bits` | `int32_t` | Stream format identifier (-15: Deflate, 15: Zlib, 31: Gzip) | Constrained to valid RFC window bit values |
| `level` | `int32_t` | Compression level | 1 to 12 |
| `stage_buf` | `uint8_t*` | Staging buffer for chunked stream accumulation | Fixed capacity (256KB or 1MB), strictly bounded |
| `stage_len` | `size_t` | Currently occupied byte length in staging buffer | `0 <= stage_len <= stage_cap` |
| `stage_cap` | `size_t` | Allocated capacity of `stage_buf` | Typically 262,144 (256KB) or 1,048,576 (1MB) |
| `running_crc32` | `uint32_t` | Hardware computed running CRC-32 | Initialized to 0, updated via `libdeflate_crc32` |
| `running_adler32`| `uint32_t` | Hardware computed running Adler-32 | Initialized to 1, updated via `libdeflate_adler32` |
| `is_finished` | `bool` | True if end of stream was reached | Transition to true on final block flush |

### B. 7Z DEFLATE Block Decoding Context

| Field Name | Type | Description | Constraints |
| :--- | :--- | :--- | :--- |
| `method_id` | `uint64_t` | 7z compression method identifier | Raw Deflate: `0x040108` or `0x40108` |
| `compressed_src` | `const uint8_t*` | Pointer to raw RFC 1951 stream payload | Non-null, bounded by input block length |
| `compressed_len` | `size_t` | Compressed payload byte count | $> 0$ |
| `unpack_buf` | `uint8_t*` | Output destination buffer | Preallocated to exact uncompressed size |
| `unpack_capacity`| `size_t` | Destination capacity | Matches expected uncompressed size |

---

## 2. Swift Data Models & Adapters

### `DeflateEngineMode`
```swift
public enum DeflateEngineMode: Int32, Sendable {
    case rawDeflate = -15
    case zlibWrapped = 15
    case gzipWrapped = 31
}
```

### `ChunkedDeflateStreamMetrics`
```swift
public struct ChunkedDeflateStreamMetrics: Sendable {
    public let uncompressedBytes: Int64
    public let compressedBytes: Int64
    public let finalCrc32: UInt32
    public let finalAdler32: UInt32
    public let compressionRatio: Double
}
```
