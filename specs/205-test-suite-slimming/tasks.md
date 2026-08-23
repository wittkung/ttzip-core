# Tasks: Swift Test Suite Boundary Formalization & Slimming

**Feature**: `205-test-suite-slimming`  
**Status**: `COMPLETED`  

---

## Tasks

- [x] **Task 1: Test Suite Audit & Boundary Formalization**
  - [x] Audit `Tests/TTZipTests/` (13 test/fixture files) and establish boundaries.
  - [x] Audit `Tests/TTZipAppTests/` (17 UI/QuickLook/Finder test files) and establish boundaries.
  - [x] Confirm Rust authoritative coverage across all 18 CLI subcommands and core codecs.

- [x] **Task 2: Prune Dead Code & Duplicate Assertions**
  - [x] Delete unreferenced `Sources/TTZipCore/Security/ReedSolomonFEC.swift` (340 LOC).
  - [x] Prune deleted OOP strategy assertions in `TTZipCoreIntegrationTests.swift`.

- [x] **Task 3: Full CI/CD Gate Verification**
  - [x] Verify `swift test` (138/138 passing).
  - [x] Verify `./scripts/lint_loc_gate.sh` (640 source files $\le 800\text{ LOC}$).
  - [x] Verify `swift run ttzip-bench gate` (100% passing).
  - [x] Verify `./scripts/run_rust_tests.sh --unit --props --fuzz` (100% passing).
  - [x] Verify `./scripts/run_local_ci_gate.sh` (4-stage gate passing 100%).
