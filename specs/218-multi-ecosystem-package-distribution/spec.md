# Specification: Feature 218 — Multi-Ecosystem Package Distribution

**Feature Identifier**: `218-multi-ecosystem-package-distribution`  
**Classification**: `[Full SDD]`  
**Status**: `SPECIFIED`  
**Target Repositories**:
- `https://github.com/wittkung/ttzip-core`
- `https://github.com/wittkung/homebrew-ttzip` (New Homebrew Tap)

---

## 1. Problem Statement & Motivation

Following the physical repository split and Python native PyO3 SDK implementation, TTZip Core engine is ready for production distribution across the 3 primary developer ecosystems:
1. **Homebrew Tap**: macOS/Linux developers need `brew install wittkung/ttzip/ttzip` to install the high-performance CLI without manually cloning or building from source.
2. **Crates.io**: Rust developers need pure-Rust microkernel crates (`ttzip-engine`, `ttzip-glue`, `ttzip-tui`) published with zero circular dependencies, valid documentation, and strict metadata.
3. **PyPI**: Python data engineers need `pip install ttzip` with pre-compiled ABI3 wheels supporting Python 3.10~3.14.

---

## 2. Requirements & Acceptance Criteria

### User Story 1: Official Homebrew Tap Repository (`wittkung/homebrew-ttzip`)
- **R1.1**: Create GitHub repository `wittkung/homebrew-ttzip` with public access.
- **R1.2**: Generate `Formula/ttzip.rb` that downloads `https://github.com/wittkung/ttzip-core/archive/refs/heads/main.tar.gz` (or tagged release), compiles using Cargo in release mode, and installs the binary `ttzip` to `bin/`.
- **R1.3**: Validate `brew test` and formula syntax with `brew audit --strict --online` standards.

### User Story 2: Rust Crates.io Publishing Verification & Dry-Run
- **R2.1**: Audit `rust/ttzip-engine/Cargo.toml`, `rust/ttzip-glue/Cargo.toml`, and `rust/ttzip-tui/Cargo.toml` for required metadata (`repository`, `homepage`, `documentation`, `keywords`, `categories`, `readme`).
- **R2.2**: Execute `scripts/publish_crates.sh --dry-run` to ensure all 3 crates package cleanly with zero errors.

### User Story 3: Python PyPI Maturin Wheel Generation
- **R3.1**: Build production release wheel via `maturin build --release` targeting ABI3 Python 3.10+.
- **R3.2**: Verify generated `.whl` in `dist/` contains signed native binary (`_ttzip.abi3.so`), type stubs (`py.typed`, `__init__.pyi`), and correct `METADATA`.

### User Story 4: Automated Unified Verification Gate
- **R4.1**: Provide `scripts/verify_distribution.sh` that checks Homebrew formula, Crates packaging, and Python wheel artifacts in one pass.
- **R4.2**: Pass the single-file $\le 800\text{ LOC}$ defense gate across all new files.
