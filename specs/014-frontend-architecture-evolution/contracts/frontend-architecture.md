# Interface Contracts: Frontend Architecture & Paradigm Evolution

- **Feature**: `014-frontend-architecture-evolution`
- **Created**: 2026-08-25
- **Status**: Completed

---

## 1. State Boundary Contracts

### 1.1 Observation Granularity Invariant
All views consuming `AppViewState` or sub-states MUST access properties directly without Combine publishers or manual `objectWillChange` triggers.

```swift
// Valid
struct TaskStatusBadge: View {
    let taskState: TaskExecutionState
    var body: some View {
        Text(taskState.statusMessage) // Only invalidates when statusMessage changes
    }
}

// Prohibited
// No .onReceive(taskState.objectWillChange) or appState.objectWillChange.send()
```

---

## 2. I/O Actor & Storage Contracts

### 2.1 `DiskDirectoryScannerActor`
```swift
public actor DiskDirectoryScannerActor {
    public static let shared: DiskDirectoryScannerActor
    public func scanDirectory(at dirURL: URL) async -> [DiskItemInfo]
}
```
- **Contract**: Executes scanning via `URL.resourceValues` in a single bulk system call. Guarantees zero blocking on the `@MainActor`.

### 2.2 `ArchiveHierarchySessionCache`
```swift
public actor ArchiveHierarchySessionCache {
    public static let shared: ArchiveHierarchySessionCache
    public func getOrFetchSession(
        for archivePath: String,
        password: String?,
        autoVaultUnlock: Bool
    ) async throws -> ArchiveHierarchySession
    public func invalidate(path: String)
    public func clearAll()
}
```
- **Contract**: Re-uses existing in-memory composite tree if `(fileByteSize, modificationTimestamp)` matches. Guarantees $O(1)$ child subpath retrieval.

---

## 3. Rendering & Preview Contracts

### 3.1 `BackgroundSyntaxTokenizer`
```swift
public actor BackgroundSyntaxTokenizer {
    public static let shared: BackgroundSyntaxTokenizer
    public func tokenize(text: String, ext: String, targetRange: NSRange) -> [TokenSpan]
}
```
- **Contract**: Executes off-main-thread regex tokenization against precompiled rules without blocking NSTextStorage or main run loop.

### 3.2 `ImageIOThumbnailService`
```swift
public actor ImageIOThumbnailService {
    public static let shared: ImageIOThumbnailService
    public func getThumbnail(for url: URL, maxPixelSize: CGFloat) async -> CGImage?
    public func purgeCache()
}
```
- **Contract**: Executes CoreGraphics downsampling on detached cooperative tasks with in-flight deduplication.
