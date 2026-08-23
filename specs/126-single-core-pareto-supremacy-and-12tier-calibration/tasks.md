# Implementation Tasks: Single-Core 12-Tier Deflate Calibration and Full Pareto Frontier Supremacy

**Branch**: `126-single-core-pareto-supremacy-and-12tier-calibration` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

---

## Phase 1: Setup & Pre-Flight

**Purpose**: Baseline measurement and environment validation

- [x] T001 [P] Establish baseline benchmark capture in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift
- [x] T002 [P] Inspect matchfinder header types and memory alignment in Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.h

---

## Phase 2: Foundational (12-Tier Architecture Foundations)

**Purpose**: Core data models and matchfinder struct definitions required across all user stories

**⚠️ CRITICAL**: Must be completed before user story implementation

- [x] T003 [P] Define 12-tier parameters and matchfinder structures in Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.h
- [x] T004 [P] Update software family models and tier metadata in Sources/TTZipCore/Benchmark/ParetoFrontierModels.swift
- [x] T005 Implement 12-tier monotonic parameter mapping and level routing in Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c
- [x] T006 Implement 12-tier unified compression API routing in Sources/CTTZipBridge/ttzip_zopfli_engine.c

**Checkpoint**: 12-tier foundational pipeline in place and compiling cleanly

---

## Phase 3: User Story 1 - 严格单调的 12 阶参数梯度校准 (Priority: P1) 🎯 MVP

**Goal**: Eliminate internal self-domination and ratio inversions ($L_3 \sim L_9$) across all corpora with guaranteed $\text{Size}(L_{k+1}) < \text{Size}(L_k)$.

**Independent Test**: Execute 12-tier sweep on 100MB Mixed Workspace and assert monotonic size decrease across all 12 levels.

### Tests for User Story 1
- [x] T007 [P] [US1] Write 12-tier strict monotonicity assertion tests in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift

### Implementation for User Story 1
- [x] T008 [US1] Implement monotonic chain search depths (0 to 128) and nice match lengths in Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c
- [x] T009 [US1] Implement tiered Zopfli iterations (2, 5, 15) and dynamic block splitting in Sources/CTTZipBridge/ttzip_zopfli_engine.c

**Checkpoint**: User Story 1 complete; 12 levels show strictly monotonic size progression on mixed workspace.

---

## Phase 4: User Story 2 - ARM64 NEON 4-Way Compact Lazy Matcher (Priority: P1)

**Goal**: Fill the 59x speed vacuum between $L_2$ and $L_3$, achieving $\ge 800\text{ MB/s}$ and $\le 3.20\text{ MB}$ on enwik8 100MB (beating `libdeflate L6`).

**Independent Test**: Run `ZipSingleCoreParetoFrontierPkTests` on enwik8 and assert $L_4 \ge 800\text{ MB/s}$ and $\le 3.20\text{ MB}$.

### Tests for User Story 2
- [x] T010 [P] [US2] Write enwik8 100MB intermediate segment benchmark assertions in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift

### Implementation for User Story 2
- [x] T011 [US2] Implement 64KB L1 4-way compact bucket table and prefix+tail lookahead filter in Sources/CTTZipBridge/native_deflate/ttzip_deflate_lazy.c
- [x] T012 [US2] Wire 4-way compact lazy matcher into chunk compression dispatcher in Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c

**Checkpoint**: User Story 2 complete; Level 4 achieves $\ge 800\text{ MB/s}$ and beats `libdeflate L6` on enwik8.

---

## Phase 5: User Story 3 - 结构化 JSON 自适应 3-Byte/4-Byte 混合哈希 (Priority: P1)

**Goal**: Boost Level 1 throughput on JSON and structured text to $\ge 5.8\text{ GB/s}$ and reduce size to $\le 0.90\text{ MB}$ (beating `libdeflate L1`).

**Independent Test**: Run `testTTZipVsLibdeflate1v1Duel_Structured_JSON` and assert $L_1 \ge 5.8\text{ GB/s}$ and $\le 0.90\text{ MB}$.

### Tests for User Story 3
- [x] T013 [P] [US3] Write Structured JSON 100MB Level 1 throughput and size test in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift

### Implementation for User Story 3
- [x] T014 [US3] Implement 128KB hybrid 3-byte direct + 4-byte 2-way SWAR matchfinder in Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c
- [x] T015 [US3] Implement dual-literal batch token emission in Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c

**Checkpoint**: User Story 3 complete; JSON Level 1 exceeds 5.8 GB/s and surpasses libdeflate L1 ratio.

---

## Phase 6: User Story 4 - 全语料多模态图表收敛与无死角 Pareto 验证 (Priority: P2)

**Goal**: Render crystal-clear, publication-grade 2x Retina Pareto frontier charts for all 3 datasets with multi-modal visual validation.

**Independent Test**: Generate 3 PNG charts and perform multi-modal visual inspection confirming TTZip forms the outer convex hull.

### Implementation for User Story 4
- [x] T016 [P] [US4] Update Raster Pareto Plotter color schemes, boundaries, and 12-point curves in Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift
- [x] T017 [US4] Execute full multi-corpus 1v1 duels and export 2x retina PNG charts in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift

**Checkpoint**: User Story 4 complete; 3 PNG charts generated and verified.

---

## Phase 7: Polish, Safety & Performance Gates

**Purpose**: End-to-end verification, contract compliance, and zero-regression hard gates

- [x] T018 [P] Verify 100% compliance with contracts/deflate-12tier-calibration-contract.json and contracts/pareto-benchmark-dataset-contract.json
- [x] T019 Run full regression test suite (525+ tests) via swift test
- [x] T020 Run 13 hard performance gates via swift test --filter XCTestPerformanceMeasureTests


---

## Dependencies & Execution Order

```text
Phase 1 (Setup) ──→ Phase 2 (Foundations) ──┬──→ Phase 3 (US1: 12-Tier Monotonic Calibration)
                                            ├──→ Phase 4 (US2: ARM64 NEON 4-Way Lazy Matcher)
                                            ├──→ Phase 5 (US3: JSON Hybrid 3-Byte Hash)
                                            └──→ Phase 6 (US4: Multi-Corpus Pareto Charts)
                                                      │
                                                      ▼
                                            Phase 7 (Gates & Polish)
```
