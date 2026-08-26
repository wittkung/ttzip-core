# Architecture Research: 020 Desktop Architecture Evolution & Full-Chain Quality Governance

- **Feature Directory**: `specs/020-desktop-architecture-and-quality-governance-evolution`
- **Classification**: `[Full SDD]`
- **Status**: `Approved & Implemented with RCA Enhancements`
- **Created**: 2026-08-26
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Domain-by-Domain Root Cause & Architectural Decisions (RCA)

### 1.1 Cooperative Task Cancellation & Atomic Rollbacks (Domain 1)
- **Root Cause**: Swift `Task.cancel()` does not preempt C-ABI blocking threads; cancellation signals were hitched to progress callbacks, creating black holes during 64MB solid chunk decompression (7z) or 5GB single-file unpack (ZIP); 7z unpack failure fell through into libarchive second-round unpacking; uncompleted archives were left on disk without rollback.
- **Architectural Decision**:
  - **Chunk-Level Cancellation**: In Rust Rayon chunk loops and libarchive streaming loops, check `token.is_cancelled()` per 64KB~1MB chunk.
  - **Immediate Fallback Abort**: When 7z returns `TTZipStatus::Cancelled`, immediately return `TTZipError::Cancelled` without falling through to libarchive.
  - **RAII Incomplete Output Rollback Guard**: Wrap output files in an auto-deletion guard in Swift/Rust; if an operation terminates with `.cancelled` or unhandled error, automatically delete partial archive artifacts from disk.
  - **Cancellation Invariant**: Under all multi-threaded load conditions, cancellation latency must be $\le 100\text{ms}$ with 0 disk residue.

### 1.2 Global Operations Queue & Monotonic Telemetry (Domain 2)
- **Root Cause**: Dynamic `runningTasks` calculation caused progress regressions (when a 1GB task finished, progress dropped from 95% to 10%); FSM lacked terminal state locks (calling `resume` could resurrect `.completed` tasks); `CompressFormSession` and `ExtractModalView` bypassed the global queue center.
- **Architectural Decision**:
  - **Monotonic Batch Progress Model**: Aggregate progress calculated over the active batch scope: $\text{Progress} = \frac{\sum \text{processed}(\text{active} + \text{completed})}{\sum \text{total}(\text{active} + \text{completed})}$, guaranteeing monotonic $0.0 \to 1.0$ progression.
  - **FSM Terminal State Guard**: Strict state machine where $\{\text{completed}, \text{failed}, \text{cancelled}\}$ cannot transition back to `.running` or `.paused`.
  - **Unified Dispatch Hub**: All modal sessions dispatch background operations as `QueuedArchiveOperation` to `ArchiveOperationsQueueCenter`.
  - **DockTile View Caching**: Reusable `DockProgressTileView` instance to avoid 30Hz `NSView` allocations.

### 1.3 Multi-Session Document Architecture & Tab Merging (Domain 3)
- **Root Cause**: Monolithic `AppViewState` singleton pattern; `VFSLz4CachePool.clearSession` invoked `cache.removeAllObjects()`, purging all tabs' VFS caches; `NotificationCenter` broadcasts caused password and error modals to pop up in all open windows/tabs simultaneously; `AppIntentDispatcher` trampled window references on `onAppear`.
- **Architectural Decision**:
  - **`ArchiveSessionContext` as View Driver**: Each Window/Tab holds an isolated `ArchiveSessionContext` instance driving its view hierarchy.
  - **Session-Partitioned VFS Cache**: `VFSLz4CachePool` keys formatted as `\(sessionId):\(entryPathHash)`; `clearSession(sessionId)` purges only keys matching that `sessionId`.
  - **Session-Scoped Modals**: Password prompts, error presentation sheets, and inspector panels are bound directly to `ArchiveSessionContext`, eliminating global `NotificationCenter` modal broadcasts.
  - **Key-Window Routing in `AppIntentDispatcher`**: Maintain `[UUID: ArchiveSessionContext]` registry and route external URLs/commands to `NSApp.keyWindow` or matching session.

