# Implementation Plan: Complete Pareto Frontier Convex Hull Dominance

**Branch**: `127-complete-pareto-frontier-convex-hull-dominance` | **Spec**: [spec.md](./spec.md) | **Research**: [research.md](./research.md)

---

## 1. Technical Context & Objectives

Achieve complete convex hull enclosure and dominance across all 4 benchmark datasets:
1. **JSON 100MB**: Upgrade `ttzip_deflate_hybrid_fast_find_matches` with 64KB direct 3-byte hash and dual-issue SWAR verification to exceed $6.2\text{ GB/s}$.
2. **Binary 100MB**: Add 4-byte instruction word-stride matching to exceed $7.5\text{ GB/s}$.
3. **Mixed Workspace 100MB**: Add early lazy cutoff (`cur_match.length >= 16`) to `ttzip_deflate_fast_lazy_find_matches` and route L2..L5 through the 64KB HT-4 compact table to exceed $350\text{ MB/s}$ and beat `libdeflate L4..L9`.
4. **Single-Core Full PK (enwik8)**: Update `testZipSingleCoreParetoFrontier` to sweep the native 12-tier engine spectrum.

---

## 2. Phase 0 Research Items
- R001 [SUBAGENT:research] 《Direct 3-Byte Direct Index Hash Vectorization for JSON》: Solved in research.md.
- R002 [SUBAGENT:research] 《ARM64 Word-Stride Matchfinder for Binary Mach-O》: Solved in research.md.
- R003 [SUBAGENT:research] 《4-Way Compact Lazy Matchfinder Tuning for Levels 2..5》: Solved in research.md.

---

## 3. Phase 1 Design Artifacts & Contracts
- `data-model.md`: Defined `DeflateTierConfig`, `ParetoBenchmarkPoint`, and matchfinder struct topologies.
- `contracts/pareto-frontier-schema.json` [SUBAGENT:research]: JSON schema for Pareto benchmark reports.
- `quickstart.md`: Verification commands and diagnostics.

---

## 4. Planned Changes by Component

### Component A: Fast Matchfinder Optimization (`Sources/CTTZipBridge/native_deflate/`)
- `ttzip_deflate_fast.c`:
  - Upgrade `ttzip_deflate_hybrid_fast_find_matches` with 15-bit direct 3-byte multiplicative hash `hash3_tab[32768]` and dual-issue SWAR verification (`diff = w_cur ^ w_cand`).
  - Add 4-byte word-stride fast path for instruction-aligned binary payloads.

### Component B: Compact Lazy Matchfinder Optimization (`Sources/CTTZipBridge/native_deflate/`)
- `ttzip_deflate_lazy.c`:
  - In `ttzip_deflate_fast_lazy_find_matches`, add early lazy cutoff when `cur_match.length >= 16` to short-circuit candidate probing at $i+1$.
  - In `ttzip_deflate_engine.c`, route Levels 2, 3, 4, and 5 through `ttzip_deflate_fast_lazy_find_matches` with calibrated probe depths (L2: 1, L3: 2, L4: 4, L5: 4 with nice_len 48).

### Component C: Single-Core Full PK Benchmark Alignment (`Tests/TTZipTests/`)
- `ZipSingleCoreParetoFrontierPkTests.swift`:
  - Align `testZipSingleCoreParetoFrontier` to execute the native 12-tier spectrum (`Tier 0..12`).
  - Ensure all 4 duels export 2x Retina PNG charts and assert complete convex hull enclosure.

---

## 5. Verification Plan
- `swift test --filter ZipSingleCoreParetoFrontierPkTests` (assert all 4 duels pass).
- `swift test --filter XCTestPerformanceMeasureTests` (assert 13 performance floors pass).
- `swift test` (assert full suite 1138+ tests pass).
