# Tasks: 130-benchmark-harness-and-methodology-investigation

## User Story 1 - Multi-Corpus Macro Compression Benchmark Suite (Priority: P1)

- [x] T001 [P] [US1] Verify and compile 8 standard data types harness in `Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/test/benchmarks/benchmark_data_types.cc`
- [x] T002 [P] [US1] Execute full 25-point macro Deflate benchmark matrix in RAM-to-RAM mode in `Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/build/test/benchmarks/benchmark_zlib`
- [x] T003 [US1] Parse and compare candidate output against baseline in `scratch/latest_rebased_macro.json`

---

## User Story 2 - Nanosecond-Precision Microarchitectural Telemetry (Priority: P2)

- [x] T004 [P] [US2] Execute match counting microbenchmark sweep across 0..256B with 64-byte alignment in `Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/build/test/benchmarks/benchmark_zlib`
- [x] T005 [US2] Verify scalar subregister extraction vs vector reduction latency across match length boundaries in `scratch/latest_rebased_compare256.json`

---

## User Story 3 - Automated Pareto Analysis & Reporting Standards (Priority: P3)

- [x] T006 [P] [US3] Implement automated comparison table generator adhering to GitHub Markdown standards in `scratch/generate_reply.py`
- [x] T007 [US3] Validate JSON output schema compliance against `specs/130-benchmark-harness-and-methodology-investigation/contracts/benchmark_report.schema.json`
- [x] T008 [US3] Synchronize latest benchmark table into upstream GitHub PR description and maintainer reply draft in `scratch/pr2416_updated_description.md`
