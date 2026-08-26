# Feature Specification: 173-refactor-god-files-and-modular-decomposition

## 1. Overview & Problem Statement
Over successive evolutionary cycles, several core Swift and Rust source files have grown beyond 500 lines into monolithic "god files" that aggregate multiple distinct responsibilities (e.g. format specifications, AST traversal, command dispatch, facade aggregation, test harness oracles, and UI layout orchestration). 

This architectural debt impairs readability, increases cognitive load, introduces subtle cross-concern coupling, and contradicts the project constitution regarding Single Responsibility Principle (SRP) and modular decomposition.

This feature systematically audits all first-party Swift and Rust files with line counts >= 500, decomposes them into focused, cohesive submodules, and verifies 100% functional equivalence and zero test/performance regression.

---

## 2. Target Files for Modular Decomposition (>= 500 LOC)

### Category A: Swift Core & Standards Monoliths
1. **`Sources/TTZipCore/Standards/StandardsComplianceChecker.swift`** (~1,355 lines)
   - *Current*: Monolithic compliance verifier mixing ZIP, 7z, TAR, GZIP, and APFS checks.
   - *Target Decomposition*: Extract per-format compliance checkers (`ZipComplianceChecker`, `SevenZipComplianceChecker`, `TarComplianceChecker`, `GzipComplianceChecker`) orchestrated by a lightweight `StandardsComplianceChecker`.
2. **`Sources/TTZipCore/Standards/ArchiveFormatStandardSpec.swift`** (~922 lines)
   - *Current*: Multi-format specification registry containing all header signatures, magic numbers, limits, and MIME mappings.
   - *Target Decomposition*: Split into modular format specifications (`ZipStandardSpec`, `SevenZipStandardSpec`, `TarStandardSpec`, `UnixFormatStandardSpec`) and a unified registry.
3. **`Sources/TTZipCore/Facades/TTZipEngineFacade.swift`** (~906 lines)
   - *Current*: Aggregates compression, extraction, inspection, command history, state machine management, and proxy extensions into one file.
   - *Target Decomposition*: Split into focused extensions and sub-facades (`TTZipEngineFacade+Compress.swift`, `TTZipEngineFacade+Extract.swift`, `TTZipEngineFacade+Inspect.swift`, `TTZipEngineFacade+Commands.swift`, `TTZipEngineFacade+StateMachines.swift`).
4. **`Sources/TTZipCore/Testing/DifferentialOracleTestHarness.swift`** (~781 lines)
   - *Current*: Contains test runners, manifest comparators, report generators, process runners, and AST hashers.
   - *Target Decomposition*: Split into `DifferentialOracleRunner`, `DifferentialManifestDiffer`, `DifferentialReportFormatter`.
5. **`Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift`** (~746 lines)
   - *Current*: Combines ANSI raster rendering, math coordinate projections, legend layout, and SVG serialization.
   - *Target Decomposition*: Split into `RasterCoordinateProjection`, `RasterASCIICanvas`, `RasterParetoExporter`.
6. **`Sources/TTZipCore/Pipeline/DeflateStreamEngine.swift`** (~644 lines) & **`ArchiveCompressionTypes.swift`** (~643 lines)
   - *Target Decomposition*: Decompose into stream state machines, pipeline stages, and domain-partitioned type files.
7. **`Sources/TTZipCLI/CLICommandRouter.swift`** (~872 lines)
   - *Target Decomposition*: Split individual CLI handlers (`CLICompressHandler`, `CLIExtractHandler`, `CLIInspectHandler`, `CLIBenchHandler`) from the router.

### Category B: Swift App & ViewModel Monoliths
8. **`Sources/TTZipApp/ViewModels/AppViewState.swift`** (~708 lines)
   - *Target Decomposition*: Split into domain state extensions (`AppViewState+Navigation.swift`, `AppViewState+Operations.swift`, `AppViewState+Search.swift`, `AppViewState+Inspector.swift`).
