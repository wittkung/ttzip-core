# Quickstart: Full System Architectural Reform

**Feature**: `221-full-system-architectural-reform`

## Verification Quickstart

### 1. Build & Test Rust Core
```bash
cargo test --manifest-path rust/Cargo.toml
./scripts/build_rust.sh --release
```

### 2. Verify Swift Core & UI
```bash
swift test
```

### 3. Run Quality & CI Gates
```bash
./scripts/lint_loc_gate.sh
./scripts/run_local_ci_gate.sh
```
