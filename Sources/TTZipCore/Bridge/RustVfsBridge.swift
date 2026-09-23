// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation

/// Thread-safe concurrent session manager isolating VFS sessions by physical path or content fingerprint.
public final class VfsSessionManager: @unchecked Sendable {
    public static let shared = VfsSessionManager()

    private let lock = NSLock()
    private var sessions: [String: RustVfsSession] = [:]
    private var accessOrder: [String] = []
    private let maxCapacity: Int

    public init(maxCapacity: Int = 16) {
        self.maxCapacity = maxCapacity
    }

    /// Computes a deterministic content fingerprint for entry catalogs.
    public static func computeFingerprint(for entries: [ArchiveEntry]) -> String {
        let count = entries.count
        guard count > 0 else { return "empty" }

        var hasher = Hasher()
        hasher.combine(count)

        if count <= 128 {
            for entry in entries {
                hasher.combine(entry.path)
                hasher.combine(entry.uncompressedSize)
                hasher.combine(entry.isDirectory)
            }
        } else {
            // Multi-point deterministic sampling across large entry arrays
            let step = max(1, count / 64)
            var i = 0
            while i < count {
                let entry = entries[i]
                hasher.combine(entry.path)
                hasher.combine(entry.uncompressedSize)
                hasher.combine(entry.isDirectory)
                i += step
            }
            let first = entries[0]
            let mid = entries[count / 2]
            let last = entries[count - 1]
            hasher.combine(first.path)
            hasher.combine(mid.path)
            hasher.combine(last.path)
            hasher.combine(first.uncompressedSize)
            hasher.combine(last.uncompressedSize)
        }

        return "fp_\(count)_\(hasher.finalize())"
    }

    /// Obtains or creates a persistent `RustVfsSession` directly from an archive on disk using `loadArchiveVfsTree`.
    /// Eliminates double serialization and entry array copying.
    public func session(
        forArchivePath archivePath: String,
        password: String? = nil
    ) -> RustVfsSession? {
        guard !archivePath.isEmpty else { return nil }
        let key = "path_\(archivePath)"

        lock.lock()
        defer { lock.unlock() }

        if let existing = sessions[key] {
            if let idx = accessOrder.firstIndex(of: key) {
                accessOrder.remove(at: idx)
            }
            accessOrder.append(key)
            return existing
        }

        guard let newSession = RustVfsSession(archivePath: archivePath, password: password) else {
            return nil
        }

        while sessions.count >= maxCapacity, !accessOrder.isEmpty {
            let oldestKey = accessOrder.removeFirst()
            sessions.removeValue(forKey: oldestKey)
        }

        sessions[key] = newSession
        accessOrder.append(key)
        return newSession
    }

    /// Obtains or creates a persistent `RustVfsSession` isolated by identifier or content fingerprint.
    public func session(
        for entries: [ArchiveEntry],
        identifier: String? = nil,
        rootName: String = ""
    ) -> RustVfsSession? {
        guard !entries.isEmpty else { return nil }
        let key = identifier ?? Self.computeFingerprint(for: entries)

        lock.lock()
        defer { lock.unlock() }

        if let existing = sessions[key] {
            if let idx = accessOrder.firstIndex(of: key) {
                accessOrder.remove(at: idx)
            }
            accessOrder.append(key)
            return existing
        }

        // If identifier points directly to an existing archive file on disk, load directly in Rust
        if let id = identifier, FileManager.default.fileExists(atPath: id),
           let directSession = RustVfsSession(archivePath: id) {
            while sessions.count >= maxCapacity, !accessOrder.isEmpty {
                let oldestKey = accessOrder.removeFirst()
                sessions.removeValue(forKey: oldestKey)
            }
            sessions[key] = directSession
            accessOrder.append(key)
            return directSession
        }

        guard let newSession = RustVfsSession(entries: entries, rootName: rootName) else {
            return nil
        }

        while sessions.count >= maxCapacity, !accessOrder.isEmpty {
            let oldestKey = accessOrder.removeFirst()
            sessions.removeValue(forKey: oldestKey)
        }

        sessions[key] = newSession
        accessOrder.append(key)
        return newSession
    }

    /// Removes a cached session for a specific identifier or archive path.
    public func removeSession(for identifier: String) {
        lock.lock()
        defer { lock.unlock() }
        let pathKey = "path_\(identifier)"
        sessions.removeValue(forKey: identifier)
        sessions.removeValue(forKey: pathKey)
        accessOrder.removeAll { $0 == identifier || $0 == pathKey }
    }

    /// Clears all cached sessions.
    public func clearCache() {
        lock.lock()
        defer { lock.unlock() }
        sessions.removeAll()
        accessOrder.removeAll()
    }
}

