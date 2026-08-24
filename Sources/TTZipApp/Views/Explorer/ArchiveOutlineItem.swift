// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import AppKit
import TTZipCore

/// Reference-type adapter for NSOutlineView items, guaranteeing pointer identity and zero _SwiftValue boxing.
public final class ArchiveOutlineItem: NSObject {
    public let node: ArchiveTreeNode
    public weak var parent: ArchiveOutlineItem?
    
    private var _cachedChildren: [ArchiveOutlineItem]?
    
    public var children: [ArchiveOutlineItem] {
        if let cached = _cachedChildren { return cached }
        guard let nodeChildren = node.children else {
            _cachedChildren = []
            return []
        }
        let mapped = nodeChildren.map { ArchiveOutlineItem(node: $0, parent: self) }
        _cachedChildren = mapped
        return mapped
    }
    
    public var isDirectory: Bool { node.isDirectory }
    public var name: String { node.name }
    public var path: String { node.path }
    public var uncompressedSize: Int64 { node.uncompressedSize }
    public var detectedEncoding: String { node.detectedEncoding }
    public var entry: ArchiveEntry? { node.entry }
    
    public init(node: ArchiveTreeNode, parent: ArchiveOutlineItem? = nil) {
        self.node = node
        self.parent = parent
        super.init()
    }
    
    public override var hash: Int { node.id.hashValue }
    public override func isEqual(_ object: Any?) -> Bool {
        guard let other = object as? ArchiveOutlineItem else { return false }
        return self.node.id == other.node.id
    }
}
