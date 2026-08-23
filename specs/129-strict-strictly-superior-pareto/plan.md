# Implementation Plan: Strict Dual-Axis Pareto Superiority over libdeflate

**Feature ID**: `129-strict-strictly-superior-pareto` | **Spec**: [spec.md](./spec.md)

---

## 1. Technical Context & Constitution Check

- **Goal**: Formulate and implement compression engine strategies so that for every single libdeflate evaluation point $p$, TTZip has at least one point $q$ strictly faster AND strictly smaller ($S(q) > S(p) \land \text{Size}(q) < \text{Size}(p)$).
- **Core Files**:
  - `Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c` (Level 1 Fast Matchfinder)
  - `Sources/CTTZipBridge/native_deflate/ttzip_deflate_lazy.c` (Level 2..9 Mid/Deep Matchfinder)
  - `Sources/CTTZipBridge/ttzip_zopfli_engine.c` (Level 10..15 Near-Optimal / Zopfli Engine)
  - `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c` (Spectrum Dispatcher)
  - `Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift` (1v1 Benchmark Duels & Strict Dominance Assertions)
- **Constitution Check**:
  - Zero CI/CD bypass (`--no-verify` forbidden).
  - 100% byte-for-byte decompression roundtrip verified with system `/usr/bin/unzip -t`.
  - Zero bare object JSON schemas in contracts.

---

## 2. Phase 0: Research Items

- R001 [SUBAGENT:research] 《Ultra-Fast Vectorized Matchfinder for Level 1 JSON & Binary Domination》: Research how to lift TTZip Level 1 throughput to >= 8.2 GB/s on Binary and >= 6.8 GB/s on JSON while maintaining smaller compressed sizes than libdeflate L1.
- R002 [SUBAGENT:research] 《Mid-Tier Near-Optimal Dynamic Depth Scaling for Levels 2..9》: Research how to achieve simultaneous speed and size superiority over libdeflate L2..L9 across all corpora.
- R003 [SUBAGENT:research] 《Zopfli Graph DP Iteration Calibration for Levels 10..15》: Research how to achieve higher throughput than libdeflate L10..L12 (>= 150 MB/s) with strictly smaller compressed sizes on all corpora.

---

## 3. Phase 1: Design Artifacts & Contracts

- [contracts/strict-superiority-schema.json](./contracts/strict-superiority-schema.json)
- [data-model.md](./data-model.md)
- [quickstart.md](./quickstart.md)

---

## 4. Phase 2: Implementation Breakdown

- **Component A**: Level 1 Fast Matchfinder Acceleration (`ttzip_deflate_fast.c`)
- **Component B**: Mid-Tier Deep Search & Dynamic Huffman Optimization (`ttzip_deflate_lazy.c`, `ttzip_deflate_engine.c`)
- **Component C**: Peak-Tier Zopfli DP Iteration Matrix (`ttzip_zopfli_engine.c`)
- **Component D**: Strict Dual-Axis Superiority Assertions (`ZipSingleCoreParetoFrontierPkTests.swift`)
