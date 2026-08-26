# Tasks: TurboBench 4D Architecture Evolution Suite

## Dependencies & User Story Order

- **Phase 1: Setup** → Validate contracts and base build configuration.
- **Phase 2: Foundational Architecture** → Shared models and C bridge thermal accessors, blocking prerequisites for all User Stories.
- **Phase 3: User Story 1 (P1)** → Pareto Frontier Skyline calculation, Terminal Unicode Braille plotting, and SVG vector graphic generation.
- **Phase 4: User Story 2 (P2)** → Thermal Throttling Guard with async actor and Transfer Speed Sheet turnaround latency projector.
- **Phase 5: User Story 3 (P3)** → Smart Codec Scenario Selector with rapid entropy probe and Virtual Multi-Block Memory Arena.
- **Phase 6: Polish & CI Integration** → Regression tests, performance floor validation, and CLI wiring.

---

## Phase 1: Setup & Project Infrastructure

- [x] T001 [P] Validate contract schemas and project configuration in `specs/086-turbobench-architecture-evolution/contracts/`
- [x] T002 [P] Declare C bridge thermal state atomic prototypes in `Sources/CTTZipBridge/include/CTTZipPlatform.h`

---

## Phase 2: Foundational Architecture

- [x] T003 [P] Implement C runtime atomic thermal state accessors in `Sources/CTTZipBridge/CTTZipPlatform.c`
- [x] T004 [P] Implement Codable data structures for Pareto, Transfer Sheet, and Recommendations in `Sources/TTZipCore/Benchmark/ParetoFrontierModels.swift`

---

## Phase 3: User Story 1 (P1) - Pareto Frontier Analysis & Triple-Tier Visualization

*Goal: Compute 2D Pareto Skyline and Monotone Convex Hull, rendering to terminal Unicode Braille canvas and standalone responsive SVG.*  
*Independent Test: `swift run ttzip-cli bench --in-memory --pareto --plot --svg-out pareto.svg`*

- [x] T005 [P] [US1] Implement 2D Skyline sweep-line and Andrew's Monotone Chain in `Sources/TTZipCore/Benchmark/ParetoFrontierCalculator.swift`
- [x] T006 [P] [US1] Implement $8\times$ sub-pixel Unicode Braille dot matrix rasterizer in `Sources/TTZipCore/Benchmark/TerminalParetoPlotter.swift`
- [x] T007 [P] [US1] Implement zero-dependency responsive dark/light standalone SVG generator in `Sources/TTZipCore/Benchmark/SVGParetoPlotter.swift`
- [x] T008 [US1] Create unit tests for Pareto dominance, convex hull, and Braille rasterization in `Tests/TTZipTests/ParetoFrontierCalculatorTests.swift`
- [x] T009 [US1] Create SVG validation and W3C compliance tests in `Tests/TTZipTests/SVGParetoPlotterTests.swift`

---

## Phase 4: User Story 2 (P2) - Thermal Throttling Guard & Transfer Speed Sheet

*Goal: Prevent DVFS downclocking jitter with adaptive cool-down scheduling, and project physical media turnaround times across 10G/1G/NVMe/WAN.*  
*Independent Test: `swift run ttzip-cli bench --in-memory --thermal-guard --transfer-sheet`*

- [x] T010 [P] [US2] Implement asynchronous `HardwareThermalCoordinator` actor in `Sources/TTZipCore/Platform/HardwareThermalCoordinator.swift`
- [x] T011 [P] [US2] Implement multi-tier media turnaround latency calculator in `Sources/TTZipCore/Benchmark/TransferSpeedSheetCalculator.swift`
- [x] T012 [US2] Integrate thermal guard pause/cooldown into `Sources/TTZipCore/Benchmark/InMemoryBenchmarkEngine.swift`
- [x] T013 [US2] Create unit tests for thermal state transitions and transfer sheet formulas in `Tests/TTZipTests/ThermalAndTransferSheetTests.swift`

---

## Phase 5: User Story 3 (P3) - Smart Codec Scenario Selector & Virtual Multi-Block Arena

*Goal: Automatically evaluate data entropy in $< 10\text{ ms}$ to recommend optimal codecs, and stage small files in a contiguous memory arena.*  
*Independent Test: `swift run ttzip-cli bench --recommend --scenario airdrop -i Sources/TTZipCore/Zip/ZipParallelExtractor.swift`*

- [x] T014 [P] [US3] Implement 3-stage cascaded entropy prober and scenario selector in `Sources/TTZipCore/Services/SmartCodecSelector.swift`
- [x] T015 [P] [US3] Implement contiguous memory arena for small-file batch staging in `Sources/TTZipCore/Buffer/VirtualMultiBlockArena.swift`
- [x] T016 [US3] Create unit tests for sub-10ms entropy probing and scenario mapping in `Tests/TTZipTests/SmartCodecSelectorTests.swift`
- [x] T017 [US3] Create unit tests for virtual multi-block memory arena in `Tests/TTZipTests/VirtualMultiBlockArenaTests.swift`

---

## Phase 6: Polish & CLI Integration

- [x] T018 [P] Add CLI options (`--pareto`, `--plot`, `--svg-out`, `--png-out`, `--thermal-guard`, `--transfer-sheet`, `--recommend`, `--scenario`) in `Sources/TTZipCore/CLI/CLIOptions.swift` and `Sources/TTZipCore/CLI/POSIXCLIArgumentParser.swift`
- [x] T019 Route and dispatch Pareto plotting, SVG/PNG export, and transfer sheets in `Sources/TTZipCLI/CLIBenchmarkRunner.swift` and `Sources/TTZipCLI/CLICommandRouter.swift`
- [x] T020 Run full regression suite and assert 100% green pass: `swift test`
- [x] T021 Execute `@speckit-converge` and `@code-review` checks across all modified and newly created files
