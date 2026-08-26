# Plan: Multi-Ecosystem Package Distribution

**Feature**: `218-multi-ecosystem-package-distribution`  
**Classification**: `[Full SDD]`  

---

## 1. Technical Implementation Phases

### Phase 1: Homebrew Tap Repository Creation & Formula Deployment
1. Create `wittkung/homebrew-ttzip` repository on GitHub via `gh repo create`.
2. Generate `Formula/ttzip.rb` with head repository support (`https://github.com/wittkung/ttzip-core.git`).
3. Commit and push `Formula/ttzip.rb` to `wittkung/homebrew-ttzip`.

### Phase 2: Rust Crates.io Packaging Validation
1. Verify `rust/ttzip-engine/Cargo.toml`, `rust/ttzip-glue/Cargo.toml`, `rust/ttzip-tui/Cargo.toml`.
2. Run `cargo package --dry-run` on each crate.

### Phase 3: Python PyPI Maturin Wheel Build
1. Run `maturin build --release --out dist/` to generate production wheel.
2. Inspect wheel zip contents and verify native ABI3 binary.

### Phase 4: Unified Verification & LOC Gate
1. Create `scripts/verify_distribution.sh`.
2. Pass single-file $\le 800\text{ LOC}$ defense gate.
