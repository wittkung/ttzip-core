# Phase 0 Research: 173-refactor-god-files-and-modular-decomposition

## Research Item R001: Swift Core Monoliths Decomposition
- **Decision**: Decompose the 4 major Swift Core monolithic files (`StandardsComplianceChecker.swift`, `ArchiveFormatStandardSpec.swift`, `TTZipEngineFacade.swift`, `DifferentialOracleTestHarness.swift`) into 22 cohesive domain-driven submodules under matching directories (`Standards/Compliance/`, `Standards/Registry/`, `Facades/`, `Testing/`).
- **Rationale**: 
  - Swift allows seamless cross-file extensions for the same `enum`/`class`/`struct` without modifying public API signatures or call sites.
  - Internal helper methods can be accessed across extensions in the same module.
  - Each extracted file is strictly bounded within 100~310 lines, completely eliminating monolithic "god files".
- **Alternatives Considered**: 
  - *Refactor into separate protocol-oriented classes with dynamic registry*: Rejected because it introduces unnecessary heap allocation and dynamic dispatch overhead on high-frequency validation hot paths.
- **Source**: 
  - `Sources/TTZipCore/Standards/StandardsComplianceChecker.swift:1-1356`
  - `Sources/TTZipCore/Standards/ArchiveFormatStandardSpec.swift:1-923`
  - `Sources/TTZipCore/Facades/TTZipEngineFacade.swift:1-907`
  - `Sources/TTZipCore/Testing/DifferentialOracleTestHarness.swift:1-782`

---

## Research Item R002: Rust Glue FFI and TUI Decomposition
- **Decision**: Decompose the 5 Rust modules (`vfs.rs`, `codecs_ffi.rs`, `main.rs`, `archive_ffi.rs`, `app.rs`) into directory-based submodules (`vfs/`, `ffi/codecs_ffi/`, `cli/`, `ffi/archive_ffi/`, `app/`) with clean `mod.rs` re-exports.
- **Rationale**: 
  - C-ABI global export symbols (`#[no_mangle] pub unsafe extern "C" fn`) remain flat and 100% symbol-compatible with `ttzip_rust_glue.h`.
  - Rust's `pub use` in `mod.rs` maintains 100% backward compatibility for all internal and external Rust crate callers.
  - Each extracted file is strictly bounded within 40~280 lines.
- **Alternatives Considered**: 
  - *Extract into separate cargo crates*: Rejected because it introduces unnecessary crate boundary overhead and versioning complexity for internal implementation details.
- **Source**: 
  - `rust/ttzip-tui/src/vfs.rs:1-897`
  - `rust/ttzip-glue/src/ffi/codecs_ffi.rs:1-886`
  - `rust/ttzip-tui/src/main.rs:1-802`
  - `rust/ttzip-glue/src/ffi/archive_ffi.rs:1-695`
  - `rust/ttzip-tui/src/app.rs:1-671`

---

## Research Item R003: UI, CLI, and Plotter Decomposition
- **Decision**: Decompose `CLICommandRouter.swift`, `AppViewState.swift`, `RasterParetoPlotter.swift`, `ArchiveExplorerView.swift`, and `FinderMillerColumnsView.swift` into 26 focused sub-components and domain extension files.
- **Rationale**: 
  - CLI command routing cleanly separates into subcommand handler extensions (`+Compress`, `+Extract`, `+Inspect`, `+Maintenance`, `+Benchmark`).
  - `AppViewState` separates into domain state extensions (`+Mediator`, `+Tasks`, `+ArchiveOperations`, `+Commands`).
  - SwiftUI views separate into independent subviews (`ArchiveExplorerHeaderBar`, `ArchiveExplorerTableView`) and interaction extensions.
  - `RasterParetoPlotter` separates mathematical coordinate projections from CoreGraphics drawing and collision detection.
  - Each file stays within 100~240 lines with zero view regressions.
- **Alternatives Considered**: 
  - *Rewrite Plotter using SwiftUI Canvas / Charts*: Rejected because CLI and headless benchmarking tools require offline 4K PNG generation without window server dependencies.
- **Source**: 
  - `Sources/TTZipCLI/CLICommandRouter.swift:1-873`
  - `Sources/TTZipApp/ViewModels/AppViewState.swift:1-709`
  - `Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift:1-747`
  - `Sources/TTZipApp/Views/ArchiveExplorerView.swift:1-562`
  - `Sources/TTZipApp/Views/Explorer/FinderMillerColumnsView.swift:1-556`
