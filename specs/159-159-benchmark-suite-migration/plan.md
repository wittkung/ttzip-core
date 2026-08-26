# Implementation Plan: Full Benchmark & Performance Suite Migration to Native C11

**Branch**: `159-159-benchmark-suite-migration` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/159-159-benchmark-suite-migration/spec.md`

---

## Summary

Migrate all remaining 34 benchmark, throughput, Pareto frontier, and stress test suites from `Tests/TTZipTests/` into 4 dedicated C11 native benchmark suites under `tests/c/` with a zero-overhead header `ttzip_benchmark_harness.h`. Register the runner `ttzip_benchmark_runner` in CMake, physically delete all 34 redundant Swift benchmark files, and verify zero warnings, zero ASan leaks, and 100% green local CI.

---

## Technical Context

- **Language/Version**: ANSI C11 & Swift 6.0
- **Primary Dependencies**: None (Pure standard C11 library + POSIX monotonic timers)
- **Storage**: In-memory synthetic buffers (Zero disk I/O bottleneck)
- **Testing**: Native C11 Benchmark Harness (`tests/c/ttzip_benchmark_harness.h`) + CMake CTest + Swift XCTest
- **Target Platform**: macOS (Apple Silicon arm64 & Intel x86_64) and Linux/FreeBSD
- **Performance Goals**: Full 4-suite benchmark matrix executes in **< 1.5 seconds** total in C
- **Constraints**: 0 compiler warnings, 0 ASan leaks, 0 cloud quota usage

---

## Constitution Check

*GATE: Passed. Complies with zero cloud quota, zero external dependency, and sub-second deterministic performance.*

- [x] Principle 1: Zero Cloud CI Quota — 100% local execution.
- [x] Principle 2: Zero Dependency Bloat — Pure ANSI C11 standard library only.
- [x] Principle 3: Performance First — Sub-1.5s total benchmark execution.
- [x] Principle 4: High Reliability & Memory Safety — AddressSanitizer and UBSan verified.

---

## Phase 0: Outline & Research

The following research items were formally investigated:
- R001 [SUBAGENT:research] 《C11 Monotonic Benchmark Architecture & Pareto Frontier Model》：High-resolution clock APIs, non-dominated sorting algorithms, and in-memory synthetic corpus generators.

*Artifact*: [research.md](./research.md)

---

## Phase 1: Design & Contracts

The following design artifacts establish data models and schema contracts:
- Data Model: [data-model.md](./data-model.md)
- Contracts:
  - `contracts/benchmark-report-schema.json` [SUBAGENT:research] — Formal JSON Schema for benchmark telemetry reporting.
- Quickstart & Verification: [quickstart.md](./quickstart.md)

---

## Project Structure & Planned Changes

```text
TTZip/
├── CMakeLists.txt                              # [MODIFY] Register ttzip_benchmark_runner & CTest benchmarks
├── tests/
│   └── c/
│       ├── ttzip_benchmark_harness.h          # [NEW] Nanosecond monotonic timer & throughput macros
│       ├── bench_main.c                        # [NEW] Standalone benchmark runner & CLI flags
│       ├── bench_codecs.c                      # [NEW] Codec throughput & compression ratio matrix
│       ├── bench_checksums.c                   # [NEW] SIMD CRC32, CRC64, Adler32, Entropy benchmarks
│       ├── bench_pareto.c                      # [NEW] Non-dominated Pareto frontier curve calculator
│       └── bench_stress_vfs.c                  # [NEW] Multi-threaded VFS & buffer stress benchmarks
└── specs/159-159-benchmark-suite-migration/
    ├── spec.md
    ├── checklists/requirements.md
    ├── plan.md
    ├── research.md
    ├── data-model.md
    ├── contracts/benchmark-report-schema.json
    └── quickstart.md
```
