# Feature Specification: Comprehensive CPI & Microarchitectural Optimization Audit

**Feature ID**: `160-cpi-microarchitecture-optimization-audit`  
**Created**: 2026-08-20  
**Status**: Ready for Planning  

---

## Clarifications

### Session 2026-08-20
- Q: What are the primary microarchitectural hardware targets and fallback constraints for the CPI audit? → A: Apple Silicon M-series (Firestorm/Avalanche/Oryon ARM64) with NEON, PMULL, and CRC32 vector extensions, with standard scalar/SWAR fallback on x86_64.
- Q: What specific microarchitecture metrics will be added to the benchmark harness? → A: Cycles Per Byte (CPB), estimated Instructions Per Cycle (IPC), Cycles Per Instruction (CPI), and memory bandwidth utilization.
- Q: What is the remediation boundary for identified CPI bottlenecks? → A: Gating FPR↔GPR transfers in NEON vector match finders and unrolling independent accumulators while strictly upholding Constitution §3 & §6.

---

## 1. Problem Statement & Motivation

In high-performance data compression and systems engineering on modern 64-bit superscalar architectures (specifically Apple Silicon M-series wide out-of-order execution cores), throughput is dictated not merely by algorithmic complexity, but by **microarchitectural instruction efficiency**—measured primarily as **CPI (Cycles Per Instruction)** and **IPC (Instructions Per Cycle)**.

On Apple Silicon Firestorm/Avalanche/Oryon cores (8-wide to 10-wide superscalar decode pipelines), peak theoretical IPC ranges from 4.0 to 8.0 (CPI ~0.125 - 0.25). However, several microarchitectural hazards can degrade CPI and stall execution pipelines:
1. **Cross-Domain Register Stalls (FPR ↔ GPR transfers)**: Vector extract instructions (`vgetq_lane`, `umov`, `fmov`) between NEON vector register files and General Purpose Registers incur 3 to 5 cycle latencies, stalling execution if placed in the middle of tight loop bodies.
2. **Read-After-Write (RAW) Dependency Chains**: Tight scalar or vector accumulation loops without independent accumulator registers force the CPU to stall waiting for operand readiness.
3. **Branch Misprediction Penalties**: Data-dependent branches in Huffman decoding and LZ77 hash chain traversal trigger pipeline flushes (costing 14–20+ execution cycles per mispredict).
4. **Memory Hierarchy & Cache Line Friction**: Non-aligned memory access, lack of structured hardware/software prefetching (`__builtin_prefetch`), and false sharing across thread boundaries across 64-byte/128-byte cache lines degrade effective IPC.
5. **Lack of Native CPI/Microarchitecture Telemetry**: Benchmark harnesses currently measure elapsed wall-clock nanoseconds and throughput (MB/s), but lack first-class estimation and tracking of cycles-per-instruction, instruction counts, and microarchitecture latency indicators.

This feature establishes a **comprehensive CPI & microarchitectural optimization audit** across TTZip's core C subsystems (`CTTZipBridge`, `Sources/CTTZipBridge/`, `tests/c/`), identifying microarchitectural bottlenecks, auditing vector register domain crossings, validating branchless SWAR/NEON routines, establishing CPI/IPC benchmarking telemetry, and ensuring strict compliance with Constitution §6 Invariant 1 (Hardware Grounding & Microarchitectural Proof).

---

## 2. User Scenarios & Priorities

### User Story 1 (Priority: P1) - Full Microarchitectural Audit of Core Hot Paths (MVP)
As a systems performance engineer, I want a complete, rigorous microarchitectural audit across all hot paths in TTZip (Checksums, Match Finders, Huffman Coders, Slicing Filters, Memory Prefetchers) to identify and document CPI bottlenecks, cross-domain register stalls, and RAW dependency hazards.

- **Acceptance Scenario 1.1**: Audit `CTTZipCRC32Neon.c`, `CTTZipAdler32Neon.c`, and CRC64 PMULL implementations for instruction-level parallelism, independent vector accumulators, and zero register spilling.
- **Acceptance Scenario 1.2**: Audit `CTTZipNEONMatchFinder.h`, `ttzip_lzma2_*.c`, and `fast-lzma2` match finders for FPR↔GPR crossing latencies, branchless early-exits, and SWAR 64-bit/256-bit unrolling efficiency.
- **Acceptance Scenario 1.3**: Audit Huffman/Entropy decoders (`ttzip_huffman_*.c`, `CTTZipStreamCoder.c`) for table lookup branch elimination and bitstream packing instruction counts.
- **Acceptance Scenario 1.4**: Audit memory prefetch pipelines (`CTTZipPrefetchPipeline.c`, `CTTZipCacheTopology.c`) for 64-byte/128-byte cache line alignment, zero false sharing, and L1/L2 prefetch hints.

