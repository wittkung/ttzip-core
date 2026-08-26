# Data Model: 175-sink-streaming-concurrency-dsl-bench-to-rust

## 1. 7z Solid In-Memory Stream Models
- **`SevenZEntryLocation`**:
  - `folder_index: usize`
  - `unpack_offset_in_folder: u64`
  - `uncompressed_size: u64`
  - `crc32: Option<u32>`
  - `is_directory: bool`
  - `is_empty_stream: bool`

## 2. Archive Filter DSL & Glob Models
- **`FilterExpr<'a>`**:
  - `MatchAll`
  - `MatchNone`
  - `FilenameGlob(&'a str)`
  - `Extension(Vec<&'a str>)`
  - `Size(ComparisonOp, u64)`
  - `Modified(ComparisonOp, i64)`
  - `And(Box<FilterExpr<'a>>, Box<FilterExpr<'a>>)`
  - `Or(Box<FilterExpr<'a>>, Box<FilterExpr<'a>>)`
  - `Not(Box<FilterExpr<'a>>)`
- **`PathPatternFilter`**:
  - `include_set: Option<GlobSet>`
  - `exclude_set: Option<GlobSet>`
  - `exclude_vcs: bool`
  - `no_mac_metadata: bool`

## 3. Benchmark & Pareto Models
- **`HardwareBenchmarkMetric`**:
  - `dictionary_size_mb: u32`
  - `thread_count: u32`
  - `compress_mips: f64`
  - `decompress_mips: f64`
  - `total_mips: f64`
  - `compress_speed_mbs: f64`
  - `decompress_speed_mbs: f64`
  - `cpu_usage_percent: f64`
  - `rating_per_usage_mips: f64`
- **`ParetoPoint`**:
  - `id: String`
  - `algorithm: String`
  - `level: i32`
  - `throughput_mbs: f64`
  - `space_savings_pct: f64`
  - `compressed_bytes: u64`
  - `uncompressed_bytes: u64`
  - `pareto_rank: usize`
  - `is_pareto_optimal: bool`
  - `is_on_convex_envelope: bool`
