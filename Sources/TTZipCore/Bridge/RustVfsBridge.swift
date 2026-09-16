// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// High-performance Safe Rust Unified VFS Bridge powered by 100% Mozilla UniFFI.
public enum RustVfsBridge {
    private static let sessionLock = NSLock()
    nonisolated(unsafe) private static var cachedSession: (count: Int, first: String, last: String, session: RustVfsSession)?
    
    /// Obtains or creates a persistent `RustVfsSession` avoiding per-keystroke tree rebuilding.
    public static func session(for entries: [ArchiveEntry], rootName: String = "") -> RustVfsSession? {
        guard !entries.isEmpty else { return nil }
        sessionLock.lock()
        defer { sessionLock.unlock() }
        let first = entries.first?.path ?? ""
        let last = entries.last?.path ?? ""
        if let cached = cachedSession,
           cached.count == entries.count,
           cached.first == first,
           cached.last == last {
            return cached.session
        }
        guard let newSession = RustVfsSession(entries: entries, rootName: rootName) else { return nil }
        cachedSession = (entries.count, first, last, newSession)
        return newSession
    }
    
    /// Explicitly purges the cached active VFS session.
    public static func clearSessionCache() {
        sessionLock.lock()
        defer { sessionLock.unlock() }
        cachedSession = nil
    }

    /// Builds or borrows a native UniFFI VFS tree object from flat ArchiveEntry list.
    public static func withTreeHandle<R>(entries: [ArchiveEntry], rootName: String = "", block: (UniFfiVfsTree) throws -> R) rethrows -> R? {
        guard let session = session(for: entries, rootName: rootName) else { return nil }
        return try block(session.uniffiTree)
    }
    
    /// Renders ASCII/Unicode hierarchical tree using Safe Rust VFS engine.
    public static func renderTree(from entries: [ArchiveEntry], rootName: String = "") -> String {
        guard let session = session(for: entries, rootName: rootName) else { return "" }
        return session.renderTree()
    }
    
    /// Performs fast fuzzy search against archive entries using persistent Safe Rust VFS session.
    public static func fuzzySearch(in entries: [ArchiveEntry], query: String) -> [ArchiveEntry] {
        guard !query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return entries
        }
        guard let session = session(for: entries) else { return entries }
        return session.fuzzySearch(query: query)
    }
    
    /// Retrieves aggregated VFS statistics (total files, directories, uncompressed size).
    public static func getStats(from entries: [ArchiveEntry], rootName: String = "") -> (totalFiles: UInt64, totalDirs: UInt64, totalSize: UInt64)? {
        guard let session = session(for: entries, rootName: rootName) else { return nil }
        return session.getStats()
    }
}
