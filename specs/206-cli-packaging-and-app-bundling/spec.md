# Spec: Standalone CLI Distribution & Native App Packaging Pipeline

**Feature**: `206-cli-packaging-and-app-bundling`  
**Classification**: `[Lean SDD]` (Build pipeline modernizing, Homebrew formula synchronization, App helper bundling)  
**Status**: `COMPLETED`  

---

## 1. Context & Objectives

Following the sinking of core functionality into the standalone Rust binary `bin/ttzip` and thin Swift GUI facade, this feature delivers:
1. **Production-Grade App Bundle Assembly**:
   - `TTZip.app` assembles with Sparkle autoupdate framework, Apple Silicon native App executable (`TTZip.app/Contents/MacOS/TTZip`).
   - Bundles the standalone Rust binary into `TTZip.app/Contents/Helpers/ttzip` with valid ad-hoc codesign, ensuring internal desktop processes can invoke headless engine capabilities without APFS case-insensitive filename collisions.
2. **Universal CLI Tarball & Multi-Shell Completions**:
   - Produces release tarball `dist/ttzip-cli-v1.0.0-darwin-universal.tar.gz` containing `bin/ttzip`, man pages, and completions for zsh, bash, and fish.
3. **Synchronized Homebrew Formulas**:
   - Updates `Formula/ttzip.rb` and `Formula/ttzip-cli.rb` with exact SHA-256 and release download URLs, with automated formula tests verifying `ttzip --version`, `ttzip doctor --json`, `ttzip a`, and `ttzip t`.

---

## 2. Verification

- **Single-File LOC Defense Gate**: 640 source files $\le 800\text{ LOC}$.
- **Codesign Audit**: `codesign -vvv --deep --strict dist/TTZip.app` passes 100%.
- **Formula Invariants**: `Formula/ttzip.rb` verified with valid checksum.
- **Local CI Gate**: 4/4 stages PASS in `./scripts/run_local_ci_gate.sh`.
