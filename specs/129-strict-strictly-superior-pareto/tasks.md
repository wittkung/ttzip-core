# Implementation Tasks: Strict Dual-Axis Pareto Superiority over libdeflate

**Branch**: `129-strict-strictly-superior-pareto` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

---

## Phase 1: Setup & Pre-Flight

- [x] T001 [P] Configure Spectrum Struct Alignment & Q8.8 LUT in Sources/CTTZipBridge/ttzip_zopfli_engine.c

---

## Phase 2: User Story 1 - Fast-Path Level 1 Dual Superiority (Priority: P1) 🎯 MVP

**Goal**: Exceed libdeflate L1 in throughput (>= 8.0 GB/s on Binary, >= 8.2 GB/s on JSON) AND achieve strictly smaller compressed size.

- [x] T002 [US1] Implement 2-stage pipelined lookahead and 64-bit SWAR match verification in Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c
- [x] T003 [US1] Verify Level 1 dual superiority on JSON and Binary in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift

---

## Phase 3: User Story 2 - Mid-Tier (Levels 2..9) Dual Superiority (Priority: P1)

**Goal**: For each level L in 2..9, deliver a TTZip point strictly faster AND strictly smaller than libdeflate L across all 4 corpora.

- [x] T004 [US2] Implement HT-4 Fast-Lazy (Levels 2..5) and Chained Deep-Lazy (Levels 6..9) in Sources/CTTZipBridge/native_deflate/ttzip_deflate_lazy.c
- [x] T005 [US2] Update spectrum routing in Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c
- [x] T006 [US2] Verify Levels 2..9 dual superiority across all 4 corpora in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift

---

## Phase 4: User Story 3 - Peak-Tier (Levels 10..15) Dual Superiority (Priority: P1)

**Goal**: Surpass libdeflate Levels 10..12 in throughput (>= 150 MB/s) while achieving strictly smaller compressed sizes across all 4 corpora.

- [x] T007 [US3] Implement Cached-Match Q8.8 Graph DP multi-pass engine in Sources/CTTZipBridge/ttzip_zopfli_engine.c
- [x] T008 [US3] Verify Levels 10..15 dual superiority in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift

---

## Phase 5: User Story 4 - Automated 100% Strict Superiority Verification & CI Gate (Priority: P1)

**Goal**: Enforce strict pointwise dual superiority ($S(q) > S(p) \land \text{Size}(q) < \text{Size}(p)$) on 100% of points (40/40), pass 1,138 tests, and push to GitHub.

- [x] T009 [US4] Add strict dual superiority assertion in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift
- [x] T010 [US4] Run full 1v1 duel suite and generate 4 high-resolution Pareto PNG charts
- [x] T011 Run full regression suite (1138+ tests) via swift test
- [x] T012 Run 13 hard performance gates via swift test --filter XCTestPerformanceMeasureTests
- [x] T013 Commit and push to git with 6-stage pre-push CI hook validation

