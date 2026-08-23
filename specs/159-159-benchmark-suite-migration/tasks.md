# Tasks: Full Benchmark & Performance Suite Migration to Native C11

**Feature**: `159-159-benchmark-suite-migration`  
**Input**: Design artifacts from `specs/159-159-benchmark-suite-migration/` (`plan.md`, `spec.md`, `data-model.md`, `contracts/`, `research.md`, `quickstart.md`)  
**Status**: Ready for Implementation  

---

## Phase 1: Setup (Pre-Flight Baseline)

**Purpose**: Confirm baseline before creating new suites

- [x] T001 Verify baseline 19 C test suites pass green via `ctest --test-dir build --output-on-failure`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Implement zero-overhead benchmark harness header

- [x] T002 Implement `tests/c/ttzip_benchmark_harness.h` (Monotonic clock, throughput MB/s, MIPS scoring, synthetic corpus generator)

---

## Phase 3: User Story 1 - Codec & Checksum Benchmarks (Priority: P1) 🎯 MVP

**Goal**: Implement native C11 benchmark suites for all 5 compression codecs and hardware vector checksums.

**Independent Test**: Compile and run `bench_codecs` and `bench_checksums`.

### Implementation for User Story 1

- [x] T003 [P] [US1] Implement Codec Throughput Benchmark Suite in `tests/c/bench_codecs.c`
- [x] T004 [P] [US1] Implement Checksum & SIMD Benchmark Suite in `tests/c/bench_checksums.c`

**Checkpoint**: User Story 1 MVP fully functional and verified in native C.

---

## Phase 4: User Story 2 - Pareto Frontier & Stress VFS Benchmarks (Priority: P2)

**Goal**: Implement non-dominated Pareto frontier curve calculations and multi-threaded in-memory VFS stress benchmarks.

**Independent Test**: Compile and run `bench_pareto` and `bench_stress_vfs`.

### Implementation for User Story 2

- [x] T005 [P] [US2] Implement Pareto Frontier Curve & Regression Suite in `tests/c/bench_pareto.c`
- [x] T006 [P] [US2] Implement Multi-threaded Buffer & Radix VFS Stress Suite in `tests/c/bench_stress_vfs.c`

**Checkpoint**: Advanced mathematical and stress benchmark suites operational.

---

## Phase 5: User Story 3 - Runner Integration, CTest & Swift Pruning (Priority: P3)

**Goal**: Implement standalone `ttzip_benchmark_runner`, register CMake benchmark targets, and prune 34 redundant Swift benchmark files.

**Independent Test**: Run `./build/ttzip_benchmark_runner --all` and verify all 5 stages of `scripts/local-ci.sh`.

### Implementation for User Story 3

- [x] T007 [US3] Implement Master Benchmark Runner CLI in `tests/c/bench_main.c`
- [x] T008 [US3] Register `ttzip_benchmark_runner` and benchmark targets in `CMakeLists.txt`
- [x] T009 [US3] Physically prune all 34 redundant benchmark Swift test files from `Tests/TTZipTests/`

---

## Phase 6: Polish & Verification

**Purpose**: Verify memory safety, zero warnings, and full CI execution

- [x] T010 Run AddressSanitizer & UBSan audit to confirm 0 memory leaks across benchmark runner
- [x] T011 Run full 5-stage local CI pipeline in `scripts/local-ci.sh`
