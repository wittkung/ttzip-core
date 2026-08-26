# Tasks: Standalone CLI Distribution & Native App Packaging Pipeline

**Feature**: `206-cli-packaging-and-app-bundling`  
**Status**: `COMPLETED`  

---

## Tasks

- [x] **Task 1: Standalone Binary Compilation Pipeline**
  - [x] Build standalone release binary `bin/ttzip` via `scripts/build_tui.sh --release`.
  - [x] Verify `bin/ttzip --version` and `bin/ttzip doctor --json` execution.

- [x] **Task 2: Production App Bundling & Helper Integration**
  - [x] Integrate helper binary bundling into `dist/TTZip.app/Contents/Helpers/ttzip`.
  - [x] Verify deep codesign on `dist/TTZip.app` (`codesign -vvv --deep --strict`).

- [x] **Task 3: Release Tarball & Homebrew Formula Pipeline**
  - [x] Generate `dist/ttzip-cli-v1.0.0-darwin-universal.tar.gz`.
  - [x] Generate `Formula/ttzip.rb` and `Formula/ttzip-cli.rb` with dynamic SHA-256 and URLs.
  - [x] Update formula `test do` blocks to test Rust standalone subcommands.

- [x] **Task 4: Zero-Regression Local CI Verification**
  - [x] Verify `./scripts/run_local_ci_gate.sh` (4-stage gate passing 100%).
