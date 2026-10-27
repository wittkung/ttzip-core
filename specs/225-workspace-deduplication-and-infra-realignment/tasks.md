# Tasks: Workspace Deduplication & Infrastructure Realignment

**Feature**: `225-workspace-deduplication-and-infra-realignment`  
**Classification**: `[Lean SDD]`  

## Phase 1: Asset Safety Preservation & Migration
- [ ] Task 1.1: Copy `Tests/ci` and `Tests/cross_language` from root to `core/Tests/`
- [ ] Task 1.2: Copy `scripts/lint_repo_hygiene.sh` from root to `core/scripts/`
- [ ] Task 1.3: Copy all unique spec directories (`001-*` .. `022-*`) from root `specs/` into `core/specs/`
- [ ] Task 1.4: Copy `metadata/` from root to `apple/metadata/`

## Phase 2: Dependency Realignment to TTKit Infra
- [ ] Task 2.1: Update `core/rust/ttzip-engine/Cargo.toml` path for `tt_i18n` to `../../../../infra/ttkit/tt-i18n-core`
- [ ] Task 2.2: Update `core/Package.swift` dependency on `TTLocalizationKit` to `../../infra/ttkit/TTLocalizationKit`
- [ ] Task 2.3: Update `apple/Package.swift` dependency on `TTLocalizationKit` to `../../infra/ttkit/TTLocalizationKit`
- [ ] Task 2.4: Remove `ttkit-localization/` from `products/ttzip/`

## Phase 3: Root Workspace Purge & Hygiene
- [ ] Task 3.1: Remove flat duplicate source/build directories from root (`Sources`, `rust`, `Vendor`, `Tests`, `sdk`, `python`, `node`, `bin`, `cmake`, `completions`, `examples`, `Formula`, `logo`, `man`, `patches`, `reports`, `resources`, `specs`, `scripts`, `dist`, `assets`, `docs`, `TTZip.xcodeproj`, `steam`)
- [ ] Task 3.2: Remove flat duplicate root files (`Package.swift`, `CMakeLists.txt`, `Makefile`, `pyproject.toml`, `ttzip.pc`, `ttzip.pc.in`, `TTZip_trademark.jpg`, `appcast.xml`, `Install-TTZip.command`, `重新构建并启动TTZip.command`, `ACKNOWLEDGEMENTS.md`, `ARCHITECTURE.md`, `BENCHMARK_MATRIX.md`, `CODE_OF_CONDUCT.md`, `CONTRIBUTING.md`, `GEMINI.md`, `LICENSE`, `LICENSE-APACHE`, `LICENSE-BSD`, `LICENSE-GPL`, `NOTICE`, `SECURITY.md`, `README_*.md`)
- [ ] Task 3.3: Write root [README.md](file:///Users/kevintung/Documents/dev/products/ttzip/README.md) and [.gitignore](file:///Users/kevintung/Documents/dev/products/ttzip/.gitignore)

## Phase 4: Verification & Gate Validation
- [ ] Task 4.1: Verify `core/` builds cleanly (`swift build` / `cargo check`)
- [ ] Task 4.2: Verify `apple/` builds cleanly (`swift build`)
- [ ] Task 4.3: Verify root workspace structure
