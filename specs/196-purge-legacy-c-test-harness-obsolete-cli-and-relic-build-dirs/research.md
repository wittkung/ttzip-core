# Phase 0 Research: 196-purge-legacy-c-test-harness-obsolete-cli-and-relic-build-dirs

## Research Item R001: Legacy C Code Base Assessment
- **Decision**: 
  - Delete `cli/main.c` and entire `cli/` directory.
  - Delete `Tests/c/` and `Tests/fuzz/` directories.
- **Rationale**: 
  - Modern TTZip CLI is implemented in `Sources/TTZipCLI` and `rust/ttzip-tui`.
  - Modern property, fuzz, and FFI tests are in `rust/ttzip-glue/tests`.
  - SwiftPM targets in `Package.swift` only compile `Tests/TTZipTests` and `Tests/TTZipAppTests`.
  - `Tests/c/` and `cli/` are unbuildable and unreferenced.
- **Alternatives Considered**: 
  - *Keep for historical reference*: Creates confusion and bloats codebase by > 8,500 LOC.
- **Source**: 
  - `Package.swift`
  - `cli/main.c`
  - `Tests/c/test_main.c`

---

## Research Item R002: Root Build Debris Purge
- **Decision**: 
  - Delete `build/` (567 MB), `build_asan/` (22.6 MB), `build_dist/` (7.3 MB), `scratch/` (7.2 MB).
- **Rationale**: 
  - Leftover untracked output directories from previous CMake builds.
  - SwiftPM uses `.build/` and `.build_mas/`.
  - Rust uses `rust/target/`.
- **Alternatives Considered**: 
  - *Keep them*: Wastes > 605 MB of SSD storage.
- **Source**: 
  - Local directory scan.
