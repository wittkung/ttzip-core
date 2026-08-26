# Tasks: 200-swift-to-rust-sinking-and-standalone-cli-architecture

## Phase 1: Standalone Rust CLI & Command Sinking (US1)
- [x] T001 [P] [US1] Ensure all CLI subcommands in `rust/ttzip-tui/src/cli/` have native handlers and JSON output.
- [x] T002 [P] [US1] Verify standalone binary `ttzip` build and execution in `rust/target/`.

## Phase 2: Swift TTZipCLI Wiring & E2E Validation (US2)
- [x] T003 [P] [US2] Verify `Sources/TTZipCLI` compiles cleanly and executes commands via engine facade.
- [x] T004 [US2] Verify `Tests/TTZipTests/CLICommandE2ETests.swift` and `CLIPOSIXStandardTests.swift` pass with zero failures.

## Phase 3: Comprehensive Verification & CI Quality Gates (US3)
- [x] T005 [P] [US3] Run `./scripts/lint_loc_gate.sh` to enforce $\le 800\text{ LOC}$ across all files.
- [x] T006 [P] [US3] Run `cargo test --workspace` on all Rust crates.
- [x] T007 [P] [US3] Run `swift test` on all Swift packages and targets.
- [x] T008 [US3] Run `./scripts/run_local_ci_gate.sh` full 4-stage validation.
