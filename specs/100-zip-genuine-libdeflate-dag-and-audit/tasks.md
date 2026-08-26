# Implementation Tasks: Genuine Libdeflate DAG Routing & Codebase Disconnect Audit

**Feature**: `specs/100-zip-genuine-libdeflate-dag-and-audit`

## Phase 1: Core C Bridge Fixes
- [x] T001 [P] [US1] Remove `(level == 6 ? 4 : level)` and fix `ttzip_get_tls_compressor` in `Sources/CTTZipBridge/CTTZipStreamCoder.c`
- [x] T002 [P] [US1] Refactor `ttzip_raw_deflate_block_compress` to use pure `libdeflate_deflate_compress` in `Sources/CTTZipBridge/CTTZipStreamCoder.c`
- [x] T003 [P] [US2] Fix `total_entries` to `uint64_t` in `Sources/CTTZipBridge/CTTZipExtract.c`
- [x] T004 [P] [US2] Fix `bz_level` and `z_level` clamping in `Sources/CTTZipBridge/CTTZipBridge_GzParallel.c`
- [x] T005 [P] [US3] Add thread-safe `dispatch_once` initialization for CRC32C table in `Sources/CTTZipBridge/CTTZipBridge_Snappy.c`
- [x] T006 [P] [US3] Fix `ArchiveWriter+Dispatch.swift` fallback branch to use `level.rawValue` instead of `advancedOptions.zstdLevel`

## Phase 2: Verification & Test Execution
- [ ] T007 [US1] Run `swift test --filter ZipMultiCoreParetoFrontierPkTests` and verify all 7 genuine tiers
- [ ] T008 [US2] Run `swift test` and assert all 525+ tests pass with zero regression
