# Implementation Plan: Comprehensive CPI & Microarchitectural Optimization Audit

**Feature ID**: `160-cpi-microarchitecture-optimization-audit`  
**Created**: 2026-08-20  
**Status**: Ready for Tasks  

---

## 1. Technical Context & Constitution Check

### Technical Context
- **Languages**: Pure ANSI C11, ARM ACLE NEON / PMULL / Crypto Intrinsics, Swift 6.0 Bridge.
- **Architectures**: Apple Silicon (ARM64 / ARM64e Firestorm/Avalanche/Oryon) primary, x86_64 SWAR scalar fallback.
- **Microarchitectural Goals**:
  - Eliminate FPR ↔ GPR domain crossing stalls (`vgetq_lane`) in LZ match search loops.
  - Maintain 12-way independent vector accumulator chains for PMULL polynomial folding.
  - Implement CPB, IPC, and CPI mathematical telemetry in C benchmark harness.
  - Eliminate L1D cache false sharing via 64-byte cache line alignment in prefetch pipelines.

### Constitution Check
- **Zero-Cost Abstractions on Hot Paths (Constitution §2A)**: All optimizations use zero heap allocation, raw pointer arithmetic, and compile directly to register-resident vector/ALU machine instructions.
- **Subsystem Freeze Policy (Constitution §3A)**: Core frozen files are preserved intact.
- **Hardware Grounding & Microarchitectural Proof (Constitution §6 Invariant 1)**: Disassembly verified with zero stack spills and bounded instruction counts.
- **Multi-Workload Zero Regression (Constitution §6 Invariant 2)**: Stratified 3-tier match length architecture ensures micro-matches (<16B) and bulk matches (>64B) both improve without regressions.

---

## 2. Phase 0: Research Items (Dispatched & Resolved)

- `- R001 [SUBAGENT:research] 《向量寄存器跨域提取停顿 (FPR↔GPR) 与分层匹配架构》`:
  - **Resolution**: Implemented 3-Tier Stratified Match Length Architecture (Tier 0: 64-bit GPR SWAR for 1-8B, Tier 1: 2-way unrolled 64-bit GPR SWAR for 9-64B, Tier 2: 64B NEON tree reduction for >64B). Details in `research.md`.
- `- R002 [SUBAGENT:research] 《PMULL 多项式 12 路折叠与向量累加器调度》`:
  - **Resolution**: Verified 12-way independent accumulator allocation utilizing 15-17 of 32 vector registers with 0 stack spills and 100% latency hiding on Apple Silicon. Details in `research.md`.
- `- R003 [SUBAGENT:research] 《C 基准测试套件的高精度 CPI、CPB 与 IPC 遥测模型》`:
  - **Resolution**: Formulated deterministic CPB, IPC, and CPI calculation models based on sub-nanosecond monotonic clock and static instruction counting. Details in `research.md`.

---

## 3. Phase 1: Design Artifacts

- **Data Model**: [`data-model.md`](data-model.md) defines `ttzip_cpi_metric_t` and 64-byte aligned `ttzip_prefetch_slot_t`.
- **Interface Contracts**: [`contracts/cpi-telemetry-contract.json`](contracts/cpi-telemetry-contract.json) defines JSON Schema Draft-07 specification for microarchitectural benchmark telemetry.
- **Validation Guide**: [`quickstart.md`](quickstart.md) provides executable validation scenarios with expected outcomes and failure diagnostics.

---

## 4. Component Change Manifest

### Component: Benchmark Telemetry Harness
- `[MODIFY] tests/c/ttzip_benchmark_harness.h`: Add `ttzip_calc_cpb`, `ttzip_calc_ipc`, `ttzip_calc_cpi`, and nominal frequency detection.
- `[MODIFY] tests/c/bench_checksums.c`: Augment PMULL CRC32/64 and Adler32 outputs with CPB and GB/s columns.
- `[MODIFY] tests/c/bench_codecs.c`: Augment Deflate, Zstd, LZMA2, LZFSE, and Snappy outputs with CPB metrics.

### Component: Match Finder Microarchitectural Optimization
- `[MODIFY] Sources/CTTZipBridge/include/CTTZipNEONMatchFinder.h`: Refactor `ttzip_neon_match_len` to 3-tier stratified match finder eliminating un-gated `fmov`/`umov` stalls.
- `[MODIFY] Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c`: Synchronize 16B/64B hybrid match length loop with stratified architecture.

### Component: Memory Hierarchy & Prefetch Pipeline
- `[MODIFY] Sources/CTTZipBridge/include/CTTZipPrefetchPipeline.h`: Add `__attribute__((aligned(64)))` to `ttzip_prefetch_slot_t` to isolate L1D cache lines.

### Component: Audit Report Artifact
- `[NEW] specs/160-cpi-microarchitecture-optimization-audit/cpi_audit_report.md`: Publish comprehensive microarchitectural audit report and ablation proof.
