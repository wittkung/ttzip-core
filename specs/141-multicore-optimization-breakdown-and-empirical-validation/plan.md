# Implementation Plan: Multi-Core Optimization Breakdown & Point-by-Point Empirical Validation

**Feature ID**: `141-multicore-optimization-breakdown-and-empirical-validation`  
**Date**: 2026-08-20  
**Status**: Planned  

---

## 1. Technical Context & Multi-Core Architecture

TTZip achieves high throughput by distributing data and tasks across Apple Silicon cores. To systematically ensure that every single optimization point delivers a positive throughput contribution without hidden regressions or lock contention, this plan defines the implementation of a dedicated single-point isolated empirical test suite (`MultiCoreOptimizationBreakdownTests.swift`), diagnostic reporting models, and documentation updates.

---

## 2. Constitution Check

- [x] **Streaming-First**: Multi-core chunking and container pipelines preserve streaming invariants.
- [x] **Defense-in-Depth**: Memory allocation adheres to bounds checking and thread-local lifetime safety.
- [x] **Zero-Copy & Low Contention**: TLS caches eliminate lock contention; 64-byte alignment prevents false sharing.
- [x] **Grounded Verification**: All tests compare isolated baseline vs optimized routines under physical hardware monotonic timers.

---

## 3. Phase 0: Research & Grounded Analysis

- R001 [SUBAGENT:research] 《C11 _Thread_local Codec Pooling vs Mutex Allocation》: Analyzed in `research.md` (TLS eliminates mutex ping-ponging and heap lock serialization).
- R002 [SUBAGENT:research] 《512KB Chunk Boundary Sizing and Multi-Tile Parallel Overhead》: Analyzed in `research.md` (512KB fits in shared L2 cluster; 64-byte alignment eliminates false sharing).
- R003 [SUBAGENT:research] 《ARMv8 PMULL Checksum Vectorization and Amdahl's Law Avoidance》: Analyzed in `research.md` (79 GB/s CRC avoids serialization bottleneck in 16-core pipelines).

---

## 4. Phase 1: Design Artifacts & Contracts

- `data-model.md`: Defines `MultiCoreOptimizationPoint`, `OptimizationPointResult`, `MultiCoreBreakdownSummary`.
- `contracts/multicore_optimization_api.json`: JSON Schema draft-07 defining data contract for single-point multi-core benchmark reports.
- `quickstart.md`: Execution manual for running single-point multi-core tests and CLI diagnostics.

---

## 5. Component Breakdown & Targeted Changes

### Component 1: Test Suite (`Tests/TTZipTests/`)
- [NEW] `MultiCoreOptimizationBreakdownTests.swift`: Contains isolated A/B differential tests for all 8 optimization points:
  1. `testOP1_ThreadLocalStorageVsMutexContention`
  2. `testOP2_BlockParallel512KBVsSingleThreadedDeflate`
  3. `testOP3_MultiTileParallelDecompressionVsSequential`
  4. `testOP4_ContainerMultiFilePackagingVsSequential`
  5. `testOP5_ContainerMultiFileExtractionVsSequential`
  6. `testOP6_ARMv8PMULLVsSoftwareTableCRC32`
  7. `testOP7_APFSDirectIOPreallocationVsUnbufferedWrite`
  8. `testOP8_TopologyAwareQoSScheduling`

### Component 2: Core Models & Diagnostic Runner (`Sources/TTZipCore/`)
- [NEW] `Sources/TTZipCore/Benchmark/MultiCoreBreakdownRunner.swift`: Programmatic runner that executes all 8 isolated benchmarks and returns a structured `MultiCoreBreakdownSummary`.

### Component 3: Documentation
- [MODIFY] `docs/PERFORMANCE.md`: Add Section 8 detailing the 8-point multi-core optimization breakdown and empirical speedup factors.
