# Implementation Plan: Near-Optimal DP Acceleration and Full-Spectrum Pareto Supremacy

## Technical Context

Following the user's tier rebalancing in `ZipCompressionProfile.swift` and `ZipSingleCoreParetoFrontierPkTests.swift`, the compression engine features an 8-tier hierarchy:
- Tier 0: Store (0)
- Tier 1: Fast (1) (deflateLevel: 1)
- Tier 2: Normal (2) (deflateLevel: 2 -> Level 3 fast lazy)
- Tier 3: Maximum (3) (deflateLevel: 6 -> Level 6 deep lazy)
- Tier 4: High (4) (deflateLevel: 12 -> Level 12 near-optimal DP parser)
- Tier 5..7: Graph Fast / Ultra Zopfli / Extreme Peak (Zopfli 2/5/15 passes)

The mission is to accelerate Tier 4 Near-Optimal DP (Level 12) from 12.1 MB/s to $\ge 35\text{ MB/s}$ via Pareto edge pruning and pass convergence ladder rescaling, while accelerating Tier 2 ($\ge 1.20\text{ GB/s}$) and Tier 3 ($\ge 880\text{ MB/s}$), establishing an unbroken outer Pareto frontier across all bitrates.

## Constitution Check

- **Zero-Cost Abstraction on Hot Paths**: In-place DP relaxation without dynamic allocations; match nodes array preallocated.
- **Hardware SIMD Vectorization**: Vector cost evaluation and ARM64 branchless select.
- **Frozen File Compliance**: Zero edits to frozen files (`ZipParallelExtractor.swift`, etc.).
- **Contract Strictness**: `contracts/near-optimal-dp-contract.json` validates against Draft-07.

## Phase 0: Research Summary

- **R001 [SUBAGENT:research]**: Length-slot endpoint pruning in `deflate_find_min_cost_path` (reducing edge evaluations by 88.6%), rescaling Level 12 `max_optim_passes = 4` and `min_improvement_to_continue = 16`, with branchless `csel` DP transitions.
- **R002 [SUBAGENT:research]**: 64-bit SWAR signature filtering in `hc_matchfinder` and NEON vector lookup for symbol cost evaluations.

## Phase 1: Design Artifacts

- [data-model.md](./data-model.md): Type definitions for `NearOptimalDPOptions`, `DualOrderSignatureProbe`, `SpectrumTierProfile`.
- [contracts/near-optimal-dp-contract.json](./contracts/near-optimal-dp-contract.json): Strict JSON Schema validation contract.
- [quickstart.md](./quickstart.md): Executable verification scenarios.

## Proposed Changes

### Component 1: `Vendor/libdeflate-upstream/lib/` & `Sources/CTTZipBridge/`

- **[MODIFY] `Vendor/libdeflate-upstream/lib/deflate_compress.c`**:
  - Implement length-slot boundary pruning in `deflate_find_min_cost_path`.
  - Rescale `max_optim_passes` and `min_improvement_to_continue` for Level 10..12 in `libdeflate_alloc_compressor_ex`.
  - Rebuild `Vendor/lib/libdeflate.a` and `Vendor/libTTZipVendor.a` via `scripts/build_libdeflate.sh`.

### Component 2: `Sources/CTTZipBridge/native_deflate/` & `Sources/TTZipCore/`

- **[MODIFY] `native_deflate/ttzip_deflate_engine.c`**:
  - Verify seamless dispatch across Tier 1 (Fast), Tier 2 (Normal), Tier 3 (Maximum), and Tier 4 (High).

### Component 3: `Tests/TTZipTests/`

- **[MODIFY] `ZipSingleCoreParetoFrontierPkTests.swift`**:
  - Execute full single-core Pareto PK with all 8 tiers and assert 100% outer hull dominance.

---

## Verification Plan

- `swift test -c release --filter SingleCoreDeflatePkTests`: Micro-benchmark asserting DP throughput floors.
- `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter ZipSingleCoreParetoFrontierPkTests`: 100MB `enwik8` live single-core PK asserting all 8 tiers on Pareto hull.
