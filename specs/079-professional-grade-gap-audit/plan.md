# Implementation Plan: 079-professional-grade-gap-audit

**Feature**: Comprehensive Professional Software Gap Audit & Architecture Plan
**Branch**: `079-professional-grade-gap-audit`
**Status**: Ready for Tasks (`@speckit-tasks`)

---

## Technical Context

TTZip has established unmatched raw performance across 16 compression formats through in-process C static bindings, Apple Silicon ARM64 NEON SIMD acceleration, Fast LZMA2, PMULL CRC64, and libdeflate. However, an exhaustive audit against top-tier macOS and desktop archiving suites (BetterZip, Keka, Bandizip, WinRAR, 7-Zip) reveals 5 major feature and UX gaps:
1. **In-place archive modification & live external editor sync**: Lack of live sync when users open and edit files in external tools like VS Code or TextEdit.
2. **macOS deep system integration**: Lack of a native QuickLook preview extension (`.appex`) for instant Spacebar preview in Finder, and lack of Finder Sync / Services context menu integration.
3. **In-memory archive integrity diagnostics & repair**: Lack of a dedicated zero-disk I/O "Test Archive" verification mode.
4. **Cross-platform clean sanitization**: Need for explicit UI profile toggles to eliminate macOS-specific `.DS_Store` and `__MACOSX` junk on Windows/Linux targets.
5. **Global background operations queue**: Need for a unified multi-task queue with concurrency throttling, Dock progress ring, and system notifications.

This plan details the technical architecture and component changes to systematically close all 5 gaps.

---

## Constitution Check

| Rule / Invariant | Status | Compliance Verification |
| :--- | :--- | :--- |
| **Hot-Path Zero-Cost Abstraction** | ✅ Compliant | Fast-path directory scanning and in-stream byte filtering use pointer arithmetic without intermediate heap allocation. |
| **Zero Shared Locks in Concurrency** | ✅ Compliant | Concurrency throttling is managed by a Swift 6 isolated Actor (`GlobalOperationsQueue`) with cooperative TaskGroups; zero mutexes inside hot parallel loops. |
| **Stream-First & Zero-Memory Invariant** | ✅ Compliant | Integrity checker uses a stream-discarding sink without allocating full-archive buffers in RAM or writing to disk. |
| **Durability & Invariant-First Security** | ✅ Compliant | In-place archive modifications execute in transactional shadow files with $O(1)$ `renamex_np` atomic swapping, preventing corruptions on abnormal termination. |
| **Code Freeze Discipline** | ✅ Compliant | All frozen ZIP core files (`ZipParallelExtractor.swift`, `ZipParallelWriter.swift`, `CTTZipExtract.c`, etc.) remain 100% untouched. |

---

## Phase 0: Research Index

