// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// Persistent Safe Rust VFS Tree session with cached lookups and zero per-keystroke allocations.
/// 100% Mozilla UniFFI-backed memory-safe tree representation.
public final class RustVfsSession: @unchecked Sendable {
    public let uniffiTree: UniFfiVfsTree
    private let entryMap: [String: ArchiveEntry]
    public let allEntries: [ArchiveEntry]
    
    public init?(entries: [ArchiveEntry], rootName: String = "") {
        guard !entries.isEmpty else { return nil }
        
        self.allEntries = entries
        var map: [String: ArchiveEntry] = [:]
        map.reserveCapacity(entries.count)
        
        var uniffiEntries: [UniFfiEntryMetadata] = []
        uniffiEntries.reserveCapacity(entries.count)
        
        for entry in entries {
            map[entry.path] = entry
            uniffiEntries.append(UniFfiEntryMetadata(entry: entry))
        }
        
        self.entryMap = map
        self.uniffiTree = UniFfiVfsTree.build(entries: uniffiEntries, rootName: rootName)
    }
    
    /// Fast read-only fuzzy search directly reusing persistent UniFFI VFS Tree object without reallocating nodes.
    public func fuzzySearch(query: String) -> [ArchiveEntry] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return allEntries
        }
        
        let matches = uniffiTree.search(query: trimmed, maxResults: 1000)
        return matches.compactMap { entryMap[$0.path] }
    }
    
    /// Fast search into fixed-capacity pre-allocated buffer via UniFFI.
    public func searchZeroAlloc(query: String, maxResults: Int = 64) -> [ArchiveEntry] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return allEntries }
        
        let matches = uniffiTree.search(query: trimmed, maxResults: UInt32(maxResults))
        return matches.compactMap { entryMap[$0.path] }
    }
}

// MARK: - Safe VFS Node Value Type

extension UniFfiVfsNodeSummary: Identifiable, @unchecked Sendable {
    public var id: String { path.isEmpty ? name : path }
    public var nodeId: UInt32 { 0 }
}

public typealias VfsNodeSummary = UniFfiVfsNodeSummary

extension RustVfsSession {
    /// Retrieves a windowed slice of child nodes for interactive zero-copy UI directory paging with exact total count.
    public func getChildrenPaged(subpath: String? = nil, offset: Int = 0, limit: Int = 100) -> (nodes: [VfsNodeSummary], total: Int) {
        let paged = uniffiTree.getChildrenPaged(subpath: subpath, offset: UInt32(offset), limit: UInt32(limit))
        return (paged.nodes, Int(paged.totalCount))
    }

    /// Backward-compatible windowed slice retrieval for child nodes.
    public func getChildren(subpath: String? = nil, offset: Int = 0, limit: Int = 100) -> (nodes: [VfsNodeSummary], total: Int) {
        return getChildrenPaged(subpath: subpath, offset: offset, limit: limit)
    }

    /// Renders ASCII/Unicode tree from persistent VFS session.
    public func renderTree() -> String {
        return uniffiTree.renderTree()
    }
    
    /// Returns aggregated statistics from persistent VFS tree.
    public func getStats() -> (totalFiles: UInt64, totalDirs: UInt64, totalSize: UInt64) {
        let stats = uniffiTree.getStats()
        return (stats.totalFiles, stats.totalDirs, stats.totalUncompressedBytes)
    }
}
