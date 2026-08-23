# Data Model: 全覆盖测试与基准遥测零回退体系 (Feature 162)

## 1. Codec Benchmark Point Result (`CodecBenchmarkPoint`)

Represents a single deterministic benchmark evaluation of a specific compression codec on a standardized corpus.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `engine_name` | `string` | No | Codec engine identifier (e.g. `libdeflate`, `zstd`, `lz4`, `lzfse`, `snappy`, `brotli`, `bzip2`, `blosclz`) |
| `corpus_type` | `string` | No | One of: `text`, `short_match`, `dna`, `random`, `literals`, `mixed`, `realistic_rgb`, `striped_rgb` |
| `payload_size_bytes` | `integer` | No | Input size in bytes (e.g. 131072, 1048576) |
| `compression_level` | `integer` | No | Compression level (1 to 19 depending on engine) |
| `compressed_size_bytes` | `integer` | No | Compressed output size in bytes |
| `compression_ratio_pct` | `number` | No | `(compressed_size / payload_size) * 100.0` |
| `compress_throughput_mbs` | `number` | No | Compression throughput in MB/s |
| `compress_cpb` | `number` | No | Cycles per byte during compression |
| `decompress_throughput_mbs` | `number` | No | Decompression throughput in MB/s |
| `decompress_cpb` | `number` | No | Cycles per byte during decompression |
| `integrity_verified` | `boolean` | No | True if `memcmp(raw, decomp, size) == 0` |

---

## 2. Container Format Benchmark Result (`FormatBenchmarkPoint`)

Represents an end-to-end container packaging and extraction performance measurement on a multi-file tree.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `format_name` | `string` | No | Container format: `zip_store`, `zip_deflate`, `tar_gz`, `tar_zst`, `tar_bz2`, `tar_xz`, `7z_solid`, `unrar` |
| `file_count` | `integer` | No | Total number of files packaged/extracted (e.g. 500, 10000) |
| `uncompressed_total_bytes` | `integer` | No | Total uncompressed payload in bytes |
| `archive_size_bytes` | `integer` | No | Packaged archive file size on disk |
| `package_duration_ms` | `number` | No | Packaging wall-clock time in milliseconds |
| `package_throughput_mbs` | `number` | No | Packaging throughput in MB/s |
| `extract_duration_ms` | `number` | No | Extraction wall-clock time in milliseconds |
| `extract_throughput_mbs` | `number` | No | Extraction throughput in MB/s |
| `peak_rss_mb` | `number` | No | Peak resident set size (RSS) in megabytes |
| `lossless_verified` | `boolean` | No | True if all extracted file hashes match source tree |

---

## 3. Zero-Regression Gate Summary (`GateReport`)

Structured report recording the sequential outcome of the 5-gate pipeline.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `timestamp` | `string` | No | ISO 8601 UTC timestamp |
| `platform` | `string` | No | Architecture and OS (e.g. `arm64-apple-darwin`) |
| `total_gates` | `integer` | No | Fixed value `5` |
| `passed_gates` | `integer` | No | Number of successfully passed gates |
| `failed_gates` | `integer` | No | Number of failed gates (0 for green build) |
| `total_duration_sec` | `number` | No | Total wall-clock time across all 5 gates |
| `overall_verdict` | `string` | No | `PASS` or `FAIL` |
| `stages` | `array<GateStageResult>` | No | 5 sequential stage detail records |
