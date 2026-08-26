# Research & Architecture Decisions: 202-full-architecture-sinking

## Decision 1: 4-Phase Lifecycle Sinking Architecture
- **Decision**: Execute the architecture transition according to the CTO Sinking Plan in 4 structured phases:
  1. **Phase 1**: Standalone CLI Engine (`Sources/TTZipCLI` 44 files + `Sources/TTZipBench` 1 file) down to `rust/ttzip-tui` binary.
  2. **Phase 2**: Core Algorithms, Streaming, Security & VFS (`Sources/TTZipCore` 91 files) down to `rust/ttzip-glue`.
  3. **Phase 3**: C-ABI Boundary Hardening (`Sources/TTZipCore/Bridge` + `Facades` 19 files) with Rust as single source of truth.
  4. **Phase 4**: Native macOS Experience Preservation (`Sources/TTZipApp` 144 files + Localization 15 files + Platform/Security macOS specific 29 files) 100% native in Swift.
- **Rationale**: Decouples UI from compute-intensive operations, eliminates Swift ARC overhead on hot paths, provides instant CLI start, and guarantees zero UI regression.
- **Alternatives Considered**:
  - *Keep CLI in Swift*: Rejected due to Swift runtime dynamic library dependency and higher startup latency (>30ms vs <5ms).
  - *Rewrite entire UI in Rust (Iced/Slint)*: Rejected because macOS-native design system (Kintsugi Gold, QuickLook, Touch ID, AppKit menus) requires 100% native Swift/AppKit APIs.
- **Source**: `docs/全面下沉计划.md`, `Sources/TTZipCLI/`, `Sources/TTZipCore/`.

---

## Decision 2: Standalone CLI Subcommand Parity & JSON Architecture
- **Decision**: Standardize all 18 CLI subcommands (`create`, `extract`, `list`, `info`, `check`, `hash`, `diff`, `tree`, `split`, `join`, `repair`, `recover`, `bench`, `doctor`, `cat`, `comment`, `convert`, `delete`, `lock`, `update`) in `rust/ttzip-tui` with `--json` output support via `serde` and `serde_json`.
- **Rationale**: Enables seamless headless scripting, automated CI integration, and machine readability while providing ANSI human formatting by default.
- **Alternatives Considered**: Ad-hoc string parsing. Rejected due to schema fragility and regression risks.
- **Source**: `rust/ttzip-tui/src/cli/args.rs`, `specs/202-full-architecture-sinking/contracts/cli_engine_contract.schema.json`.

---

## Decision 3: SIMD Hardware Acceleration & Memory Page Pool in Rust Core
- **Decision**: Implement CRC64 with ARM64 PMULL / NEON assembly intrinsics, Galois Field matrix operations in Reed-Solomon FEC, LZ4-compressed VFS blocks, and zero-copy byte buffers inside `rust/ttzip-glue`.
- **Rationale**: Achieves theoretical peak I/O and CPU throughput without ARC allocation overhead or garbage collection pauses.
- **Alternatives Considered**: Swift SIMD intrinsics. Rejected due to Swift ARC boxing overhead on complex memory block transformations.
- **Source**: `Sources/TTZipCore/Crypto/`, `Sources/TTZipCore/Security/`, `rust/ttzip-glue/src/`.

---

## Decision 4: Strict 8-Byte Aligned C-ABI FFI Glue
- **Decision**: Expose all core operations through explicit C-ABI functions and repr(C) structs with fixed-width integers, clamped pointers, and panic hooks preventing cross-language stack unwinding.
- **Rationale**: Guarantees ABI compatibility across compiler versions and ensures Swift facade wrappers remain thin and zero-cost.
- **Alternatives Considered**: Swift-Rust CXX bridge. Rejected to maintain zero third-party build framework dependencies.
- **Source**: `Sources/TTZipCore/Bridge/ArchiveEngineBridge.swift`, `rust/ttzip-glue/src/ffi/`.
