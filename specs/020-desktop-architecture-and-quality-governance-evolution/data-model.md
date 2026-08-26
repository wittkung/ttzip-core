# Data Models & State Architecture: 020 Desktop Architecture Evolution & Quality Governance

- **Feature Directory**: `specs/020-desktop-architecture-and-quality-governance-evolution`
- **Classification**: `[Full SDD]`
- **Status**: `Specified`
- **Created**: 2026-08-26
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Domain Entities & State Structures

### 1.1 `ArchiveSessionContext` (Per-Window / Per-Tab State)
```swift
@Observable
@MainActor
public final class ArchiveSessionContext: Identifiable {
    public let id: UUID
    public var windowTitle: String
    public var currentArchivePath: String?
    public var currentDirectory: URL
    public var activePassword: String?
    public var currentEntries: [ArchiveEntry]
    public var selectedEntryID: String?
    public var activePreviewFileURL: URL?
    public var activePreviewFileName: String?
    public var searchQuery: String
    public var isBuildingTree: Bool
    public var rootNodes: [ArchiveTreeNode]
    public var filteredEntries: [ArchiveEntry]
    public let vfsCacheSessionId: String
    
    public init(id: UUID = UUID(), initialURL: URL? = nil) { ... }
}
```

### 1.2 `QueuedArchiveOperation` & `ArchiveTaskHandle`
```swift
public struct QueuedArchiveOperation: Identifiable, Sendable {
    public let id: UUID
    public let name: String
    public let operationType: ArchiveOperationType
    public var state: ArchiveTaskExecutionState
    public var bytesProcessed: Int64
    public var totalBytes: Int64
    public var currentEntryName: String
    public var throughputMBs: Double
    public var elapsedSeconds: Double
    public var errorMessage: String?
    public let createdAt: Date
    public let handle: TaskExecutionHandle
}

public enum ArchiveTaskExecutionState: String, Sendable, Codable {
    case queued
    case running
    case paused
    case completed
    case failed
    case cancelled
}
```

### 1.3 `AppErrorPayload` & `AppErrorReporter`
```swift
public struct AppErrorPayload: Identifiable, Sendable {
    public let id: UUID
    public let title: String
    public let message: String
    public let diagnosticCode: String
    public let technicalDetails: String?
    public let recoveryActionTitle: String?
    public let recoveryHandler: (@Sendable () -> Void)?
    
    public init(
        title: String,
        message: String,
        diagnosticCode: String,
        technicalDetails: String? = nil,
        recoveryActionTitle: String? = nil,
        recoveryHandler: (@Sendable () -> Void)? = nil
    ) { ... }
}
```

---

## 2. State Transition Diagrams

### 2.1 Task Execution & Cooperative Cancellation Lifecycle

```
                    ┌─────────────────────────┐
                    │         Queued          │
                    └────────────┬────────────┘
                                 │
                                 ▼ (Engine Dispatched)
                    ┌─────────────────────────┐
         ┌─────────►│         Running         │◄────────┐
         │          └──────┬───────────┬──────┘         │
(Resume) │                 │           │                │ (Pause)
         │                 │           ▼                │
         │                 │    ┌─────────────┐         │
         └─────────────────┼────┤   Paused    ├─────────┘
                           │    └─────────────┘
                           │
       ┌───────────────────┼───────────────────┐
       ▼ (Complete)        ▼ (Error)           ▼ (Cancel Signal)
┌──────────────┐   ┌──────────────┐   ┌────────────────────────┐
│  Completed   │   │    Failed    │   │       Cancelled        │
│ (Reveal Dst) │   │ (AppError)   │   │ (Rollback Partial Tmp) │
└──────────────┘   └──────────────┘   └────────────────────────┘
```

### 2.2 Multi-Session Window / Tab Lifecycle

```
Finder / CLI / URL Trigger
          │
          ▼
AppIntentDispatcher (@MainActor)
          │
          ├──> Focus existing Session (if URL match)
          │
          └──> Instantiate new `ArchiveSessionContext`
                    │
                    ▼
          `NSWindow` (tabbingMode = .preferred)
                    │
                    ├── Render `ArchiveExplorerView(session)`
                    ├── Bind Isolated VFS LZ4 Cache Pool
                    └── On Window Close ──> Clear Session Cache & Temp Files
```
