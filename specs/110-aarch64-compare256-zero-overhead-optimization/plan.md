# Implementation Plan: AArch64 compare256 2x-Unrolled NEON+VORR+UMAXV Optimization

**Feature Branch / Directory**: `specs/110-aarch64-compare256-zero-overhead-optimization`  
**Target Upstream**: `zlib-ng/zlib-ng` (`arch/arm/compare256_neon.c`)  
**Status**: APPROVED & IMPLEMENTED  

---

## Technical Context

- **Target Architecture**: AArch64 (ARMv8.0-A baseline, Apple Silicon M1-M5, ARM Cortex-A7x, Neoverse N/V).
- **Compilers**: Apple Clang 17+, Clang 14+, GCC 11+, MSVC on ARM64.
- **Core Function**: `compare256_neon_static(const uint8_t *src0, const uint8_t *src1)` in `arch/arm/compare256_neon.c`.
- **Inlined Caller Sites**: `longest_match_neon` and `longest_match_roll_neon` (via `match_tpl.h`).

---

## Architecture Design: 2x-Unrolled NEON + VORR + Single-UMAXV

```
                  ┌─────────────────────────────────┐
                  │ Loop Stride: 32 Bytes per iter  │
                  └────────────────┬────────────────┘
                                   │
              ┌────────────────────┴────────────────────┐
              ▼                                         ▼
      Chunk 1 (16 Bytes)                        Chunk 2 (16 Bytes)
  LDR Q0, [src1, offset]                    LDR Q2, [src1, offset]
  LDR Q1, [src1], #16                       LDR Q3, [src1], #16
  CMP1 = VEORQ_U8(Q0, Q1)                   CMP2 = VEORQ_U8(Q2, Q3)
              │                                         │
              └────────────────────┬────────────────────┘
                                   │
                         ANY_DIFF = VORRQ_U8(CMP1, CMP2)
                                   │
                         UMAXV_B2 = VMAXVQ_U8(ANY_DIFF)
                                   │
                        ┌──────────┴──────────┐
               [ == 0 ] │                     │ [ != 0 ]
                        ▼                     ▼
                 All 32B Match!          Pinpoint Mismatch
                 len += 32               Check CMP1.Lane0 (0..7B)
                 continue;               Check CMP1.Lane1 (8..15B)
                                         Check CMP2.Lane0 (16..23B)
                                         Check CMP2.Lane1 (24..31B)
```

---

## Phase 0: Grounded Research Index

- `R001` [SUBAGENT:research]: AArch64 Match Counting Practices (libdeflate vs zstd vs Snappy vs zlib-ng).
- `R002` [SUBAGENT:research]: Microarchitecture Pipeline Analysis (Apple Silicon dual-load pipes & UMAXV latency).
- `R003` [SUBAGENT:research]: Inlining & Register Pressure Audit in `longest_match_neon`.
- `R004` [SUBAGENT:research]: Empirical Five-Architecture Benchmark Matrix (Develop vs 16B-UMAXV vs SWAR64 vs Cascaded vs Unrolled2x).

---

## Phase 1: Design Artifacts Index

- `contracts/compare256_neon_contract.json`: Draft-07 contract specifying loop step, lane layout, and zero-spill invariants.
- `data-model.md`: In-register data layout and execution state transition model.
- `quickstart.md`: Standalone verification commands for regression testing and full-spectrum benchmarking.
- `tasks.md`: Structured implementation and validation task list.
