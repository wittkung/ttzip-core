// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Represents a hierarchical tree node for archive file and directory navigation.
/// Backed by optional native UniFFIVfsTree handle for zero-copy windowed paging without full tree instantiation.
public struct ArchiveTreeNode: Identifiable, Sendable, Equatable {
    public var id: String { path }
    public let name: String
    public let path: String
    public let uncompressedSize: Int64
    public let isDirectory: Bool

    let _detectedEncoding: String?
    let _compressedSize: Int64?
    let _crc32: UInt32?
    let _posixPermissions: UInt32?
    let _isEncrypted: Bool?

    public var detectedEncoding: String { _detectedEncoding ?? "UTF-8" }

    // Extended VFS attributes
    public var compressedSize: Int64 { _compressedSize ?? 0 }
    public var crc32: UInt32 { _crc32 ?? 0 }
    public let modificationDate: Date?
    public var posixPermissions: UInt32 { _posixPermissions ?? 0 }
    public var isEncrypted: Bool { _isEncrypted ?? false }
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
                var accumulated: [ArchiveTreeNode] = []
                var currentOffset = 0
                let batchSize = 1000
                while true {
                    let paged = tree.getChildrenPaged(subpath: path, offset: UInt32(currentOffset), limit: UInt32(batchSize))
                    let valid = paged.nodes.filter { !$0.name.isEmpty && $0.name != "." && $0.path != path }
                    accumulated.append(contentsOf: valid.map { ArchiveTreeNode(summary: $0, vfsTree: tree) })
                    currentOffset += paged.nodes.count
                    if currentOffset >= Int(paged.totalCount) || paged.nodes.isEmpty {
                        break
                    }
                }
                return accumulated
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
                let mergedPerms = _posixPermissions ?? explicit.posixPermissions
                let mergedCompressed = _compressedSize ?? explicit.compressedSize
                let mergedCrc = _crc32 ?? explicit.crc32
                let mergedModDate = modificationDate ?? explicit.modificationDate
                let mergedEncoding = _detectedEncoding ?? explicit.detectedEncoding
                let mergedEncrypted = _isEncrypted ?? explicit.isEncrypted
                if mergedPerms == explicit.posixPermissions &&
                    mergedCompressed == explicit.compressedSize &&
                    mergedCrc == explicit.crc32 &&
                    mergedModDate == explicit.modificationDate &&
                    mergedEncoding == explicit.detectedEncoding &&
                    mergedEncrypted == explicit.isEncrypted
                {
                    return explicit
                }
                return ArchiveEntry(
                    path: explicit.path,
                    uncompressedSize: explicit.uncompressedSize,
                    isDirectory: explicit.isDirectory,
                    detectedEncoding: mergedEncoding,
                    modificationDate: mergedModDate,
                    isEncrypted: mergedEncrypted,
                    isDataEncrypted: explicit.isDataEncrypted,
                    isMetadataEncrypted: explicit.isMetadataEncrypted,
                    encryptionMethod: explicit.encryptionMethod,
                    posixPermissions: mergedPerms,
                    compressedSize: mergedCompressed,
                    crc32: mergedCrc
                )
            }
            return ArchiveEntry(
                path: path,
                uncompressedSize: uncompressedSize,
                isDirectory: isDirectory,
                detectedEncoding: detectedEncoding,
                modificationDate: modificationDate,
                isEncrypted: isEncrypted,
                posixPermissions: _posixPermissions,
                compressedSize: _compressedSize,
                crc32: _crc32
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
        detectedEncoding: String? = nil,
        children: [ArchiveTreeNode]? = nil,
        entry: ArchiveEntry? = nil,
        compressedSize: Int64? = nil,
        crc32: UInt32? = nil,
        modificationDate: Date? = nil,
        posixPermissions: UInt32? = nil,
        isEncrypted: Bool? = nil,
        hasChildren: Bool = false,
        vfsTree: UniFfiVfsTree? = nil
    ) {
        self.name = name
        self.path = path
        self.uncompressedSize = uncompressedSize
        self.isDirectory = isDirectory
        self._detectedEncoding = detectedEncoding ?? entry?.detectedEncoding
        self._children = children
        self._entry = entry
        self._compressedSize = compressedSize ?? entry?.compressedSize
        self._crc32 = crc32 ?? entry?.crc32
        self.modificationDate = modificationDate ?? entry?.modificationDate
        self._posixPermissions = posixPermissions ?? entry?.posixPermissions
        self._isEncrypted = isEncrypted ?? entry?.isEncrypted
        self.hasChildren = hasChildren || (children != nil && !children!.isEmpty)
        self.vfsTree = vfsTree
    }

    /// Constructs a lightweight tree node directly from native UniFFI VfsNodeSummary.
    public init(summary: UniFfiVfsNodeSummary, vfsTree: UniFfiVfsTree? = nil) {
        let modDate = summary.mtimeEpochSecs > 0
            ? Date(timeIntervalSince1970: TimeInterval(summary.mtimeEpochSecs))
            : nil
        self.name = summary.name
        self.path = summary.path
        self.uncompressedSize = Int64(summary.uncompressedSize)
        self.isDirectory = summary.isDirectory
        self._detectedEncoding = "UTF-8"
        self._children = nil
        self._entry = nil
        self._compressedSize = Int64(summary.compressedSize)
        self._crc32 = summary.crc32
        self.modificationDate = modDate
        self._posixPermissions = summary.mode
        self._isEncrypted = summary.isEncrypted
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

    /// Safe fallback empty node for boundary recovery and invalid index guards.
    public static let empty = ArchiveTreeNode(
        name: "",
        path: "",
        uncompressedSize: 0,
        isDirectory: false
    )
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
            detectedEncoding: self._detectedEncoding,
            children: clonedChildren,
            entry: self._entry,
            compressedSize: self._compressedSize,
            crc32: self._crc32,
            modificationDate: self.modificationDate,
            posixPermissions: self._posixPermissions,
            isEncrypted: self._isEncrypted,
            hasChildren: self.hasChildren,
            vfsTree: self.vfsTree
        )
    }
}
