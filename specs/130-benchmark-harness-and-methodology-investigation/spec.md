# Feature Specification: 130-benchmark-harness-and-methodology-investigation

**Feature Branch**: `130-benchmark-harness-and-methodology-investigation`

**Created**: 2026-08-19

**Status**: Draft

**Input**: User description: "好好调研 bench mark 仓库里的相关情况" (Thoroughly research and benchmark repository-wide benchmarking infrastructure, timing harness, multi-corpus evaluation, and methodology standards).

---

## Executive Summary

Benchmark reliability is the foundational prerequisite for all algorithmic optimizations, hardware acceleration, and upstream community contributions. This feature establishes a standardized, reproducible, and noise-isolated benchmarking and microarchitectural telemetry framework across TTZip's native compression pipelines and upstream compression engines (zlib-ng, libdeflate, liblzma, zstd).

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Multi-Corpus Macro Compression Benchmark Suite (Priority: P1)

As a compression engine developer and performance auditor,
I want to execute comprehensive, repeatable macro-level benchmarks across 8 standardized data types and multiple compression levels in 100% memory-isolated RAM-to-RAM mode,
So that I can evaluate end-to-end compression throughput, verify zero-regression Pareto dominance, and provide statistically sound benchmark tables for pull request submissions.

**Why this priority**:
End-to-end macro compression represents real-world application performance and is the definitive criterion used by upstream maintainers and downstream consumers to accept algorithmic changes.

**Independent Test**:
Can be fully verified by running the complete 25-point macro test matrix across all 8 corpora (`text`, `striped_rgb`, `random`, `dna`, `literals`, `short_match`, `mixed`, `realistic_rgb`) and validating that mean, standard deviation, and relative speedup are recorded and compared against baseline without disk I/O noise.

**Acceptance Scenarios**:
1. **Given** a baseline benchmark run and a candidate optimization build, **When** the macro benchmark suite executes on standard test corpora, **Then** a structured comparison matrix is produced showing baseline time, candidate time, delta percentage, and statistical significance.
2. **Given** uncompressible or high-entropy data (e.g. `random`, `literals`), **When** compression benchmarks run at fast levels (L1/L3), **Then** any early-mismatch latency degradation is explicitly identified and gated.

---

### User Story 2 - Nanosecond-Precision Microarchitectural Telemetry & Match Counting Microbenchmark (Priority: P2)

As a systems engineer optimizing SIMD intrinsics and assembly hot paths,
I want to measure the exact per-byte and per-stride match comparison latency (from 0 to 256 bytes) with sub-nanosecond precision and hardware cache-line alignment,
So that I can isolate vector reduction bubbles, register forwarding latency, and branch prediction penalties at each match length boundary.

**Why this priority**:
Microbenchmarks provide immediate diagnostic feedback for instruction-level tuning, allowing developers to optimize specific hardware stages (scalar extraction, discrete stepping, unrolled vector loops) before macro integration.

**Independent Test**:
Can be verified by executing length-sweep microbenchmarks from 0 to 256 bytes across 10,000,000 iterations per point with hardware cache-line alignment and compiler memory barriers, generating a continuous latency curve.

**Acceptance Scenarios**:
1. **Given** match lengths spanning 0 to 256 bytes, **When** the microbenchmark executes, **Then** latency is recorded with sub-nanosecond precision across key architectural boundaries (0B, 1B, 8B, 16B, 32B, 48B, 64B, 128B, 256B).
2. **Given** different memory alignment offsets (0 to 15 bytes), **When** comparisons run across misaligned buffers, **Then** memory alignment penalties are quantified.

---

### User Story 3 - Cross-Engine Pareto Frontier & Compression Density Analysis (Priority: P3)

As a compression architect and product manager,
I want to benchmark and plot TTZip's native compression pipelines against industry baselines (zlib-ng, libdeflate, Apple Compression framework, zstd) across both throughput (MB/s) and compression ratio (%),
So that TTZip's Tier 1/2 routing decisions and performance trade-offs are empirically justified.

