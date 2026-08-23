# Phase 1 Data Model: Single-Core DEFLATE Engine

**Feature Directory**: `specs/113-single-core-surpass-libdeflate`
**Created**: 2026-08-19
**Status**: Completed

---

## 1. Core Entities & Structures

### Entity: `CompressionContext`
Encapsulates pre-allocated match-finder structures, hash tables, bitstream emission accumulators, and hardware dispatch pipelines.

| Field Name | Type | Required | Constraints / Description |
| :--- | :--- | :--- | :--- |
| `level` | `integer` | Yes | Compression level, integer $\in [1, 9]$ |
| `strategy` | `string` | Yes | Enum: `"fast_greedy"`, `"lazy"`, `"near_optimal"` |
| `window_size` | `integer` | Yes | Sliding window size in bytes, fixed to `32768` for RFC 1951 |
| `mem_level` | `integer` | Yes | Memory level allocation factor $\in [1, 9]$ |
| `hardware_simd` | `string` | Yes | Enum: `"neon"`, `"sse4_1"`, `"avx2"`, `"scalar"` |

---

### Entity: `DecompressionContext`
Encapsulates pre-allocated 12-bit dual-symbol Huffman decoding tables, offset tables, and branchless 64-bit bitstream cursor state.

| Field Name | Type | Required | Constraints / Description |
| :--- | :--- | :--- | :--- |
| `window_size` | `integer` | Yes | Sliding window capacity in bytes, minimum `32768` |
| `lut_bits` | `integer` | Yes | Primary Huffman lookup table bit depth, fixed to `12` |
| `max_output_size` | `integer` | Yes | Maximum permissible uncompressed buffer capacity in bytes |
| `hardware_simd` | `string` | Yes | Enum: `"neon"`, `"sse4_1"`, `"avx2"`, `"scalar"` |

---

### Entity: `CompressionRequest`
Specifies input payload characteristics and compression parameters.

| Field Name | Type | Required | Constraints / Description |
| :--- | :--- | :--- | :--- |
| `data_size` | `integer` | Yes | Size of raw input stream in bytes, $\ge 0$ |
| `compression_level` | `integer` | Yes | Level $\in [1, 9]$ |
| `payload_type` | `string` | Yes | Enum: `"text"`, `"binary"`, `"structured"`, `"high_entropy"`, `"auto"` |
| `checksum_type` | `string` | Yes | Enum: `"crc32"`, `"adler32"`, `"none"` |

---

### Entity: `CompressionResult`
Reports execution metrics and output metadata for single-core compression.

| Field Name | Type | Required | Constraints / Description |
| :--- | :--- | :--- | :--- |
| `success` | `boolean` | Yes | Indicates successful compression execution |
| `bytes_in` | `integer` | Yes | Raw input byte count, $\ge 0$ |
| `bytes_out` | `integer` | Yes | Compressed output byte count, $\ge 0$ |
| `compression_ratio` | `number` | Yes | Uncompressed / Compressed ratio, $\ge 0.0$ |
| `throughput_mb_s` | `number` | Yes | Processing throughput in MB/s, $\ge 0.0$ |
| `duration_ns` | `integer` | Yes | Execution elapsed time in nanoseconds, $\ge 0$ |
| `checksum` | `integer` | Yes | 32-bit computed checksum value ($\in [0, 4294967295]$) |
| `error_code` | `string` | Yes | Enum: `"ok"`, `"invalid_argument"`, `"buffer_too_small"`, `"out_of_memory"`, `"internal_error"` |
| `error_message` | `string` | No | Diagnostic error description |

---

### Entity: `DecompressionRequest`
Specifies input compressed bitstream and target buffer parameters.

| Field Name | Type | Required | Constraints / Description |
| :--- | :--- | :--- | :--- |
| `compressed_size` | `integer` | Yes | Size of compressed DEFLATE payload in bytes, $\ge 0$ |
| `expected_uncompressed_size` | `integer` | Yes | Expected or allocated output buffer capacity in bytes, $\ge 0$ |
| `checksum_type` | `string` | Yes | Enum: `"crc32"`, `"adler32"`, `"none"` |

---

### Entity: `DecompressionResult`
Reports execution metrics and output validation for single-core decompression.

| Field Name | Type | Required | Constraints / Description |
| :--- | :--- | :--- | :--- |
| `success` | `boolean` | Yes | Indicates successful decompression execution |
| `bytes_in` | `integer` | Yes | Consumed compressed bytes, $\ge 0$ |
| `bytes_out` | `integer` | Yes | Emitted uncompressed bytes, $\ge 0$ |
| `throughput_mb_s` | `number` | Yes | Decompression throughput in MB/s, $\ge 0.0$ |
| `duration_ns` | `integer` | Yes | Execution elapsed time in nanoseconds, $\ge 0$ |
| `checksum` | `integer` | Yes | 32-bit computed checksum value ($\in [0, 4294967295]$) |
| `error_code` | `string` | Yes | Enum: `"ok"`, `"invalid_stream"`, `"bad_checksum"`, `"buffer_too_small"`, `"truncated_data"`, `"internal_error"` |
| `error_message` | `string` | No | Diagnostic error description |

---

### Entity: `BenchmarkMetricRecord`
Records benchmark comparison against libdeflate.

| Field Name | Type | Required | Constraints / Description |
| :--- | :--- | :--- | :--- |
| `test_id` | `string` | Yes | Unique test run identifier |
| `corpus_name` | `string` | Yes | Name of dataset corpus (e.g. `"silesia"`, `"enwik8"`) |
| `corpus_size_bytes` | `integer` | Yes | Dataset size in bytes |
| `algorithm` | `string` | Yes | Enum: `"ttzip_single_core_deflate"`, `"libdeflate"`, `"zlib_ng"` |
| `level` | `integer` | Yes | Compression level $\in [1, 9]$ |
| `compress_mb_s` | `number` | Yes | Single-core compression throughput |
| `decompress_mb_s` | `number` | Yes | Single-core decompression throughput |
| `ratio` | `number` | Yes | Compression ratio |
| `libdeflate_comp_ratio_delta` | `number` | Yes | Ratio percentage delta vs libdeflate ($\Delta\%$) |
| `libdeflate_comp_speed_delta` | `number` | Yes | Compression speed percentage delta vs libdeflate ($\Delta\%$) |
| `libdeflate_decomp_speed_delta` | `number` | Yes | Decompression speed percentage delta vs libdeflate ($\Delta\%$) |
