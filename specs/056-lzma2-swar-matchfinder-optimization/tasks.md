# Tasks for Feature 056: LZMA2 SWAR Match Finder Optimization

## Phase 1: Setup & Prerequisites
- [x] T001 [P] [US1] Verify build baseline and existing tests with `swift test --filter FastLZMA2Tests` in `Tests/TTZipTests/FastLZMA2Tests.swift`

## Phase 2: User Story 1 - SWAR Match Length Implementation
- [x] T002 [US1] Refactor `ttzip_match_len_neon` in `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c` to use 64-bit SWAR unaligned loads, `v1 ^ v2`, and `__builtin_ctzll`
- [x] T003 [US1] Verify bounds safety and fallback loop for `< 8` byte tails in `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c`
- [x] T004 [US1] Ensure header comments and prototype alignment in `Sources/CTTZipBridge/include/ttzip_lzma_hc4_neon.h`

## Phase 3: User Story 2 - Integration & Fast-Path Preservation
- [x] T005 [P] [US2] Verify non-zero data HC4 matching in `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`
- [x] T006 [P] [US2] Assert zero-block vector bypass integrity (`ttzip_is_block_all_zero_neon`) in `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`

## Phase 4: User Story 3 - Verification & Performance Floor Gates
- [x] T007 [US3] Execute full unit tests with `swift test --filter FastLZMA2Tests` and `swift test --filter SevenZipBridgeTests`
- [x] T008 [US3] Execute performance floor verification with `swift test --filter XCTestPerformanceMeasureTests`
- [x] T009 [US3] Perform contract compliance audit against `specs/056-lzma2-swar-matchfinder-optimization/contracts/lzma2_match_finder_contract.json`
