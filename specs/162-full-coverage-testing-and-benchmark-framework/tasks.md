# Tasks: 全覆盖测试与基准遥测零回退体系 (Feature 162)

**Feature ID**: `162-full-coverage-testing-and-benchmark-framework`  
**Created**: 2026-08-20  
**Status**: Ready for Implementation  

---

## Phase 1: Setup & Foundational Infrastructure

- [x] T001 Update `CMakeLists.txt` to register `bench_formats.c` under `ttzip_benchmark_runner` target and link all necessary codec libraries
- [x] T002 Verify and calibrate 8-corpus generator with zero-allocation caller buffers in `Sources/CTTZipBridge/CTTZipCorpusGen.c`

---

## Phase 2: User Story 1 (P1) - Full 10-Codec In-Memory Microarchitectural Benchmark Suite

- [x] T003 [P] [US1] Extend `tests/c/bench_codecs.c` with LZ4 (Fast L1 / HC L9) compress & decompress loops with CPB calculation
- [x] T004 [P] [US1] Extend `tests/c/bench_codecs.c` with Brotli (Q6 / Q9) and Bzip2 (L1 / L9) compress & decompress loops with CPB calculation
- [x] T005 [P] [US1] Extend `tests/c/bench_codecs.c` with BloscLZ (L1 / L9) compress & decompress loops and 8-corpus sweep support

---

## Phase 3: User Story 2 (P2) - Container Format & Extraction Pipeline Benchmark Suite

- [x] T006 [P] [US2] Implement temporary isolated VFS fixture builder and Peak RSS memory telemetry in `tests/c/bench_formats.c`
- [x] T007 [P] [US2] Implement ZIP (Store & Deflate) packaging and extraction benchmark with Peak RSS tracking in `tests/c/bench_formats.c`
- [x] T008 [P] [US2] Implement TAR.GZ, TAR.ZST, TAR.BZ2, TAR.XZ packaging and extraction benchmarks in `tests/c/bench_formats.c`
- [x] T009 [P] [US2] Implement 7Z and UnRAR container extraction benchmarks in `tests/c/bench_formats.c`
- [x] T010 [US2] Wire `--formats` and `--all` CLI subcommands in `tests/c/bench_main.c`

---

## Phase 4: User Story 3 (P3) - 5-Gate Zero-Regression Automated Pipeline

- [x] T011 [US3] Create unified 5-stage automated gate runner in `scripts/run_optimization_gate.sh`
- [x] T012 [US3] Add structured JSON report exporter in `scripts/run_optimization_gate.sh` conforming to `contracts/benchmark-telemetry-schema.json`

---

## Phase 5: Verification, Benchmarking & Documentation

- [x] T013 [US3] Execute full C test runner `./build/ttzip_c_test_runner all`
- [x] T014 [US3] Execute full C benchmark runner `./build/ttzip_benchmark_runner --all`
- [x] T015 [US3] Execute 5-gate pipeline `./scripts/run_optimization_gate.sh --bail --json build/gate_report.json`
