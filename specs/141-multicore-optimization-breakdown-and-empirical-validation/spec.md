# Feature Specification: Multi-Core Optimization Breakdown & Point-by-Point Empirical Validation

**Feature ID**: `141-multicore-optimization-breakdown-and-empirical-validation`  
**Status**: Specified  
**Author**: Antigravity / TTZip Performance Architecture Group  
**Target Milestone**: TTZip v1.1 Multi-Core Governance  

---

## 1. Executive Summary & Objective

TTZip achieves high physical throughput on Apple Silicon by combining multiple orthogonal multi-core optimization techniques across CPU scheduling, memory pooling, container decomposition, hardware SIMD hashing, and APFS zero-copy I/O. To ensure strict architectural hygiene and avoid regression or hidden contention, this specification formalizes the **point-by-point decomposition of all 8 core multi-core optimizations in TTZip**, builds dedicated single-point isolated empirical benchmark harnesses, and validates that each independent optimization contributes a strictly positive performance delta ($\Delta > 0\%$) over its unoptimized baseline under physical monotonic timers.

---

## 2. Taxonomy of the 8 Multi-Core Optimization Points

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        TTZIP 8-POINT MULTI-CORE OPTIMIZATION TAXONOMY                  │
├──────┬────────────────────────────────────────────┬────────────────────────────────────┤
│ ID   │ Multi-Core Optimization Point              │ Underlying Mechanism & Invariant   │
├──────┼────────────────────────────────────────────┼────────────────────────────────────┤
│ OP-1 │ C11 _Thread_local Zero-Lock State Pool     │ Lock-free TLS per-thread codec pool│
│ OP-2 │ 512KB Block-Level Parallel Compression     │ Chunked concurrent libdeflate      │
│ OP-3 │ Multi-Tile Parallel Block Decompression    │ Independent chunk extraction       │
│ OP-4 │ Container-Level Multi-File Concurrency     │ GCD/TaskGroup parallel packaging   │
│ OP-5 │ Multi-File Concurrent Extraction Pipeline  │ Direct-to-disk parallel unpacking  │
│ OP-6 │ ARMv8 PMULL Hardware Vectorized CRC32/64   │ 4-way unrolled vmull_p64 SIMD      │
│ OP-7 │ APFS fstore_t & Direct I/O Preallocation   │ Contiguous disk block reservation  │
│ OP-8 │ Apple Silicon P/E-Core Topology Scheduling │ QoS-aware compute/IO work split    │
└──────┴────────────────────────────────────────────┴────────────────────────────────────┘
```

---

## 3. User Scenarios & Functional Requirements

### User Story 1: Comprehensive Point-by-Point Multi-Core Census (Priority: P1)
* As a performance engineer, I need all multi-core optimizations in TTZip to be explicitly enumerated, categorized by layer (Memory / Codec / Container / Hashing / I/O / Scheduling), and isolated into benchmarkable single-point units.

### User Story 2: Isolated Single-Point Empirical Test Suite (Priority: P1)
* As a core developer, I need an automated test suite (`MultiCoreOptimizationBreakdownTests.swift`) that runs isolated A/B differential tests for each of the 8 optimization points against its respective unoptimized baseline.

### User Story 3: Positive Delta & Monotonic Non-Regression Gate (Priority: P1)
* As a CI maintainer, I need every single optimization point to demonstrate a measurable, statistically significant positive throughput gain ($\text{Throughput}_{\text{optimized}} / \text{Throughput}_{\text{baseline}} > 1.0$) with zero data corruption (100% SHA-256 / CRC32 bit-exact integrity).

### User Story 4: Telemetry & Benchmark Diagnostic Reporting (Priority: P2)
* As an engineering lead, I need structured logging and CLI diagnostic output that reports each optimization point's baseline throughput, optimized throughput, speedup ratio, and pass/fail status.

---

## 4. Functional Requirements (FR-001 ~ FR-008)

- **FR-001 [OP-1 TLS State Pool]**: Verify that C11 `_Thread_local` compressor/decompressor caching provides $\ge 1.5\text{x}$ throughput over global mutex-locked state allocation during multi-threaded concurrent compression.
- **FR-002 [OP-2 Parallel Block Compression]**: Verify that 512KB chunk concurrent compression scales across all physical cores, achieving $\ge 2.0\text{x}$ speedup over single-threaded compression for buffers $\ge 2\text{MB}$.
- **FR-003 [OP-3 Parallel Block Decompression]**: Verify that multi-tile parallel decompression achieves $\ge 1.8\text{x}$ speedup over single-threaded sequential decompression.
- **FR-004 [OP-4 Container Multi-File Packaging]**: Verify that concurrent multi-file packaging in `ZipParallelWriter` achieves $\ge 2.5\text{x}$ speedup over sequential single-threaded file-by-file archiving on $\ge 50$ files.
- **FR-005 [OP-5 Container Multi-File Extraction]**: Verify that concurrent multi-file extraction in `ZipParallelExtractor` achieves $\ge 2.0\text{x}$ speedup over sequential extraction.
- **FR-006 [OP-6 ARMv8 PMULL Hardware Checksum]**: Verify that 4-way unrolled `vmull_p64` CRC32/64 execution delivers $\ge 10.0\text{x}$ speedup over software slice-by-8 / table lookup on 1MB+ buffers.
- **FR-007 [OP-7 APFS Direct I/O Preallocation]**: Verify that `fstore_t` preallocation reduces file fragmentation and eliminates multi-thread disk lock overhead, ensuring steady-state write throughput.
- **FR-008 [OP-8 Topology-Aware QoS Scheduling]**: Verify that routing compute tasks to `userInitiated`/`userInteractive` QoS activates high-performance P-cores without throttling.

---

## 5. Success Criteria (SC-001 ~ SC-004)

- **SC-001 (100% Positive Delta)**: All 8 optimization points must individually achieve $\text{Speedup} > 1.0\text{x}$ in empirical differential tests without exception.
- **SC-002 (Bit-Exact Integrity)**: All parallel compressed and extracted payloads must match the reference SHA-256 and CRC32 hash 100%.
- **SC-003 (Zero Warnings & Strict Concurrency)**: All test suites and benchmark runners must compile cleanly under Swift 6.0 Strict Concurrency with 0 warnings and 0 errors.
- **SC-004 (Empirical Report Generation)**: Automated benchmark output generates a clean Markdown/Console report displaying all 8 points, baselines, optimized throughputs, and speedup multiples.

---

## 6. Clarifications & Invariant Alignment

- **Q1: How should each single point be isolated from other optimizations during testing?**
  - **Decision**: Each test in the suite directly compares an isolated pair: an unoptimized baseline routine (e.g., global mutex allocation, serial loop, software CRC table) against the exact optimized routine (e.g., TLS pool, parallel loop, ARMv8 PMULL), keeping data payload, buffer sizes, and compression level identical.
- **Q2: What happens if an optimization shows a negative delta on small payloads?**
  - **Decision**: Sizing thresholds (such as 512KB for block parallelism) are part of the optimization specification; the test harness validates both small-payload fast-path fallback and large-payload multi-core scaling.
