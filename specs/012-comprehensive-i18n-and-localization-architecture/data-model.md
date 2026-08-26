# Data Model: 012 Comprehensive i18n and Localization Architecture

- **Feature Directory**: `specs/012-comprehensive-i18n-and-localization-architecture`
- **Created**: 2026-08-25

---

## 1. Entity Definitions

### 1.1 `AppLanguage` (Core Language Descriptor)
- **File**: `core/Sources/TTZipCore/Localization/AppLanguage.swift`
- **Enum Cases**:
  - `en` ("en", "English")
  - `zhHans` ("zh-Hans", "简体中文")
  - `zhHant` ("zh-Hant", "繁體中文")
  - `ja` ("ja", "日本語")
  - `de` ("de", "Deutsch")
  - `fr` ("fr", "Français")
  - `es` ("es", "Español")
- **Properties**:
  - `id: String { rawValue }`
  - `bcp47: String`
  - `displayName: String`
  - `isRTL: Bool { false }`

### 1.2 `L10n` Namespaces & `LocaleKeyProtocol`
- **File**: `core/Sources/TTZipCore/Localization/LocaleKey.swift`
- **Groups**:
  - `L10n.Common` (cancel, ok, done, save, close, retry, delete, apply, search, ...)
  - `L10n.Sidebar` (homeAndExtract, newArchive, presets, benchmark, vault, licensing, settings, queue, ...)
  - `L10n.Explorer` (columnsView, gridView, listView, sortByName, emptyDirectory, ...)
  - `L10n.Compress` (title, startAction, format, level, solidArchive, splitVolume, encryption, ...)
  - `L10n.Extract` (title, action, here, toSubfolder, destination, autoOpenFolder, passwordPrompt, ...)
  - `L10n.Benchmark` (throughput, compressionRatio, duration, memoryUsage, peakThroughput, ...)
  - `L10n.Presets` (title, createNew, duplicate, resetDefaults, proConfig, undo, redo, ...)
  - `L10n.Vault` (title, unlockPrompt, addPassword, emptyVault, biometricPrompt, ...)
  - `L10n.Settings` (title, general, localization, language, byteUnits, unitSI, unitIEC, ...)
  - `L10n.Queue` (title, activeTasks, overallThroughput, taskCompressing, taskExtracting, ...)
  - `L10n.Preview` (loading, unsupported, fullScreen, exitFullScreen, pageCount, dimensions, ...)
  - `L10n.Menu` (about, hide, quit, closeWindow, minimize, zoom, fileMenu, editMenu, undo, redo, cut, copy, paste, selectAll, ...)
  - `L10n.Dialogs` (confirmDeleteTitle, confirmDeleteMessage, overwriteTitle, overwriteMessage, ...)
  - `L10n.Errors` (fileNotFound, permissionDenied, diskFull, passwordRequiredHeaderAndData, passwordRequiredPayload, engineFailure, ...)
  - `L10n.Units` (bytes, kb, mb, gb, tb, mbPerSec, seconds, percent, itemsCount, coresCount, ...)
  - `L10n.CLI` (usageHeader, subcommands, globalOptions, errorMissingArg, benchRunning, testSummary, ...)
  - `L10n.Notification` (taskCompletedTitle, taskCompletedBody, taskFailedTitle, threatInterceptedTitle, ...)
  - `L10n.Diagnostics` *(New)* (standards, compliance, extraFields, signatures, ...)
  - `L10n.Recovery` *(New)* (multiCoreRecovery, candidateWords, dictionaryAttack, ...)
  - `L10n.QuickLook` *(New)* (tableHeaderName, tableHeaderSize, renderedFooter, compressedFormat, itemsOmittedFormat)
  - `L10n.FinderSync` *(New)* (extractHereTitle, extractSubfolderTitle, compress7zTitle, compressZipTitle, ...)

### 1.3 `PasswordStrengthTier`
- **File**: `core/Sources/TTZipCore/PasswordVaultModels.swift`
- **Enum Cases**:
  - `veryWeak = "vault.strength_very_weak"`
  - `weak = "vault.strength_weak"`
  - `medium = "vault.strength_medium"`
  - `strong = "vault.strength_strong"`
  - `veryStrong = "vault.strength_very_strong"`
- **Conformances**: `String, CaseIterable, LocaleKeyProtocol, Sendable`

### 1.4 `MenuTag` & Topology Structs
- **File**: `apple/Sources/TTZipApp/Services/AppKitMenuSynchronizer.swift`
- **Constants**:
  - `Tag.appMenu = 100`, `Tag.fileMenu = 110`, `Tag.editMenu = 120`, `Tag.viewMenu = 130`, `Tag.windowMenu = 140`, `Tag.helpMenu = 150`
  - Submenu Tags: `1001` to `1599`
  - Action Selector Mapping: `[Selector: any LocaleKeyProtocol]`