**Why this priority**:
Ensures that TTZip continuously maintains Pareto dominance (strictly superior speed at equal compression ratio, or strictly superior compression ratio at equal speed) across all supported file types.

**Independent Test**:
Can be verified by running the cross-engine benchmark suite and generating convex hull Pareto frontier charts comparing compression ratio vs compression/decompression speed.

**Acceptance Scenarios**:
1. **Given** standard benchmark corpora (Silesia, Enwik8, Calgary), **When** multi-engine benchmarks are executed across all compression levels, **Then** a Pareto frontier dataset is generated highlighting Pareto-optimal tiers.

---

## Edge Cases

- **CPU Frequency Scaling & Thermal Throttling**: How does the benchmark harness ensure timing stability when macOS dynamic CPU boost or thermal throttling alters clock rates? The harness must perform warmup passes and report median/mean across multi-iteration runs.
- **Thread Affinity & Multiprocessing Noise**: What happens when background OS tasks preempt benchmark worker threads? The benchmark runner must execute in single-core isolated mode with high process priority.
- **Buffer Cache Warmth**: How to prevent cold-cache skew on initial runs? The benchmark harness must execute minimum warmup iterations to ensure L1/L2 data cache residency before recording timestamps.
- **Zero Allocation Invariance**: How to ensure measurement code does not trigger heap allocations or garbage collection during timing windows? All timing loops must execute zero-allocation routines with pre-allocated buffers.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide automated execution of macro Deflate benchmarks across all 8 standard data types (`text`, `striped_rgb`, `random`, `dna`, `literals`, `short_match`, `mixed`, `realistic_rgb`) and compression levels (L1, L3, L6, L9).
- **FR-002**: System MUST support 100% in-memory RAM-to-RAM execution mode to guarantee zero filesystem I/O jitter.
- **FR-003**: System MUST provide microsecond and nanosecond-resolution timing harnesses using monotonic hardware clocks (`CLOCK_MONOTONIC`).
- **FR-004**: System MUST evaluate match length counting microbenchmarks across continuous length spectrums (0 to 256 bytes) with 64-byte hardware cache alignment.
- **FR-005**: System MUST compute and export comparative benchmark statistics, including baseline time, candidate time, delta percentage ($\Delta\%$), and regression flags.
- **FR-006**: System MUST support specialized strategy benchmarks (`Z_DEFAULT_STRATEGY`, `Z_FILTERED`, `Z_HUFFMAN_ONLY`, `Z_RLE`, `Z_FIXED`, and NoCRC mode).
- **FR-007**: System MUST output structured JSON benchmark results compatible with automated analysis scripts and GitHub pull request markdown generators.

---

## Success Criteria *(mandatory)*

- **SC-001**: Macro benchmark execution across all 25 test points completes within 60 seconds with repeatable timing variance ($\le 2.0\%$ standard error across runs).
- **SC-002**: Microbenchmark latency resolution achieves sub-nanosecond accuracy ($< 0.1\text{ ns}$ precision per operation) across 10,000,000 iterations.
- **SC-003**: Automated benchmark report generation produces complete GitHub-compatible markdown tables within 2 seconds of benchmark completion.
- **SC-004**: 100% of benchmark runs execute with zero heap allocations during the active timing window.

---

## Clarifications

### Session Log: 2026-08-19
- **Q1: Baseline Comparison Source**: Should the benchmark matrix compare against in-tree `develop` baseline or system default zlib?
  - *Decision*: In-tree `develop` baseline with bit-identical compiler flags (`-O3 -DNDEBUG`) to isolate algorithmic improvements.
- **Q2: Microbenchmark Stride Granularity**: What is the required resolution for match counting sweeps?
  - *Decision*: 0 to 256 bytes granular per-byte measurement around boundary transitions (0..16, 24, 32, 40, 48, 64, 80, 96, 128, 160, 192, 224, 256) with 10M iterations/point.
- **Q3: Timing Stability Safeguards**: How are thermal and CPU frequency fluctuations mitigated?
  - *Decision*: 100% In-Memory RAM-to-RAM pre-allocated buffers, initial warmup loops, and multiple repetitions taking the mean/median.
