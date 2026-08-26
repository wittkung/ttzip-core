# Quickstart & Verification Guide: 201-swift-to-rust-full-architecture-sinking

## 1. Standalone Rust CLI Build & Verification
```bash
# Build standalone Rust binary
cargo build --release -p ttzip-tui

# Test subcommands
target/release/ttzip --help
target/release/ttzip doctor --json
```

## 2. Test Execution
```bash
# Run all Rust tests
cargo test --workspace

# Run all Swift tests
swift test
```

## 3. LOC & CI Quality Gates
```bash
# Check LOC constraint (<= 800 LOC)
./scripts/lint_loc_gate.sh

# Run full 4-stage automated gate
./scripts/run_local_ci_gate.sh
```
