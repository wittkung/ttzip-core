# Feature Specification: Workspace Deduplication & Infrastructure Realignment

**Feature**: `225-workspace-deduplication-and-infra-realignment`  
**Classification**: `[Lean SDD]` (Workspace layout cleanup, root deduplication, dependency path update to infra/ttkit)  
**Status**: `SPECIFIED`  

---

## 1. Executive Summary & Objective

### 1.1 Problem Statement
The workspace root directory (`/Users/kevintung/Documents/dev/products/ttzip`) currently contains a duplicate flat file tree of the original core engine alongside the true split repositories (`core/` and `apple/`). Furthermore, `ttkit-localization` (an infrastructure SDK belonging to `dev/infra/ttkit`) was duplicated into `products/ttzip`.

### 1.2 Target Topology
1. **Infrastructure Decoupling**: Realign all `tt_i18n` (Cargo) and `TTLocalizationKit` (SPM) dependency paths from local `ttzip/ttkit-localization` to `../../infra/ttkit`.
2. **Zero-Loss Migration**: Sync unique test harnesses (`Tests/ci`, `Tests/cross_language`), hygiene linters (`scripts/lint_repo_hygiene.sh`), App Store metadata (`metadata/`), and legacy specs to `core/` and `apple/`.
3. **Workspace Purge**: Remove all flat duplicate source folders (`Sources/`, `rust/`, `Vendor/`, `Tests/`, `sdk/`, `python/`, `node/`, `bin/`, `cmake/`, `specs/`, etc.) and root build configurations (`Package.swift`, `CMakeLists.txt`, `Makefile`) from `products/ttzip`.
4. **Clean Root Entrypoint**: Establish a unified workspace `README.md` and `.gitignore`.

---

## 2. Requirements

- **REQ-001**: `core/rust/ttzip-engine/Cargo.toml` MUST resolve `tt-i18n-core` from `../../../../infra/ttkit/tt-i18n-core` (or sibling infra).
- **REQ-002**: `core/Package.swift` and `apple/Package.swift` MUST resolve `TTLocalizationKit` cleanly.
- **REQ-003**: `apple/Package.swift` MUST continue resolving `../core` cleanly.
- **REQ-004**: No unique test fixtures, scripts, or release metadata from the root directory shall be lost.
- **REQ-005**: All duplicate files in the root directory shall be purged.
- **REQ-006**: `core/` and `apple/` must compile cleanly without missing file errors.
