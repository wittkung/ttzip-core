# Tasks: ZIP 7-Tier Graph-Theoretic & Advzip Conquest Hierarchy

**Feature ID**: `099-zip-graph-and-advzip-hierarchy`  
**Status**: In Progress  

---

## Dependencies & Execution Order

```
Phase 1 (Types & Levels) ──→ Phase 2 (C & Block Writer Engines) ──→ Phase 3 (Plotter & UI) ──→ Phase 4 (Tests & Gates)
```

---

## Phase 1: Core Type System & 7-Tier Hierarchy Setup

- [ ] T001 [US1] Update `ArchiveCompressionLevel` with `.level1` through `.level7` support and `effectiveZipRawLevel` in `Sources/TTZipCore/ArchiveCompressionTypes.swift`
- [ ] T002 [US1] Update `ArchiveCompressionFormat.zip.supportedLevels` to include all 7 golden tiers in `Sources/TTZipCore/ArchiveCompressionTypes.swift`

---

## Phase 2: High-Speed Graph DAG & Advzip-4 Peak Engines

- [ ] T003 [P] [US2] Implement Bounded-Lookahead DAG Shortest-Path routing for Level 5 (150~400 MB/s @ ~96.85%) in `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`
- [ ] T004 [P] [US3] Implement 15-Pass Iterative Reweighting & Dynamic Block Splitting for Level 7 (Peak $\ge 97.02\%$, beats advzip-4) in `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`

---

## Phase 3: Pareto Plotter (Compressed Size in MB) & UI Alignment

- [ ] T005 [P] [US4] Reconstruct `RasterParetoPlotter` X-axis to display physical compressed size in MB (smaller is better, oriented to the right) in `Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift`
- [ ] T006 [P] [US5] Update 7-tier labels and tile descriptions in `Sources/TTZipApp/Views/Components/CompressIntegratedConfigSectionView.swift`

---

## Phase 4: Benchmark Verification, Test Suite & CI Convergence

- [ ] T007 [US6] Update `ZipMultiCoreParetoFrontierPkTests` to test and render all 7 golden tiers in `Tests/TTZipTests/ZipMultiCoreParetoFrontierPkTests.swift`
- [ ] T008 [US6] Execute full regression and performance gate suite to assert zero regression across all formats
