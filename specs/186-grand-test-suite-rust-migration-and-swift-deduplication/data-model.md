# Data Model: 186-grand-test-suite-rust-migration-and-swift-deduplication

## 1. Test Matrix Dimensions (`rust/ttzip-glue/tests/`)
- **`FormatTestVector`**:
  - `format_name: String`
  - `compression_level: i32`
  - `encryption: Option<String>`
  - `split_volumes: bool`
  - `expected_checksum_match: bool`

## 2. Test Report Metrics
- **`SuiteSummary`**:
  - `rust_test_count: usize`
  - `swift_test_count: usize`
  - `total_runtime_seconds: f64`
  - `failures: usize`