- R001 [SUBAGENT:research] 《归档就地修改与外部编辑器双向同步架构》：基于父目录 `DispatchSourceFileSystemObject` + 350ms 防抖 + 事务性 Shadow 换名回写，彻底免疫 Inode 解除与原子安全保存脱轨问题。（见 [`research.md#r001`](research.md#r001)）
- R002 [SUBAGENT:research] 《macOS Quick Look 原生预览与 Finder 深度集成架构》：基于纯数据 `QLPreviewProvider` 输出轻量 HTML5 预览，毫秒级响应（<= 25ms），零 UI 泄漏；FinderSync 扩展提供 $O(1)$ 模式匹配上下文动作。（见 [`research.md#r002`](research.md#r002)）
- R003 [SUBAGENT:research] 《跨平台文件清洗与 macOS 高保真元数据管线》：在单趟遍历中以零开销过滤 `.DS_Store` / `__MACOSX` / `._*`，设置 `COPYFILE_DISABLE=1` 并强制 Unicode NFC 规范化；高保真模式下通过 PAX 与 `copyfile(3)` 完整还原 xattr 与 ACL。（见 [`research.md#r003`](research.md#r003)）
- R004 [SUBAGENT:research] 《Swift Concurrency 全局多任务调度队列与 Dock 进度中枢》：基于 Swift 6 Actor 实现 1~8 动态并发限制与优先级调度，AppKit 端 30~60Hz 节流驱动 `NSApp.dockTile` 进度环与后台通知。（见 [`research.md#r004`](research.md#r004)）

---

## Phase 1: Design Artifacts Index

- **Data Models**: [`data-model.md`](data-model.md)
- **Zero Bare-Object Contracts**:
  - [`contracts/in-place-edit-session.json`](contracts/in-place-edit-session.json)
  - [`contracts/quicklook-preview-payload.json`](contracts/quicklook-preview-payload.json)
  - [`contracts/finder-sync-action.json`](contracts/finder-sync-action.json)
  - [`contracts/archive-integrity-report.json`](contracts/archive-integrity-report.json)
  - [`contracts/global-operations-queue-event.json`](contracts/global-operations-queue-event.json)
- **Validation Guide**: [`quickstart.md`](quickstart.md)

---

## Component Changes & Architecture Breakdown

### 1. `Sources/TTZipCore/` (Core Engines & Infrastructure)

- **[NEW] `Sources/TTZipCore/InPlaceEdit/InPlaceArchiveMutationEngine.swift`**:
  - Manages live staging, directory-level kqueue file watching, debounced hash checks, and atomic shadow repack for ZIP, 7Z, and TAR archives.
- **[MODIFY] `Sources/TTZipCore/FileWatcherEngine.swift`**:
  - Enhance directory-level monitoring with `NOTE_WRITE | NOTE_EXTEND | NOTE_ATTRIB | NOTE_LINK | NOTE_RENAME`, debounce handling, and multi-save session lifecycle.
- **[NEW] `Sources/TTZipCore/ConcurrencyPatterns/GlobalOperationsQueue.swift`**:
  - Swift 6 actor-isolated task scheduler with dynamic `maxConcurrentOperations` (1~8), priority queues, cooperative cancellation, and `AsyncStream` progress feeds.
- **[MODIFY] `Sources/TTZipCore/ArchiveIntegrityChecker.swift`**:
  - Add non-destructive, zero-disk-write stream verification mode returning detailed `ArchiveIntegrityReport`.
- **[MODIFY] `Sources/TTZipCore/ArchiveFilterOptions.swift` & `PathPatternFilterEngine.swift`**:
  - Add Unicode NFC normalization hook and formalize `SanitizationProfile` (`.crossPlatformClean` vs `.macOSHighFidelity`).

### 2. `Sources/TTZipApp/` (UI, ViewModels & macOS System Integrations)

- **[MODIFY] `Sources/TTZipApp/Views/ArchiveExplorerView.swift` & `ArchiveTreeStore.swift`**:
  - Add double-click to open in external editor, in-place edit status banners, drag-and-drop into archive support, and Delete key item removal.
- **[NEW] `Sources/TTZipApp/Services/DockProgressManager.swift`**:
  - Throttled 30Hz~60Hz `NSDockTile` custom progress ring drawing and badge updates.
- **[NEW] `Sources/TTZipApp/Services/SystemNotificationManager.swift`**:
  - Background `UNUserNotificationCenter` dispatch with action buttons.
- **[NEW] `Sources/TTZipApp/Views/OperationsQueueWindow.swift` & `OperationsQueueViewModel.swift`**:
  - Dedicated floating window / drawer displaying active, paused, queued, and completed archiving jobs with pause/resume controls.
- **[NEW] `Sources/TTZipApp/Views/ArchiveIntegrityView.swift`**:
  - Dedicated UI panel for running "Test Archive" operations and displaying formatted corruption diagnostics.

### 3. Extensions & System Targets

- **`TTZipQuickLookExtension`**:
  - Lightweight data-based `QLPreviewProvider` serving responsive HTML5 preview via `QuickLookPreviewEngine`.
- **`TTZipFinderSyncExtension`**:
  - `FIFinderSync` extension hooking into Finder context menus via `FinderSyncHelper`.

### 4. `Tests/TTZipTests/` (Verification Suites)

- **[NEW] `Tests/TTZipTests/InPlaceArchiveEditSyncTests.swift`**
- **[NEW] `Tests/TTZipTests/CrossPlatformSanitizationTests.swift`**
- **[NEW] `Tests/TTZipTests/ArchiveIntegrityCheckerTests.swift`**
- **[NEW] `Tests/TTZipTests/GlobalOperationsQueueTests.swift`**
