# Tasks: 096-full-codebase-standards-and-defensive-hardening

## Phase 1: User Story 1 - C Bridge Warning Elimination & Defensive Hardening (Priority: P1)

**Story Goal**: Eliminate all compiler warnings in C bridge files and apply struct magic verification and free-poisoning.
**Independent Test**: `swift build` produces clean output with 0 warnings.

- [x] T001 [P] [US1] Fix unused parameter warnings in `Sources/CTTZipBridge/CTTZipSpawnPipelines.c` and `Sources/CTTZipBridge/CTTZipSliceProfiler.c`
- [x] T002 [P] [US1] Fix unused variable warnings in `Sources/CTTZipBridge/CTTZipFilterPipeline.c` and `Sources/CTTZipBridge/CTTZipExtract.c`
- [x] T003 [P] [US1] Apply `TTZIP_POISON_FREE` struct sentinel poisoning on destruction in `Sources/CTTZipBridge/CTTZipStreamCoder.c`

---

## Phase 2: User Story 2 - C Bridge Header Hoare Triple Standardization (Priority: P1)

**Story Goal**: Standardize Doxygen Hoare Triple annotations and `TTZIP_API` exports across all bridge headers.
**Independent Test**: Code inspection and `swift build`.

- [x] T004 [P] [US2] Standardize Doxygen contracts in `Sources/CTTZipBridge/include/ttzip_7z_header_parser.h` and `Sources/CTTZipBridge/include/ttzip_tar_native.h`
- [x] T005 [P] [US2] Standardize Doxygen contracts in `Sources/CTTZipBridge/include/CTTZipChecksum.h` and `Sources/CTTZipBridge/include/CTTZipSuperChunk.h`
- [x] T006 [P] [US2] Standardize Doxygen contracts in `Sources/CTTZipBridge/include/CTTZipHeuristicTuner.h` and `Sources/CTTZipBridge/include/CTTZipPluginRegistry.h`

---

## Phase 3: User Story 3 - Swift Core Layer DocC & Pointer Invariants (Priority: P2)

**Story Goal**: Standardize DocC Design-by-Contract annotations and verify pointer safety in Swift core layer.
**Independent Test**: `swift test --filter CoreEngineExhaustiveCoverageTests`.

- [x] T007 [P] [US3] Standardize DocC annotations in `Sources/TTZipCore/Crypto/HardwareChecksumAdapter.swift`
- [x] T008 [P] [US3] Verify pointer bounds and DocC annotations in `Sources/TTZipCore/Adapters/CUnsafeBufferAdapter.swift`

---

## Phase 4: User Story 4 - SPDX Compliance & Zero-Regression Performance Gates (Priority: P2 & P3)

**Story Goal**: Verify 100% repository-wide SPDX header compliance and pass all 13 constitutional performance gates.
**Independent Test**: `swift test --filter XCTestPerformanceMeasureTests` and full `swift test`.

- [x] T009 [P] [US4] Audit and assert 100% SPDX header coverage across all Swift, C, and header files
- [x] T010 [US4] Execute full test suite regression and verify all 13 constitutional performance throughput floors
