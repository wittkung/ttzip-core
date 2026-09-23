// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation

/// Represents a hierarchical tree node for archive file and directory navigation.
/// Backed by optional native UniFFIVfsTree handle for zero-copy windowed paging without full tree instantiation.
public struct ArchiveTreeNode: Identifiable, Sendable, Equatable {
    public var id: String { path }
    public let name: String
    public let path: String
    public let uncompressedSize: Int64
    public let isDirectory: Bool
    public let detectedEncoding: String

    // Extended VFS attributes
    public let compressedSize: Int64
    public let crc32: UInt32
    public let modificationDate: Date?
    public let posixPermissions: UInt32
    public let isEncrypted: Bool
    public let hasChildren: Bool

    // Native UniFFI VFS tree handle for windowed pagination
    public let vfsTree: UniFfiVfsTree?

    var _children: [ArchiveTreeNode]?
    var _entry: ArchiveEntry?

    public var children: [ArchiveTreeNode]? {
        get {
            if let explicit = _children {
                return explicit
            }
            if let tree = vfsTree, isDirectory {
                let paged = tree.getChildrenPaged(subpath: path, offset: 0, limit: 1000)
                return paged.nodes.map { ArchiveTreeNode(summary: $0, vfsTree: tree) }
            }
            return nil
        }
        set {
            _children = newValue
        }
    }

    public var entry: ArchiveEntry? {
        get {
            if let explicit = _entry {
                return explicit
            }
            return ArchiveEntry(
                path: path,
                uncompressedSize: uncompressedSize,
                isDirectory: isDirectory,
                detectedEncoding: detectedEncoding,
                modificationDate: modificationDate,
                isEncrypted: isEncrypted
            )
        }
        set {
            _entry = newValue
        }
    }

    public init(
        id: String? = nil,
        name: String,
        path: String,
        uncompressedSize: Int64,
        isDirectory: Bool,
        detectedEncoding: String = "UTF-8",
        children: [ArchiveTreeNode]? = nil,
        entry: ArchiveEntry? = nil,
        compressedSize: Int64 = 0,
        crc32: UInt32 = 0,
        modificationDate: Date? = nil,
        posixPermissions: UInt32 = 0,
        isEncrypted: Bool = false,
        hasChildren: Bool = false,
        vfsTree: UniFfiVfsTree? = nil
    ) {
        self.name = name
        self.path = path
        self.uncompressedSize = uncompressedSize
        self.isDirectory = isDirectory
        self.detectedEncoding = detectedEncoding
        self._children = children
        self._entry = entry
        self.compressedSize = compressedSize
        self.crc32 = crc32
        self.modificationDate = modificationDate
        self.posixPermissions = posixPermissions
        self.isEncrypted = isEncrypted
        self.hasChildren = hasChildren || (children != nil && !children!.isEmpty)
        self.vfsTree = vfsTree
    }

    /// Constructs a lightweight tree node directly from native UniFFI VfsNodeSummary.
    public init(summary: UniFfiVfsNodeSummary, vfsTree: UniFfiVfsTree? = nil) {
        self.name = summary.name
        self.path = summary.path
        self.uncompressedSize = Int64(summary.uncompressedSize)
        self.isDirectory = summary.isDirectory
        self.detectedEncoding = "UTF-8"
        self._children = nil
        self._entry = nil
        self.compressedSize = Int64(summary.compressedSize)
        self.crc32 = summary.crc32
        self.modificationDate = summary.mtimeEpochSecs > 0 ? Date(timeIntervalSince1970: TimeInterval(summary.mtimeEpochSecs)) : nil
        self.posixPermissions = summary.mode
        self.isEncrypted = summary.isEncrypted
        self.hasChildren = summary.hasChildren
        self.vfsTree = vfsTree
    }

    public static func == (lhs: ArchiveTreeNode, rhs: ArchiveTreeNode) -> Bool {
        return lhs.path == rhs.path &&
            lhs.uncompressedSize == rhs.uncompressedSize &&
            lhs.isDirectory == rhs.isDirectory &&
            lhs.name == rhs.name &&
            lhs._children == rhs._children
    }
}

// MARK: - PrototypeCopyable Prototype Pattern Extension

extension ArchiveTreeNode: PrototypeCopyable {
    /// Creates a deep clone of the entire tree hierarchy.
    public func clone() -> ArchiveTreeNode {
        return cloneTree()
    }

    /// Recursively deep-copies this tree node and all descendants.
    /// - Returns: Independent cloned `ArchiveTreeNode` subtree.
    public func cloneTree() -> ArchiveTreeNode {
        let clonedChildren = _children?.map { $0.cloneTree() }
        return ArchiveTreeNode(
            name: self.name,
            path: self.path,
            uncompressedSize: self.uncompressedSize,
            isDirectory: self.isDirectory,
            detectedEncoding: self.detectedEncoding,
            children: clonedChildren,
            entry: self._entry,
            compressedSize: self.compressedSize,
            crc32: self.crc32,
            modificationDate: self.modificationDate,
            posixPermissions: self.posixPermissions,
            isEncrypted: self.isEncrypted,
            hasChildren: self.hasChildren,
            vfsTree: self.vfsTree
        )
    }
}