### 1.4 Deep VFS Tree Navigation & AppKit Outline Expansion (Domain 4)
- **Root Cause**: AppKit `NSOutlineView` uses a lazy linear row table where collapsed items have `row(forItem:) == -1`, causing deep selections from search to silently fail. Naive DFS search on 100k+ entries took 15ms+ on the main thread.
- **Architectural Decision**:
  - **Recursive Ancestor Auto-Expansion**: `findAncestorChain` resolves the ancestor list `[Root, ..., Parent, Leaf]` and expands all parents top-down with synchronous animation duration `0.0`, ensuring `outlineView.row(forItem: leaf)` returns a valid row index $\ge 0$.
  - **Path-Prefix Pruning**: Optimize search from $O(N)$ to $O(D \cdot B)$ by checking `targetPath.hasPrefix(node.path + "/")`, achieving $<0.02\text{ms}$ resolution on 100,000 nodes.

### 1.5 Memory-Budgeted Media Previews with ImageIO (Domain 5)
- **Root Cause**: `NSImage(contentsOf: url)` decoded 50MP~100MP images into 120MB~390MB uncompressed RGBA bitmaps; continuous browsing pushed dirty RAM over 1.9GB; fallback path `?? NSImage(contentsOf: url)` and in-memory streams bypassed downsampling.
- **Architectural Decision**:
  - **Strict ImageIO Downsampling**: Use `CGImageSourceCreateThumbnailAtIndex` with `kCGImageSourceThumbnailMaxPixelSize = 2048` and `kCGImageSourceShouldCache = false`, decoding directly at viewport resolution in background tasks.
  - **Eliminate Unbounded Fallbacks**: Remove all `?? NSImage(contentsOf: url)` fallbacks; in-memory data previews use `ImageIOThumbnailCache.downsample(data:)`.
  - **Memory Metric CI Gate**: Enforce peak resident RAM $< 150\text{MB}$ under 50MP image batch previewing.

### 1.6 Universal Error Diagnosis & High-Fidelity Classification (Domain 6)
- **Root Cause**: `ArchiveReader.inspect` swallowed UniFFI `TTZipError.CorruptHeader` via `try?` and re-threw `passwordRequired`, falsely telling users that corrupted files were encrypted; `extractSync` diluted all errors into `readFailed(code: -1)`; `catch` blocks were used as UI resetting shortcuts.
- **Architectural Decision**:
  - **Fidelity-Preserving Error Mapping**: Prohibit `try?` at FFI boundary; map `TTZipError.CorruptHeader` strictly to `ArchiveError.corruptedData`, and only throw `.passwordRequired` when header encryption is genuinely verified.
  - **Granular Diagnostic Codes**: `AppErrorReporter` surfaces structured codes (`ERR_CORRUPT_HEADER`, `ERR_CRC_MISMATCH`, `ERR_PASSWORD_REQUIRED`, `ERR_IO_DENIED`) with technical details and copy-to-clipboard actions.
  - **Zero Silent Catches**: All catch blocks must either handle domain recovery or forward to `AppErrorReporter.shared.reportError`.

### 1.7 Engineering Hygiene, LOC Gate & Meta-Tooling (Domain 7)
- **Root Cause**: `apple/scripts/lint_loc_gate.sh` relative path resolved to `core/Sources`, producing a false-green for `apple/Sources`; `lint-contracts.sh` was an unverified echo script; SPM ignored orphaned directories not in `Package.swift`.
- **Architectural Decision**:
  - **CLI-Based LOC Gate with Baseline Assertion**: `python3 core/scripts/lint_loc_gate.py --dir <target> --min-files 10 --max-loc 800`, failing with Exit Code 2 if scanned files $< \text{min\_files}$.
  - **SPM Manifest-to-Disk Symmetry Check**: `scripts/lint_repo_hygiene.sh` parses `Package.swift` targets and asserts 0 unreferenced directories on disk.
  - **Meta-Tooling Self-Tests**: Test suite in `tests/ci/` verifying that linters reject 801 LOC files and empty scan directories.
