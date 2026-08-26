# Feature Specification: 200-swift-to-rust-sinking-and-standalone-cli-architecture

**Pipeline Level**: `[Full SDD]`

## 1. Executive Summary & Strategic Motivation
1. **Pristine Standalone Rust CLI**: Complete the sinking of all CLI commands (create, extract, list, info, hash, diff, lock, tree, split, join, comment, convert, delete, update, repair, recover, bench, doctor) into the standalone Rust CLI engine (`rust/ttzip-tui` / `ttzip` binary).
2. **Zero Swift Runtime Overhead on CLI**: The standalone Rust CLI binary operates with zero dependency on the Swift runtime, providing instant $< 5\text{ms}$ cold start, cross-platform portability, and direct in-process access to the Rust microkernel.
3. **Swift CLI Direct Delegation**: Wire `TTZipCLI` in Swift as a thin, ultra-lightweight entry point or pass-through proxy to the Rust native engine, ensuring 100% backward compatibility for all existing scripts and CI pipelines.
4. **Zero Regressions & CI Gate Convergence**: Ensure 100% pass across all Swift tests (`swift test`), all Rust tests (`cargo test --workspace`), and local CI quality gates.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Standalone Rust CLI Complete Parity (US1)
- **Given** the standalone Rust CLI binary `ttzip`
- **When** the user invokes any archive operation (`create`, `extract`, `list`, `info`, `hash`, `diff`, `tree`, `split`, `join`, `repair`, `recover`, `bench`, `doctor`)
- **Then** the Rust CLI executes natively with streaming zero-copy I/O, SIMD hardware acceleration, and returns standard POSIX exit codes (0 on success, non-zero on error).
- **And** supports `--json` structured output matching JSON contract specifications.

### User Scenario 2: Swift TTZipCLI Delegation & Slimming (US2)
- **Given** invoking the Swift executable target `ttzip-cli`
- **When** arguments are supplied
- **Then** `TTZipCLI` seamlessly delegates execution to the Rust native core / C-ABI microkernel or runs the standard POSIX flow.
- **And** all existing CLI E2E tests (`CLICommandE2ETests.swift`, `CLIPOSIXStandardTests.swift`) pass 100%.

### User Scenario 3: Quality Gates & Full Verification (US3)
- **Given** running the automated verification pipeline
- **When** executing `lint_loc_gate.sh`, `swift test`, `cargo test --workspace`, and `run_local_ci_gate.sh`
- **Then** all files strictly adhere to the $\le 800\text{ LOC}$ limit, zero compile warnings, zero test failures, and 100% gate pass.

---

## 3. Success Metrics
1. All 18 CLI subcommands fully operational in standalone Rust binary.
2. Swift `ttzip-cli` and Rust `ttzip` pass all end-to-end POSIX contract tests.
3. `cargo test --workspace` and `swift test` 100% PASS.
4. Local CI Gate (`./scripts/run_local_ci_gate.sh`) 100% GREEN.
