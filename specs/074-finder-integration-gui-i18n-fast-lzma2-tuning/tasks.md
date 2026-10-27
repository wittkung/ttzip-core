# Tasks: macOS Finder Integration, Desktop GUI Bilingual Localization, and Fast LZMA2 Micro-Tuning

**Feature Branch**: `074-finder-integration-gui-i18n-fast-lzma2-tuning`  
**Created**: 2026-08-18  
**Status**: Completed  
**Spec**: [`spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/074-finder-integration-gui-i18n-fast-lzma2-tuning/spec.md) | **Plan**: [`plan.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/074-finder-integration-gui-i18n-fast-lzma2-tuning/plan.md)

---

## Phase 1: Setup & Foundational Infrastructure

**Purpose**: Core data models and localization bridges.

- [x] T001 [P] Implement `QuickLookPreviewEngine.swift` in `Sources/TTZipCore/QuickLook/QuickLookPreviewEngine.swift`.
- [x] T002 Implement `AppLocalizationState.swift` in `Sources/TTZipApp/Services/AppLocalizationState.swift` bridging `TTZipLocalizationManager` with SwiftUI.

---

## Phase 2: User Story 1 - macOS Finder Native Integration & QuickLook Previews (Priority: P1) 🎯 MVP

**Goal**: Deliver Finder context menu actions and spacebar QuickLook HTML5 directory tree previews.

**Independent Test**: `swift test --filter QuickLookPreviewTests`.

- [x] T003 [P] [US1] Implement `FinderSyncHelper.swift` context menu actions in `Sources/TTZipCore/FinderSyncHelper.swift`.
- [x] T004 [US1] Update `Sources/TTZipApp/Info.plist` with full 16-format UTI imported declarations and document type bindings.
- [x] T005 [P] [US1] Create unit test suite `Tests/TTZipTests/QuickLookPreviewTests.swift` validating header-only preview generation and UTI coverage.

---

## Phase 3: User Story 2 - Desktop App Complete Bilingual Localization & Preferences (Priority: P1)

**Goal**: Provide instant runtime language switching between 简体中文 and English in Preferences and sync menu bar items.

**Independent Test**: `swift test --filter GUILocalizationTests`.

- [x] T006 [P] [US2] Expand `LocaleCatalogZhHans.swift` and `LocaleCatalogEn.swift` in `Sources/TTZipCore/Localization/Catalogs/` for all desktop views, sheets, and menu items.
- [x] T007 [US2] Redesign `SettingsView.swift` in `Sources/TTZipApp/Views/SettingsView.swift` into a 5-tab macOS standard Preferences interface.
- [x] T008 [P] [US2] Implement `AppKitMenuSynchronizer.swift` in `Sources/TTZipApp/Services/AppKitMenuSynchronizer.swift` for dynamic menu title synchronization.
- [x] T009 [P] [US2] Create unit test suite `Tests/TTZipTests/GUILocalizationTests.swift` validating runtime switching and catalog completeness.

---

## Phase 4: User Story 3 - Fast LZMA2 Micro-Architecture Tuning & Zero-Regression (Priority: P1) ⚡️

**Goal**: Apply targeted hybrid SWAR+NEON length scanner, lookahead prefetching, and thread-local workspace reuse with zero regression.

**Independent Test**: `swift test --filter XCTestPerformanceMeasureTests`.

- [x] T010 [US3] **Step 1 & 2**: Demarcate scope and capture pre-optimization baseline throughput on `XCTestPerformanceMeasureTests`.
- [x] T011 [P] [US3] **Step 3**: Optimize `fast-lzma2/count.h`, `fast-lzma2/radix_engine.h`, and `ttzip_lzma_hc4_neon.c` with hybrid SWAR+NEON and lookahead prefetching.
- [x] T012 [US3] **Step 4**: Execute post-optimization differential audit and assert zero performance regression across all 13 hard performance floors.

---

## Phase 5: Polish & Full Verification

**Purpose**: Run all regression gates, performance floors, and CI pipelines.

- [x] T013 Run full unit test suite (`swift test`).
- [x] T014 Run local 6-stage automated CI gate (`./scripts/run_local_ci_gate.sh`).
- [x] T015 Execute `speckit-converge` and `speckit-analyze` to assert 100% specification and implementation convergence.
