# Tasks: 012 Comprehensive i18n and Localization Architecture Overhaul

- **Feature Directory**: `specs/012-comprehensive-i18n-and-localization-architecture`
- **Specification**: `specs/012-comprehensive-i18n-and-localization-architecture/spec.md`
- **Implementation Plan**: `specs/012-comprehensive-i18n-and-localization-architecture/plan.md`

---

## Phase 1: Setup & Foundational Infrastructure

Goal: Establish expanded localization keys, preference synchronization structures, and shared SwiftUI primitives.

- [x] T001 Expand `L10n` namespace hierarchy in `core/Sources/TTZipCore/Localization/LocaleKey.swift`
- [x] T002 [P] Implement cross-process AppGroup preferences store in `core/Sources/TTZipCore/Localization/TTZipPreferencesStore.swift`
- [x] T003 [P] Implement `L10nText` and `L10nLabel` reactive primitives in `apple/Sources/TTZipApp/Theme/L10nPrimitives.swift`

---

## Phase 2: User Story 1 (Genuine Multi-Language Catalogs & Terminology)

Goal: Completely rebuild all 7 language catalogs with 100% genuine translations and positional format specifiers.

- [x] T004 [P] [US1] Rebuild German catalog with 100% genuine translations in `core/Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+De.swift`
- [x] T005 [P] [US1] Rebuild French catalog with 100% genuine translations in `core/Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+Fr.swift`
- [x] T006 [P] [US1] Rebuild Spanish catalog with 100% genuine translations in `core/Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+Es.swift`
- [x] T007 [P] [US1] Rebuild Japanese catalog with 100% genuine translations in `core/Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+Ja.swift`
- [x] T008 [P] [US1] Standardize macOS HIG terminology in Simplified Chinese in `core/Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+ZhHans.swift`
- [x] T009 [P] [US1] Standardize Taiwan Traditional Chinese regional terminology in `core/Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+ZhHant.swift`
- [x] T010 [P] [US1] Align English base catalog with new namespaces in `core/Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+En.swift`

---

## Phase 3: User Story 2 (AppKit Menu Topology & Dynamic Switch Reactivity)

Goal: Eliminate string matching from AppKit menus and guarantee sub-millisecond dynamic language switching.

- [x] T011 [US2] Refactor `AppKitMenuSynchronizer` with permanent Tag and Selector topology in `apple/Sources/TTZipApp/Services/AppKitMenuSynchronizer.swift`
- [x] T012 [P] [US2] Implement reactive SwiftUI `TTZipMenuCommands` in `apple/Sources/TTZipApp/Views/TTZipMenuCommands.swift`
- [x] T013 [P] [US2] Connect `AppLocalizationState` to `TTZipPreferencesStore` and `TTZipMenuCommands` in `apple/Sources/TTZipApp/TTZipApp.swift`
- [x] T014 [P] [US2] Clean up hardcoded strings in `apple/Sources/TTZipApp/Views/Components/CompressIntegratedConfigSectionView.swift`
- [x] T015 [P] [US2] Clean up hardcoded strings in `apple/Sources/TTZipApp/Views/Explorer/MillerColumnItemRowView+ContextMenu.swift`

---

## Phase 4: User Story 3 (System Extensions, FinderSync, QuickLook & Bundle Localization)

Goal: Localize QuickLook HTML rendering, synchronize FinderSync cross-process preferences, and provide `InfoPlist.strings`.

- [x] T016 [P] [US3] Parameterize `generateHTMLPreview` with language support in `core/Sources/TTZipCore/QuickLook/QuickLookPreviewEngine.swift`
- [x] T017 [P] [US3] Localize QuickLook preview view controller and error pages in `apple/Sources/TTZipQuickLook/QuickLookPreviewViewController.swift`
- [x] T018 [P] [US3] Integrate Darwin notification listener and JIT preference sync in `apple/Sources/TTZipFinderSync/FinderSync.swift`
- [x] T019 [P] [US3] Localize context menu titles and templates in `core/Sources/TTZipCore/FinderSyncHelper.swift`
- [x] T020 [P] [US3] Create 7x `.lproj/InfoPlist.strings` and update bundle script in `apple/scripts/bundle_app.sh`

---

## Phase 5: User Story 4 (Core Models, Error Pipeline & PasswordVault Localization)

Goal: Eliminate string concatenation in error formatting and localize security vault modules.

- [x] T021 [P] [US4] Implement `PasswordStrengthTier` and localized Touch ID auth in `core/Sources/TTZipCore/PasswordVaultManager.swift`
- [x] T022 [P] [US4] Localize biometric authentication errors and prompts in `core/Sources/TTZipCore/PasswordVaultModels.swift`
- [x] T023 [P] [US4] Refactor `ArchiveError.localizedDescription` and `TTZipLocalizationManager` in `core/Sources/TTZipCore/Localization/TTZipLocalizationManager.swift`
- [x] T024 [P] [US4] Support dynamic built-in preset localization in `core/Sources/TTZipCore/PresetManager.swift`
- [x] T025 [P] [US4] Bridge localized outputs and `--lang` flag in `core/Sources/TTZipBench/main.swift`

---

## Phase 6: Polish, View Hardcoding Elimination & CI Security Tests

Goal: Clean up remaining view hardcoding and establish automated CI regression tests.

- [x] T026 [P] Clean up hardcoded strings in `apple/Sources/TTZipApp/Views/PasswordPromptSheetView.swift`
- [x] T027 [P] Clean up hardcoded strings in `apple/Sources/TTZipApp/Views/ArchiveInspectorSheet.swift`
- [x] T028 [P] Clean up hardcoded strings in `apple/Sources/TTZipApp/Views/Benchmark/BenchmarkConfigSectionView.swift`
- [x] T029 Implement 4-gate security and anti-copy test suite in `core/Tests/TTZipTests/TTZipLocalizationSecurityTests.swift`
- [x] T030 Verify end-to-end multi-language switching and contract linting
