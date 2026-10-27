// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

public typealias UniFFIVfsTree = UniFfiVfsTree
public typealias VfsNodeSummary = UniFfiVfsNodeSummary
public typealias VfsPagedResult = UniFfiVfsPagedResult
public typealias VfsMatch = UniFfiVfsMatch
public typealias VfsStats = UniFfiVfsStats

/// Persistent Safe Rust VFS Tree session with cached lookups and zero per-keystroke allocations.
/// 100% Mozilla UniFFI-backed memory-safe tree representation.
public final class RustVfsSession: @unchecked Sendable {
    public let uniffiTree: UniFfiVfsTree
    public let archivePath: String?
    private let entryMap: [String: ArchiveEntry]?
    private let _allEntries: [ArchiveEntry]?

    /// Direct constructor wrapping an existing native UniFFIVfsTree handle.
    public init(tree: UniFfiVfsTree, archivePath: String? = nil, entries: [ArchiveEntry]? = nil) {
        self.uniffiTree = tree
        self.archivePath = archivePath
        self._allEntries = entries
        if let entries = entries {
            var map: [String: ArchiveEntry] = [:]
            map.reserveCapacity(entries.count)
            for entry in entries {
                map[entry.path] = entry
            }
            self.entryMap = map
        } else {
            self.entryMap = nil
        }
    }

    /// Directly loads and constructs the VFS tree in native Rust memory from an archive on disk.
    /// Eliminates cross-FFI entry serialization roundtrips and Swift heap overhead.
    public convenience init?(archivePath: String, password: String? = nil) {
        do {
            let tree = try loadArchiveVfsTree(archivePath: archivePath, password: password)
            self.init(tree: tree, archivePath: archivePath)
        } catch {
            return nil
        }
    }

    /// Fallback constructor for in-memory synthesized archive entries (e.g. testing or dynamic archives).
    public convenience init?(entries: [ArchiveEntry], rootName: String = "") {
        guard !entries.isEmpty else { return nil }
        var uniffiEntries: [UniFfiEntryMetadata] = []
        uniffiEntries.reserveCapacity(entries.count)
        for entry in entries {
            uniffiEntries.append(UniFfiEntryMetadata(entry: entry))
        }
        let tree = UniFfiVfsTree.build(entries: uniffiEntries, rootName: rootName)
        self.init(tree: tree, archivePath: nil, entries: entries)
    }

    /// Provides access to catalog entries. Lazily queries root node if constructed directly from archivePath.
    public var allEntries: [ArchiveEntry] {
        if let existing = _allEntries { return existing }
        let paged = uniffiTree.getChildrenPaged(subpath: nil, offset: 0, limit: UInt32.max)
        return paged.nodes.map { summary in
            ArchiveEntry(
                path: summary.path,
                uncompressedSize: Int64(summary.uncompressedSize),
                isDirectory: summary.isDirectory,
                detectedEncoding: "UTF-8",
                modificationDate: summary.mtimeEpochSecs > 0 ? Date(timeIntervalSince1970: TimeInterval(summary.mtimeEpochSecs)) : nil,
                isEncrypted: summary.isEncrypted
            )
        }
    }

    /// Fast read-only fuzzy search directly reusing persistent UniFFI VFS Tree object without reallocating nodes.
    public func fuzzySearch(query: String) -> [ArchiveEntry] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return allEntries
        }

        let matches = uniffiTree.search(query: trimmed, maxResults: 1000)
        return matches.map { match in
            if let map = entryMap, let entry = map[match.path] {
                return entry
            }
            return ArchiveEntry(
                path: match.path,
                uncompressedSize: Int64(match.size),
                isDirectory: match.isDirectory,
                detectedEncoding: "UTF-8",
                modificationDate: nil,
                isEncrypted: false
            )
        }
    }

    /// Fast search into fixed-capacity pre-allocated buffer via UniFFI.
    public func searchZeroAlloc(query: String, maxResults: Int = 64) -> [ArchiveEntry] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return allEntries }

        let matches = uniffiTree.search(query: trimmed, maxResults: UInt32(maxResults))
        return matches.map { match in
            if let map = entryMap, let entry = map[match.path] {
                return entry
            }
            return ArchiveEntry(
                path: match.path,
                uncompressedSize: Int64(match.size),
                isDirectory: match.isDirectory,
                detectedEncoding: "UTF-8",
                modificationDate: nil,
                isEncrypted: false
            )
        }
    }

    /// Direct zero-copy search returning raw UniFFI match records.
    public func searchMatches(query: String, maxResults: Int = 64) -> [UniFfiVfsMatch] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return [] }
        return uniffiTree.search(query: trimmed, maxResults: UInt32(maxResults))
    }
}

// MARK: - Safe VFS Node Value Type

extension UniFfiVfsNodeSummary: Identifiable, @unchecked Sendable {
    public var id: String { path.isEmpty ? name : path }
    public var nodeId: UInt32 { 0 }
}

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
