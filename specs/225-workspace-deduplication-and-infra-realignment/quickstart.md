# Quickstart Validation: Workspace Deduplication & Infrastructure Realignment

## Prerequisites
- Rust toolchain (`cargo`, `rustc`)
- Swift 6.0 toolchain (`swift`, `xcodebuild`)
- `dev/infra/ttkit` present at `/Users/kevintung/Documents/dev/infra/ttkit`

## Verification Scenarios

### Scenario 1: Verify Core Build Autonomy
```bash
cd /Users/kevintung/Documents/dev/products/ttzip/core
swift build
cd rust
cargo check --workspace
```
**Expected Outcome**: Swift core and Rust engine compile with zero errors.

### Scenario 2: Verify Apple Application Resolution & Build
```bash
cd /Users/kevintung/Documents/dev/products/ttzip/apple
swift build
```
**Expected Outcome**: `TTZipApp` and extensions resolve `../core` and `../../infra/ttkit/TTLocalizationKit` and compile cleanly.

### Scenario 3: Verify Root Directory Hygiene
```bash
cd /Users/kevintung/Documents/dev/products/ttzip
ls -la
```
**Expected Outcome**: Only `core/`, `apple/`, `homebrew/`, `upstream/`, `memory/`, `.agents/`, `specs/`, `README.md`, `.gitignore`, `AGENTS.md` exist at the root level.
