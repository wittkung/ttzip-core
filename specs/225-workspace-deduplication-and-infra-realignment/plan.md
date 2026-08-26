# Implementation Plan: Workspace Deduplication & Infrastructure Realignment

**Branch**: `main` | **Date**: 2026-08-26 | **Spec**: [specs/225-workspace-deduplication-and-infra-realignment/spec.md](file:///Users/kevintung/Documents/dev/products/ttzip/specs/225-workspace-deduplication-and-infra-realignment/spec.md)

## Summary
Purge all legacy duplicate flat source files and build manifests from the root workspace directory, relocate unique test/metadata assets to `core/` and `apple/`, update dependency paths to point to the canonical `dev/infra/ttkit` location, and verify that both `core` and `apple` build cleanly.

## Technical Context
- **Language/Version**: Swift 6.0, Rust 2021 edition
- **Primary Dependencies**: `dev/infra/ttkit` (`tt-i18n-core`, `TTLocalizationKit`)
- **Target Platform**: macOS 14.0+, iOS 17.0+
- **Project Type**: Multi-repository workspace layout (`core`, `apple`, `homebrew`, `upstream`)
- **Testing**: `swift build` & `swift test` in `core` and `apple`, `cargo check` in `core/rust`

## Constitution Check
- Constitution Level 0 (Invariants): Zero regression on fast paths, memory constraints, and architecture isolation.
- Zero Loss Rule: All unique test fixtures and scripts must be preserved in `core/Tests/` and `core/scripts/`.

## Execution Phases

### Phase 0: Asset Migration & Safety Preservation
1. Copy `Tests/ci` and `Tests/cross_language` to `core/Tests/`.
2. Copy `scripts/lint_repo_hygiene.sh` to `core/scripts/lint_repo_hygiene.sh`.
3. Copy all unique specs from root `specs/` into `core/specs/`.
4. Copy `metadata/` into `apple/metadata/`.

### Phase 1: Dependency Realignment to TTKit Infra
1. Update `core/rust/ttzip-engine/Cargo.toml` path from `../../../ttkit-localization/tt-i18n-core` to `../../../../infra/ttkit/tt-i18n-core`.
2. Update `core/Package.swift` and `apple/Package.swift` to point `TTLocalizationKit` to `../../infra/ttkit/TTLocalizationKit`.
3. Remove `ttkit-localization/` from `products/ttzip/`.

### Phase 2: Root Directory Purge
1. Delete redundant directories: `Sources`, `rust`, `Vendor`, `Tests`, `sdk`, `python`, `node`, `bin`, `cmake`, `completions`, `examples`, `Formula`, `logo`, `man`, `patches`, `reports`, `resources`, `specs`, `scripts`, `dist`, `assets`, `docs`, `TTZip.xcodeproj`.
2. Delete redundant files: `Package.swift`, `CMakeLists.txt`, `Makefile`, `pyproject.toml`, `ttzip.pc`, `ttzip.pc.in`, `TTZip_trademark.jpg`, `appcast.xml`, `Install-TTZip.command`, `重新构建并启动TTZip.command`, duplicate markdown documents.
3. Write clean workspace [README.md](file:///Users/kevintung/Documents/dev/products/ttzip/README.md) and [.gitignore](file:///Users/kevintung/Documents/dev/products/ttzip/.gitignore).

### Phase 3: Build & Verification
1. Validate `core/`: Run `swift build` and `cargo check`.
2. Validate `apple/`: Run `swift build` to confirm `TTZipApp` builds with clean SPM resolution.