9. **`Sources/TTZipApp/Views/ArchiveExplorerView.swift`** (~561 lines) & **`FinderMillerColumnsView.swift`** (~555 lines)
   - *Target Decomposition*: Extract sub-components (Toolbar, Status Bar, Breadcrumbs, Item Rows, Context Menus).

### Category C: Rust Glue & TUI Monoliths
10. **`rust/ttzip-tui/src/vfs.rs`** (~896 lines) & **`rust/ttzip-tui/src/main.rs`** (~801 lines) & **`rust/ttzip-tui/src/app.rs`** (~670 lines)
    - *Target Decomposition*: Decompose VFS node management, tree traversal, fuzzy scoring, CLI argument parsing, and event loop orchestration.
11. **`rust/ttzip-glue/src/ffi/codecs_ffi.rs`** (~885 lines) & **`rust/ttzip-glue/src/ffi/archive_ffi.rs`** (~694 lines)
    - *Target Decomposition*: Decompose into codec-specific FFI modules (`ffi/deflate.rs`, `ffi/zstd.rs`, `ffi/lzma2.rs`, `ffi/lzfse.rs`, `ffi/crypto.rs`).

---

## 3. User Scenarios & Acceptance Criteria

### User Scenario 1: Clean Architectural Boundaries & Maintainability
- Given any first-party source file in `Sources/` or `rust/src/`
- When inspected for length and complexity
- Then no single file shall exceed 500 physical lines (except third-party vendored C code in `Sources/CTTZipBridge/fast-lzma2`, `lzfse`, `zopfli`), and each file shall adhere to single-responsibility encapsulation.

### User Scenario 2: Zero Functional & Behavioral Regression
- Given the entire TTZip test suite (850+ unit/integration tests, property tests, mutation fuzzing, differential oracles)
- When executed with `swift test` and `cargo test`
- Then 100% of tests must pass with 0 failures and 0 warnings.

### User Scenario 3: Binary Size & Performance Non-Regression
- Given automated CI benchmark runs (`ttzip-bench gate`, `./scripts/run_local_ci_gate.sh`)
- When throughput and compression ratios are measured
- Then zero performance degradation (<0.1% variance) is observed compared to the pre-refactoring baseline.

---

## 4. Success Metrics
1. **File Length Compliance**: 100% of first-party Swift and Rust files <= 500 LOC.
2. **Test Pass Rate**: 100% across all 850+ Swift tests and all Rust property/fuzz/unit tests.
3. **CI Gate**: Full `./scripts/run_local_ci_gate.sh` passes 7/7 stages cleanly.

---

## 5. Clarifications
- **Q1: Should vendored third-party C files (`fast-lzma2`, `lzfse`, `zopfli`) in `Sources/CTTZipBridge/` be modified or split?**
  - **Decision**: No. Upstream vendor libraries are isolated and maintained as-is under their respective third-party directories. Only first-party TTZip code (`Sources/TTZip*`, `Sources/CTTZipBridge/CTTZipBridge.c`, `rust/`) is subject to the <= 500 LOC rule.
- **Q2: How should large Swift Facades and ViewModels be split without breaking public API signatures?**
  - **Decision**: Use Swift `extension` files (`TTZipEngineFacade+Compress.swift`, `AppViewState+Navigation.swift`, etc.) in matching subdirectories to maintain 100% ABI and call-site compatibility while keeping each source file focused and < 500 LOC.
- **Q3: How should large Rust modules (`vfs.rs`, `codecs_ffi.rs`, `archive_ffi.rs`) be structured?**
  - **Decision**: Convert flat large files into Rust submodules (`vfs/mod.rs`, `vfs/tree.rs`, `vfs/matcher.rs`; `ffi/codecs/mod.rs`, `ffi/codecs/deflate.rs`, etc.) with clean re-exports at module boundaries.

