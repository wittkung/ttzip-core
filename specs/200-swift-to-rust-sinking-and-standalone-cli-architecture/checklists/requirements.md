# Requirements Checklist: 200-swift-to-rust-sinking-and-standalone-cli-architecture

## User Scenario 1: Standalone Rust CLI Complete Parity (US1)
- [x] R1.1: `ttzip` standalone binary supports all 18 subcommands (`list`, `extract`, `create`, `recover`, `repair`, `split`, `join`, `bench`, `doctor`, `tree`, `hash`, `info`, `diff`, `lock`, `comment`, `convert`, `delete`, `update`).
- [x] R1.2: Support `--json` structured output across all subcommands conforming to JSON schema contracts.
- [x] R1.3: Zero runtime dependency on Swift libraries.

## User Scenario 2: Swift TTZipCLI Delegation & Slimming (US2)
- [x] R2.1: `TTZipCLI` in Swift delegates commands cleanly to low-level native engine.
- [x] R2.2: All Swift E2E tests (`CLICommandE2ETests.swift`, `CLIPOSIXStandardTests.swift`) pass 100%.

## User Scenario 3: Quality Gates & Full Verification (US3)
- [x] R3.1: All source files $\le 800\text{ LOC}$.
- [x] R3.2: `cargo test --workspace` passes 100%.
- [x] R3.3: `swift test` passes 100%.
- [x] R3.4: `./scripts/run_local_ci_gate.sh` passes 100%.
