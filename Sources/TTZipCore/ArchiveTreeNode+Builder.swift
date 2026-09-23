// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation

// MARK: - ArchiveTreeNode Pagination & Construction Extensions

extension ArchiveTreeNode {
    /// Retrieves a windowed slice of child nodes for interactive zero-copy UI directory paging with exact total count.
    public func getChildrenPaged(offset: Int = 0, limit: Int = 100) -> (nodes: [ArchiveTreeNode], total: Int) {
        guard isDirectory else { return ([], 0) }

        if let tree = vfsTree {
            let paged = tree.getChildrenPaged(subpath: path, offset: UInt32(offset), limit: UInt32(limit))
            let childNodes = paged.nodes.map { ArchiveTreeNode(summary: $0, vfsTree: tree) }
            return (childNodes, Int(paged.totalCount))
        }

        if let directChildren = _children {
            let total = directChildren.count
            guard offset < total else { return ([], total) }
            let end = min(offset + limit, total)
            return (Array(directChildren[offset..<end]), total)
        }

        return ([], 0)
    }

    /// Builds root level nodes backed by a persistent `UniFFIVfsTree`.
    /// Nodes are lazily paged and do NOT instantiate full recursive tree hierarchies in memory.
    public static func buildRootNodes(from tree: UniFfiVfsTree, limit: Int = 1000) -> [ArchiveTreeNode] {
        let paged = tree.getChildrenPaged(subpath: nil, offset: 0, limit: UInt32(limit))
        return paged.nodes.map { ArchiveTreeNode(summary: $0, vfsTree: tree) }
    }

    /// Obtains a paged window of children for a given subpath from a native `UniFFIVfsTree`.
    public static func getChildrenPaged(
        from tree: UniFfiVfsTree,
        subpath: String? = nil,
        offset: Int = 0,
        limit: Int = 100
    ) -> (nodes: [ArchiveTreeNode], total: Int) {
        let paged = tree.getChildrenPaged(subpath: subpath, offset: UInt32(offset), limit: UInt32(limit))
        let nodes = paged.nodes.map { ArchiveTreeNode(summary: $0, vfsTree: tree) }
        return (nodes, Int(paged.totalCount))
    }
}

// MARK: - ArchiveTreeBuilder

/// Builds a hierarchical list of `ArchiveTreeNode` objects from flat entries or UniFFIVfsTree.
public final class ArchiveTreeBuilder: @unchecked Sendable {
    /// Builds root nodes from flat archive entries.
    public static func buildTree(from entries: [ArchiveEntry]) -> [ArchiveTreeNode] {
        return FastArchiveTreeBuilder.buildTree(from: entries)
    }

    /// Builds root nodes directly backed by persistent UniFFIVfsTree without recursive class object allocation.
    public static func buildTree(from tree: UniFfiVfsTree, limit: Int = 1000) -> [ArchiveTreeNode] {
        return ArchiveTreeNode.buildRootNodes(from: tree, limit: limit)
    }

    /// Builds root nodes from an existing RustVfsSession.
    public static func buildTree(from session: RustVfsSession, limit: Int = 1000) -> [ArchiveTreeNode] {
        return ArchiveTreeNode.buildRootNodes(from: session.uniffiTree, limit: limit)
    }

    /// Directly opens an archive file and builds root nodes backed by UniFFIVfsTree.
    public static func buildTree(archivePath: String, password: String? = nil, limit: Int = 1000) -> [ArchiveTreeNode]? {
        guard let session = RustVfsBridge.session(forArchivePath: archivePath, password: password) else {
            return nil
        }
        return buildTree(from: session.uniffiTree, limit: limit)
    }
}

// MARK: - ArchiveComponentTreeBuilder

/// High-performance builder for composite tree representation from flat entries or filesystem directories.
public enum ArchiveComponentTreeBuilder {
    private final class MutableDirNode {
        let name: String
        let path: String
        var entry: ArchiveEntry?
        var modificationDate: Date?
        var subdirectories: [String: MutableDirNode] = [:]
        var leafFiles: [String: ArchiveLeafFile] = [:]
        var customChildren: [String: ArchiveComponentProtocol] = [:]

        init(name: String, path: String, entry: ArchiveEntry? = nil, modificationDate: Date? = nil) {
            self.name = name
            self.path = path
            self.entry = entry
            self.modificationDate = modificationDate
        }

