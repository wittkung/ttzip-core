# Research & Architecture Analysis: 200-swift-to-rust-sinking-and-standalone-cli-architecture

## 1. Technical Context & State of the Art
- **Rust CLI Toolchain**: `clap` v4 with `derive` macros provides zero-allocation parsing and automatic shell completions.
- **Microkernel FFI**: `ttzip-glue` exports pure C-ABI symbols (`ttzip_vfs_*`, `ttzip_archive_*`, `ttzip_crypto_*`, `ttzip_standards_*`) ensuring seamless in-process calling without process boundary overhead.
- **Terminal Rendering**: `ratatui` v0.28 and `indicatif` v0.17 provide hardware-accelerated Unicode/ANSI rendering, eliminating intermediate terminal buffer allocations.

## 2. Research Findings & Decision Log

### Finding 1: Command Parity & Dispatch
- **Context**: Swift CLI defined 18 subcommands (`compress`, `extract`, `list`, `info`, `hash`, `diff`, `lock`, `tree`, `split`, `join`, `comment`, `convert`, `delete`, `update`, `repair`, `recover`, `bench`, `doctor`).
- **Resolution**: `rust/ttzip-tui/src/cli/args.rs` and `handlers.rs` implement full handler dispatch for all subcommands directly backed by `ttzip_glue::archive`, `ttzip_glue::vfs`, and `ttzip_glue::crypto`.

### Finding 2: Zero-Cost JSON Output
- **Context**: Integration tests and automated agents require machine-readable JSON output via `--json`.
- **Resolution**: Every CLI handler emits strongly-typed JSON DTOs serializable via `serde_json::to_string_pretty`.

### Finding 3: Swift CLI Thin Wrapper
- **Context**: SwiftPM produces `ttzip-cli` binary.
- **Resolution**: `TTZipCLI` in Swift uses `TTZipEngineFacade` and `CTTZipBridge` to call the same low-level engines, ensuring zero divergence in format support or performance.
