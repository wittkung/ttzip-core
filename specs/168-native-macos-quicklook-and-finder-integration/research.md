# Research Findings: macOS 原生 Quick Look 与 Finder 拖拽集成 (Feature 168)

## R001 [SUBAGENT:research]: macOS QLPreviewController & SwiftUI .quickLookPreview Binding Architecture

- **Decision**:
  Adopt SwiftUI native `.quickLookPreview($selectedPreviewURL)` in `HomeExplorerContainerView` and `ArchiveExplorerView`. Integrate local key monitoring (`NSEvent.addLocalMonitorForEvents(matching: .keyDown)` on `keyCode == 49` for Space bar):
  1. For disk files: Set `$selectedPreviewURL = URL(fileURLWithPath: diskItem.path)`.
  2. For in-archive virtual items: Asynchronously extract the entry payload via `ArchiveSelectiveExtractor.shared.extractSingleEntryData`, stage the bytes atomically via `EphemeralPreviewCacheManager`, and set `$selectedPreviewURL = stagedURL`.
  3. If `$selectedPreviewURL` is already non-nil for current selection, pressing Space toggles it to `nil` to dismiss preview (matching macOS Finder behavior).
- **Rationale**:
  - SwiftUI's `.quickLookPreview(Binding<URL?>)` (macOS 11+) delegates window layering, focus management, dismissal animations, and keyboard navigation (Esc, Space, Arrow keys) to the OS Quick Look framework.
  - `ArchiveSelectiveExtractor.shared.extractSingleEntryData` leverages memory-mapped ZIP Central Directory random seek tables (`ZipSeekTable`), reading single entries in sub-millisecond time without decompressing full archives.
- **Source**:
  - `Sources/TTZipApp/Views/Explorer/HomeExplorerContainerView.swift:84-97`
  - `Sources/TTZipCore/ArchiveSelectiveExtractor.swift:99-144`

---

## R002 [SUBAGENT:research]: NSItemProvider & File Promise Provider for Finder Drag-and-Drop

- **Decision**:
  Implement a dual-tier drag-and-drop provider mechanism:
  1. **Direct Disk Items**: Return `NSItemProvider(object: fileURL as NSURL)`.
  2. **In-Archive Virtual Entries**: Implement `ArchiveFilePromiseProvider` using `NSFilePromiseProvider` and `NSFilePromiseProviderDelegate`:
     - Provide filename and UTI via `filePromiseProvider(_:fileNameForType:)`.
     - In `filePromiseProvider(_:writePromiseTo:completionHandler:)`, perform lazy extraction directly into the Finder destination URL via `ArchiveSelectiveExtractor.shared.extractSingleEntryData`.
  3. **Midway Error Handling**: If extraction fails during drop, pass the error to `completionHandler(error)`. Finder aborts the transfer and purges uncommitted temporary files without corrupting target directory.
- **Rationale**: Virtual entries do not exist on disk prior to a drop. Eagerly extracting files during drag start blocks the UI thread. `NSFilePromiseProvider` defers decompression I/O until the exact moment Finder acknowledges a valid drop target.
- **Source**:
  - `Sources/TTZipApp/Views/Explorer/MillerColumnItemRowView.swift:138-196`
  - `Sources/TTZipCore/ArchiveSelectiveExtractor.swift:21-96`

---

## R003 [SUBAGENT:research]: Ephemeral Preview Cache Lifecycle & Security Guard

- **Decision**:
  Implement `EphemeralPreviewCacheManager` as a Swift `actor`:
  1. **Isolated Sandbox Path**: Root directory `FileManager.default.temporaryDirectory.appendingPathComponent("ttzip_previews_\(UUID().uuidString)", isDirectory: true)` with POSIX `0o700` (`S_IRWXU`) and file permissions `0o600` (`S_IRUSR | S_IWUSR`).
  2. **Atomic File Staging**: Write uncompressed data to temporary sibling file with `O_CREAT | O_EXCL`, followed by atomic POSIX `rename(2)` to destination path.
  3. **Multi-Tier Cleanup Guard**:
     - *Tier 1 (Normal Termination)*: Observes `NSApplication.willTerminateNotification` to purge preview directory immediately.
     - *Tier 2 (Process Exit Fallback)*: Registers a C `atexit` callback for normal process exit.
     - *Tier 3 (Startup Garbage Collection)*: Scans temporary directory on launch for stale `ttzip_previews_*` directories older than 24 hours and removes them.
     - *Tier 4 (In-Session LRU Quota)*: Max quota 500 MB / 100 items, evicting least-recently-accessed items when exceeded.
- **Rationale**: Temporary files from sensitive archives must not be left unencrypted in world-readable temp directories. Atomic rename guarantees file integrity when external `quicklookd` accesses the file.
- **Source**:
  - `Sources/TTZipApp/Services/PreviewLRUCacheManager.swift:11-122`
  - `Sources/TTZipCore/Utilities/TempDirectoryCleanUpManager.swift:10-58`
