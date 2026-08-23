# Implementation Plan: 200-swift-to-rust-sinking-and-standalone-cli-architecture

## 1. Architectural Strategy
1. **Consolidate Standalone Rust CLI Engine**: Ensure all CLI command handlers in `rust/ttzip-tui/src/cli/` execute natively with robust error handling and `--json` support.
2. **Swift CLI Alignment**: Ensure `Sources/TTZipCLI` delegates to the unified engine without unnecessary overhead while preserving all Swift XCTest E2E expectations.
3. **Automated Verification**: Execute the multi-stage CI gate:
   - Stage 1: LOC Gate (`./scripts/lint_loc_gate.sh`)
   - Stage 2: Swift compilation and tests (`swift build && swift test`)
   - Stage 3: Rust compilation and tests (`cargo test --workspace`)
   - Stage 4: Local CI/CD Gate (`./scripts/run_local_ci_gate.sh`)

## 2. File Modification & Component Map
- `rust/ttzip-tui/src/cli/`: Standalone Rust CLI subcommands and integration tests.
- `Sources/TTZipCLI/`: Swift CLI interface and dispatching logic.
- `Tests/TTZipTests/`: CLI E2E tests and POSIX standard validation.

## 3. Verification Plan
- Unit tests: `cargo test --workspace`
- Integration tests: `swift test --filter CLICommandE2ETests`
- Full CI Gate: `./scripts/run_local_ci_gate.sh`
