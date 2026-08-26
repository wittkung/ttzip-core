# Tasks: Full Frontend Chinese & English Bilingual Localization (全面完成前端中英完整适配)

**Feature Branch**: `152-frontend-bilingual-localization`
**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

---

## Phase 1: Setup & Foundational Infrastructure

**Purpose**: Core localization engine expansion, strongly-typed namespaces, and 100% catalog key parity.

- [x] T001 [P] Expand LocaleKeyProtocol and implement 15 strongly typed L10n sub-enums and reflective allKeyGroups in Sources/TTZipCore/Localization/LocaleKey.swift
- [x] T002 [P] Populate complete English catalog string dictionary in Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+En.swift
- [x] T003 [P] Populate complete Simplified Chinese catalog string dictionary in Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+ZhHans.swift
- [x] T004 [P] Populate complete Traditional Chinese catalog string dictionary in Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+ZhHant.swift
- [x] T005 [P] Synchronize matching keys for Japanese, German, French, and Spanish fallback catalogs in Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+Ja.swift, LocaleCatalog+De.swift, LocaleCatalog+Fr.swift, LocaleCatalog+Es.swift
- [x] T006 [P] Implement 5-dimension catalog parity and format specifier validation test suite in Tests/TTZipTests/LocalizationIntegrityTests.swift

**Checkpoint**: Foundation ready — all 15 L10n namespaces and 7 language catalogs compile and pass integrity tests.

---

## Phase 2: User Story 1 - Real-Time Zero-Restart Language Switching (Priority: P1) 🎯 MVP

**Goal**: Enable real-time language toggling (<10ms) across preferences, sidebar, and shell layout with zero state resets.

**Independent Test**: Switch language between English and Chinese in Settings, and assert that sidebar tabs, main toolbar, and preference panels update immediately without restarting or losing state.

- [x] T007 [P] [US1] Add reactive formatting helpers (formatBytes, formatThroughput, formatPercent, plural) in Sources/TTZipApp/Services/AppLocalizationState.swift
- [x] T008 [P] [US1] Refactor SettingsView to remove all isZh ternary branches and resolve all tabs and options via l10n.t(...) in Sources/TTZipApp/Views/SettingsView.swift
- [x] T009 [P] [US1] Refactor MacEditorialSidebar to use l10n.t(...) for all navigation tabs, headers, dates, and hardware badges in Sources/TTZipApp/Views/Sidebar/MacEditorialSidebar.swift
- [x] T010 [P] [US1] Refactor MainView, BreadcrumbPathBar, and LiquidGlassOmnibar to use l10n.t(...) in Sources/TTZipApp/Views/MainView.swift and Sources/TTZipApp/Views/Components/LiquidGlassOmnibar.swift
- [x] T011 [US1] Verify reactive language switching and state preservation in Tests/TTZipAppTests/GUILocalizationTests.swift

**Checkpoint**: User Story 1 MVP fully functional — real-time language switching is active.

---

## Phase 3: User Story 2 - Comprehensive Component String Coverage Across All Workspaces (Priority: P1)

**Goal**: Eradicate all hardcoded strings and isZh branches across all 67+ SwiftUI views in TTZipApp.

**Independent Test**: Navigate to Explorer, Compress, Extract, Benchmark, Presets, Vault, Queue, and Preview workspaces in English and Chinese, verifying 100% localized string rendering.

