# Tasks: 201-swift-to-rust-full-architecture-sinking

## Phase 1: Standalone Rust CLI & Command Sinking (US1)
- [x] T001 [P] [US1] Complete all CLI subcommand variants and DTOs in `rust/ttzip-tui/src/cli/args.rs`.
- [x] T002 [P] [US1] Implement missing CLI handlers (`info`, `check`, `hash`, `tree`, `diff`, `doctor`, `cat`, `comment`, `convert`, `delete`, `lock`, `update`) in `rust/ttzip-tui/src/cli/handlers/`.
- [x] T003 [P] [US1] Wire all subcommand handlers into `rust/ttzip-tui/src/cli/mod.rs` and verify `--json` output matching `contracts/cli_engine_contract.schema.json`.
- [x] T004 [US1] Verify standalone binary `ttzip` build and execution in `rust/target/`.

## Phase 2: Core Algorithmic & VFS Sinking Verification (US2)
- [x] T005 [P] [US2] Verify CRC64/PMULL, Reed-Solomon FEC, VFS LZ4 cache pool, and ZipExtraField parsers in `rust/ttzip-glue/`.
- [x] T006 [P] [US2] Run `cargo test --workspace` and verify 100% test pass.

## Phase 3: Swift TTZipCLI Wiring & E2E Validation (US3)
- [x] T007 [P] [US3] Verify `Sources/TTZipCLI` compiles cleanly and delegates commands via `TTZipEngineFacade`.
- [x] T008 [US3] Verify `Tests/TTZipTests/CLICommandE2ETests.swift` and `CLIPOSIXStandardTests.swift` pass with zero failures.

## Phase 4: Comprehensive Verification & CI Quality Gates (US3)
- [x] T009 [P] [US3] Run `./scripts/lint_loc_gate.sh` to enforce <= 800 LOC across all files.
- [x] T010 [P] [US3] Run `swift test` across all Swift packages and targets.
- [x] T011 [US3] Run `./scripts/run_local_ci_gate.sh` full 4-stage automated validation.
