# Tasks: Physical Two-Repository Split & Independent Release Pipeline

**Feature**: `216-repository-split-and-independent-release-pipeline`  
**Directory**: `specs/216-repository-split-and-independent-release-pipeline`  
**Spec Path**: `specs/216-repository-split-and-independent-release-pipeline/spec.md`  
**Plan Path**: `specs/216-repository-split-and-independent-release-pipeline/plan.md`  

---

## Phase 1: Split Execution Script & Workspace Preparation

- [x] T001 [P] Create `scripts/split_repositories.sh` implementing automated local cloning and selective subtree filtering into `../ttzip-core` and `../ttzip-apple`.
- [x] T002 Verify git history and author attribution for Witt Kung are retained in both repository branches.

---

## Phase 2: Autonomous `ttzip-core` Repository Configuration

- [x] T003 [P] [US1] In `../ttzip-core`, create pure `Package.swift` without AppKit or Sparkle dependencies.
- [x] T004 [US1] In `../ttzip-core`, verify `Cargo.toml` workspace compiles independently (`cargo check --workspace`).
- [x] T005 [US1] In `../ttzip-core`, configure `scripts/install_local_git_hooks.sh` and install `.git/hooks/pre-push`.
- [x] T006 [US1] In `../ttzip-core`, run `swift test` and `cargo test` verifying 100% green pass.

---

## Phase 3: Autonomous `ttzip-apple` Repository Configuration

- [x] T007 [P] [US2] In `../ttzip-apple`, create `Package.swift` declaring dependency on `ttzip-core`.
- [x] T008 [US2] In `../ttzip-apple`, configure `scripts/install_local_git_hooks.sh` and install `.git/hooks/pre-push`.
- [x] T009 [US2] In `../ttzip-apple`, run `swift test` verifying all UI and extension unit tests pass.

---

## Phase 4: Release Automation & Homebrew Formula

- [x] T010 [P] [US3] In `../ttzip-core`, create `scripts/generate_homebrew_formula.sh` generating `Formula/ttzip.rb` with SHA256 calculation.
- [x] T011 [US3] In `../ttzip-core`, create `scripts/publish_crates.sh` validating `cargo package --workspace`.

---

## Phase 5: Verification & Zero-Cloud CI Hardening

- [x] T012 [P] [US4] Execute `./scripts/run_local_ci_gate.sh --bail` in `../ttzip-core`.
- [x] T013 [US4] Execute `./scripts/run_local_ci_gate.sh --bail` in `../ttzip-apple`.
- [x] T014 [US4] Execute `scripts/lint_loc_gate.sh` across both repositories enforcing $\le 800\text{ LOC}$ limit.
