# Tasks: C Test Harness Expansion & Advanced Microkernel Migration

**Feature**: `156-156-test-harness-expansion`  
**Input**: Design artifacts from `specs/156-156-test-harness-expansion/` (`plan.md`, `spec.md`, `data-model.md`, `contracts/`, `research.md`, `quickstart.md`)  
**Status**: Completed (100% Verified)  

---

## Phase 1: Setup (Pre-Flight Baseline)

**Purpose**: Confirm operational baseline before implementing new suites

- [x] T001 Verify baseline 8 C test suites pass green via `ctest --test-dir build --output-on-failure`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Confirm header inclusion and test harness compatibility

- [x] T002 Prepare CTest suite registration mappings in `tests/c/ttzip_test_harness.h`

---

## Phase 3: User Story 1 - Blosc2, In-Place Huffman, and Snappy Engines (Priority: P1) 🎯 MVP

**Goal**: Implement native C11 test suites for BloscLZ byte-level compression, in-place canonical Huffman tree bounds, and Snappy framed streams.

**Independent Test**: Run `./build/ttzip_c_test_runner blosc_engine` and `./build/ttzip_c_test_runner huffman_inplace` and `./build/ttzip_c_test_runner snappy_engine`.

### Implementation for User Story 1

- [x] T003 [P] [US1] Implement BloscLZ, BitGroom mantissa quantization, and SuperChunk tests in `tests/c/test_blosc_engine.c`
- [x] T004 [P] [US1] Implement Canonical Huffman Kraft-McMillan and Adaptive Block Split tests in `tests/c/test_huffman_inplace.c`
- [x] T005 [P] [US1] Implement Snappy raw block, framed streams, and CRC32c tests in `tests/c/test_snappy_engine.c`

**Checkpoint**: User Story 1 MVP fully functional and verified in native C.

---

## Phase 4: User Story 2 - Apple DMG Demuxing, LZFSE, and Radix Archive Tree (Priority: P2)

**Goal**: Implement native C11 test suites for Apple UDIF DMG trailers, LZFSE decompression, and Radix trie virtual filesystems.

**Independent Test**: Run `./build/ttzip_c_test_runner dmg_lzfse` and `./build/ttzip_c_test_runner archive_tree`.

### Implementation for User Story 2

- [x] T006 [P] [US2] Implement Apple DMG UDIF koly trailer and LZFSE decompression tests in `tests/c/test_dmg_lzfse.c`
- [x] T007 [P] [US2] Implement Radix Trie hierarchy, search, and memory bounds tests in `tests/c/test_archive_tree.c`

**Checkpoint**: Full 13-suite microkernel coverage operational.

---

## Phase 5: User Story 3 - CTest Runner Integration & Swift Pruning (Priority: P3)

**Goal**: Register all 13 suites in `test_main.c` and `CMakeLists.txt`, and prune redundant Swift FFI wrappers.

**Independent Test**: Run `ctest --test-dir build --output-on-failure` and verify 14/14 CTest targets pass green.

### Implementation for User Story 3

- [x] T008 [US3] Register 5 new test suite runners in `tests/c/test_main.c`
- [x] T009 [US3] Register 5 new CTest targets in `CMakeLists.txt`
- [x] T010 [US3] Prune redundant C-wrapper Swift test files from `Tests/TTZipTests/`

---

## Phase 6: Polish & Verification

**Purpose**: Verify memory safety, zero warnings, and full CI execution

- [x] T011 Run AddressSanitizer & UBSan audit to confirm 0 memory leaks across all 13 suites
- [x] T012 Run full 5-stage local CI pipeline in `scripts/local-ci.sh`
