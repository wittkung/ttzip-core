# Implementation Plan: libdeflate-Aligned Single-Core DEFLATE Engine with Apple Silicon Optimization

## Technical Context

The goal is to adopt the canonical `libdeflate` compression pipeline architecture (256 KB 16-bit relative index `hc_matchfinder`, 8-byte `deflate_sequence` representation, Moffat-Katajainen Huffman builder, and dynamic block splitting) as the native baseline in TTZip, matching libdeflate's gold standard compression ratio (3.34 MB at Level 3 and 3.21 MB at Level 6 on 100MB `enwik8`), and then surgically apply three Apple Silicon ARM64 hardware optimization patches (NEON `lz_extend`, multi-candidate load unrolling, and 64-bit fused sequence bitstream packing) to strictly dominate libdeflate in throughput ($\ge 1.20\text{ GB/s}$ at Level 3, $\ge 800\text{ MB/s}$ at Level 6) across the entire upper-right Pareto envelope.

## Constitution Check

- **Zero-Cost Abstraction on Hot Paths**: No heap allocation inside match finding or block splitting; matchfinder footprint bounded at 256 KB.
- **Fast-Path Bypass Preservation**: 100% in-process C execution with zero CLI sub-processes.
- **Frozen File Compliance**: Zero edits to `ZipParallelExtractor.swift`, `CTTZipExtract.c`, or `CTTZipBridge_Crypto.c`.
- **Strict Contract Compliance**: `contracts/aligned-deflate-engine-contract.json` validates against Draft-07.

## Phase 0: Research Summary

- **R001 [SUBAGENT:research]**: Structure canonical pipeline with 256 KB `struct hc_matchfinder` (`hash3_tab[32768]`, `hash4_tab[65536]`, `next_tab[32768]`), 8-byte `struct deflate_sequence`, and dynamic entropy-guided block splitting (`SOFT_MAX_BLOCK_LENGTH = 300,000`).
- **R002 [SUBAGENT:research]**: 3 Apple Silicon hardware patches: (1) Hybrid 128-bit NEON + 64-bit GPR SWAR Tier-0 `lz_extend`; (2) 1-step load prefetching & candidate unrolling in `hc_matchfinder_longest_match`; (3) 64-bit GPR fused sequence bitstream packing.

## Phase 1: Design Artifacts

- [data-model.md](./data-model.md): Type definitions for `CanonicalDeflateSequence`, `CompactMatchfinderState`, `EngineOptimizationProfile`.
- [contracts/aligned-deflate-engine-contract.json](./contracts/aligned-deflate-engine-contract.json): Strict JSON Schema validation contract.
- [quickstart.md](./quickstart.md): Executable verification scenarios and diagnostics.

## Proposed Changes

### Component 1: `Sources/CTTZipBridge/`

- **[MODIFY] `native_deflate/ttzip_deflate_engine.c` / `ttzip_deflate_engine.h`**:
  - Integrate canonical 256 KB `hc_matchfinder` and `deflate_sequence` pipeline for Levels 3..6.
  - Implement Tier 3 Fast Greedy/Lazy (`max_search_depth = 12`, `nice_match_len = 16~32`) and Tier 4 Deep Lazy (`max_search_depth = 35`, `nice_match_len = 65`, 2-step lookahead).
  - Apply Patch 1 (ARM64 NEON `lz_extend_neon`), Patch 2 (multi-candidate prefetch), and Patch 3 (fused 64-bit bitstream packing).

### Component 2: `Tests/TTZipTests/`

- **[MODIFY] `ZipSingleCoreParetoFrontierPkTests.swift`**:
  - Verify that TTZip Tier 3 ($\le 3.34\text{ MB}$, $\ge 1.20\text{ GB/s}$) and Tier 4 ($\le 3.21\text{ MB}$, $\ge 800\text{ MB/s}$) strictly dominate libdeflate Level 3 and Level 6.

---

## Verification Plan

- `swift test -c release --filter SingleCoreDeflatePkTests`: Assert single-core throughput and space savings floors.
- `swift test -c release --filter SingleCoreDeflateOracleTests`: Assert 100% round-trip SHA-256 integrity.
- `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter ZipSingleCoreParetoFrontierPkTests`: Assert complete upper-right Pareto dominance.
