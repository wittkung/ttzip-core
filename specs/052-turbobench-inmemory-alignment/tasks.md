# Tasks: TurboBench & lzbench In-Memory Benchmarking & High-Precision Timer Calibration Suite

## Dependencies & User Story Order

- **Phase 1: Setup** → Initializes contracts and build configurations.
- **Phase 2: Foundational Architecture** → Platform hardware timer in C (`CTTZipPlatformTimer`) and Swift wrapper (`PlatformMonotonicTimer`), blocking prerequisites for all User Stories.
- **Phase 3: User Story 1 (P1)** → Pure in-memory benchmark engine (`InMemoryBenchmarkEngine`), page-aligned buffers, warmup passes, and adaptive time-clamping.
- **Phase 4: User Story 2 (P2)** → Timer calibration suite, nanosecond resolution validation, and removal of legacy `CACurrentMediaTime()` dependencies across core engines.
- **Phase 5: User Story 3 (P3)** → Standardized TurboBench / lzbench throughput and compression ratio formulas, CLI integration, and JSON report generation.
- **Phase 6: Polish & CI Integration** → Regression suite, performance floor verification, and documentation.

---

## Phase 1: Setup & Project Infrastructure

- [x] T001 [P] Validate contract schemas and project configuration in `specs/052-turbobench-inmemory-alignment/contracts/`
- [x] T002 [P] Export C timer function signatures in `Sources/CTTZipBridge/include/CTTZipBridge.h`

---

## Phase 2: Foundational Architecture (Platform Hardware Monotonic Timers)

- [x] T003 [P] Implement cross-platform nanosecond monotonic timer in `Sources/CTTZipBridge/CTTZipPlatformTimer.c`
- [x] T004 [P] Declare C timer interface and timebase structures in `Sources/CTTZipBridge/include/CTTZipPlatformTimer.h`
- [x] T005 Implement Swift high-resolution timer abstraction in `Sources/TTZipCore/Platform/PlatformMonotonicTimer.swift`
- [x] T006 Implement unit tests for timer monotonicity and resolution in `Tests/TTZipTests/PlatformMonotonicTimerTests.swift`

---

## Phase 3: User Story 1 (P1) - Pure In-Memory Benchmark Engine

*Goal: Execute pure in-memory compression/decompression benchmarks on pre-allocated contiguous buffers with zero disk I/O, 1-pass warmup, and 500ms time clamping.*  
*Independent Test: `swift test --filter InMemoryBenchmarkSuiteTests`*

- [x] T007 [P] [US1] Define in-memory benchmark models and configuration structures in `Sources/TTZipCore/Benchmark/InMemoryBenchmarkModels.swift`
- [x] T008 [US1] Implement pure in-memory benchmarking engine with 16KB-aligned buffer pooling and 1-pass warmup in `Sources/TTZipCore/Benchmark/InMemoryBenchmarkEngine.swift`
- [x] T009 [US1] Implement adaptive time-window loop clamping (500ms target) and zero-allocation inner timing loops in `Sources/TTZipCore/Benchmark/InMemoryBenchmarkEngine.swift`
- [x] T010 [P] [US1] Implement bitwise roundtrip `memcmp` verification in `Sources/TTZipCore/Benchmark/InMemoryBenchmarkEngine.swift`
- [x] T011 [US1] Create unit and repeatability regression tests in `Tests/TTZipTests/InMemoryBenchmarkSuiteTests.swift`

---

## Phase 4: User Story 2 (P2) - Cross-Platform Hardware Monotonic Timer Calibration

*Goal: Calibrate hardware timers across macOS (Apple Silicon & Intel) and Windows, deprecating legacy `CACurrentMediaTime()`.*  
*Independent Test: Verify sub-100ns tick precision and zero drift in `PlatformMonotonicTimerTests`.*

- [x] T012 [P] [US2] Implement timer calibration diagnostics and hardware frequency query in `Sources/TTZipCore/Platform/PlatformMonotonicTimer.swift`
- [x] T013 [US2] Replace legacy `CACurrentMediaTime()` with `PlatformMonotonicTimer.nowNanoseconds()` in `Sources/TTZipCLI/CLIBenchmarkRunner.swift`
- [x] T014 [P] [US2] Replace legacy `CACurrentMediaTime()` with `PlatformMonotonicTimer.nowNanoseconds()` in `Sources/TTZipCLI/CLIBenchmarkRunner+RealFile.swift`
- [x] T015 [US2] Replace legacy `CACurrentMediaTime()` with `PlatformMonotonicTimer.nowNanoseconds()` in `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner+Executors.swift`

---

## Phase 5: User Story 3 (P3) - TurboBench & lzbench Metric Formula Standardization & Report Export

*Goal: Standardize throughput (decimal MB/s) and compression ratio formulas, adding CLI flags and JSON report export.*  
*Independent Test: `swift run ttzip-cli bench --in-memory --compat-turbobench`*

- [x] T016 [P] [US3] Implement TurboBench / lzbench metric calculation formulas in `Sources/TTZipCore/Benchmark/InMemoryBenchmarkModels.swift`
- [x] T017 [US3] Implement TurboBench Markdown table and JSON report serialization matching `inmemory-benchmark-result.schema.json` in `Sources/TTZipCore/Benchmark/InMemoryBenchmarkEngine.swift`
- [x] T018 [P] [US3] Add CLI arguments (`--in-memory`, `--compat-turbobench`, `--min-duration`, `--warmup`) in `Sources/TTZipCLI/CLIArgumentParser.swift` and `Sources/TTZipCLI/CLIOptions.swift`
- [x] T019 [US3] Route and dispatch in-memory benchmark commands in `Sources/TTZipCLI/CLICommandRouter.swift` and `Sources/TTZipCLI/CLIBenchmarkRunner.swift`

---

## Phase 6: Polish & CI Quality Gates

- [x] T020 [P] Validate schema compliance of exported JSON reports with `contracts/inmemory-benchmark-result.schema.json`
- [x] T021 Run full test suite and performance floor gate: `swift test` and `swift test --filter InMemoryBenchmarkSuiteTests`
- [x] T022 Execute `@speckit-converge` and `@code-review` checks across all modified and newly created files
