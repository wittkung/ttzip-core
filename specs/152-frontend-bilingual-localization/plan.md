# Implementation Plan: Full Frontend Chinese & English Bilingual Localization (全面完成前端中英完整适配)

**Branch**: `152-frontend-bilingual-localization` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/152-frontend-bilingual-localization/spec.md`

## Summary
Provide 100% comprehensive, end-to-end Chinese (Simplified & Traditional) and English bilingual (plus 7-language ready) frontend localization across TTZip. Upgrade the `L10n` core architecture to 15 strongly typed domain namespaces, establish 1:1 catalog key parity across all language packs, eliminate all hardcoded string literals and ad-hoc `isZh` ternary branches across 67+ SwiftUI views in `Sources/TTZipApp/Views/`, and implement dynamic AppKit system menu and system notification synchronization with sub-5ms zero-restart language switching.

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + SwiftUI / AppKit.
**Primary Dependencies**: Foundation, SwiftUI, AppKit, UserNotifications. (Zero third-party code generation dependencies).
**Storage**: `UserDefaults` (`TTZip_AppSelectedLanguage`) for user language preference persistence.
**Testing**: Swift Package Manager XCTest (`swift test`, `LocalizationIntegrityTests`, `GUILocalizationTests`).
**Target Platform**: macOS 14.0+ (Apple Silicon ARM64 & Intel x86_64).
**Project Type**: macOS Desktop Application (`TTZipApp`) & Core Framework (`TTZipCore`).
**Performance Goals**: Language switching latency < 10ms (< 2ms on Apple Silicon); Zero memory allocations in inner-loop string lookups.
**Constraints**: Zero view-tree rebuilds (preserve SwiftUI structural identity & active task states); 100% catalog key parity.
**Scale/Scope**: 15 L10n namespaces, ~250 localized strings per language pack across 7 languages (1,750+ total entries), 67+ SwiftUI view files refactored.

## Constitution Check

- **Architecture & Boundaries**: Conforms to Swift 6.0 + macOS 14.0+. Zero changes to frozen C bridge compression kernels.
- **Performance Floors**: In-memory static dictionary lookups guarantee O(1) string retrieval with zero heap allocation overhead on hot paths.
- **Stream-First & Bounds-First**: Dynamic formatters use strict buffer bounds and localized parameter handling.
- **Logging Discipline**: All diagnostic logs routed via `TTLogger`.
- **Quality Gates**: All 525+ tests must pass with 100% parity across string catalogs.

## Phase 0: Research & Investigation

- [x] - R001 [SUBAGENT:research] 《L10n 架构设计与动态键命名空间解耦》：如何将当前平铺的 LocaleKey 扩展为覆盖全部前端视图领域的强类型命名空间，并保持统一分发与 1:1 键一致性？ (Completed in [research.md](./research.md))
- [x] - R002 [SUBAGENT:research] 《SwiftUI 视图层无缝响应式国际化与零重置流转》：如何使 67+ 视图中的静态与动态文本在语言切换时原地重绘，避免破坏视图状态并消除所有 isZh 分支？ (Completed in [research.md](./research.md))
- [x] - R003 [SUBAGENT:research] 《系统级 AppKit 菜单栏与通知国际化同步策略》：在 macOS 原生生命周期中，如何确保 NSApplication.mainMenu 动态菜单项与 SystemNotificationManager 通知文案实时更新？ (Completed in [research.md](./research.md))

## Phase 1: Design Artifacts & Contracts

- [x] Data Model specification created: [data-model.md](./data-model.md)
- [x] Interface Contracts created in [contracts/](./contracts/):
  - [localization-state-schema.json](./contracts/localization-state-schema.json)
  - [menu-sync-schema.json](./contracts/menu-sync-schema.json)
  - [notification-event-schema.json](./contracts/notification-event-schema.json)
- [x] Validation Quickstart guide created: [quickstart.md](./quickstart.md)

## Component Breakdown & Planned Changes

### 1. TTZipCore / Localization Engine
- `Sources/TTZipCore/Localization/LocaleKey.swift`: Expand `L10n` with 15 strongly typed sub-enums (`Common`, `Sidebar`, `Explorer`, `Compress`, `Extract`, `Benchmark`, `Presets`, `Vault`, `Settings`, `Queue`, `Preview`, `Menu`, `Dialogs`, `Errors`, `Units`, `CLI`) and dynamic `allKeyGroups` reflector.
- `Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+En.swift`: Populate 100% complete English catalog.
- `Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+ZhHans.swift`: Populate 100% complete Simplified Chinese catalog.
- `Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+ZhHant.swift`: Populate 100% complete Traditional Chinese catalog.
- `Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+*.swift` (`Ja`, `De`, `Fr`, `Es`): Synchronize matching keys with elegant fallbacks.
- `Sources/TTZipCore/FinderSyncHelper.swift`: Refactor right-click contextual menus to use `L10n.Menu` keys.

### 2. TTZipApp / Services
- `Sources/TTZipApp/Services/AppLocalizationState.swift`: Add unified formatting helpers (`formatBytes`, `formatThroughput`, `formatPercent`, `plural`).
- `Sources/TTZipApp/Services/AppKitMenuSynchronizer.swift`: Implement dual-heuristic top-level & submenu selector/tag traversal.
- `Sources/TTZipApp/Services/SystemNotificationManager.swift`: Connect `UNUserNotificationCenter` to localized notification templates.
- `Sources/TTZipApp/Services/SystemDialogHelper.swift`: Localize `NSOpenPanel` prompts and add `NSAlert` localized helpers.
- `Sources/TTZipApp/TTZipApp.swift`: Bind `UNUserNotificationCenterDelegate` for foreground banner presentation.

### 3. TTZipApp / Views Refactoring (67+ Files)
- **Sidebar & Shell**: `MacEditorialSidebar.swift`, `MainView.swift`, `LiquidGlassOmnibar.swift`, `BreadcrumbPathBarView.swift`.
- **Explorer & Outline**: `ArchiveExplorerView.swift`, `NativeArchiveOutlineView.swift`, `InspectorColumnView.swift`, `DiskDirectoryBrowserView.swift`, `FinderMillerColumnsView.swift`, `MillerColumnItemRowView.swift`, `SingleMillerColumnView.swift`, `FolderCompositionPieChartView.swift`, `FolderMediaArtboardView.swift`, `HomeDropZoneView.swift`, `HomeExplorerContainerView.swift`.
- **Compress Workflow**: `CompressModalView.swift`, `CompressModalHeaderView.swift`, `CompressIntegratedConfigSectionView.swift`, `CompressTargetConfigSectionView.swift`, `CompressAdvancedOptionsSectionView.swift`, `CompressFileListView.swift`, `CompressionGuideSheetView.swift`, `CompressionProInspectorPanel.swift`, `CompressionProgressModalView.swift`, `CompressionSummarySheetView.swift`.
- **Extract Workflow**: `ExtractModalView.swift`, `PasswordPromptSheetView.swift`, `ArchiveInspectorSheet.swift`, `ArchiveIntegrityView.swift`.
- **Benchmark Center**: `BenchmarkView.swift`, `BenchmarkViewModel.swift`, `BenchmarkCompetitorPanel.swift`, `BenchmarkConfigSectionView.swift`, `BenchmarkHardwareBannerView.swift`, `BenchmarkResultRowView.swift`, `LiveBenchmarkSpeedDialView.swift`.
- **Presets & Vault**: `PresetWorkspaceView.swift`, `PresetMasterListView.swift`, `PresetEditorCardView.swift`, `PresetOptionTiles.swift`, `PasswordVaultView.swift`, `PasswordVaultAddModalSheet.swift`, `PasswordVaultEntryRowView.swift`, `PasswordVaultPopoverView.swift`.
- **Settings & Operations**: `SettingsView.swift`, `OperationsQueueView.swift`.
- **Previews**: `MediaPreviewView.swift`, `AudioWaveformVisualizerView.swift`, `CodeSyntaxPreviewView.swift`, `DocxDocumentReaderView.swift`, `EPUBBookReaderPreviewView.swift`, `InteractiveZoomImageView.swift`, `PDFDocumentPreviewView.swift`, `StreamingTextNSView.swift`, `UnifiedAudioPlayerView.swift`, `VideoAudioPlayerPreviewView.swift`.

### 4. Tests
- `Tests/TTZipTests/LocalizationIntegrityTests.swift`: Expand 5-dimension parity validation tests (100% 7-language coverage, 0 orphan keys, format specifier safety).
- `Tests/TTZipAppTests/GUILocalizationTests.swift`: Test instant language switching and UI state preservation.
