# Implementation Plan: Single-Core L3/L4 Intermediate Pareto Dominance

## Technical Context

The goal is to eliminate the performance collapse between single-core Tier 3 (Level 3) and Tier 4 (Level 4/6), decoupling their match finding and parsing pipelines so that TTZip strictly dominates `libdeflate` across the intermediate spectrum (Tier 3 $\ge 1.20\text{ GB/s}$ vs libdeflate Level 3 $\sim 1.07\text{ GB/s}$; Tier 4 $\ge 850\text{ MB/s}$ vs libdeflate Level 6 $\sim 749\text{ MB/s}$), creating a continuous, superior Pareto frontier envelope in the upper-right quadrant.

## Constitution Check

- **Zero-Cost Abstraction on Hot Paths**: No heap allocations (`malloc`/`free`/`realloc`/`Data(count:)`) inside compression hot loops.
- **Fast-Path Bypass Preservation**: Direct in-process C execution on ARM64 without external CLI processes or fallback degradation.
- **Frozen File Compliance**: Zero edits to `CTTZipBridge_Crypto.c`, `CTTZipExtract.c`, or frozen zip engine files.
- **Strict Contract Compliance**: `contracts/single-core-pareto-supremacy-contract.json` adheres to Draft-07 with zero bare objects.

## Phase 0: Research Summary

- **R001 [SUBAGENT:research]**: Decouple Tier 3 to 128KB 2-Way Inline Fast-Lazy matchfinder with tail-only match skips; Decouple Tier 4 to 192KB Compact 2-Step Lookahead (`lazy2`) with logarithmic distance entropy weighting.
- **R002 [SUBAGENT:research]**: 4-Way concurrent candidate dispatch with dual-anchor 64-bit GPR SWAR prefix mismatch filtering (`rbit` + `clz`) and nice match length early break.
- **R003 [SUBAGENT:research]**: 64KB/128KB cache-resident block chunking with 256KB thread-local token buffer and seamless 32KB sliding window history preservation across chunks.

## Phase 1: Design Artifacts

- [data-model.md](./data-model.md): Type definitions for `TierProfileConfig`, `ChunkStreamingContext`, `ParetoBenchmarkEvaluation`.
- [contracts/single-core-pareto-supremacy-contract.json](./contracts/single-core-pareto-supremacy-contract.json): Strict JSON Schema validation contract.
- [quickstart.md](./quickstart.md): Executable verification scenarios and failure diagnostics.

## Proposed Changes

### Component 1: `Sources/CTTZipBridge/native_deflate/`

- **[MODIFY] `ttzip_deflate_engine.h`**:
  - Add compact 16-bit relative index structures for `ttzip_deflate_lazy_mf_t` (reducing state from 768KB to 192KB).
  - Add `ttzip_deflate_fast_lazy_mf_t` (128KB L1D-resident 2-way inline table).
  - Declare `ttzip_deflate_fast_lazy_find_matches` and `ttzip_deflate_deep_lazy_find_matches`.
- **[MODIFY] `ttzip_deflate_lazy.c`**:
  - Implement Tier 3 Fast-Lazy matchfinder with 128KB table, depth=4, nice=32, and tail-only skip.
  - Implement Tier 4 Deep-Lazy parser with 2-step lookahead (`lazy2`), dual-anchor GPR SWAR filtering, and entropy distance weighting.
- **[MODIFY] `ttzip_deflate_engine.c`**:
  - Implement 64KB/128KB cache-resident block chunking loop with fixed TLS token buffer.
  - Wire distinct profile parameters for Tier 3 (`tier_level = 3`) and Tier 4 (`tier_level = 4`).

### Component 2: `Tests/TTZipTests/`

- **[MODIFY] `Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift`**:
  - Verify that Tier 1, Tier 2, Tier 3, and Tier 4 generate distinct, monotonic Pareto coordinates and dominate libdeflate.
- **[MODIFY] `Tests/TTZipTests/SingleCoreDeflatePkTests.swift`**:
  - Add specific Level 3 and Level 4 single-core throughput and space savings validation assertions.
- **[MODIFY] `Tests/TTZipTests/SingleCoreDeflateOracleTests.swift`**:
  - Validate 100% round-trip fidelity for Tier 3 and Tier 4 generated bitstreams.

---

## Verification Plan

- `swift test -c release --filter SingleCoreDeflatePkTests`: Assert Level 3 >= 1200 MB/s, Level 4 >= 850 MB/s.
- `swift test -c release --filter SingleCoreDeflateOracleTests`: Assert 100% SHA-256 byte-exact match.
- `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter ZipSingleCoreParetoFrontierPkTests`: Assert full upper-right Pareto dominance and export new PNG plot.
