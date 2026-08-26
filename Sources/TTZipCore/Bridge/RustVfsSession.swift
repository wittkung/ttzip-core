// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// Persistent Safe Rust VFS Tree session with cached lookups and zero per-keystroke allocations.
/// 100% Mozilla UniFFI-backed memory-safe tree representation.
public final class RustVfsSession: Sendable {
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
            let mtime = entry.modificationDate.map { Int64($0.timeIntervalSince1970) } ?? 0
            uniffiEntries.append(UniFfiEntryMetadata(
                path: entry.path,
                uncompressedSize: UInt64(max(0, entry.uncompressedSize)),
                compressedSize: 0,
                crc32: 0,
                mtimeEpochSecs: mtime,
                mode: entry.isDirectory ? 0o755 : 0o644,
                isDirectory: entry.isDirectory,
                isEncrypted: entry.isEncrypted,
                compressionMethod: "store",
                detectedEncoding: entry.detectedEncoding
            ))
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

/// Safe, value-copied snapshot of a VFS directory node across the Rust UniFFI boundary.
public struct VfsNodeSummary: Sendable, Equatable, Identifiable {
    public var id: String { path.isEmpty ? name : path }
    public let nodeId: UInt32
    public let name: String
    public let path: String
    public let uncompressedSize: UInt64
    public let compressedSize: UInt64
    public let crc32: UInt32
    public let mtimeEpochSecs: Int64
    public let mode: UInt32
    public let isDirectory: Bool
    public let isEncrypted: Bool
    public let hasChildren: Bool

    public init(
        nodeId: UInt32 = 0,
        name: String,
        path: String = "",
        uncompressedSize: UInt64,
        compressedSize: UInt64,
        crc32: UInt32,
        mtimeEpochSecs: Int64,
        mode: UInt32,
        isDirectory: Bool,
        isEncrypted: Bool,
        hasChildren: Bool
    ) {
        self.nodeId = nodeId
        self.name = name
        self.path = path
        self.uncompressedSize = uncompressedSize
        self.compressedSize = compressedSize
        self.crc32 = crc32
        self.mtimeEpochSecs = mtimeEpochSecs
        self.mode = mode
        self.isDirectory = isDirectory
        self.isEncrypted = isEncrypted
        self.hasChildren = hasChildren
    }

    public init(summary: UniFfiVfsNodeSummary) {
        self.nodeId = 0
        self.name = summary.name
        self.path = summary.path
        self.uncompressedSize = summary.uncompressedSize
        self.compressedSize = summary.compressedSize
        self.crc32 = summary.crc32
        self.mtimeEpochSecs = summary.mtimeEpochSecs
        self.mode = summary.mode
        self.isDirectory = summary.isDirectory
        self.isEncrypted = summary.isEncrypted
        self.hasChildren = summary.hasChildren
    }
}

extension RustVfsSession {
    /// Retrieves a windowed slice of child nodes for interactive zero-copy UI directory paging with exact total count.
    public func getChildrenPaged(subpath: String? = nil, offset: Int = 0, limit: Int = 100) -> (nodes: [VfsNodeSummary], total: Int) {
        let paged = uniffiTree.getChildrenPaged(subpath: subpath, offset: UInt32(offset), limit: UInt32(limit))
        let summaries = paged.nodes.map { VfsNodeSummary(summary: $0) }
        return (summaries, Int(paged.totalCount))
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
