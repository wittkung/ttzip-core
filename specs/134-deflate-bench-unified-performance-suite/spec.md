# Feature Specification: Deflate-Bench Unified Performance & Test Suite Modernization

**Feature Branch**: `134-deflate-bench-unified-performance-suite`  
**Created**: 2026-08-20  
**Status**: Draft  
**Input**: User description: "/speckit-plan /goal 完成完整的重构和整个测试体系的升级 ，好好学习zlibng"

---

## 1. Executive Summary & Problem Definition

TTZip currently possesses an extensive but severely bloated test suite comprising 170+ Swift test files. Specifically, the performance and benchmarking layer suffers from:
1. **File Bloat & Fragmented Overlap**: Over 18 separate benchmark test files exist, testing similar functions repeatedly with overlapping coverage.
2. **Disk I/O & Sandbox Noise**: Many tests allocate temporary files on disk via `IsolatedTempSandbox`, benchmarking SSD filesystem latencies rather than pure algorithmic throughput.
3. **Flaky Hard-Floor Regressions & Slow CI**: Standard test runs take 15–30 seconds, occasionally failing CI hooks due to CPU thermal throttling and OS scheduler interference.

Learning from the gold-standard methodology of `zlib-ng`'s `deflate_bench`, this feature refactors TTZip's entire performance testing architecture into a unified, 100% in-memory, deterministic 3-tier benchmark matrix while purging deprecated and redundant test files to achieve sub-3-second CI test times.

---

## 2. User Scenarios & Testing *(mandatory)*

### User Story 1 - Native In-Memory 8-Workload Corpus Engine (Priority: P1)

As a performance engineer or CI pipeline, I want a deterministic, zero-allocation in-memory corpus generator in Swift/C that reproduces `zlib-ng`'s 8 physical entropy workloads (`text`, `striped_rgb`, `dna`, `mixed`, `short_match`, `random`, `literals`, `realistic_rgb`), so that benchmarks can run entirely in RAM with zero disk I/O and zero external dataset dependencies.

**Why this priority**: Foundational building block for all downstream in-memory codec and container benchmarks.

**Independent Test**:
Can be fully tested by running `TTZipCorpusGeneratorTests`, asserting that all 8 workloads produce bit-exact deterministic byte streams with expected entropy and match characteristics.

**Acceptance Scenarios**:
1. **Given** requested workload `striped_rgb` and size 1MB, **When** generated in memory, **Then** it produces 1048576 bytes with repeating 3-byte RGB patterns in under 1 millisecond.
2. **Given** requested workload `short_match` and size 128KB, **When** compressed by Deflate, **Then** over 80% of match evaluations resolve to 8–16 bytes.

---

### User Story 2 - Unified Core Codec Benchmark Suite (Priority: P1)

As a core developer, I want a unified in-memory benchmark suite (`TTZipCoreCodecBenchmarks.swift`) that runs a parameterized 50-point matrix across compression levels 1–9 and reports throughput (MB/s), duration (µs/ms), and CV statistics, so that any codec optimization or regression can be measured in < 1 second.

**Why this priority**: Replaces fragmented single-core deflate tests with a single standardized, Google-Benchmark-compatible matrix.

**Independent Test**:
Can be tested by executing `swift test --filter TTZipCoreCodecBenchmarks`, verifying all 50 points complete in < 1.5s with structured terminal metrics.

**Acceptance Scenarios**:
1. **Given** TTZip's `LibdeflateAccelerator` and `DeflateStreamEngine`, **When** evaluated on the 8 workloads, **Then** throughput is measured across L1, L3, L6, and L9 with median  \le 1.5\%$.
2. **Given** Level 1 on 128KB `text`, **When** executed, **Then** compression throughput exceeds 1500 MB/s.

---

### User Story 3 - Multi-Core Parallel Container Benchmark Suite (Priority: P2)

