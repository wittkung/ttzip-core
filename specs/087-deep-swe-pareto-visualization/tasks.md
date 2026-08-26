# Implementation Tasks: Academic-Grade Pareto Frontier Visualization

## Phase 1: Setup & Data Model Extension

- [x] T001 [P] [US1] Define SoftwareFamily enum and SoftwareFamilyTrajectory model in Sources/TTZipCore/Benchmark/ParetoFrontierModels.swift
- [x] T002 [P] [US1] Implement SoftwareFamilyClassifier pattern matching in Sources/TTZipCore/Benchmark/ParetoFrontierModels.swift

## Phase 2: CoreGraphics DeepSWE Raster Plotter (User Story 1 - PNG Rendering)

- [x] T003 [US1] Implement Fritsch-Carlson monotone cubic spline curve generation in Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift
- [x] T004 [US1] Implement dynamic focus window with nice-step selector in Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift
- [x] T005 [US1] Implement 8-slot greedy AABB collision avoidance label layout in Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift
- [x] T006 [US1] Implement Hero blue pill badge and halo ribbon beam rendering in Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift

## Phase 3: Vector SVG DeepSWE Plotter (User Story 2 - SVG Rendering)

- [x] T007 [P] [US2] Implement standalone pure white DeepSWE vector layout in Sources/TTZipCore/Benchmark/SVGParetoPlotter.swift
- [x] T008 [P] [US2] Implement Cubic Bézier trajectory path and ribbon SVG export in Sources/TTZipCore/Benchmark/SVGParetoPlotter.swift

## Phase 4: Integration & Benchmark PK Harness (User Story 3 - 100MB Verification)

- [x] T009 [US3] Integrate software family clustering and DeepSWE chart export in Tests/TTZipTests/SoftwareParetoFrontierPkTests.swift
- [x] T010 [US3] Verify zero memory leaks, sub-15ms rendering performance and run end-to-end regression tests via swift test --filter SoftwareParetoFrontierPkTests
