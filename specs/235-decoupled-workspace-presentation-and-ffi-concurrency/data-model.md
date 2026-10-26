# Data Model: 235 - Decoupled Workspace Presentation, Sandbox RAII Leases, and FFI Concurrency Hardening

> **Target Version**: TTWorkspace v6.3  
> **Status**: Accepted Data Model Specification  

---

## 1. Domain Entities & Type Definitions

### 1.1 Multi-Tab Workspace Session
```swift
// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

public struct WorkspaceTabSession: Identifiable, Sendable {
    public let id: UUID
    public var kind: TabKind
    public var historyStack: WorkspaceHistoryStack
    public var viewState: TabViewState

    public enum TabKind: Equatable, Sendable {
        case fileSystem(URL)
        case archive(archiveURL: URL, innerPath: String)
        case device(deviceID: String, devicePath: String)
        case storageInspector(targetURL: URL)
    }

    public init(id: UUID = UUID(), kind: TabKind, historyStack: WorkspaceHistoryStack = .empty(), viewState: TabViewState = .init()) {
        self.id = id
        self.kind = kind
        self.historyStack = historyStack
        self.viewState = viewState
    }
}

public struct WorkspaceHistoryStack: Equatable, Sendable {
    public private(set) var backStack: [NavigationSnapshot]
    public private(set) var current: NavigationSnapshot
    public private(set) var forwardStack: [NavigationSnapshot]

    public struct NavigationSnapshot: Equatable, Sendable {
        public let kind: WorkspaceTabSession.TabKind
        public let timestamp: TimeInterval
    }

    public static func empty(root: WorkspaceTabSession.TabKind = .fileSystem(FileManager.default.homeDirectoryForCurrentUser)) -> Self {
        Self(backStack: [], current: NavigationSnapshot(kind: root, timestamp: Date().timeIntervalSince1970), forwardStack: [])
    }

    public mutating func push(_ newKind: WorkspaceTabSession.TabKind) {
        backStack.append(current)
        current = NavigationSnapshot(kind: newKind, timestamp: Date().timeIntervalSince1970)
        forwardStack.removeAll(keepingCapacity: true)
    }

    public mutating func goBack() -> WorkspaceTabSession.TabKind? {
        guard let previous = backStack.popLast() else { return nil }
        forwardStack.append(current)
        current = previous
        return current.kind
    }

    public mutating func goForward() -> WorkspaceTabSession.TabKind? {
        guard let next = forwardStack.popLast() else { return nil }
        backStack.append(current)
        current = next
        return current.kind
    }
}

public struct TabViewState: Equatable, Sendable {
    public var millerColumnPaths: [URL] = []
    public var selectedPaths: [URL] = []
    public var scrollTargetURL: URL? = nil
    public var searchQuery: String = ""
}
```

---

### 1.2 Presentation Coordinator & Modal Hierarchy
```swift
// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import Observation

public enum ActiveModalPresentation: Equatable, Sendable {
    case none
    // Priority Tier 1: Fluid Overlays
    case globalSearch
    case omnibarQuickPeeker(URL)
    // Priority Tier 2: Standard Sheets
    case compressConfig(sources: [URL])
    case archiveInspector(URL)
    case preferences
    // Priority Tier 3: Critical Alerts
    case passwordPrompt(context: PasswordContext)
    case fatalRecoveryAlert(title: String, message: String)

    public var priority: Int {
        switch self {
        case .none: return 0
        case .globalSearch, .omnibarQuickPeeker: return 1
        case .compressConfig, .archiveInspector, .preferences: return 2
        case .passwordPrompt, .fatalRecoveryAlert: return 3
        }
    }
}

public struct PasswordContext: Equatable, Sendable {
    public let archiveURL: URL
    public let attemptCount: Int
}

@MainActor
@Observable
public final class PresentationCoordinator {
    public static let shared = PresentationCoordinator()

    public private(set) var activeModal: ActiveModalPresentation = .none

    public func present(_ modal: ActiveModalPresentation) {
        guard modal.priority >= activeModal.priority else { return }
        self.activeModal = modal
    }

    public func dismiss() {
        self.activeModal = .none
    }

    public var isModalActive: Bool {
        activeModal != .none
    }
}
```

---

### 1.3 Sandbox Security Scope Lease
```swift
// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import os

public final class SecurityScopeLease: @unchecked Sendable {
    public let url: URL
    private let onRelease: @Sendable (URL) -> Void
    private let isReleased = OSAllocatedUnfairLock(initialState: false)

    public init(url: URL, onRelease: @escaping @Sendable (URL) -> Void) {
        self.url = url
        self.onRelease = onRelease
    }

    public func release() {
        let shouldRelease = isReleased.withLock { released -> Bool in
            guard !released else { return false }
            released = true
            return true
        }
        if shouldRelease {
            onRelease(url)
        }
    }

    deinit {
        release()
    }
}
```

---

### 1.4 Right Inspector Surface & Selection Aggregation
```swift
// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

public enum InspectorSurfaceMode: Equatable, Sendable {
    case empty
    case single(entry: FileSystemEntryMetadata)
    case multiple(metrics: AggregatedSelectionMetrics)
    case storageInspectorActive
}

public struct FileSystemEntryMetadata: Equatable, Sendable {
    public let url: URL
    public let name: String
    public let sizeInBytes: Int64
    public let isDirectory: Bool
    public let modifiedDate: Date
    public let mimeType: String
}

public struct AggregatedSelectionMetrics: Equatable, Sendable {
    public let totalItemCount: Int
    public let totalByteSize: Int64
    public let directoryCount: Int
    public let fileCount: Int
    public let largestItemName: String?
    public let largestItemBytes: Int64
    public let topMimeTypes: [String: Int]
}
```

---

### 1.5 Carmack 8-Byte Prefix Sorting (Rust & C-ABI)
```rust
// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TTSortEntry {
    pub prefix: u64, // Big-endian normalized character prefix
    pub node_index: u32,
    pub flags: u32,
}

impl PartialOrd for TTSortEntry {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TTSortEntry {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.prefix.cmp(&other.prefix)
    }
}
```
