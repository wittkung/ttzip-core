# Implementation Plan: Strict Pointwise Pareto Dominance over libdeflate

**Feature ID**: `128-strict-pointwise-pareto-dominance` | **Spec**: [spec.md](./spec.md)

---

## 1. Technical Context & Constitution Check

- **Goal**: Guarantee that for every libdeflate evaluation point, TTZip has at least one test point located strictly in its upper-right quadrant (faster throughput AND smaller compressed size).
- **Core Files**:
  - `Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c` (Level 1 Fast Matchfinder)
  - `Sources/CTTZipBridge/native_deflate/ttzip_deflate_lazy.c` (Level 2..9 Mid/Deep Matchfinder)
  - `Sources/CTTZipBridge/ttzip_zopfli_engine.c` (Level 10..12 Near-Optimal / Zopfli Engine)
  - `Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift` (1v1 Duels and Automated Dominance Assertions)
- **Constitution Authority**:
  - Zero bare objects in contracts.
  - Zero CI/CD bypass (`--no-verify` forbidden).
  - 100% byte-for-byte decompression roundtrip verified with system `/usr/bin/unzip -t`.

---

## 2. Phase 0: Research Items

- R001 [SUBAGENT:research] 《Level 1 4-Byte Stride & Prefetch Fast-Path Vectorization》: Research how to increase Level 1 single-core throughput from 5.45 GB/s to >= 6.5 GB/s on JSON and from 5.87 GB/s to >= 7.8 GB/s on Binary Mach-O on Apple Silicon ARM64.
- R002 [SUBAGENT:research] 《Compact HT-4 Matchfinder Mid-Tier Calibration for Level 2..5》: Research depth and nice match length parameters for Level 2..5 to achieve >= 2.2 GB/s on JSON/Binary and >= 450 MB/s on Mixed Workspace while beating libdeflate L2..L5 compressed sizes.
- R003 [SUBAGENT:research] 《Deep Lazy Matchfinder Calibration for Level 6..9》: Research hash chain depth and SWAR vector probe loop for Level 6..9 to achieve >= 750 MB/s at <= 3.20 MB on enwik8.

---

## 3. Phase 1: Design Artifacts & Contracts

- [contracts/pointwise-dominance-schema.json](./contracts/pointwise-dominance-schema.json)
- [data-model.md](./data-model.md)
- [quickstart.md](./quickstart.md)

---

## 4. Phase 2: Implementation Breakdown

- **Component A**: Level 1 Fast Matchfinder Optimization (`ttzip_deflate_fast.c`)
- **Component B**: Level 2..5 Mid-Tier Matchfinder Optimization (`ttzip_deflate_lazy.c`, `ttzip_deflate_engine.c`)
- **Component C**: Level 6..9 Deep Lazy Matchfinder Optimization (`ttzip_deflate_lazy.c`, `ttzip_zopfli_engine.c`)
- **Component D**: Automated Pointwise Dominance Assertions & Chart Exporters (`ZipSingleCoreParetoFrontierPkTests.swift`)
