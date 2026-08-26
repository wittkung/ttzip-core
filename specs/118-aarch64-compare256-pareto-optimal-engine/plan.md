# Implementation Plan: AArch64 Pareto-Optimal Zero-Regression compare256 Engine

**Feature**: `specs/118-aarch64-compare256-pareto-optimal-engine`

## Technical Context

- **Target Architecture**: AArch64 (ARMv8-A/ARMv9-A with NEON SIMD) and ARMv7 fallback.
- **Goal**: Deliver a zero-regression, maximum-throughput `compare256_neon` implementation in `arch/arm/compare256_neon.c` that dominates baseline across 0..256 bytes.
- **Core Components**:
  1. `arch/arm/compare256_neon.c`: Cascade 16B/32B Vector Comparison Engine.
  2. `test/benchmarks/benchmark_compare256.cc`: Benchmark harness verification.

## Constitution Check

- ✅ **No Bare Objects**: Fully typed contracts in `contracts/benchmark-result.json`.
- ✅ **Deterministic Invariants**: Tested against reference linear oracle.
- ✅ **Hard Performance Floor**: Gate 0 and Gate 1 invariants strictly enforced.
- ✅ **Dual-Build & CTest Compliant**: Verified under CMake and strict compiler flags.

## Phase 0: Research Index

- `- R001 [SUBAGENT:research] 《0..15 Byte Early-Exit Latency & Subregister Aliasing vs GPR SWAR》` (documented in `research.md`)
- `- R002 [SUBAGENT:research] 《32..256 Byte Vector Loop Unrolling & Branch Consolidation》` (documented in `research.md`)

## Phase 1: Artifacts & Contracts

- Data Model: `data-model.md`
- Schema Contract: `contracts/benchmark-result.json`
- Validation & Verification: `quickstart.md`

## Planned Component Changes

### Component: `arch/arm/`
- **[MODIFY]** `compare256_neon.c`: Integrate cascaded 16B preamble with 32B unrolled vector loop for AArch64 post-indexed mode and preserve portable ARMv7 fallback.