### User Story 2 (Priority: P2) - CPI, IPC & Microarchitecture Telemetry in C Benchmark Harness
As a benchmark and CI engineer, I want `ttzip_benchmark_harness.h` and the C benchmark suite (`tests/c/bench_*.c`) augmented with cycle estimation, IPC/CPI models, instruction count estimators, and memory bandwidth efficiency metrics.

- **Acceptance Scenario 2.1**: Implement CPI/IPC calculation helpers and cycle-per-byte (CPB) telemetry in `tests/c/ttzip_benchmark_harness.h`.
- **Acceptance Scenario 2.2**: Integrate CPI, CPB, and instruction efficiency output into `bench_codecs.c` and `bench_checksums.c`.
- **Acceptance Scenario 2.3**: Generate structured JSON and Markdown audit telemetry reports reporting CPB and IPC across all supported codecs and payload tiers (1KB to 16MB).

### User Story 3 (Priority: P3) - Microarchitectural Remediation & Zero-Regression Validation
As a core maintainer, I want identified high-friction CPI bottlenecks remediated with branchless alternatives and vector accumulator unrolling, verified by the 5-stage local CI pipeline and AddressSanitizer with zero compiler warnings.

- **Acceptance Scenario 3.1**: Eliminate redundant FPR↔GPR vector lane extractions in `CTTZipNEONMatchFinder.h` and match finding loops by vector-level comparison gating.
- **Acceptance Scenario 3.2**: Verify that all assembly and C optimizations maintain zero memory leaks, zero UBSan issues, and pass all C and Swift test suites.
- **Acceptance Scenario 3.3**: Ensure 100% compliance with Constitution §6 Invariants 1–5 (Hardware Grounding, Multi-Workload Zero Regression, Single-Variable Ablation, Maintainer Reverence, Atomic Commit Hygiene).

---

## 3. Functional Requirements

- **FR-001**: The system MUST conduct a comprehensive microarchitectural audit of all hot paths in `Sources/CTTZipBridge/` covering:
  1. ARM64 PMULL / CRC32 / CRC64 / Adler32 checksum engines.
  2. LZ77 NEON and SWAR 64-bit / 256-bit vector match finders.
  3. Canonical Huffman and bitstream stream coders.
  4. Cache topology, prefetch pipelines, and memory alignment buffers.
- **FR-002**: The audit MUST evaluate instruction dependency chains, register pressure, stack spilling, and FPR ↔ GPR domain crossing latencies.
- **FR-003**: The C benchmark harness `tests/c/ttzip_benchmark_harness.h` MUST be enhanced to compute:
  - Cycles Per Byte (CPB).
  - Estimated Instructions Per Cycle (IPC) and Cycles Per Instruction (CPI).
  - Effective memory bandwidth utilization (GB/s).
- **FR-004**: Dedicated benchmark tests in `tests/c/bench_checksums.c` and `tests/c/bench_codecs.c` MUST output CPB and IPC metrics for both micro-buffers (1KB-64KB) and large streaming buffers (1MB-16MB).
- **FR-005**: All code modifications MUST compile with `clang` and `swiftc` with **0 compiler warnings and 0 linker warnings**.
- **FR-006**: All changes MUST pass 100% of C unit tests (`tests/c/test_main`) and Swift test suite under AddressSanitizer and UndefinedBehaviorSanitizer.
- **FR-007**: A structured audit report `specs/160-cpi-microarchitecture-optimization-audit/cpi_audit_report.md` MUST be generated summarizing all findings, baseline CPB/IPC numbers, remediated hotspots, and microarchitecture proof.

---

## 4. Success Criteria

- **SC-001 (Microarchitectural Proof & Zero Regressions)**: Comprehensive audit completed across all 5 core subsystems with disassembly/microarchitectural verification conforming to Constitution §6.
- **SC-002 (CPI & CPB Instrumentation)**: Native C benchmark harness accurately calculates CPB and IPC across all benchmark suites with sub-nanosecond clock precision.
- **SC-003 (Hotspot Remediation)**: Eliminating identified FPR↔GPR stalls in NEON match finding improves micro-match throughput by $\ge 5\%$ without regression on long matches.
- **SC-004 (Clean CI Execution)**: `scripts/local-ci.sh` (or `tests/c/test_main`) executes with 100% pass rate, 0 ASan/UBSan issues, and 0 compiler warnings.

---

## 5. Assumptions & Dependencies

- **Platform Assumption**: Primary microarchitecture targets are Apple Silicon (ARM64 / ARM64e with NEON, PMULL, CRC32, SHA3 instructions) with full backwards compatibility for x86_64.
- **Compiler Compatibility**: Clang 15+ / Apple Clang 15.0+ and GCC 12+ supporting `__builtin_prefetch`, `__builtin_clzll`, `__builtin_ctzll`, and ARM ACLE intrinsics (`<arm_neon.h>`, `<arm_acle.h>`).
- **Safety Invariant**: Frozen core files per Constitution §3 remain intact unless specifically targeted for approved bridge micro-optimizations.
