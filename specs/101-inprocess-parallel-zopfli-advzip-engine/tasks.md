# Implementation Tasks: In-Process 18-Core Parallel Zopfli/Advzip Engine

**Feature**: `specs/101-inprocess-parallel-zopfli-advzip-engine`

## Phase 1: In-Process 18-Core C Engine & Orchestration
- [x] T001 [P] [US1] Expose in-process multi-pass Zopfli & dynamic block splitting C API in `Sources/CTTZipBridge/include/ttzip_zopfli_engine.h` and `Sources/CTTZipBridge/ttzip_zopfli_engine.c`
- [x] T002 [P] [US1] Implement 18-core concurrent block compression with 32KB window warmup in `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift` and remove all `Process()` calls
- [x] T003 [P] [US2] Expand `pigzLevels` in `Tests/TTZipTests/ZipMultiCoreParetoFrontierPkTests.swift` to all 11 native levels: `[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11]`

## Phase 2: Verification & Test Execution
- [x] T004 [US1] Run `swift test --filter ZipMultiCoreParetoFrontierPkTests` and verify all 11 pigz points + in-process TTZip 8 points
- [x] T005 [US2] Run `swift test` and assert all 525+ tests pass with zero regression
