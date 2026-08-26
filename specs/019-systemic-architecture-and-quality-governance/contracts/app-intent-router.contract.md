# Contract: App Intent & Multi-Entrypoint Routing Architecture

- **Specification**: `specs/019-systemic-architecture-and-quality-governance`
- **Domain**: IPC, URL Scheme, AppKit Menu, Drag & Drop, Context Menu
- **Language Mode**: Swift 6 Strict Concurrency (`@MainActor`, `Sendable`)

---

## 1. Type-Safe Intent Specifications

```swift
public enum AppIntentSource: String, Sendable, Codable {
    case finderSync = "finder_sync"
    case urlScheme = "url_scheme"
    case appKitMenu = "appkit_menu"
    case dragAndDrop = "drag_and_drop"
    case contextMenu = "context_menu"
    case toolbar = "toolbar"
    case internalNavigation = "internal_navigation"
}

public struct CompressIntentOptions: Sendable, Codable, Equatable {
    public let presetID: UUID?
    public let targetFormat: ArchiveCompressionFormat?
    public let separateArchives: Bool
    public let deleteSourceAfterCompression: Bool
    public let customOutputPath: String?
    public let password: String?
}

public struct ExtractIntentOptions: Sendable, Codable, Equatable {
    public let destinationDirectory: URL?
    public let isSmartExtract: Bool
    public let deleteSourceAfterExtraction: Bool
    public let password: String?
    public let targetEntrySubpaths: [String]?
}

public enum AppIntent: Sendable, Equatable {
    case openArchive(url: URL, password: String?)
    case navigateToDirectory(url: URL)
    case switchTab(tab: WorkspaceTab)
    case pickAndOpenArchive
    case createArchive(sourcePaths: [String], options: CompressIntentOptions)
    case extractArchive(archivePaths: [String], options: ExtractIntentOptions)
    case inspectArchive(archivePath: String)
    case verifyIntegrity(archivePath: String)
    case promptPassword(archivePath: String)
    case autofillVaultPassword(archivePath: String)
    case addFilesToArchive(archivePath: String, sourcePaths: [String], destinationSubfolder: String?)
    case deleteArchiveEntries(archivePath: String, entryPaths: [String])
    case previewItem(url: URL)
    case revealInFinder(url: URL)
}
```

---

## 2. Ingestion & Parsing Contract (`AppIntentParser`)

```swift
public enum AppIntentParser {
    /// Converts incoming URL into an AppIntentEnvelope.
    public static func parse(url: URL, sourceHint: AppIntentSource = .urlScheme) -> AppIntentEnvelope?
    
    /// Safely extracts POSIX file paths from NSItemProvider without type-cast failure traps.
    public static func extractPaths(from providers: [NSItemProvider]) async -> [String]
}
```

---

## 3. Invariant Matrix

1. **No Silent Drops**: Every incoming `AppIntentEnvelope` MUST produce a deterministic state change or explicit error alert. No action type may fall into `default: break` without handling.
2. **Path Sanitization**: Every file path MUST be expanded (tilde, symlinks), stripped of quotes/newlines, and existence-checked (`FileManager.fileExists`) before dispatching.
3. **Thread Concurrency Safety**: Dispatch operations execute exclusively on `@MainActor` without blocking the main run loop.
