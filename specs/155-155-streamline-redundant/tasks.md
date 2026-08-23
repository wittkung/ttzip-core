# Tasks: Streamlining Redundant Swift C-Wrapper Tests

**Feature**: `155-155-streamline-redundant`  
**Input**: Design artifacts from `specs/155-155-streamline-redundant/` (`plan.md`, `spec.md`, `data-model.md`, `contracts/`, `research.md`, `quickstart.md`)  
**Status**: Completed (100% Verified)  

---

## Phase 1: Setup (Pre-Flight Verification)

**Purpose**: Confirm native CTest baseline before touching Swift test files

- [x] T001 Verify all 9 CTest suites pass green via `ctest --test-dir build --output-on-failure`
- [x] T002 Verify Swift test build baseline via `swift build --build-tests`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Confirm retention of Swift architectural pattern tests and ConcurrencyBridge

**⚠️ CRITICAL**: Must not prune any GoF design pattern tests or Swift 6 concurrency tests

---

## Phase 3: User Story 1 - Prune Redundant C-Wrapper Swift Tests (Priority: P1) 🎯 MVP

**Goal**: Cleanly delete the 7 redundant C-wrapper Swift test files whose invariant coverage is 100% asserted in `tests/c/test_*.c`.

**Independent Test**: Run `swift build --build-tests` and confirm successful compilation with 0 errors.

### Implementation for User Story 1

- [x] T003 [P] [US1] Remove redundant Zip-Slip C wrapper test in `Tests/TTZipTests/ZipSlipDefenseTests.swift`
- [x] T004 [P] [US1] Remove redundant Deflate oracle C wrapper test in `Tests/TTZipTests/SingleCoreDeflateOracleTests.swift`
- [x] T005 [P] [US1] Remove redundant 7z header parser C wrapper test in `Tests/TTZipTests/SevenZipHeaderParserTests.swift`
- [x] T006 [P] [US1] Remove redundant branchless decompression C wrapper test in `Tests/TTZipTests/BranchlessDecompTests.swift`
- [x] T007 [P] [US1] Remove redundant dual-symbol LUT C wrapper test in `Tests/TTZipTests/StreamingDecompressorDualSymbolLutTests.swift`
- [x] T008 [P] [US1] Remove redundant SWAR benchmark C wrapper test in `Tests/TTZipTests/SwarOptimizationBenchmarkTests.swift`
- [x] T009 [P] [US1] Remove redundant PMULL CRC differential C wrapper test in `Tests/TTZipTests/CRC32PmullDifferentialTests.swift`

**Checkpoint**: 7 redundant test files cleanly removed; SwiftPM test workspace is streamlined.

---

## Phase 4: User Story 2 - Solidify Dual-Engine Boundaries & CI Execution (Priority: P2)

**Goal**: Validate that all Swift architectural tests run cleanly and CI passes with zero warnings.

**Independent Test**: Run `./scripts/local-ci.sh` and verify all 5 stages pass with 0 warnings and 0 errors.

### Implementation for User Story 2

- [x] T010 [US2] Rebuild and run Swift test suites via `swift test --filter "ConcurrencyBridgeTests|ObserverPatternTests"`
- [x] T011 [US2] Execute full 5-stage local CI pipeline in `scripts/local-ci.sh`

---

## Phase 5: Polish & Final Quality Audit

**Purpose**: Verify clean build, zero compiler warnings, and artifact consistency

- [x] T012 Run clean multi-target verification across Swift and CMake to confirm 0 warnings
