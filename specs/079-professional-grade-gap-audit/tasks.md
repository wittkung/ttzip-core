# Tasks: 079-professional-grade-gap-audit

**Input**: Design documents from `/specs/079-professional-grade-gap-audit/`
**Prerequisites**: `spec.md`, `plan.md`, `research.md`, `data-model.md`, `contracts/`, `quickstart.md`

## Format: `[ID] [P?] [Story] Description`
- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g. US1, US2, US3, US4, US5)

---

## Phase 1: Setup & Core Data Models

**Purpose**: Establish domain models and contracts in Swift matching `data-model.md` and JSON schemas in `contracts/`.

- [x] T001 [P] [US1] Create `InPlaceEditSession` model and states in `Sources/TTZipCore/InPlaceEdit/InPlaceEditSession.swift` conforming to `contracts/in-place-edit-session.json`
- [x] T002 [P] [US2] Create `QuickLookPreviewPayload` model and `PreviewTreeNode` in `Sources/TTZipCore/QuickLook/QuickLookPreviewData.swift` conforming to `contracts/quicklook-preview-payload.json`
- [x] T003 [P] [US2] Create `FinderSyncActionRequest` in `Sources/TTZipCore/Services/FinderSyncActionRequest.swift` conforming to `contracts/finder-sync-action.json`
- [x] T004 [P] [US3] Create `ArchiveIntegrityReport` and `CorruptedEntryDetail` in `Sources/TTZipCore/Security/ArchiveIntegrityReport.swift` conforming to `contracts/archive-integrity-report.json`
- [x] T005 [P] [US5] Create `GlobalOperationsQueueEvent` and `QueuedArchiveOperation` in `Sources/TTZipCore/ConcurrencyPatterns/GlobalOperationsQueueModels.swift` conforming to `contracts/global-operations-queue-event.json`

---

## Phase 2: User Story 1 - 归档就地修改与外部编辑器双向同步 (Priority: P1)

**Purpose**: Implement in-place staging, parent-directory kqueue file monitoring, debounced external save detection, and transactional atomic shadow archive repack.

- [x] T006 [US1] Enhance `FileWatcherEngine.swift` with parent directory monitoring (`NOTE_WRITE`, `NOTE_EXTEND`, `NOTE_ATTRIB`, `NOTE_LINK`, `NOTE_RENAME`) and 350ms debounce window in `Sources/TTZipCore/FileWatcherEngine.swift`
- [x] T007 [US1] Implement `InPlaceArchiveMutationEngine.swift` in `Sources/TTZipCore/InPlaceEdit/InPlaceArchiveMutationEngine.swift` supporting fast ZIP central directory stream-copy and transactional shadow file swap (`renamex_np` / `replaceItemAtURL`)
- [x] T008 [US1] Integrate external editor launching (`NSWorkspace.shared.open`) and live edit session tracking into `ArchiveExplorerView.swift` and `ArchiveTreeStore.swift` in `Sources/TTZipApp/`
- [x] T009 [US1] Implement drag-and-drop into archive tree and keyboard Delete key in-place removal in `ArchiveExplorerView.swift`
- [x] T010 [P] [US1] Create unit and integration test suite `InPlaceArchiveEditSyncTests.swift` in `Tests/TTZipTests/InPlaceArchiveEditSyncTests.swift`

---

## Phase 3: User Story 2 - macOS Quick Look 预览与 Finder 深度集成 (Priority: P1)

**Purpose**: Deliver sub-50ms QuickLook HTML5 data preview provider and Finder Sync context menu integration.

- [x] T011 [US2] Harden `QuickLookPreviewEngine.swift` in `Sources/TTZipCore/QuickLook/QuickLookPreviewEngine.swift` with streaming header inspection and dark/light adaptive standalone HTML5 rendering
- [x] T012 [US2] Implement `TTZipQuickLookProvider` conforming to `QLPreviewProvider` for `com.apple.quicklook.preview` extension target
- [x] T013 [US2] Implement `TTZipFinderSyncController` bridging `FinderSyncHelper` to `FIFinderSync` for `com.apple.FinderSync` context menu extension target
- [x] T014 [US2] Update `Info.plist` and entitlements for all 16 supported archive format UTIs and default handler registration
- [x] T015 [P] [US2] Create integration test suite `QuickLookPreviewEngineTests.swift` in `Tests/TTZipTests/QuickLookPreviewEngineTests.swift`

