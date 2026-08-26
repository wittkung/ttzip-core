# Tasks: 013 Rust & UniFFI Unified i18n & Cross-Platform Localization Engine

- **Feature Directory**: `specs/013-rust-uniffi-unified-i18n-engine`
- **Specification**: `specs/013-rust-uniffi-unified-i18n-engine/spec.md`
- **Implementation Plan**: `specs/013-rust-uniffi-unified-i18n-engine/plan.md`

---

## Phase 1: Setup & Foundational Infrastructure

Goal: Establish Rust i18n module architecture, zero-allocation lookup mechanics, and CLDR formatting engine.

- [x] T001 Create Rust i18n module entry and registry in `core/rust/ttzip-engine/src/i18n/mod.rs`
- [x] T002 [P] Implement static catalog lookup engine and fallback mechanics in `core/rust/ttzip-engine/src/i18n/catalog.rs`
- [x] T003 [P] Implement Rust CLDR number, byte size (SI/IEC) and throughput formatter in `core/rust/ttzip-engine/src/i18n/formatting.rs`

---

## Phase 2: User Story 1 (7-Language Rust Dictionaries)

Goal: Port all 7 language catalogs (398 keys each) into compile-time static slices in Rust `.rodata`.

- [x] T004 [P] [US1] Port English dictionary to Rust static table in `core/rust/ttzip-engine/src/i18n/catalogs/en.rs`
- [x] T005 [P] [US1] Port Simplified Chinese dictionary to Rust static table in `core/rust/ttzip-engine/src/i18n/catalogs/zh_hans.rs`
- [x] T006 [P] [US1] Port Traditional Chinese dictionary to Rust static table in `core/rust/ttzip-engine/src/i18n/catalogs/zh_hant.rs`
- [x] T007 [P] [US1] Port Japanese dictionary to Rust static table in `core/rust/ttzip-engine/src/i18n/catalogs/ja.rs`
- [x] T008 [P] [US1] Port German dictionary to Rust static table in `core/rust/ttzip-engine/src/i18n/catalogs/de.rs`
- [x] T009 [P] [US1] Port French dictionary to Rust static table in `core/rust/ttzip-engine/src/i18n/catalogs/fr.rs`
- [x] T010 [P] [US1] Port Spanish dictionary to Rust static table in `core/rust/ttzip-engine/src/i18n/catalogs/es.rs`

---

## Phase 3: User Story 2 (UniFFI Exports & Code Generation)

Goal: Export strongly-typed i18n APIs through UniFFI and regenerate FFI bridge artifacts.

- [x] T011 [US2] Declare `AppLanguage`, `ByteSizeStandard`, and `TTZipLocalizationEngine` in `core/rust/ttzip-engine/src/uniffi_api/mod.rs`
- [x] T012 [P] [US2] Implement Rust native unit tests for 398-key parity and formatters in `core/rust/ttzip-engine/src/i18n/tests.rs`
- [x] T013 [US2] Recompile Rust library and regenerate UniFFI Swift scaffolding in `core/Sources/CTTZipBridge/` and `core/Sources/TTZipCore/`

---

## Phase 4: User Story 3 & 4 (Swift Thin Bridge & End-to-End Verification)

Goal: Connect Swift GUI and CLI layers to the Rust UniFFI engine and ensure 100% test passing.

- [x] T014 [P] [US3] Refactor `TTZipLocalizationManager.swift` to delegate dictionary queries to UniFFI engine in `core/Sources/TTZipCore/Localization/TTZipLocalizationManager.swift`
- [x] T015 [P] [US3] Refactor `ByteSizeFormatter.swift` and `ThroughputFormatter.swift` to delegate formatting to UniFFI in `core/Sources/TTZipCore/Localization/TTZipLocalizationManager.swift`
- [x] T016 [P] [US4] Wire CLI and headless error code localization in `core/Sources/TTZipBench/main.swift`
- [x] T017 [US4] Run full CI verification suites (`cargo test`, `swift test` in `core/` and `apple/`)
