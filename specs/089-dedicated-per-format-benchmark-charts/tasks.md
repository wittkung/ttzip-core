# Implementation Tasks: Dedicated Per-Format Benchmark Charts & Multi-Software Suite

## Phase 1: Setup & Data Model Extension

- [x] T001 [P] [US1] Define DedicatedFormatSession in Sources/TTZipCore/Benchmark/FormatMatrixTaxonomy.swift
- [x] T002 [P] [US1] Add Apple Native multi-level points (ditto, zip -1, zip -6) in Tests/TTZipTests/SoftwareParetoFrontierPkTests.swift

## Phase 2: Per-Format Dedicated Chart Export (User Story 1 - Dedicated Renderers)

- [x] T003 [US1] Implement per-format filtering and dedicated PNG export in Tests/TTZipTests/SoftwareParetoFrontierPkTests.swift
- [x] T004 [US1] Implement per-format filtering and dedicated SVG export in Tests/TTZipTests/SoftwareParetoFrontierPkTests.swift

## Phase 3: Verification & Report Generation (User Story 2 - Publication-Grade Delivery)

- [x] T005 [US2] Update pareto_frontier_report.md to embed all dedicated charts in a structured multi-format gallery
- [x] T006 [US2] Verify end-to-end multi-format chart generation via swift test --filter SoftwareParetoFrontierPkTests
