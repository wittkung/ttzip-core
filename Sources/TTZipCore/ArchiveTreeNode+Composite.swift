// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation

// MARK: - ArchiveTreeNode Composite Protocol Conformance

extension ArchiveTreeNode: ArchiveComponentProtocol {
    public var sizeBytes: Int64 {
        if isDirectory, let children = children, !children.isEmpty {
            return children.reduce(0) { $0 + $1.sizeBytes }
        }
        return uncompressedSize
    }

    public func getChildren() -> [ArchiveComponentProtocol] {
        if let direct = _children, !direct.isEmpty {
            return direct
        }
        if let tree = vfsTree, isDirectory {
            let paged = tree.getChildrenPaged(subpath: path, offset: 0, limit: 1000)
            return paged.nodes.map { ArchiveTreeNode(summary: $0, vfsTree: tree) }
        }
        return []
    }

    /// Converts this node into a composite Component (Leaf or Composite Directory).
    public func toComponent() -> ArchiveComponentProtocol {
        if isDirectory {
            let childComponents = (children ?? []).map { $0.toComponent() }
            return ArchiveCompositeDirectory(name: name, path: path, entry: entry, children: childComponents)
        } else {
            return ArchiveLeafFile(name: name, path: path, sizeBytes: uncompressedSize, entry: entry)
        }
    }

    /// Reconstructs an `ArchiveTreeNode` from a composite Component.
    public init(component: ArchiveComponentProtocol, detectedEncoding: String = "UTF-8") {
        self.name = component.name
        self.path = component.path
        self.isDirectory = component.isDirectory
        self.detectedEncoding = detectedEncoding
        self.compressedSize = 0
        self.crc32 = 0
        self.modificationDate = nil
        self.posixPermissions = 0
        self.isEncrypted = false
        self.vfsTree = nil

        if let leaf = component as? ArchiveLeafFile {
            self.uncompressedSize = leaf.sizeBytes
            self._entry = leaf.entry
            self._children = nil
            self.hasChildren = false
        } else if let composite = component as? ArchiveCompositeDirectory {
            self._entry = composite.entry
            let childComponents = composite.getChildren()
            let childNodes = childComponents.map { ArchiveTreeNode(component: $0, detectedEncoding: detectedEncoding) }
            self.uncompressedSize = childNodes.reduce(0) { $0 + $1.uncompressedSize }
            self._children = childNodes
            self.hasChildren = !childNodes.isEmpty
        } else {
            self.uncompressedSize = 0
            self._entry = nil
            self._children = nil
            self.hasChildren = false
        }
    }
}
