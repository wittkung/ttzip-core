# Implementation Tasks: Strict Pointwise Pareto Dominance over libdeflate

**Branch**: `128-strict-pointwise-pareto-dominance` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

---

## Phase 1: Setup & Pre-Flight

- [x] T001 [P] Verify 100% L1D cache residency and struct memory alignment in Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.h

---

## Phase 2: Foundational (Engine Parameter Calibration)

- [x] T002 [P] Update tier option routing for Levels 6..9 in Sources/CTTZipBridge/ttzip_zopfli_engine.c
- [x] T003 Update tier option routing for Levels 2..5 in Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c

---

## Phase 3: User Story 1 - JSON & Binary Level 1 Throughput Domination (Priority: P1) 🎯 MVP

**Goal**: Exceed $6.2\text{ GB/s}$ on Structured JSON 100MB and exceed $7.5\text{ GB/s}$ on Binary Mach-O 100MB while keeping smaller compressed sizes than `libdeflate L1`.

- [x] T004 [US1] Implement pipelined lookahead prefetch `__builtin_prefetch` and adaptive 4-byte stride loop in Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c
- [x] T005 [US1] Verify JSON and Binary Level 1 throughput in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift

---

## Phase 4: User Story 2 - Mid-Tier (Level 2..5) Pointwise Upper-Right Dominance (Priority: P1)

**Goal**: Guarantee Level 2..5 achieve $\ge 2.2\text{ GB/s}$ on JSON/Binary and $\ge 450\text{ MB/s}$ on Mixed Workspace (dominating `libdeflate L2..L5`).

- [x] T006 [US2] Implement early lazy bypass and probe tuning for Level 2..5 in Sources/CTTZipBridge/native_deflate/ttzip_deflate_lazy.c
- [x] T007 [US2] Verify Level 2..5 throughput and compression ratio across all 4 corpora in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift

---

## Phase 5: User Story 3 - Deep Tier (Level 6..9) and Peak Ratio Calibration (Priority: P1)

**Goal**: Guarantee Level 6..9 achieve $\ge 780\text{ MB/s}$ at $\le 3.20\text{ MB}$ on enwik8 (dominating `libdeflate L6..L9`).

- [x] T008 [US3] Calibrate chain depth (8/16/32/64) and nice match lengths in Sources/CTTZipBridge/native_deflate/ttzip_deflate_lazy.c
- [x] T009 [US3] Verify enwik8 Level 6..9 throughput and ratio in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift

---

## Phase 6: User Story 4 - Automated Pointwise Dominance Assertions & Full Suite Verification (Priority: P1)

**Goal**: Add automated pointwise assertions for all libdeflate points and export verified Retina PNG Pareto charts.

- [x] T010 [US4] Add automated pointwise dominance assertions ($S_{\text{ttzip}} \ge S_{\text{lib}} \land R_{\text{ttzip}} \ge R_{\text{lib}}$) in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift
- [x] T011 [US4] Run full 1v1 duel suite and verify all 4 generated Pareto charts in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift
- [x] T012 Run full regression test suite (1138+ tests) via swift test
- [x] T013 Run 13 hard performance gates via swift test --filter XCTestPerformanceMeasureTests