/// High-performance Safe Rust Unified VFS Bridge powered by 100% Mozilla UniFFI.
public enum RustVfsBridge {
    /// Obtains or creates a persistent `RustVfsSession` directly from an archive file path.
    public static func session(
        forArchivePath archivePath: String,
        password: String? = nil
    ) -> RustVfsSession? {
        VfsSessionManager.shared.session(forArchivePath: archivePath, password: password)
    }

    /// Obtains or creates a persistent `RustVfsSession` avoiding per-keystroke tree rebuilding.
    public static func session(
        for entries: [ArchiveEntry],
        identifier: String? = nil,
        rootName: String = ""
    ) -> RustVfsSession? {
        VfsSessionManager.shared.session(for: entries, identifier: identifier, rootName: rootName)
    }

    /// Explicitly purges all cached VFS sessions.
    public static func clearSessionCache() {
        VfsSessionManager.shared.clearCache()
    }

    /// Explicitly removes a specific cached session by identifier.
    public static func removeSession(for identifier: String) {
        VfsSessionManager.shared.removeSession(for: identifier)
    }

    /// Directly executes a block with native UniFFIVfsTree loaded from an archive file path.
    public static func withTreeHandle<R>(
        archivePath: String,
        password: String? = nil,
        block: (UniFfiVfsTree) throws -> R
    ) rethrows -> R? {
        guard let session = session(forArchivePath: archivePath, password: password) else { return nil }
        return try block(session.uniffiTree)
    }

    /// Builds or borrows a native UniFFI VFS tree object from flat ArchiveEntry list.
    public static func withTreeHandle<R>(
        entries: [ArchiveEntry],
        identifier: String? = nil,
        rootName: String = "",
        block: (UniFfiVfsTree) throws -> R
    ) rethrows -> R? {
        guard let session = session(for: entries, identifier: identifier, rootName: rootName) else { return nil }
        return try block(session.uniffiTree)
    }

    /// Renders ASCII/Unicode hierarchical tree directly from archive file path.
    public static func renderTree(
        archivePath: String,
        password: String? = nil
    ) -> String {
        guard let session = session(forArchivePath: archivePath, password: password) else { return "" }
        return session.renderTree()
    }

    /// Renders ASCII/Unicode hierarchical tree using Safe Rust VFS engine.
    public static func renderTree(
        from entries: [ArchiveEntry],
        identifier: String? = nil,
        rootName: String = ""
    ) -> String {
        guard let session = session(for: entries, identifier: identifier, rootName: rootName) else { return "" }
        return session.renderTree()
    }

    /// Performs fast fuzzy search against archive directly using persistent Safe Rust VFS session.
    public static func fuzzySearch(
        archivePath: String,
        query: String,
        password: String? = nil
    ) -> [ArchiveEntry] {
        guard let session = session(forArchivePath: archivePath, password: password) else { return [] }
        return session.fuzzySearch(query: query)
    }

    /// Performs fast fuzzy search against archive entries using persistent Safe Rust VFS session.
    public static func fuzzySearch(
        in entries: [ArchiveEntry],
        identifier: String? = nil,
        query: String
    ) -> [ArchiveEntry] {
        guard !query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return entries
        }
        guard let session = session(for: entries, identifier: identifier) else { return entries }
        return session.fuzzySearch(query: query)
    }

    /// Retrieves aggregated VFS statistics directly from archive file path.
    public static func getStats(
        archivePath: String,
        password: String? = nil
    ) -> (totalFiles: UInt64, totalDirs: UInt64, totalSize: UInt64)? {
        guard let session = session(forArchivePath: archivePath, password: password) else { return nil }
        return session.getStats()
    }

    /// Retrieves aggregated VFS statistics (total files, directories, uncompressed size).
    public static func getStats(
        from entries: [ArchiveEntry],
        identifier: String? = nil,
        rootName: String = ""
    ) -> (totalFiles: UInt64, totalDirs: UInt64, totalSize: UInt64)? {
        guard let session = session(for: entries, identifier: identifier, rootName: rootName) else { return nil }
        return session.getStats()
    }

    /// Retrieves a windowed slice of child nodes directly from archive file path for zero-copy UI directory paging.
    public static func getChildrenPaged(
        archivePath: String,
        subpath: String? = nil,
        offset: Int = 0,
        limit: Int = 100,
        password: String? = nil
    ) -> (nodes: [VfsNodeSummary], total: Int) {
        guard let session = session(forArchivePath: archivePath, password: password) else { return ([], 0) }
        return session.getChildrenPaged(subpath: subpath, offset: offset, limit: limit)
    }
}
