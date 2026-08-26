# Tasks: Full 22-File Swift Test Migration to C11

**Feature**: `158-157-test-migration`  
**Input**: Design artifacts from `specs/158-157-test-migration/` (`plan.md`, `spec.md`, `data-model.md`, `contracts/`, `research.md`, `quickstart.md`)  
**Status**: Ready for Implementation  

---

## Phase 1: Setup (Pre-Flight Baseline)

**Purpose**: Confirm baseline before creating new suites

- [x] T001 Verify baseline 14 C test suites pass green via `ctest --test-dir build --output-on-failure`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Confirm header inclusion and test harness compatibility

- [x] T002 Verify `ttzip_test_harness.h` macro interfaces for SIMD and entropy testing

---

## Phase 3: User Story 1 - SIMD Checksums & Entropy Models (Priority: P1) 🎯 MVP

**Goal**: Implement native C11 test suites for Adler-32 NEON, CRC64-XZ, and SWAR Shannon entropy.

**Independent Test**: Run `./build/ttzip_c_test_runner adler_crc64` and `./build/ttzip_c_test_runner entropy_evaluator`.

### Implementation for User Story 1

- [x] T003 [P] [US1] Implement Adler-32 NEON and CRC64-XZ tests in `tests/c/test_adler_crc64.c`
- [x] T004 [P] [US1] Implement Shannon Entropy and dynamic routing tests in `tests/c/test_entropy_evaluator.c`

**Checkpoint**: User Story 1 MVP fully functional and verified in native C.

---

## Phase 4: User Story 2 - Match Finders & Blosc2 Slicing (Priority: P2)

**Goal**: Implement native C11 test suites for LZ77 hash chains, ring dictionaries, and Blosc2 micro-slicing.

**Independent Test**: Run `./build/ttzip_c_test_runner matchfinder_advanced` and `./build/ttzip_c_test_runner blosc_slicing`.

### Implementation for User Story 2

- [x] T005 [P] [US2] Implement Hash Chain matchers and ring dictionary tests in `tests/c/test_matchfinder_advanced.c`
- [x] T006 [P] [US2] Implement Blosc2 micro-slicing and SuperChunk tests in `tests/c/test_blosc_slicing.c`

**Checkpoint**: Advanced compression and container microkernels operational.

---

## Phase 5: User Story 3 - 7z KDF Crypto, LZ4/Snappy Fuzzing, CTest Integration & Swift Pruning (Priority: P3)

**Goal**: Implement 7z ARMv8 KDF and LZ4/Snappy fuzzing in C, register all 19 suites in CTest, and prune 22 redundant Swift test files.

**Independent Test**: Run `ctest --test-dir build --output-on-failure` and verify 20/20 CTest targets pass green.

### Implementation for User Story 3

- [x] T007 [P] [US3] Implement 7z ARMv8 KDF, LZ4 and Snappy fuzzing tests in `tests/c/test_crypto_lz4_snappy.c`
- [x] T008 [US3] Update test runner dispatcher in `tests/c/test_main.c` (all 19 suites)
- [x] T009 [US3] Register 5 new test suites in `CMakeLists.txt`
- [x] T010 [US3] Physically prune all 22 redundant Swift test files from `Tests/TTZipTests/`

---

## Phase 6: Polish & Verification

**Purpose**: Verify memory safety, zero warnings, and full CI execution

- [x] T011 Run AddressSanitizer & UBSan audit to confirm 0 memory leaks across all 19 suites
- [x] T012 Run full 5-stage local CI pipeline in `scripts/local-ci.sh`
