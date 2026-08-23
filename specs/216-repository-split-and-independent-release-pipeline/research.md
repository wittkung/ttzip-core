# Research & Decision Matrix: Physical Repository Split & Release Topology

**Feature**: `216-repository-split-and-independent-release-pipeline`  
**Status**: `COMPLETED`  

---

## 1. Repository Split Methodologies: Git Subtree vs. Git Filter-Repo vs. Branch Cloning

### Decision
Use a **Deterministic Two-Step Branch Filtering & Extraction Strategy**:
1. **Repository A (`ttzip-core`)**: Retain complete underlying commit history, remove `Sources/TTZipApp`, `Sources/TTZipQuickLook`, `Sources/TTZipFinderSync`, and UI scripts, reconfigure `Package.swift` to pure library.
2. **Repository B (`ttzip-apple`)**: Retain complete application commit history, remove `rust/` (which now lives in `ttzip-core`), point SPM dependency to `ttzip-core`.

### Rationale
- `git filter-repo` / selective directory pruning preserves 100% of the authorship and commit timestamps for Witt Kung without creating disconnected commit hashes.
- Allows both repositories to be published immediately to GitHub as `wittkung/ttzip-core` and `wittkung/ttzip-apple`.

---

## 2. Package Dependency Binding: Local Path vs. Remote URL

### Decision
Support **Dual-Mode SPM Configuration** in `ttzip-apple/Package.swift`:
- **Local Dev Mode**: `.package(path: "../ttzip-core")` (for instant local multi-repo development without pushing tags).
- **Release / CI Mode**: `.package(url: "https://github.com/wittkung/ttzip-core.git", from: "1.0.0")`.

### Rationale
- Prevents development friction while ensuring clean standalone CI for release builds.

---

## 3. Crates.io & Homebrew Formula Publishing Architecture

### Decision
- `ttzip-engine` and `ttzip-cli` within `ttzip-core` configured with complete metadata (`description`, `license`, `repository`, `documentation`, `keywords`) ready for `cargo publish`.
- Homebrew formula generator script `scripts/generate_homebrew_formula.sh` automatically computes SHA256 checksums of tagged release tarballs for `brew install ttzip`.
