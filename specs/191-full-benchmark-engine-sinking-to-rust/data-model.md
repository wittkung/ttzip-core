# Data Model: 191-full-benchmark-engine-sinking-to-rust

## 1. Rust Benchmark Matrix Output Model
- **`BenchmarkMatrixSummary`**:
  - `timestamp_utc: String`
  - `total_points: usize`
  - `duration_ms: f64`
  - `results: Vec<CodecBenchmarkPointResult>`
  - `pareto_points: Vec<ParetoCodecPoint>`

## 2. Codec Benchmark Point Result
- **`CodecBenchmarkPointResult`**:
  - `codec_name: String`
  - `level: i32`
  - `original_size: usize`
  - `compressed_size: usize`
  - `compression_ratio: f64`
  - `compression_speed_mb_s: f64`
  - `decompression_speed_mb_s: f64`
  - `is_pareto: bool`
