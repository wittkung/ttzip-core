# Data Model: 073-media-preview-audit-tui-engine-acceleration

## Entities & Type System

### 1. `TUISessionState` (Terminal User Interface Explorer State)

Represents the mutable state of an active interactive terminal session in `ttzip-cli explore`.

```swift
public struct TUISessionState: Sendable, Equatable {
    public var archivePath: String
    public var currentDirectoryPath: String
    public var cursorIndex: Int
    public var scrollOffset: Int
    public var expandedPaths: Set<String>
    public var selectedPaths: Set<String>
    public var visibleRows: [TUIVisibleRow]
    public var isPeeking: Bool
    public var peekContent: TUIPeekContent?
    public var flashMessage: String?
    public var isExiting: Bool
    public var terminalRows: Int
    public var terminalCols: Int
}
```

### 2. `TUIVisibleRow` (Virtualized Viewport Row Item)

Represents a single visible row in the interactive terminal directory tree.

```swift
public struct TUIVisibleRow: Sendable, Equatable, Identifiable {
    public var id: String { path }
    public let name: String
    public let path: String
    public let isDirectory: Bool
    public let depth: Int
    public let isExpanded: Bool
    public let sizeBytes: Int64
    public let formattedSize: String
    public var isSelected: Bool
    public let indentationPrefix: String
    public let iconEmoji: String
}
```

### 3. `TUIPeekContent` (In-Terminal Quick Peek Content)

Encapsulates formatted text lines or binary hex dumps for the in-terminal preview overlay (`p`).

```swift
public struct TUIPeekContent: Sendable, Equatable {
    public let filePath: String
    public let mimeType: String
    public let uncompressedSize: Int64
    public let formattedSize: String
    public let lines: [String]
    public let hexDump: String?
    public let metadata: [String: String]
    public let isTruncated: Bool
}
```

### 4. `MediaDownsampleConfig` (CoreGraphics Thumbnail Extraction Configuration)

Controls zero-allocation downsampling parameters for high-resolution images in `TTZipApp`.

```swift
public struct MediaDownsampleConfig: Sendable, Equatable {
    public let maxPixelDimension: Int
    public let shouldCacheImmediately: Bool
    public let honorExifTransform: Bool

    public static let standardPreview = MediaDownsampleConfig(
        maxPixelDimension: 2048,
        shouldCacheImmediately: true,
        honorExifTransform: true
    )
}
```

### 5. `MediaAuditReport` (Diagnostic and Memory Audit Record)

Records memory footprint and lifecycle audit assertions across media preview components.

```swift
public struct MediaAuditReport: Sendable, Codable, Equatable {
    public let rawImageByteSize: Int64
    public let downsampledImageByteSize: Int64
    public let memoryReductionPercentage: Double
    public let isAVPlayerCleanlyTornDown: Bool
    public let isTimeObserverRemoved: Bool
    public let isCoreAudioHALReleased: Bool
    public let virtualDragPromiseSupported: Bool
    public let spacebarQuickLookLatencyMs: Double
}
```
