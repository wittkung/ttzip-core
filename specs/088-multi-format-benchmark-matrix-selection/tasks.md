# Implementation Tasks: Multi-Tier Format Selection & Benchmark Architecture

## Phase 1: Setup & Format Matrix Taxonomy

- [x] T001 [P] [US1] Create BenchmarkFormatTier, FormatMatrixPreset, and CompositeScoreReport models in Sources/TTZipCore/Benchmark/FormatMatrixTaxonomy.swift
- [x] T002 [P] [US1] Implement Geometric Mean composite score & Pareto Efficiency Index (PEI) calculator in Sources/TTZipCore/Benchmark/FormatMatrixTaxonomy.swift

## Phase 2: CLI Integration (User Story 1 - Format Matrix Selector)

- [x] T003 [US1] Add --format-matrix CLI argument support (4tier, classic, modern, all16) in Sources/TTZipCore/CLI/CLIOptions.swift and POSIXCLIArgumentParser.swift
- [x] T004 [US1] Connect format matrix selector to CLIBenchmarkRunner in Sources/TTZipCLI/CLIBenchmarkRunner.swift

## Phase 3: Automated PK & Visualization Linking (User Story 2 - Multi-Tier Verification)

- [x] T005 [US2] Update SoftwareParetoFrontierPkTests to compute and log 4-Tier composite scores and PEI in Tests/TTZipTests/SoftwareParetoFrontierPkTests.swift
- [x] T006 [US2] Verify end-to-end composite ranking correctness and ensure zero performance regressions via swift test --filter SoftwareParetoFrontierPkTests