---

## Phase 4: User Story 3 - 归档无盘内存完整性体检与损坏应急修复 (Priority: P2)

**Purpose**: Deliver zero-disk I/O multi-core stream decoding verification and disaster repair console.

- [x] T016 [US3] Upgrade `ArchiveIntegrityChecker.swift` in `Sources/TTZipCore/ArchiveIntegrityChecker.swift` to support pure memory stream-discarding verification across all 16 formats with CRC32/SHA-256 validation
- [x] T017 [US3] Enhance `ArchiveRepairEngine.swift` in `Sources/TTZipCore/ArchiveRepairEngine.swift` with truncated stream salvage and damaged block bypass
- [x] T018 [US3] Create `ArchiveIntegrityView.swift` and `ArchiveIntegrityViewModel.swift` in `Sources/TTZipApp/Views/` with visual diagnostics and failure entry breakdown
- [x] T019 [P] [US3] Create unit and artificial corruption test suite `ArchiveIntegrityCheckerTests.swift` in `Tests/TTZipTests/ArchiveIntegrityCheckerTests.swift`

---

## Phase 5: User Story 4 - 跨平台纯净文件清洗与 macOS 元数据控制 (Priority: P2)

**Purpose**: Deliver single-pass zero-allocation sanitization (`.DS_Store`, `__MACOSX`, `._*`) and Unicode NFC normalization.

- [x] T020 [US4] Add Unicode NFC precomposition normalization and `COPYFILE_DISABLE` suppression in `Sources/TTZipCore/Zip/ZipDirectoryScanner.swift` and `ArchiveWriter.swift`
- [x] T021 [US4] Add `SanitizationProfile` preset switcher in `Sources/TTZipCore/ArchiveFilterOptions.swift` (`.crossPlatformClean` vs `.macOSHighFidelity`)
- [x] T022 [US4] Expose "Clean for Windows/Linux (.DS_Store & __MACOSX Stripped)" toggle in `CompressModalView.swift` and `PresetEditorCardView.swift` in `Sources/TTZipApp/`
- [x] T023 [P] [US4] Create cross-platform sanitization test suite `ArchiveSanitizationFilterTests.swift` in `Tests/TTZipTests/ArchiveSanitizationFilterTests.swift`

---

## Phase 6: User Story 5 - 全局后台多任务调度队列与 Dock 进度中枢 (Priority: P3)

**Purpose**: Implement Swift 6 actor-isolated task queue with dynamic concurrency throttling (1~8), Dock progress ring, and system notifications.

- [x] T024 [US5] Implement `GlobalOperationsQueue.swift` actor in `Sources/TTZipCore/ConcurrencyPatterns/GlobalOperationsQueue.swift` with priority scheduling and cooperative cancellation
- [x] T025 [US5] Implement `DockProgressManager.swift` in `Sources/TTZipApp/Services/DockProgressManager.swift` with 30~60Hz throttled custom `NSDockTile` drawing and badge tracking
- [x] T026 [US5] Implement `SystemNotificationManager.swift` in `Sources/TTZipApp/Services/SystemNotificationManager.swift` using `UNUserNotificationCenter`
- [x] T027 [US5] Implement `OperationsQueueWindow.swift` and `OperationsQueueViewModel.swift` in `Sources/TTZipApp/Views/` with live throughput telemetry and pause/resume controls
- [x] T028 [P] [US5] Create multi-task queue test suite `GlobalOperationsQueueTests.swift` in `Tests/TTZipTests/GlobalOperationsQueueTests.swift`

---

## Phase 7: Verification & Convergence

**Purpose**: Full-suite regression and schema consistency analysis.

- [x] T029 Execute full test suite `swift test` across all 525+ tests and new test suites
- [x] T030 Execute performance gate `swift test --filter XCTestPerformanceMeasureTests` ensuring zero throughput regression
