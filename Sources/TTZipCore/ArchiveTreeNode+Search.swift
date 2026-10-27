// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

// MARK: - ArchiveTreeNode Search, Traversal & Metrics Extensions

extension ArchiveTreeNode {
    /// Performs zero-allocation fuzzy search directly against native `UniFFIVfsTree` returning lightweight tree nodes.
    public static func search(
        in tree: UniFfiVfsTree,
        query: String,
        maxResults: Int = 100
    ) -> [ArchiveTreeNode] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return [] }
        let matches = tree.search(query: trimmed, maxResults: UInt32(maxResults))
        return matches.map { m in
            ArchiveTreeNode(
                name: m.name,
                path: m.path,
                uncompressedSize: Int64(m.size),
                isDirectory: m.isDirectory,
                detectedEncoding: "UTF-8",
                children: nil,
                entry: nil,
                vfsTree: tree
            )
        }
    }

    /// Recursively locates a descendant node matching the exact specified path.
    public func findNode(byPath targetPath: String) -> ArchiveTreeNode? {
        if self.path == targetPath {
            return self
        }
        guard isDirectory, let children = self.children else { return nil }
        for child in children {
            if targetPath == child.path || targetPath.hasPrefix(child.path + "/") {
                if let found = child.findNode(byPath: targetPath) {
                    return found
                }
            }
        }
        return nil
    }

    /// Performs a case-insensitive recursive search within in-memory child nodes matching name or path substring.
    public func search(matching query: String) -> [ArchiveTreeNode] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return [] }
        return filter { node in
            node.name.localizedCaseInsensitiveContains(trimmed) ||
                node.path.localizedCaseInsensitiveContains(trimmed)
        }
    }

    /// Traverses and collects all nodes in this subtree matching the supplied predicate.
    public func filter(predicate: (ArchiveTreeNode) -> Bool) -> [ArchiveTreeNode] {
        var results: [ArchiveTreeNode] = []
        if predicate(self) {
            results.append(self)
        }
        if let children = self.children {
            for child in children {
                results.append(contentsOf: child.filter(predicate: predicate))
            }
        }
        return results
    }

    /// Flattens this node and all recursive descendant nodes into a single linear array.
    public func flattenDescendants() -> [ArchiveTreeNode] {
        var list: [ArchiveTreeNode] = [self]
        if let children = self.children {
            for child in children {
                list.append(contentsOf: child.flattenDescendants())
            }
        }
        return list
    }

    /// Computes the total number of regular (non-directory) files in this subtree.
    public func totalDescendantFilesCount() -> Int {
        if !isDirectory { return 1 }
        guard let children = self.children else { return 0 }
        return children.reduce(0) { $0 + $1.totalDescendantFilesCount() }
    }

    /// Computes the total number of directories contained in this subtree.
    public func totalDescendantDirectoriesCount() -> Int {
        guard isDirectory, let children = self.children else { return 0 }
        return 1 + children.reduce(0) { $0 + $1.totalDescendantDirectoriesCount() }
    }

    /// Computes the aggregate uncompressed byte size of all leaf files in this subtree.
    public func totalDescendantSize() -> Int64 {
        if !isDirectory { return uncompressedSize }
        guard let children = self.children else { return 0 }
        return children.reduce(0) { $0 + $1.totalDescendantSize() }
    }
}
