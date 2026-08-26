# Feature Specification: 013 Rust & UniFFI Unified i18n & Cross-Platform Localization Engine

- **Feature Directory**: `specs/013-rust-uniffi-unified-i18n-engine`
- **Classification**: `[Full SDD]`
- **Status**: `Specified`
- **Created**: 2026-08-25
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Executive Summary & Problem Statement

In Feature 012, TTZip successfully cured front-end UI hardcoding, established genuine 7-language catalogs (398 keys/language), and resolved AppKit menu/system extension synchronizations within the Swift layer.

However, from a holistic systems architecture standpoint:
1. **Catalog Fragmentation & Code Duplication Risk**: The 7 language dictionaries currently reside purely inside Swift (`LocaleCatalog+*.swift`). When expanding TTZip to Python SDK, Go/Java/C# SDKs, Rust native CLI (`ttzip-tui` / headless Linux), or cross-platform desktop (Windows/Linux), each language ecosystem would need a duplicated, redundant set of dictionaries, creating inevitable synchronization drift and fake-localization regressions.
2. **Runtime Overhead & Memory Duplication**: Maintaining multiple copies of static strings across Swift and other SDKs duplicates memory footprints in multi-process or multi-runtime integrations.
3. **Primitive String Formatting**: Number, throughput, and byte size formatting currently rely on manual character replacements (`replacingOccurrences(of: ".", with: ",")`) rather than zero-allocation Unicode CLDR / ICU4X standard rules.

This feature moves the single source of truth (SSOT) of all localized catalogs, formatting rules, and error code translations down into the core Rust engine (`core/rust/ttzip-engine/src/i18n/`), and exports them cleanly through **Mozilla UniFFI** (`uniffi::export`). The Swift GUI and all multi-language SDKs consume this single unified Rust localization engine without manual C-ABI pointers.

---

## 2. User Stories & Acceptance Criteria

### User Story 1: Single Source of Truth in Rust (SSOT)
- **As a** cross-platform developer and maintainer,
- **I want** all 7 language catalogs (398 keys each) baked into the Rust binary at compile time via zero-allocation static structures,
- **So that** Swift GUI, Python SDK, Kotlin/Android, C#, and CLI share the exact same dictionary with zero duplication.
- **Acceptance Criteria**:
  - `core/rust/ttzip-engine/src/i18n/` contains all 7 language dictionaries (En, ZhHans, ZhHant, Ja, De, Fr, Es).
  - String lookups execute in $O(1)$ time with zero heap allocation per lookup.
  - Rust unit tests verify 100% key completeness across all 7 languages.

### User Story 2: Type-Safe UniFFI Multi-Language Bindings
- **As a** Swift / Python / C# / Kotlin SDK consumer,
- **I want** strongly-typed enums (`AppLanguage`, `ByteSizeStandard`) and a thread-safe `TTZipLocalizationEngine` generated automatically by UniFFI,
- **So that** multi-language text lookup, error localization, and number formatting are available natively in my language with full compile-time safety.
- **Acceptance Criteria**:
  - `uniffi_api/mod.rs` exports `AppLanguage`, `ByteSizeStandard`, and `TTZipLocalizationEngine`.
  - UniFFI generates Swift bindings without manual C pointers or raw `UnsafePointer` manipulation.
  - SDK unit tests verify dynamic language switching and error string localization.

### User Story 3: Swift GUI Thin-Bridge Migration
- **As a** macOS user using the TTZip GUI,
- **I want** UI components (`L10nText`, `L10nLabel`, menus, QuickLook, FinderSync) to continue functioning with real-time reactivity,
- **So that** the transition to the Rust UniFFI engine is completely seamless with zero visual or latency regression (< 5ms).
- **Acceptance Criteria**:
  - `TTZipLocalizationManager.swift` delegates catalog resolution and byte size formatting to UniFFI `TTZipLocalizationEngine`.
  - All existing 184 Swift package unit tests continue to pass 100%.

### User Story 4: Rust-Native CLDR Formatters & Error Localization
- **As a** CLI and headless server user on Linux / macOS,
- **I want** throughput, byte size (SI vs IEC), plural rules, and archive errors to format according to local language conventions natively in Rust,
- **So that** command-line outputs and telemetry logs are fully localized without requiring Cocoa/AppKit runtimes.
- **Acceptance Criteria**:
  - Rust engine provides `format_bytes(bytes, standard, lang)`, `format_throughput(mbs, lang)`, and `localize_error(error_code, lang)`.
  - Decimal delimiters (`,` for De/Fr/Es, `.` for En/Zh/Ja) format accurately without external dynamic libraries.

---

## 3. Functional Requirements

- **FR-001**: Rust core engine MUST implement an internal `i18n` module containing 7 language catalogs with 398 standardized keys.
- **FR-002**: Rust `i18n` engine MUST support fallback to English (`En`) when a key is absent in a target catalog.
- **FR-003**: Rust `i18n` engine MUST provide zero-allocation compile-time lookup structures.
- **FR-004**: Rust engine MUST export `AppLanguage` enum through UniFFI covering `En`, `ZhHans`, `ZhHant`, `Ja`, `De`, `Fr`, and `Es`.
- **FR-005**: Rust engine MUST export `ByteSizeStandard` enum through UniFFI covering `MetricSI` and `BinaryIEC`.
- **FR-006**: Rust engine MUST export `TTZipLocalizationEngine` object through UniFFI with `get_string`, `format_bytes`, `format_throughput`, and `localize_error` methods.
- **FR-007**: UniFFI bindings MUST be regenerated and updated in `core/Sources/CTTZipBridge/include/` and `core/Sources/TTZipCore/Generated/`.
- **FR-008**: Swift `TTZipLocalizationManager` MUST bridge to UniFFI `TTZipLocalizationEngine` while maintaining backward compatibility with `LocaleKeyProtocol`.
- **FR-009**: Swift `ByteSizeFormatter` and `ThroughputFormatter` MUST delegate formatting to the Rust engine.
- **FR-010**: All Swift UI components and tests MUST pass without modification to their public API.

---

## 4. Success Criteria

- **SC-001 (Zero Duplication)**: 100% of the 398 localization keys across 7 languages are maintained solely within Rust `core/rust/ttzip-engine/src/i18n/`.
- **SC-002 (Lookup Latency)**: Rust in-memory string lookup latency is $< 10\text{ ns}$ per operation.
- **SC-003 (Binding Safety)**: Zero manual C header or raw pointer code in Swift; 100% generated via UniFFI.
- **SC-004 (Test Coverage)**: 100% pass rate on all Rust `cargo test` and Swift `swift test` (184+ tests).
- **SC-005 (Cross-Platform Ready)**: Python SDK and Rust CLI can query localized strings and formats directly.
