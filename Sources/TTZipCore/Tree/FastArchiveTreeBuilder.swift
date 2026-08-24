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

import Foundation

/// High-throughput Trie-based directory tree builder for massive archives (100k+ entries).
///
/// Optimizations:
/// 1. Zero-allocation `Substring` scanning eliminates intermediate path string concatenations.
/// 2. Single-pass hierarchy generation avoids dual-pass class-to-struct deep copying.
/// 3. Fast directory-first sorting replaces heavyweight localized ICU comparisons on hot paths.
public final class FastArchiveTreeBuilder: @unchecked Sendable {
    private final class FastNode {
        let name: String
        let fullPath: String
        var isDirectory: Bool
        var size: Int64 = 0
        var detectedEncoding: String = "UTF-8"
        var entry: ArchiveEntry?
        var children: [String: FastNode] = [:]

        init(name: String, fullPath: String, isDirectory: Bool, entry: ArchiveEntry? = nil) {
            self.name = name
            self.fullPath = fullPath
            self.isDirectory = isDirectory
            self.entry = entry
        }
    }

    /// Builds a structured `ArchiveTreeNode` hierarchy from flat archive entries in $O(N \cdot D)$ time.
    public static func buildTree(from entries: [ArchiveEntry]) -> [ArchiveTreeNode] {
        guard !entries.isEmpty else { return [] }
        let root = FastNode(name: "", fullPath: "", isDirectory: true)

        for entry in entries {
            let path = entry.path
            var current = root
            var searchStart = path.startIndex

            while searchStart < path.endIndex {
                let slashIndex = path[searchStart...].firstIndex(of: "/") ?? path.endIndex
                let component = path[searchStart..<slashIndex]

                if component.isEmpty {
                    if slashIndex < path.endIndex {
                        searchStart = path.index(after: slashIndex)
                        continue
                    } else {
                        break
                    }
                }

                let isLast = (slashIndex == path.endIndex)
                let compStr = String(component)
                let subPath = String(path[..<slashIndex])

                if let child = current.children[compStr] {
                    current = child
                } else {
                    let isDir = isLast ? entry.isDirectory : true
                    let newNode = FastNode(
                        name: compStr,
                        fullPath: subPath,
                        isDirectory: isDir,
                        entry: isLast ? entry : nil
                    )
                    current.children[compStr] = newNode
                    current = newNode
                }

                if isLast {
                    current.size = entry.uncompressedSize
                    current.entry = entry
                    break
                }
                searchStart = path.index(after: slashIndex)
            }
        }

        func convert(node: FastNode) -> ArchiveTreeNode {
            if !node.isDirectory {
                return ArchiveTreeNode(
                    id: node.fullPath,
                    name: node.name,
                    path: node.fullPath,
                    uncompressedSize: node.size,
                    isDirectory: false,
                    detectedEncoding: node.detectedEncoding,
                    children: nil,
                    entry: node.entry
                )
            }

            let sortedChildren = node.children.values.sorted { a, b in
                if a.isDirectory != b.isDirectory { return a.isDirectory }
                return a.name < b.name
            }.map { convert(node: $0) }

            let totalSize = sortedChildren.reduce(Int64(0)) { $0 + $1.uncompressedSize }
            return ArchiveTreeNode(
                id: node.fullPath,
                name: node.name,
                path: node.fullPath,
                uncompressedSize: totalSize,
                isDirectory: true,
                detectedEncoding: node.detectedEncoding,
                children: sortedChildren,
                entry: node.entry
            )
        }

        return root.children.values.sorted { a, b in
            if a.isDirectory != b.isDirectory { return a.isDirectory }
            return a.name < b.name
        }.map { convert(node: $0) }
    }
}
