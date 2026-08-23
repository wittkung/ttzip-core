# Implementation Plan: Full Architecture Sinking & Swift-Rust Boundary Execution

**Branch**: `202-full-architecture-sinking` | **Date**: 2026-08-22 | **Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/202-full-architecture-sinking/spec.md)

**Input**: Feature specification from `specs/202-full-architecture-sinking/spec.md`

---

## Summary

This plan executes the end-to-end architecture sinking strategy across the 373 Swift source and test files in TTZip. The work is structured into four sequential phases:
1. **Phase 1**: Standalone Rust CLI (`ttzip-tui`) full feature parity across all 18 archive subcommands with sub-5ms cold startup and `--json` support.
2. **Phase 2**: Core compute, VFS page pool, and SIMD hardware acceleration sinking into `ttzip-glue`.
3. **Phase 3**: Thin C-ABI facade consolidation in `TTZipCore/Bridge` and `TTZipCore/Facades`.
4. **Phase 4**: Pristine macOS GUI experience preservation in `TTZipApp` and passing all CI gates.

---

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`), Rust 1.80+ (2021 edition), C11
**Primary Dependencies**: `clap` 4.x, `ratatui` 0.28, `indicatif` 0.17, `serde` / `serde_json`, `rayon`, `tokio`, Apple AppKit / SwiftUI
**Storage**: Virtual File System (VFS) in-memory cache, LZ4 compressed memory pools
**Testing**: `cargo test --workspace`, `swift test`, XCTest integration suites
**Target Platform**: macOS 14.0+ (Apple Silicon ARM64 NEON & Intel x86_64)
**Project Type**: Mixed Desktop App + Standalone CLI + Native Systems Engine
**Performance Goals**: CLI cold start $< 5\text{ms}$, 60fps UI rendering, streaming memory footprint $\le 128\text{MB}$
**Constraints**: Zero regression across all 525+ Swift tests, strict 800 LOC limit per file, no unhandled C-ABI exceptions

---

## Constitution Check

*GATE: Passed prior to design and re-verified post-design.*

- [x] **Stream-First Invariant**: Bounded memory buffers ($\le 128\text{MB}$), zero unbounded memory allocations on hot paths.
- [x] **Invariant-First Defense**: POSIX-level path sanitization and Zip Slip prevention.
- [x] **Bounds-First Safety**: Strict 8-byte aligned C-ABI structs and RAII cleanup (`Drop`).
- [x] **Oracle-First Validation**: Golden corpus verification and cross-format differential tests.
- [x] **Quality Gate Compliance**: Passes `./scripts/lint_loc_gate.sh` and `./scripts/run_local_ci_gate.sh`.

---

## Project Structure

### Documentation (this feature)

```text
specs/202-full-architecture-sinking/
├── plan.md              # Implementation plan (this file)
├── research.md          # Architectural decisions & research
├── data-model.md        # CLI & C-ABI data structures
├── quickstart.md        # Runnable verification guide
├── contracts/           # CLI JSON schema & C-ABI headers
│   ├── cli_engine_contract.schema.json
│   └── rust_swift_c_abi.h
├── checklists/
│   └── requirements.md
└── tasks.md             # Task decomposition (generated in tasks phase)
```

### Source Code Mapping

```text
rust/
├── ttzip-tui/           # Standalone CLI & interactive TUI engine
│   ├── src/cli/         # Subcommand handlers (18 commands + JSON output)
│   └── src/app/         # Interactive TUI modal handlers
└── ttzip-glue/          # Core compute, VFS, SIMD, and C-ABI export
    ├── src/crypto/      # CRC64, PMULL SIMD, Adler32
    ├── src/security/    # Reed-Solomon FEC, ZipSlip defense
    ├── src/vfs/         # MemoryPagePool, VFSLz4CachePool
    └── src/ffi/         # C-ABI export functions

Sources/
├── TTZipCLI/            # Thin Swift CLI delegator
├── TTZipCore/           # Thin Swift facades & C-ABI bindings
└── TTZipApp/            # 100% native macOS GUI & services (144 files preserved)
```

---

## Complexity Tracking

| Aspect | Justification | Tradeoff / Mitigation |
| :--- | :--- | :--- |
| Dual-layer CLI (Rust binary + Swift CLI target) | Provides standalone fast CLI for POSIX scripting while keeping SwiftPM target intact for Xcode developers | Both call identical core engines with standardized arguments |
| C-ABI FFI Glue | Avoids complex CXX wrappers and keeps pure C header interchange | Strict automated layout validation in tests |
