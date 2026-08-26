# Implementation Plan: 012 Comprehensive i18n and Localization Architecture Overhaul

- **Feature Directory**: `specs/012-comprehensive-i18n-and-localization-architecture`
- **Type**: `[Full SDD]`
- **Status**: `Draft`
- **Created**: 2026-08-25

---

## 1. Technical Context

- **Platform**: macOS 14.0+ / Swift 6.0 Strict Concurrency / AppKit & SwiftUI / Cocoa XPC Extensions
- **Subsystems**:
  - `TTZipCore/Localization/`: `LocaleKey.swift`, `AppLanguage.swift`, `TTZipLocalizationManager.swift`, `Catalogs/LocaleCatalog+*.swift` (7 languages)
  - `TTZipCore/Services/`: `PasswordVaultManager.swift`, `PasswordVaultModels.swift`, `PresetManager.swift`, `FinderSyncHelper.swift`, `QuickLookPreviewEngine.swift`
  - `TTZipApp/Services/`: `AppLocalizationState.swift`, `AppKitMenuSynchronizer.swift`, `TTZipPreferencesStore.swift`, `SystemNotificationManager.swift`, `SystemDialogHelper.swift`
  - `TTZipApp/Views/`: 87 SwiftUI View files across Compress, Explorer, PasswordVault, Benchmark, Preview, Settings, Inspector
  - `TTZipApp/Resources/`: 7x `.lproj/InfoPlist.strings` directories
  - `TTZipFinderSync/` & `TTZipQuickLook/`: Isolated macOS app extensions with AppGroup preference bridge

---

## 2. Constitution Checks & Guardrails

| Constitution Principle | Status | Compliance Verification Method |
| :--- | :--- | :--- |
| **Zero-Subprocess Policy** | ✅ PASS | All localization lookups are zero-I/O in-memory lookups; zero CLI subprocesses used for translation. |
| **Single-File LOC ($\le 800$)** | ✅ PASS | Large view files broken down into subcomponents; all catalogs modularized. |
| **Zero In-Tree Path Invariant** | ✅ PASS | Catalogs and localized strings compiled directly into binaries and frameworks without filesystem path dependencies. |
| **Living & Executable Examples** | ✅ PASS | `TTZipLocalizationSecurityTests` and Quickstart validation scenarios runnable on every commit. |
| **Transparent Packaging** | ✅ PASS | `.lproj` resources copied cleanly into `TTZip.app/Contents/Resources/` by `bundle_app.sh`. |

---

## 3. Work Breakdown Structure (Phases)

### Phase 1: Specifications, Contracts & Catalog Foundation
- **T001**: Expand `LocaleKey.swift` with namespaces (`Diagnostics`, `Recovery`, `QuickLook`, `FinderSync`, `Preview`, `Benchmark`) adding 70+ strong keys.
- **T002**: Rebuild `LocaleCatalog+De.swift`, `LocaleCatalog+Fr.swift`, `LocaleCatalog+Es.swift`, `LocaleCatalog+Ja.swift` with 100% genuine translations.
- **T003**: Align `LocaleCatalog+ZhHans.swift` and `LocaleCatalog+ZhHant.swift` with Apple macOS HIG standard terminology.
- **T004**: Convert all multi-argument format strings to positional specifiers (`%1$@`, `%2$d`).

### Phase 2: Core Subsystem, Extension IPC & AppKit Menu Architecture
- **T005**: Create `TTZipPreferencesStore` with AppGroup suite (`group.com.metastudyline.ttzip`) and Darwin notification dispatch.
- **T006**: Completely refactor `AppKitMenuSynchronizer` using permanent Tag + Selector topology mapping.
- **T007**: Refactor `QuickLookPreviewEngine` to accept `language:` parameter and generate localized HTML.
- **T008**: Update `FinderSync.swift` with Darwin observer and JIT preference fallback.
- **T009**: Create 7x `.lproj/InfoPlist.strings` for macOS Finder bundle localization and update `bundle_app.sh`.

### Phase 3: Core Models, Errors & SwiftUI View Hierarchy Clean-up
- **T010**: Refactor `PasswordStrengthTier` and `BiometricAuthError` in `PasswordVaultManager.swift`.
- **T011**: Refactor `ArchiveError.localizedDescription` to eliminate string concatenation and map to `L10n.Errors`.
- **T012**: Introduce `L10nText` and `L10nLabel` primitives and replace 884+ hardcoded strings across all SwiftUI views.
- **T013**: Migrate form layouts in `CompressIntegratedConfigSectionView` and `SettingsView` to adaptive `Grid` / `GridRow`.

### Phase 4: Automated Testing & CI Gating
- **T014**: Implement `TTZipLocalizationSecurityTests` covering 100% keys, anti-copy-paste rules, and format fuzzing.
- **T015**: Implement AST static linter script and integrate into CI gate.
