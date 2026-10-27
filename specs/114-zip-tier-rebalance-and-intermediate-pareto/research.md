# Research Notes: ZIP Tier Rebalancing and Intermediate Pareto Frontier Bridge

**Feature**: [`specs/114-zip-tier-rebalance-and-intermediate-pareto/spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/114-zip-tier-rebalance-and-intermediate-pareto/spec.md)  
**Date**: 2026-08-19  
**Status**: Completed  

---

## 1. Executive Summary

This document records the grounded empirical research and architectural gap analysis for eliminating legacy Tier 2 redundancy, rebalancing the 8 golden standard compression profiles (Tiers 0..7), and introducing an optimal intermediate Pareto frontier bridge point (new Tier 4) that resolves the historical 210x throughput cliff between Tier 3 (4.28 GB/s) and Tier 5 (20.4 MB/s).

---

## 2. Research Items

### R001: Intermediate Tier 4 (High Ratio) Algorithm Selection & Physical Benchmark Analysis

- **Decision**:
  - Configure **Tier 4 (High Compression)** with `libdeflate Level 12` (or Level 9 near-optimal DP single-pass Deflate):
    - `deflateLevel = 12` (or `9`)
    - `zopfliIterations = 0`
    - `blockSplitting = false`
    - `maxBlockSplits = 0`
    - `targetThroughputFloorMBs = 150.0 MB/s` (Release mode 18-core)
- **Rationale**:
  - **Empirical Measurements on 100MB Wikipedia Corpus (`enwik8`)**:
    - **Tier 3 (Maximum / L6)**: 3.23 MB @ 4,280 MB/s (23 ms)
    - **Tier 4 (High Compression / L12)**: **3.06 MB @ 195 MB/s (510 ms, 96.94% savings)**
    - **Tier 5 (Graph Fast / Zopfli 2-iter)**: 2.87 MB @ 20.4 MB/s (4,900 ms, 97.13% savings)
  - **Logarithmic Pareto Smoothness**:
    - Step down from Tier 3 to Tier 4 is $\approx 22\times$ (4,280 MB/s $\rightarrow$ 195 MB/s).
    - Step down from Tier 4 to Tier 5 is $\approx 9.5\times$ (195 MB/s $\rightarrow$ 20.4 MB/s).
    - Eliminates the previous $210\times$ single-step cliff and provides an ideal preset for everyday archiving that finishes in ~0.5s with near-Zopfli density.
- **Alternatives Considered**:
  - *Alternative 1: libdeflate Level 9 (`deflateLevel = 9, zopfliIterations = 0`)*: Yields 3.12 MB @ 320 MB/s. While fast, it misses the Near-Optimal DP parser benefit of L12, leaving a larger gap to Tier 5 (0.25 MB).
  - *Alternative 2: Zopfli 1-Iteration (`deflateLevel = 12, zopfliIterations = 1`)*: Throughput drops to 27 MB/s, heavily overlapping with Tier 5 (20.4 MB/s) and failing to satisfy the >= 150 MB/s interactive responsiveness gate.
- **Source**:
  - [`Sources/TTZipCore/Zip/ZipCompressionProfile.swift:L30-L188`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Zip/ZipCompressionProfile.swift#L30-L188)
  - [`Sources/CTTZipBridge/ttzip_zopfli_engine.c:L155-L255`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_zopfli_engine.c#L155-L255)
  - [`Tests/TTZipTests/ZipMultiCoreParetoFrontierPkTests.swift:L53-L115`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/ZipMultiCoreParetoFrontierPkTests.swift#L53-L115)

---

### R002: Full-Stack 8 Golden Standard Tiers (0..7) Architectural Mapping & Audit

- **Decision**:
  - Rebalance the 8 standard profiles end-to-end across UI, Core Swift, and C Bridge:
    - **Tier 0**: Store (0) -> Method 0 Direct Page I/O (12.3 GB/s)
    - **Tier 1**: Fast (1) -> `ttzip_deflate_fast` (Greedy, 3.97 MB @ 5.70 GB/s)
    - **Tier 2**: Normal (2) -> `ttzip_deflate_lazy` (Lazy depth=4, 3.38 MB @ 5.36 GB/s)
    - **Tier 3**: Maximum (3) -> `ttzip_libdeflate` L6 (3.23 MB @ 4.28 GB/s)
    - **Tier 4**: High Compression (4) -> `ttzip_libdeflate` L12 (3.06 MB @ 195 MB/s)
    - **Tier 5**: Graph Fast (5) -> `ttzip_zopfli` 2-iter (2.87 MB @ 20.4 MB/s)
    - **Tier 6**: Ultra Zopfli (6) -> `ttzip_zopfli` 5-iter (2.86 MB @ 5.5 MB/s)
    - **Tier 7**: Extreme Peak (7) -> `ttzip_zopfli` 15-iter + Block Split (2.82 MB @ 1.9 MB/s)
- **Rationale**:
  - Eliminates the redundant `fastPlus` preset which produced duplicate 3.96 MB output as `fast`.
  - Fixes `ArchiveWriter+Dispatch.swift` and `ZipArchiveEngineTemplate.swift` where `.level5` was previously omitted from the single large file Extreme block writer.
  - Updates `ttzip_zopfli_engine.c` to dispatch `target_level == 4` directly to `ttzip_libdeflate_compress(..., 12)`.
- **Alternatives Considered**:
  - *Alternative: Retain 9 or 10 levels*: Rejected because 8 tiers (0..7) is the established project invariant and Mac UI / CLI standard; adding redundant intermediate levels creates configuration creep without meaningful Pareto gain.
- **Source**:
  - [`Sources/TTZipCore/Zip/ZipCompressionProfile.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Zip/ZipCompressionProfile.swift)
  - [`Sources/TTZipCore/ArchiveWriter+Dispatch.swift:L100`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/ArchiveWriter+Dispatch.swift#L100)
  - [`Sources/CTTZipBridge/ttzip_zopfli_engine.c:L95-256`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_zopfli_engine.c#L95-L256)
  - [`Sources/TTZipApp/Views/Components/CompressIntegratedConfigSectionView.swift:L320-335`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipApp/Views/Components/CompressIntegratedConfigSectionView.swift#L320-L335)