- [x] T012 [P] [US2] Refactor File Explorer and Miller column views to use l10n.t(...) and typed DiskItemKind in Sources/TTZipApp/Views/ArchiveExplorerView.swift, Sources/TTZipApp/Views/Explorer/FinderMillerColumnsView.swift, Sources/TTZipApp/Views/Explorer/NativeArchiveOutlineView.swift, Sources/TTZipApp/Views/Explorer/InspectorColumnView.swift, Sources/TTZipApp/Views/Explorer/MillerColumnItemRowView.swift, Sources/TTZipApp/Views/Explorer/SingleMillerColumnView.swift, Sources/TTZipApp/Views/Explorer/DiskDirectoryBrowserView.swift, Sources/TTZipApp/Views/Explorer/FolderCompositionPieChartView.swift, Sources/TTZipApp/Views/Explorer/FolderMediaArtboardView.swift, Sources/TTZipApp/Views/Explorer/HomeDropZoneView.swift, and Sources/TTZipApp/Views/Explorer/HomeExplorerContainerView.swift
- [x] T013 [P] [US2] Refactor Compress Modal and child inspector panels to use l10n.t(...) in Sources/TTZipApp/Views/CompressModalView.swift, Sources/TTZipApp/Views/Components/CompressModalHeaderView.swift, Sources/TTZipApp/Views/Components/CompressIntegratedConfigSectionView.swift, Sources/TTZipApp/Views/Components/CompressTargetConfigSectionView.swift, Sources/TTZipApp/Views/Components/CompressAdvancedOptionsSectionView.swift, Sources/TTZipApp/Views/CompressFileListView.swift, Sources/TTZipApp/Views/CompressionGuideSheetView.swift, Sources/TTZipApp/Views/CompressionProInspectorPanel.swift, Sources/TTZipApp/Views/CompressionProgressModalView.swift, and Sources/TTZipApp/Views/CompressionSummarySheetView.swift
- [x] T014 [P] [US2] Refactor Extract Modal and Password Vault views to use l10n.t(...) in Sources/TTZipApp/Views/ExtractModalView.swift, Sources/TTZipApp/Views/PasswordPromptSheetView.swift, Sources/TTZipApp/Views/PasswordVaultView.swift, Sources/TTZipApp/Views/PasswordVaultAddModalSheet.swift, Sources/TTZipApp/Views/PasswordVaultEntryRowView.swift, Sources/TTZipApp/Views/PasswordVaultPopoverView.swift, Sources/TTZipApp/Views/ArchiveInspectorSheet.swift, and Sources/TTZipApp/Views/ArchiveIntegrityView.swift
- [x] T015 [P] [US2] Refactor Benchmark dashboard views to use l10n.t(...) and l10n.format(...) in Sources/TTZipApp/Views/BenchmarkView.swift, Sources/TTZipApp/Views/Benchmark/BenchmarkViewModel.swift, Sources/TTZipApp/Views/Benchmark/BenchmarkCompetitorPanel.swift, Sources/TTZipApp/Views/Benchmark/BenchmarkConfigSectionView.swift, Sources/TTZipApp/Views/Benchmark/BenchmarkHardwareBannerView.swift, Sources/TTZipApp/Views/Benchmark/BenchmarkResultRowView.swift, and Sources/TTZipApp/Views/Benchmark/LiveBenchmarkSpeedDialView.swift
- [x] T016 [P] [US2] Refactor Presets workspace and Operations queue views to use l10n.t(...) in Sources/TTZipApp/Views/PresetWorkspaceView.swift, Sources/TTZipApp/Views/PresetMasterListView.swift, Sources/TTZipApp/Views/PresetEditorCardView.swift, Sources/TTZipApp/Views/PresetOptionTiles.swift, and Sources/TTZipApp/Views/OperationsQueueView.swift
- [x] T017 [P] [US2] Refactor Media and Document preview views to use l10n.t(...) in Sources/TTZipApp/Views/MediaPreviewView.swift, Sources/TTZipApp/Views/Preview/AudioWaveformVisualizerView.swift, Sources/TTZipApp/Views/Preview/CodeSyntaxPreviewView.swift, Sources/TTZipApp/Views/Preview/DocxDocumentReaderView.swift, Sources/TTZipApp/Views/Preview/EPUBBookReaderPreviewView.swift, Sources/TTZipApp/Views/Preview/InteractiveZoomImageView.swift, Sources/TTZipApp/Views/Preview/PDFDocumentPreviewView.swift, Sources/TTZipApp/Views/Preview/StreamingTextNSView.swift, Sources/TTZipApp/Views/Preview/UnifiedAudioPlayerView.swift, and Sources/TTZipApp/Views/Preview/VideoAudioPlayerPreviewView.swift

**Checkpoint**: User Stories 1 AND 2 complete — all 67+ SwiftUI views fully localized.

---

## Phase 4: User Story 3 - macOS Native Menu Bar & System Notification Localization (Priority: P2)

**Goal**: Complete dynamic synchronization of macOS AppKit main menu, Finder context menus, and system notifications.

**Independent Test**: Inspect macOS top-level menu bar and trigger task completion notifications to verify correct language display.

- [x] T018 [P] [US3] Overhaul AppKitMenuSynchronizer with top-level and submenu selector/tag traversal in Sources/TTZipApp/Services/AppKitMenuSynchronizer.swift
- [x] T019 [P] [US3] Refactor SystemNotificationManager to use localized templates and bind UNUserNotificationCenterDelegate in Sources/TTZipApp/Services/SystemNotificationManager.swift and Sources/TTZipApp/TTZipApp.swift
- [x] T020 [P] [US3] Refactor SystemDialogHelper to use l10n.t(...) for NSOpenPanel and NSAlert in Sources/TTZipApp/Services/SystemDialogHelper.swift
- [x] T021 [P] [US3] Refactor FinderSyncHelper to use L10n.Menu keys in Sources/TTZipCore/FinderSyncHelper.swift

**Checkpoint**: All macOS platform integration menus and notifications fully localized.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Final verification, static audits, and regression test gate.

- [x] T022 [P] Implement static hardcoded string audit test in Tests/TTZipTests/LocalizationIntegrityTests.swift
- [x] T023 Run full test suite via swift test and ensure all 530+ tests pass with zero regressions

---

## Dependencies & Execution Order

- **Setup & Foundational (Phase 1)**: Can start immediately. Blocks all User Story phases.
- **User Story 1 (Phase 2)**: Depends on Phase 1 completion.
- **User Story 2 (Phase 3)**: Depends on Phase 1 completion. Can execute in parallel with User Story 1 or sequentially.
- **User Story 3 (Phase 4)**: Depends on Phase 1 completion.
- **Polish (Phase 5)**: Depends on Phase 1-4 completion.