As an archiver architect, I want a unified parallel container benchmark suite (`TTZipContainerParallelBenchmarks.swift`) that evaluates multi-threaded block-parallel compression and extraction across the 8 physical workloads without disk I/O, so that container overhead (central directory, AES, headers) is tested cleanly.

**Why this priority**: Extends codec benchmarks to multi-core zip streaming and batch processing.

**Independent Test**:
Can be tested by running `swift test --filter TTZipContainerParallelBenchmarks`, asserting multi-core scalability on 100MB synthetic blocks and 10,000 in-memory virtual files.

**Acceptance Scenarios**:
1. **Given** a 100MB in-memory stream across all 8 workloads, **When** compressed with 8-thread block parallel ZIP, **Then** compression throughput scales linearly (>= 3.5x single core).

---

### User Story 4 - Test Suite De-cluttering & CI Speedup (Priority: P2)

As a project maintainer, I want to safely remove and consolidate 15+ redundant, slow, disk-bound benchmark files in `Tests/TTZipTests/`, so that standard `swift test` runs cleanly in under 3 seconds without flaky timeouts.

**Why this priority**: Eliminates technical debt, removes CI friction, and reduces repository maintenance overhead.

**Independent Test**:
Can be validated by running `swift test`, confirming total test execution time is < 3.5 seconds and 100% of core unit tests pass.

**Acceptance Scenarios**:
1. **Given** the clean test suite, **When** `swift test` is run without benchmark flags, **Then** all unit, standards compliance, oracle, and security tests pass 100% in < 3.5s.
2. **Given** redundant test files identified for removal, **When** consolidated into the 3 unified suites, **Then** test coverage does not drop.

---

## 3. Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide a native C/Swift in-memory corpus generator (`Sources/CTTZipBridge/ttzip_corpus.c` & `Sources/TTZipCore/Benchmark/TTZipCorpusGenerator.swift`) implementing the 8 standard workloads.
- **FR-002**: The corpus generator MUST produce bit-exact deterministic byte streams identical to the PRNG algorithms in `zlib-ng`'s `data_type.cc`.
- **FR-003**: System MUST provide `TTZipCoreCodecBenchmarks.swift` to benchmark raw stream compression (Deflate, Zstd, LZ4, Brotli) across all 8 workloads and sizes (128KB & 1MB).
- **FR-004**: System MUST provide `TTZipContainerParallelBenchmarks.swift` to benchmark multi-core block-parallel ZIP and virtual batch streams.
- **FR-005**: All benchmark suites MUST calculate and display throughput in MB/s, duration in µs/ms, and Coefficient of Variation ( = rac{\sigma}{\mu}$).
- **FR-006**: System MUST consolidate and safely remove at least 12 redundant and slow benchmark files in `Tests/TTZipTests/`.
- **FR-007**: System MUST ensure standard `swift test` executes in $\le 3.5$ seconds on Apple Silicon.
- **FR-008**: System MUST update `scripts/upstream_crossover_bench.py` to support native Swift/C benchmark invocations.

---

## 4. Success Criteria *(mandatory)*

- **SC-001 (Zero Disk I/O)**: 100% of core codec benchmarks execute in RAM without writing to temporary disk files.
- **SC-002 (CI Acceleration)**: Total standard unit test execution time drops from > 15 seconds to **$\le 3.5$ seconds**.
- **SC-003 (Workload Parity)**: All 8 `deflate_bench` workloads are fully ported and verified against reference distributions.
- **SC-004 (Codebase Cleanliness)**: Elimination of 15+ redundant test files with zero drop in functional code coverage.

---

## 5. Key Entities & Data Model

- **TTZipWorkloadType**: `text`, `striped_rgb`, `dna`, `mixed`, `short_match`, `random`, `literals`, `realistic_rgb`
- **TTZipCorpusBuffer**: In-memory contiguous byte buffer with length and checksum.
- **TTZipBenchmarkMatrixResult**: Data structure capturing throughput (MB/s), duration (ns), level, size, and CV.
