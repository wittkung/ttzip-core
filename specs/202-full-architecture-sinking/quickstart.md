# Quickstart & Verification Guide: Full Architecture Sinking

## 1. Standalone Rust CLI Build & Verification
```bash
# Build standalone release binary
cargo build --release -p ttzip-tui

# Verify CLI startup and help output
./rust/target/release/ttzip --help

# Verify structured JSON output
./rust/target/release/ttzip doctor --json
```

## 2. Rust Workspace Test Suite
```bash
# Execute 100% of Rust unit, integration, and fuzz tests
cargo test --workspace
```

## 3. Swift Integration & Facade Test Suite
```bash
# Execute full Swift test suite ensuring zero regression across CLI & Core facades
swift test
```

## 4. Code Quality & Automated CI Gate Verification
```bash
# Verify LOC boundary (<= 800 LOC per file)
./scripts/lint_loc_gate.sh

# Run full 4-stage automated gate
./scripts/run_local_ci_gate.sh
```
