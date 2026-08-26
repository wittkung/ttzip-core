# Tasks: C Test Harness Migration & Dual-Engine Testing Architecture

**Feature**: `154-c-test-harness-migration`  
**Input**: Design artifacts from `specs/154-c-test-harness-migration/` (`plan.md`, `spec.md`, `data-model.md`, `contracts/`, `research.md`, `quickstart.md`)  
**Status**: Completed (100% Verified)  

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create test directory layout and prepare CMake build definitions

- [x] T001 Create C test directory structure at `tests/c/`
- [x] T002 Configure CMake test options `BUILD_TESTING` and `ENABLE_SANITIZERS` in `CMakeLists.txt`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core zero-dependency C11 test framework and runner entrypoint

**⚠️ CRITICAL**: Must be complete before any test suite implementation

- [x] T003 Implement zero-allocation C11 test harness in `tests/c/ttzip_test_harness.h`
- [x] T004 Implement unified test runner and sub-command dispatcher in `tests/c/test_main.c`

**Checkpoint**: Core harness and runner compilation verified — test suites can now be implemented in parallel.

---

## Phase 3: User Story 1 - Instant Native C Test Suite via CTest & CMake (Priority: P1) 🎯 MVP

**Goal**: Establish instant (< 100ms) CTest execution for hardware checksums, magic format sniffing, and defensive security.

**Independent Test**: Run `ctest --test-dir build --output-on-failure` or `./build/ttzip_c_test_runner crc_neon` and observe 100% pass in < 20ms.

### Implementation for User Story 1

- [x] T005 [P] [US1] Implement hardware PMULL/NEON and software table CRC32 & CRC64 parity tests in `tests/c/test_crc_neon.c`
- [x] T006 [P] [US1] Implement 16-format sub-nanosecond signature and offset sniffing tests in `tests/c/test_magic_sniff.c`
- [x] T007 [P] [US1] Implement defensive Zip-Slip and path traversal interception tests in `tests/c/test_security_zipslip.c`
- [x] T008 [US1] Register `ttzip_c_test_runner` and CTest suite targets in `CMakeLists.txt`

**Checkpoint**: User Story 1 MVP fully functional, passes via CMake and CTest in < 50ms.

---

## Phase 4: User Story 2 - Complete Microkernel Algorithmic Coverage (Priority: P2)

**Goal**: Migrate string natural sorting, Deflate/Zopfli, 7z/LZMA2, Tar containers, and concurrency threadpool into C test suites.

**Independent Test**: Run `./build/ttzip_c_test_runner all` and verify all 8 suites pass in < 50ms with 0 failures.

### Implementation for User Story 2

- [x] T009 [P] [US2] Implement C11 natural numeric string sort tests with strict weak ordering in `tests/c/test_strnatcmp.c`
- [x] T010 [P] [US2] Implement Deflate RFC 1951, Zopfli iterative compression, and in-place Huffman tests in `tests/c/test_deflate_zopfli.c`
- [x] T011 [P] [US2] Implement 7z Varint boundaries, solid block parsing, and Fast-LZMA2 tests in `tests/c/test_7z_lzma2.c`
- [x] T012 [P] [US2] Implement Tar/Pax headers, SWAR octal decoders, and directory tree tests in `tests/c/test_tar_container.c`
- [x] T013 [P] [US2] Implement threadpool dispatch, counting semaphore, and memory budget queries in `tests/c/test_concurrency_threadpool.c`
- [x] T014 [US2] Update `tests/c/test_main.c` and `CMakeLists.txt` with all 8 registered test suites

**Checkpoint**: Full microkernel algorithmic test coverage achieved in native C.

---

## Phase 5: User Story 3 - Dual-Engine CI Decoupling & Swift Streamlining (Priority: P3)

**Goal**: Integrate C test runner as Stage 1 in `scripts/local-ci.sh` and streamline Swift tests for AppKit GUI/ViewModel focus.

**Independent Test**: Run `./scripts/local-ci.sh` and confirm C tests run in Stage 1 (< 0.1s) and all CI stages pass with 0 warnings.

### Implementation for User Story 3

- [x] T015 [US3] Prepend CMake CTest execution as Stage 1 in `scripts/local-ci.sh`
- [x] T016 [US3] Streamline redundant Swift C-wrapper test files in `Tests/TTZipTests/` to delegate algorithmic assertions to C test harness
- [x] T017 [US3] Run full local CI pipeline to verify dual-engine testing synchronization in `scripts/local-ci.sh`

---

## Phase 6: Polish & Verification

**Purpose**: Validate telemetry contract, ASan memory safety, zero warnings, and quickstart scenarios

- [x] T018 [P] Validate telemetry JSON output against `specs/154-c-test-harness-migration/contracts/c-test-runner-schema.json`
- [x] T019 Run AddressSanitizer & UBSan build audit to verify 0 memory leaks and 0 undefined behavior
- [x] T020 Run `quickstart.md` validation scenarios and confirm 100% green execution
