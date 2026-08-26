# Feature Specification: Full Migration of Performance & Benchmark Suites to Native C11

**Feature ID**: `159-159-benchmark-suite-migration`  
**Created**: 2026-08-20  
**Status**: Ready for Plan  

---

## 1. Problem Statement & Motivation

Currently, 34 benchmark, throughput measurement, Pareto frontier calculation, and synthetic corpus stress test files remain in Swift (`Tests/TTZipTests/`). 

In Swift, benchmark execution suffers from:
1. **ARC and Task runtime jitter**: Swift async task dispatching and ARC reference counting induce non-deterministic latency spikes in nanosecond-scale codec measurements.
2. **Slow Compilation**: Compiling 34 heavy benchmark files adds significant overhead to `swift test`.
3. **Lack of Hardware Precision**: Codec throughput (MB/s), MIPS scoring, Pareto frontier calculation, and SIMD stress matrices are mathematical operations that execute orders of magnitude faster and more predictably in native ANSI C11 with direct monotonic clock calls (`mach_absolute_time` / `clock_gettime`).

This feature migrates all 34 Swift benchmark and performance suites to a unified, ultra-high-speed native C11 benchmark engine under `tests/c/`, registers them in CMake, prunes the redundant Swift benchmark files, and validates zero memory leaks and 100% green local CI.

---

## 2. User Scenarios & Priorities

### User Story 1 (Priority: P1) - Native C Benchmark Harness & Codec Throughput (MVP)
As a performance engineer, I want a zero-overhead C11 benchmark harness (`ttzip_benchmark_harness.h`) that measures single-core and multi-threaded codec compression/decompression throughput (Deflate, Zstd, LZMA2, LZFSE, Snappy) and checksum throughput (CRC32, CRC64, Adler32) with sub-nanosecond clock precision.

- **Acceptance Scenario 1.1**: Implement `tests/c/ttzip_benchmark_harness.h` with nanosecond monotonic timers, throughput calculators (MB/s), MIPS estimators, and memory bandwidth profilers.
- **Acceptance Scenario 1.2**: Implement `tests/c/bench_codecs.c` measuring Deflate (L1/L6/L9), Zstandard (L1/L3), Fast-LZMA2 (L3/L6), LZFSE, and Snappy compression/decompression throughput.
- **Acceptance Scenario 1.3**: Implement `tests/c/bench_checksums.c` measuring PMULL/NEON CRC32, CRC64, Adler-32, and SWAR Shannon entropy across 1KB..16MB buffer tiers.

### User Story 2 (Priority: P2) - Pareto Frontier, MIPS Scoring & Stress VFS Benchmarks
As a systems engineer, I want Pareto frontier curve calculation, MIPS performance score modeling, and multi-threaded in-memory VFS stress benchmarks executed in C11.

- **Acceptance Scenario 2.1**: Implement `tests/c/bench_pareto.c` calculating Pareto optimal points (compression ratio vs MB/s), efficiency frontiers, and regression gates.
- **Acceptance Scenario 2.2**: Implement `tests/c/bench_stress_vfs.c` executing multi-threaded Radix tree 50k-node search, 2GB synthetic stream buffer stress, and DSE-immune memory scrubbing.

### User Story 3 (Priority: P3) - C Benchmark Runner Integration, Swift Pruning & Local CI
As a CI/CD engineer, I want a dedicated `ttzip_benchmark_runner` binary, CTest benchmark integration, complete deletion of the 34 redundant Swift benchmark files, and 100% green 5-stage local CI verification.

- **Acceptance Scenario 3.1**: Implement `tests/c/bench_main.c` supporting `--all`, `--codecs`, `--checksums`, `--pareto`, `--stress`, and `--json` telemetry reporting.
- **Acceptance Scenario 3.2**: Register `ttzip_benchmark_runner` and benchmark CTest targets in `CMakeLists.txt`.
- **Acceptance Scenario 3.3**: Physically prune all 34 redundant benchmark Swift files from `Tests/TTZipTests/`.
- **Acceptance Scenario 3.4**: Validate AddressSanitizer/UBSan and 5-stage local CI pipeline with 0 warnings.

---

## 3. Functional Requirements

- **FR-001**: The system MUST implement a zero-dependency ANSI C11 benchmark harness `tests/c/ttzip_benchmark_harness.h`.
- **FR-002**: The system MUST implement 4 dedicated C11 benchmark suites (`bench_codecs.c`, `bench_checksums.c`, `bench_pareto.c`, `bench_stress_vfs.c`).
- **FR-003**: The master runner `ttzip_benchmark_runner` MUST support sub-command filtering and output structured Markdown and JSON benchmark summaries.
- **FR-004**: All 34 redundant benchmark and performance Swift test files MUST be physically deleted from `Tests/TTZipTests/`.
- **FR-005**: All targets (`cmake --build build`, `swift build`, `swift build --build-tests`) MUST compile with **0 compiler warnings and 0 linker warnings**.
- **FR-006**: The benchmark engine MUST execute cleanly under AddressSanitizer and UndefinedBehaviorSanitizer with **0 memory leaks and 0 UB**.

---

## 4. Success Criteria

- **SC-001 (Deterministic High-Speed Benchmarks)**: Complete 4-suite benchmark matrix runs in **< 1.5 seconds** total in C (compared to > 15s in Swift).
- **SC-002 (Throughput & Ratio Parity)**: Native C benchmarks measure exact MB/s throughput across all 5 codecs without ARC runtime interference.
- **SC-003 (34 Swift Files Pruned)**: Exactly 34 redundant files removed from `Tests/TTZipTests/`, further slashing Swift compilation and test cycle times.
- **SC-004 (Local CI 100% Green)**: All 5 stages of `scripts/local-ci.sh` execute cleanly with 0 quota.
