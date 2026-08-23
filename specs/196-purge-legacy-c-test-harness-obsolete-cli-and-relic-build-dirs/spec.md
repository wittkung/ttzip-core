# Feature Specification: 196-purge-legacy-c-test-harness-obsolete-cli-and-relic-build-dirs

## 1. Executive Summary & Strategic Motivation
1. Purge obsolete `cli/` folder (`cli/main.c` 289 LOC unbuildable legacy C CLI).
2. Purge `Tests/c/` (35 files) and `Tests/fuzz/` (2 files) totaling > 8,000 LOC of dead C test harnesses replaced by Rust property/fuzz suites and Swift E2E tests.
3. Purge untracked legacy root build directories (`build/`, `build_asan/`, `build_dist/`, `scratch/`) freeing > 605 MB disk space.
4. Remove redundant `scripts/build_mas.sh` (16 LOC).
5. Align `ARCHITECTURE.md` and `.gitignore` to modern Swift 6 + Safe Rust microkernel architecture.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Clean and Unified Testing Domain
- **Given** running repository testing
- **When** checking `Tests/`
- **Then** only `TTZipTests` and `TTZipAppTests` exist in SwiftPM.
- **And** all Rust property/fuzz tests live in `rust/ttzip-glue/tests`.
- **And** zero dead `.c` files exist in `Tests/`.

### User Scenario 2: Pristine Root Workspace
- **Given** inspecting the project root
- **When** checking for temporary build directories
- **Then** `build/`, `build_asan/`, `build_dist/`, `scratch/`, and `cli/` are absent.
- **And** > 605 MB of disk space is reclaimed.

---

## 3. Success Metrics
1. Delete `cli/` (289 LOC).
2. Delete `Tests/c/` and `Tests/fuzz/` (37 files, > 8,000 LOC).
3. Purge `build/`, `build_asan/`, `build_dist/`, `scratch/` (> 605 MB).
4. Remove `scripts/build_mas.sh`.
5. Update `ARCHITECTURE.md` and `.gitignore`.
6. Pass 4-stage local CI gate in $< 10\text{s}$.
