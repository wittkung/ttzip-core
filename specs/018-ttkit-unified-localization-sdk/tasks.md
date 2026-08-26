# Tasks: 018 TTKit Unified Localization SDK & Ecosystem Migration

- **Feature ID**: `018-ttkit-unified-localization-sdk`
- **Specification**: [spec.md](./spec.md)
- **Plan**: [plan.md](./plan.md)
- **Data Model**: [data-model.md](./data-model.md)
- **Contracts**: [contracts/](./contracts/)

---

## Phase 1: Standalone Core Engine (ttkit-localization)

- [x] T001 Initialize standalone Rust crate in `ttkit-localization/tt-i18n-core/Cargo.toml` with UniFFI scaffolding
- [x] T002 Implement compile-time zero-allocation static slice lookup and BCP-47 language parser in `ttkit-localization/tt-i18n-core/src/catalog.rs`
- [x] T003 Implement CLDR formatting engine (SI/IEC byte sizes, throughput, decimal delimiters, error localization) in `ttkit-localization/tt-i18n-core/src/cldr.rs`
- [x] T004 Implement `TTLocalizationEngine` UniFFI export object in `ttkit-localization/tt-i18n-core/src/engine.rs`
- [x] T005 Add comprehensive Rust unit tests for lookup, delimiters, and fallback in `ttkit-localization/tt-i18n-core/src/tests.rs`

---

## Phase 2: Apple Native Swift 6 SDK (TTLocalizationKit)

- [x] T006 Initialize Swift Package in `ttkit-localization/TTLocalizationKit/Package.swift` with Swift 6 strict concurrency
- [x] T007 Implement UniFFI bridging layer and `TTLocalizationManager` in `ttkit-localization/TTLocalizationKit/Sources/TTLocalizationCore/LocalizationManager.swift`
- [x] T008 Implement Swift 6 `@Observable` `LocalizationState` and reactive `L10nText`/`L10nLabel` in `ttkit-localization/TTLocalizationKit/Sources/TTLocalizationUI/LocalizationState.swift`
- [x] T009 Implement 3-tier topological `AppKitMenuSynchronizer` in `ttkit-localization/TTLocalizationKit/Sources/TTLocalizationAppKit/AppKitMenuSynchronizer.swift`
- [x] T010 Implement `DarwinNotificationBridge` and AppGroup preferences store in `ttkit-localization/TTLocalizationKit/Sources/TTLocalizationIPC/DarwinNotificationBridge.swift`

---

## Phase 3: CodeGen & CI Quality Governance Tooling (tt-l10n-tools)

- [x] T011 Initialize CLI crate in `ttkit-localization/tt-l10n-tools/Cargo.toml` with `clap` and `serde_json`
- [x] T012 Implement contract-to-code generator (JSON -> Rust `.rodata` slices and Swift `LocaleKeyProtocol` enums) in `ttkit-localization/tt-l10n-tools/src/codegen.rs`
- [x] T013 Implement 4-stage anti-fake translation detection algorithm in `ttkit-localization/tt-l10n-tools/src/validator.rs`
- [x] T014 Implement format specifier (`%1$@`, `%2$d`) consistency and fuzzing harness in `ttkit-localization/tt-l10n-tools/src/fuzzer.rs`

---

## Phase 4: Single Source of Truth Contract & Code Generation Dogfooding

- [x] T015 Consolidate 415 keys across 7 languages into `ttkit-localization/contracts/ttzip-catalog.json`
- [x] T016 Execute `tt-l10n-tools` CLI to validate parity, anti-fake thresholds, and format specifiers
- [x] T017 Execute `tt-l10n-tools` CLI to generate Rust `.rodata` slices and Swift `LocaleKey.swift` with keyword escaping

---

## Phase 5: TTZip Host Migration & System Extensions Alignment

- [x] T018 Migrate `ttzip-engine` to depend on `ttkit-localization/tt-i18n-core`
- [x] T019 Migrate `TTZipCore` and `TTZipApp` to depend on `ttkit-localization/TTLocalizationKit`
- [x] T020 Align native SwiftUI `SettingsView.swift` & `SettingsView+Tabs.swift` with 7 languages and instant switching
- [x] T021 Run full integration regression test suites across Rust, Swift Core, and macOS GUI