        func toImmutableDirectory() -> ArchiveCompositeDirectory {
            var dir = ArchiveCompositeDirectory(
                name: name,
                path: path,
                entry: entry,
                modificationDate: modificationDate
            )
            for (_, subDir) in subdirectories {
                dir.addDirect(component: subDir.toImmutableDirectory())
            }
            for (_, leaf) in leafFiles {
                dir.addDirect(component: leaf)
            }
            for (_, custom) in customChildren {
                dir.addDirect(component: custom)
            }
            return dir
        }
    }

    /// Constructs an immutable `ArchiveCompositeDirectory` hierarchy from a flat array of archive entries.
    public static func buildTree(from entries: [ArchiveEntry]) -> ArchiveCompositeDirectory {
        let root = MutableDirNode(name: "root", path: "")
        for entry in entries {
            let parts = entry.path.split(separator: "/").map(String.init)
            guard !parts.isEmpty else { continue }

            var current = root
            for i in 0..<(parts.count - 1) {
                let dirName = parts[i]
                let dirPath = parts[0...i].joined(separator: "/")
                if let existing = current.subdirectories[dirName] {
                    current = existing
                } else {
                    let newDir = MutableDirNode(name: dirName, path: dirPath)
                    current.subdirectories[dirName] = newDir
                    current = newDir
                }
            }

            let leafName = parts.last!
            if entry.isDirectory {
                if current.subdirectories[leafName] == nil {
                    let newDir = MutableDirNode(name: leafName, path: entry.path, entry: entry, modificationDate: entry.modificationDate)
                    current.subdirectories[leafName] = newDir
                }
            } else {
                let leaf = ArchiveLeafFile(
                    name: leafName,
                    path: entry.path,
                    sizeBytes: entry.uncompressedSize,
                    entry: entry,
                    modificationDate: entry.modificationDate,
                    compressedSizeBytes: nil,
                    crc32: nil
                )
                current.leafFiles[leafName] = leaf
            }
        }
        return root.toImmutableDirectory()
    }

    /// Constructs an immutable `ArchiveCompositeDirectory` hierarchy from an on-disk folder path.
    public static func buildTree(fromDiskPath diskPath: String) -> ArchiveCompositeDirectory {
        let url = URL(fileURLWithPath: diskPath)
        let name = url.lastPathComponent
        let root = MutableDirNode(name: name, path: diskPath)

        let fm = FileManager.default
        guard let enumerator = fm.enumerator(at: url, includingPropertiesForKeys: [.fileSizeKey, .isDirectoryKey]) else {
            return root.toImmutableDirectory()
        }

        for case let fileURL as URL in enumerator {
            let relativePath = fileURL.path.replacingOccurrences(of: diskPath + "/", with: "")
            let resourceValues = try? fileURL.resourceValues(forKeys: [.fileSizeKey, .isDirectoryKey])
            let isDir = resourceValues?.isDirectory ?? false
            let size = Int64(resourceValues?.fileSize ?? 0)

            let parts = relativePath.split(separator: "/").map(String.init)
            guard !parts.isEmpty else { continue }

            var current = root
            for i in 0..<(parts.count - 1) {
                let dirName = parts[i]
                let dirPath = diskPath + "/" + parts[0...i].joined(separator: "/")
                if let existing = current.subdirectories[dirName] {
                    current = existing
                } else {
                    let newDir = MutableDirNode(name: dirName, path: dirPath)
                    current.subdirectories[dirName] = newDir
                    current = newDir
                }
            }

            let leafName = parts.last!
            if isDir {
                if current.subdirectories[leafName] == nil {
                    let newDir = MutableDirNode(name: leafName, path: fileURL.path)
                    current.subdirectories[leafName] = newDir
                }
            } else {
                let leaf = ArchiveLeafFile(name: leafName, path: fileURL.path, sizeBytes: size)
                current.leafFiles[leafName] = leaf
            }
        }
        return root.toImmutableDirectory()
    }
}

// MARK: - ArchiveReading Tree Inspection Extension

extension ArchiveReading {
    public func inspectTree(archivePath: String, password: String? = nil, candidatePasswords: [String]? = nil) async throws -> ArchiveCompositeDirectory {
        let entries = try await inspect(archivePath: archivePath, password: password, candidatePasswords: candidatePasswords)
        return ArchiveComponentTreeBuilder.buildTree(from: entries)
    }
}

