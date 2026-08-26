# Implementation Plan: 013 Rust & UniFFI Unified i18n & Cross-Platform Localization Engine

- **Feature Directory**: `specs/013-rust-uniffi-unified-i18n-engine`
- **Classification**: `[Full SDD]`
- **Status**: `Planning`
- **Created**: 2026-08-25
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Technical Context & Architectural Architecture

### 1.1 Scope of Changes
- **Rust Engine Core (`core/rust/ttzip-engine/`)**:
  - `src/i18n/mod.rs`: Compile-time dictionary tables with $O(1)$ lookups and English fallback.
  - `src/i18n/catalogs/`: Static 398-key dictionaries for `en`, `zh_hans`, `zh_hant`, `ja`, `de`, `fr`, `es`.
  - `src/i18n/formatting.rs`: High-speed CLDR number/byte formatting (SI/IEC) and plural rules.
  - `src/uniffi_api/mod.rs`: UniFFI export declarations (`AppLanguage`, `ByteSizeStandard`, `TTZipLocalizationEngine`).
- **UniFFI Code Generation**:
  - Update `core/rust/Cargo.toml` with `uniffi` proc-macro exports.
  - Generate updated Swift bindings and C headers via `uniffi-bindgen`.
- **Swift Integration Layer (`core/Sources/TTZipCore/Localization/`)**:
  - Bridge `TTZipLocalizationManager` to `TTZipLocalizationEngine`.
  - Re-route `ByteSizeFormatter` and `ThroughputFormatter` to Rust.
  - Preserve 100% existing Swift public interfaces (`LocaleKey`, `L10nText`, `L10nLabel`).

### 1.2 Constitution Check
- **Zero-Subprocess Policy**: Fully compliant. All localization calls are in-process memory lookups via UniFFI direct foreign function bindings.
- **Strict Single-File LOC Threshold ($\le 800$ LOC)**: Catalogs in Rust will be modularized per-language (`en.rs`, `zh_hans.rs`, etc.), each keeping $\le 450$ lines.
- **Zero In-Tree Path Invariant**: Compliant. All dictionaries are baked directly into binary `.rodata`.

---

## 2. Execution Phases

### Phase 0: Research & Benchmarking (`research.md`)
- Investigate `phf` vs `match` vs static array slices for zero-allocation Rust string lookups.
- Evaluate UniFFI record vs object export patterns for thread-safe global engines.
- Research CLDR delimiter rules for German, French, Spanish, Japanese, and Chinese.

### Phase 1: Design Artifacts (`data-model.md`, `contracts/`, `quickstart.md`)
- Define entity structures, enum mappings, and UniFFI interface contracts.
- Provide end-to-end verification quickstart for Rust CLI, Python SDK, and Swift App.

### Phase 2: Implementation & UniFFI Code Generation
- Implement Rust `i18n` subsystem and unit tests.
- Export `TTZipLocalizationEngine` via UniFFI proc macros.
- Generate Swift scaffolding and update `TTZipCore`.

### Phase 3: Swift GUI Thin-Bridge Refactoring
- Connect `TTZipLocalizationManager` to Rust UniFFI engine.
- Verify full test suite (184+ tests).
