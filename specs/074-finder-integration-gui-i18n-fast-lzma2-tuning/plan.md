# Implementation Plan: Feature 074 — Finder Integration, GUI Bilingual Localization, and Fast LZMA2 Micro-Tuning

**Feature Branch**: `074-finder-integration-gui-i18n-fast-lzma2-tuning`  
**Created**: 2026-08-18  
**Status**: Planned  
**Spec**: [`spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/074-finder-integration-gui-i18n-fast-lzma2-tuning/spec.md) | **Research**: [`research.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/074-finder-integration-gui-i18n-fast-lzma2-tuning/research.md)

---

## Technical Context & Constitution Check

1. **Architecture**:
   - `TTZipCore`: `FinderSyncHelper.swift`, `QuickLookPreviewEngine.swift`, `TTZipLocalizationManager.swift`, `ArchiveReader.swift`.
   - `TTZipApp`: `AppLocalizationState.swift`, `SettingsView.swift`, `AppKitMenuSynchronizer.swift`, `Info.plist`.
   - `CTTZipBridge`: `ttzip_lzma_hc4_neon.c`, `fast-lzma2/count.h`, `fast-lzma2/radix_engine.h`.
2. **Performance & Safety Invariants**:
   - Zero-cost hot paths: Zero allocations in `ttzip_lzma_hc4_neon.c`.
   - 100% frozen isolation for ZIP and TAR engines.
   - Four-step closed-loop performance protocol strictly applied for LZMA2 micro-tuning.

---

## Phase 0: Research & Grounded Discovery

- R001 [SUBAGENT:research] 《macOS Finder Integration & QuickLook Preview Generator》: Investigate `QLPreviewProvider`, `FIFinderSync`, in-process header inspection, and 16-format UTI declarations.
- R002 [SUBAGENT:research] 《Desktop GUI Bilingual Localization & Runtime Language Switcher》: Investigate `TTZipLocalizationManager`, `AppLocalizationState`, `SettingsView` 5-tab redesign, and `NSMenu` synchronization.
- R003 [SUBAGENT:research] 《Fast LZMA2 Micro-Architecture Hardware Tuning & Zero-Regression Guardrails》: Investigate hybrid SWAR+NEON length scanner, lookahead prefetching, and 13 performance floors.

---

## Phase 1: Contracts, Data Models & Quickstart

- Data Model: [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/074-finder-integration-gui-i18n-fast-lzma2-tuning/data-model.md)
- Contracts:
  - [SUBAGENT:research] [`contracts/finder_action_event.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/074-finder-integration-gui-i18n-fast-lzma2-tuning/contracts/finder_action_event.json)
  - [SUBAGENT:research] [`contracts/gui_localization_state.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/074-finder-integration-gui-i18n-fast-lzma2-tuning/contracts/gui_localization_state.json)
- Quickstart Guide: [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/074-finder-integration-gui-i18n-fast-lzma2-tuning/quickstart.md)

---

## Phase 2: User Story 1 - macOS Finder Native Integration & QuickLook Previews

- Implement `QuickLookPreviewEngine.swift` in `Sources/TTZipCore/QuickLook/` generating non-blocking HTML/CSS directory tree previews.
- Integrate `FinderSyncHelper.swift` with context menu actions (`extract_here`, `compress_quick_7z`, `inspect_archive`, etc.).
- Update `Sources/TTZipApp/Info.plist` with full 16-format UTI imported declarations.
- Create unit test suite `Tests/TTZipTests/QuickLookPreviewTests.swift`.

---

## Phase 3: User Story 2 - Desktop App Complete Bilingual Localization & Preferences

- Implement `AppLocalizationState.swift` in `Sources/TTZipApp/Services/` providing `@MainActor ObservableObject` language switching.
- Redesign `SettingsView.swift` in `Sources/TTZipApp/Views/Settings/` with 5 tabs (General, Presets, Vault, Localization, License).
- Implement `AppKitMenuSynchronizer.swift` to update system menu bar items dynamically.
- Create unit test suite `Tests/TTZipTests/GUILocalizationTests.swift`.

---

## Phase 4: User Story 3 - Fast LZMA2 Micro-Architecture Tuning & Zero-Regression

- Apply two-tier hybrid SWAR + NEON match length scanner in `Sources/CTTZipBridge/fast-lzma2/count.h` and `fast-lzma2/radix_engine.h`.
- Inject lookahead cache line prefetching in `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c`.
- Reuse thread-local 64KB aligned workspaces in `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`.
- Verify positive throughput gain and run `XCTestPerformanceMeasureTests` asserting 0 regression.

---

## Phase 5: Verification & CI/CD Gate

- Run all unit tests (`swift test`).
- Run local 6-stage CI gate (`./scripts/run_local_ci_gate.sh`).
- Execute `speckit-converge` and `speckit-analyze`.
