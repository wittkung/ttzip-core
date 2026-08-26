# Tasks for Feature 057: Global 64-bit SWAR Acceleration & Benchmarking

## Phase 1: Setup & Baseline Benchmark
- [x] T001 [P] [US1] Create benchmark test harness `Tests/TTZipTests/SwarOptimizationBenchmarkTests.swift` to measure baseline performance of encoding detection and header sniffing

## Phase 2: User Story 1 - SWAR Encoding Detection
- [x] T002 [US1] Refactor `ttzip_detect_encoding_fast` in `Sources/CTTZipBridge/CTTZipUtils.c` to use 64-bit SWAR 8-byte ASCII loop (`(v & 0x8080808080808080ULL) == 0`) with safe tail fallback
- [x] T003 [US1] Verify encoding detection correctness across pure ASCII, UTF-8 Chinese, and GB18030 with `swift test --filter CharsetDetectorTests`

## Phase 3: User Story 2 - Format Header Sniffing Optimization
- [x] T004 [US2] Refactor `ttzip_detect_format_from_header` in `Sources/CTTZipBridge/ttzip_native_archive.c` to replace `memcmp` with direct unaligned integer comparisons for 7z, TAR, XZ, ZSTD, and LZ4
- [x] T005 [US2] Verify format sniffing correctness with `swift test --filter FormatSupportTests` and `swift test --filter SevenZipBridgeTests`

## Phase 4: User Story 3 - Comprehensive Benchmarking & Performance Gate
- [x] T006 [US3] Run `SwarOptimizationBenchmarkTests` to measure and record optimized performance metrics and calculate speedup ratios
- [x] T007 [US3] Run full performance gate verification `swift test --filter XCTestPerformanceMeasureTests` to assert zero performance regression
- [x] T008 [US3] Perform contract compliance audit against `specs/057-swar-global-acceleration-and-benchmarking/contracts/swar_global_acceleration_contract.json`
