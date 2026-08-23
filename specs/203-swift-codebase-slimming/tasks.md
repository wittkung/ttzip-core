# Tasks: Swift Codebase Slimming & Redundant Code Purge

**Input**: Feature specification from `specs/203-swift-codebase-slimming/spec.md`
**Pipeline**: `[Lean SDD]`

---

## Phase 1: Setup & Target Dependency Audit

- [x] T001 [P] [US1] Audit dependencies between `Sources/TTZipCLI`, `Sources/TTZipCore`, and `Tests/TTZipTests`
- [x] T002 [P] [US1] Identify obsolete duplicate Swift CLI handlers in `Sources/TTZipCLI`

---

## Phase 2: User Story 1 - Lean Swift Build & Redundancy Purge (Priority: P1)

- [x] T003 [P] [US1] Slim `Sources/TTZipCLI` by pruning redundant duplicate command sub-classes and retaining core POSIX arguments and entry point
- [x] T004 [P] [US1] Clean up dead legacy code in `Sources/TTZipCore` while preserving all models, facades, and FFI bridges used by `TTZipApp`
- [x] T005 [US1] Verify Swift compilation via `swift build`

---

## Phase 3: User Story 2 - Zero Regression & Verification (Priority: P2)

- [x] T006 [P] [US2] Run `swift test` across all targets to verify zero regression
- [x] T007 [P] [US2] Run `cargo test --workspace` to verify Rust engine integrity
- [x] T008 [US2] Run `./scripts/lint_loc_gate.sh` to verify LOC thresholds across all files
- [x] T009 [US2] Execute full 4-stage automated gate via `./scripts/run_local_ci_gate.sh`

---

## Phase 4: Polish & Documentation

- [x] T010 Update `docs/全面下沉计划.md` and codebase inventory metrics
