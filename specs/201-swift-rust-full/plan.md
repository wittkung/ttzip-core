# Implementation Plan: 201-swift-to-rust-full-architecture-sinking

## 1. Architectural Strategy
1. **Consolidate Standalone Rust CLI Engine**: Ensure all 18 CLI command handlers in `rust/ttzip-tui/src/cli/` execute natively with robust error handling and `--json` support.
2. **Core Algorithmic Sinking**: Validate all compute-intensive, VFS, crypto, and streaming subsystems in `rust/ttzip-glue`.
3. **Swift CLI Alignment**: Ensure `Sources/TTZipCLI` delegates cleanly to `TTZipEngineFacade`.
4. **macOS GUI Purity**: Preserve all 144 files in `Sources/TTZipApp` without FFI regression.
5. **Automated Verification**: Execute the multi-stage CI gate:
   - Stage 1: LOC Gate (`./scripts/lint_loc_gate.sh`)
   - Stage 2: Swift compilation and tests (`swift test`)
   - Stage 3: Deflate-Bench matrix gate (`swift run ttzip-bench gate`)
   - Stage 4: Rust industrial suite (`./scripts/run_rust_tests.sh --unit --props --fuzz`)
   - Stage 5: Full automated local CI gate (`./scripts/run_local_ci_gate.sh`)

## 2. File Modification & Component Map
- `rust/ttzip-tui/src/cli/`: Standalone Rust CLI subcommands, arguments, and integration tests.
- `rust/ttzip-glue/`: Core Rust compression, decompression, VFS, crypto, standards, and FFI bridges.
- `Sources/TTZipCLI/`: Swift CLI interface and dispatching logic.
- `Sources/TTZipCore/`: Swift core facade and FFI bridge layers.
- `Tests/TTZipTests/`: CLI E2E tests and POSIX standard validation.

## 3. Verification Plan
- Unit tests: `cargo test --workspace`
- Integration tests: `swift test`
- Full CI Gate: `./scripts/run_local_ci_gate.sh`
