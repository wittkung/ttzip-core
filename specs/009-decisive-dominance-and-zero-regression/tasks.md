# Implementation Tasks: Feature 009 (Decisive Dominance & Zero Regression)

## Phase 1: Setup & Environment Validation
- [x] Task 1.1: Verify benchmark environment and clean test disk cache <!-- id: 1.1 -->

## Phase 2: User Story 1 (US1) - 7Z 500MB L1 显著超越 (>= 5,500 MB/s)
- [x] Task 2.1 [P]: In `ttzip_lzma2_enc_native.c` and `ttzip_lzma2_fast_encoder.c`, tune 500MB large stream block sizing and HC3 zero-stream dictionary depth for >= 5,500 MB/s. <!-- id: 2.1 -->
- [x] Task 2.2 [P]: Verify 7Z 500MB L1 no-encryption and AES-256 compression exceed 7zz by >= 5%. <!-- id: 2.2 -->

## Phase 3: User Story 2 (US2) - TAR.ZST Direct In-Process 解压突破 (>= 7,000 MB/s)
- [x] Task 3.1 [P]: Implement direct `ZSTD_decompressStream` and tar parser in `ttzip_tar_zstd_direct.c`. <!-- id: 3.1 -->
- [x] Task 3.2 [P]: Direct incompressibility bypass for high-entropy payloads in `ttzip_tar_zstd_direct.c`. <!-- id: 3.2 -->

## Phase 4: User Story 3 (US3) - 全格式全量对决与零倒退断言
- [x] Task 4.1: Run `AllFormatsPkSuiteTests` and verify 92/92 dominance. <!-- id: 4.1 -->
- [x] Task 4.2: Run `python3 scripts/audit_performance_regression.py` and verify ZERO regressions > 10%. <!-- id: 4.2 -->
- [x] Task 4.3: Verify 11 performance gates with `swift test --filter XCTestPerformanceMeasureTests`. <!-- id: 4.3 -->
- [x] Task 4.4: Verify full test suite with `./scripts/run_all_tests.sh`. <!-- id: 4.4 -->

