# Implementation Plan: Physical Two-Repository Split & Independent Release Pipeline

**Feature**: `216-repository-split-and-independent-release-pipeline`  
**Classification**: `[Full SDD]`  
**Status**: `PLANNED`  
**Spec Path**: `specs/216-repository-split-and-independent-release-pipeline/spec.md`  

---

## 1. Technical Context

- **Source Monorepo**: `/Users/kevintung/Documents/dev/TTZip`
- **Output Target Repositories**:
  - `../ttzip-core` (`wittkung/ttzip-core` · BSD-3-Clause OR Apache-2.0)
  - `../ttzip-apple` (`wittkung/ttzip-apple` · GPL-3.0-or-later)
- **Toolchains**: Git 2.40+, Swift 6.0, Cargo / Rust 1.80+, Python 3.10+.
- **Zero-Cloud CI**: Independent local Git hooks (`.git/hooks/pre-push`) in both repositories.

---

## 2. Phased Execution Roadmap

### Phase 1: Split Execution Script & Workspace Preparation
- [ ] Create `scripts/split_repositories.sh` to deterministically create the two repository trees from the current monorepo.
- [ ] Ensure full commit history and author metadata are preserved.

### Phase 2: Autonomous `ttzip-core` Repository Configuration
- [ ] Generate pure `Package.swift` in `ttzip-core` with 0 UI dependencies.
- [ ] Verify `Cargo.toml` workspace compiles independently (`cargo check --workspace`).
- [ ] Configure `ttzip-core` local Git pre-push hook and 4-stage local CI gate.
- [ ] Run `swift test` and `cargo test` in `ttzip-core`.

### Phase 3: Autonomous `ttzip-apple` Repository Configuration
- [ ] Generate `ttzip-apple/Package.swift` pointing to `ttzip-core`.
- [ ] Configure `ttzip-apple` local Git pre-push hook and UI regression gate.
- [ ] Run `swift test` in `ttzip-apple`.

### Phase 4: Release Automation & Homebrew Formula
- [ ] Create `scripts/generate_homebrew_formula.sh` in `ttzip-core` for `brew install ttzip`.
- [ ] Create `scripts/publish_crates.sh` for publishing `ttzip-engine` and `ttzip-cli` to crates.io.

### Phase 5: Verification & Zero-Cloud CI Hardening
- [ ] Execute `./scripts/run_local_ci_gate.sh` across both repositories independently.
- [ ] Verify 0 cloud runner minutes used and 100% LOC gate ($\le 800\text{ LOC}$) pass.

---

## 3. Verification Plan

1. **`ttzip-core` CI Gate**:
   ```bash
   cd ../ttzip-core && ./scripts/run_local_ci_gate.sh --bail
   ```
2. **`ttzip-apple` CI Gate**:
   ```bash
   cd ../ttzip-apple && ./scripts/run_local_ci_gate.sh --bail
   ```
3. **SPM Linkage Test**:
   ```bash
   cd ../ttzip-apple && swift test
   ```
