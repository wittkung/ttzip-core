// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

// MARK: - UniFFI Entry Metadata Protocol Extensions

/// Extends native UniFFI Entry Metadata with Identifiable, Sendable, and domain properties
/// eliminating redundant intermediate mirror structs and field-by-field copies.
extension UniFfiEntryMetadata: Identifiable, @unchecked Sendable {
    public var id: String { path }

    public var modificationDate: Date? {
        get {
            mtimeEpochSecs > 0 ? Date(timeIntervalSince1970: TimeInterval(mtimeEpochSecs)) : nil
        }
        set {
            mtimeEpochSecs = newValue.map { Int64($0.timeIntervalSince1970) } ?? 0
        }
    }

    public var posixPermissions: UInt32? {
        get { mode }
        set { if let v = newValue { mode = v } }
    }

    public var isSymlink: Bool {
        (mode & 0o170000) == 0o120000
    }

    public var symlinkTarget: String? {
        nil
    }

    public var encryptionMethod: String? {
        get { isEncrypted ? compressionMethod : nil }
        set { if let v = newValue { compressionMethod = v } }
    }

    public var mimeType: String {
        ArchiveMimeMapper.mimeType(forExtension: (path as NSString).pathExtension.lowercased())
    }

    public init(
        path: String,
        uncompressedSize: Int64 = 0,
        compressedSize: Int64? = nil,
        crc32: UInt32? = nil,
        modificationDate: Date? = nil,
        posixPermissions: UInt32? = nil,
        isDirectory: Bool = false,
        isSymlink: Bool = false,
        symlinkTarget: String? = nil,
        isEncrypted: Bool = false,
        encryptionMethod: String? = nil,
        detectedEncoding: String = "UTF-8",
        mimeType: String = "application/octet-stream"
    ) {
        let mtime = modificationDate.map { Int64($0.timeIntervalSince1970) } ?? 0
        self.init(
            path: path,
            uncompressedSize: UInt64(max(0, uncompressedSize)),
            compressedSize: UInt64(max(0, compressedSize ?? 0)),
            crc32: crc32 ?? 0,
            mtimeEpochSecs: mtime,
            mode: posixPermissions ?? (isDirectory ? 0o755 : 0o644),
            isDirectory: isDirectory,
            isEncrypted: isEncrypted,
            compressionMethod: encryptionMethod ?? (isEncrypted ? "aes256" : "deflate"),
            detectedEncoding: detectedEncoding
        )
    }

    /// Constructs metadata directly from a runtime `ArchiveEntry`.
    public init(entry: ArchiveEntry) {
        let mtime = entry.modificationDate.map { Int64($0.timeIntervalSince1970) } ?? 0
        self.init(
            path: entry.path,
            uncompressedSize: UInt64(max(0, entry.uncompressedSize)),
            compressedSize: 0,
            crc32: 0,
            mtimeEpochSecs: mtime,
            mode: entry.isDirectory ? 0o755 : 0o644,
            isDirectory: entry.isDirectory,
            isEncrypted: entry.isEncrypted,
            compressionMethod: entry.encryptionMethod ?? (entry.isEncrypted ? "aes256" : "deflate"),
            detectedEncoding: entry.detectedEncoding
        )
    }
}

public typealias ArchiveEntryMetadata = UniFfiEntryMetadata

// MARK: - Metadata Pool

//
//


/// Shared intrinsic string and metadata interning pool for archive entries using Swift 6 Actor concurrency.
///
/// Reduces memory footprint when browsing massive archives by canonicalizing shared path prefixes, extensions,
/// and MIME types.
public actor ArchiveEntryMetadataPool {
    public static let shared = ArchiveEntryMetadataPool()

    private var pathPool: [String: String] = [:]
    private var extensionPool: [String: String] = [:]
    private var mimeTypePool: [String: String] = [:]
    private var directoryPrefixPool: [String: String] = [:]

    private static let predefinedMimeTypes: [String: String] = [
        "swift": "text/x-swift",
        "js": "application/javascript",
        "json": "application/json",
        "html": "text/html",
        "css": "text/css",
        "png": "image/png",
        "jpg": "image/jpeg",
        "jpeg": "image/jpeg",
        "gif": "image/gif",
        "svg": "image/svg+xml",
        "pdf": "application/pdf",
        "zip": "application/zip",
        "7z": "application/x-7z-compressed",
        "tar": "application/x-tar",
        "gz": "application/gzip",
        "zst": "application/zstd",
        "txt": "text/plain",
        "md": "text/markdown",
        "c": "text/x-c",
        "cpp": "text/x-c++",
        "h": "text/x-chdr",
        "py": "text/x-python",
        "rs": "text/x-rust",
        "go": "text/x-go",
        "xml": "application/xml",
        "mp3": "audio/mpeg",
        "mp4": "video/mp4",
        "mov": "video/quicktime"
    ]

    public var maxPathPoolCapacity: Int = 50_000

    public init() {
        for (ext, mime) in Self.predefinedMimeTypes {
            extensionPool[ext] = ext
            mimeTypePool[mime] = mime
        }
    }

    // MARK: - Interning API

    public func internPath(_ path: String) -> String {
        guard !path.isEmpty else { return "" }
        if let existing = pathPool[path] {
            return existing
        }
        if pathPool.count >= maxPathPoolCapacity {
            pathPool.removeAll(keepingCapacity: false)
        }
        pathPool[path] = path
        return path
    }

    public func internExtension(_ ext: String) -> String {
        let lowerExt = ext.lowercased()
        guard !lowerExt.isEmpty else { return "" }
        if let existing = extensionPool[lowerExt] {
            return existing
        }
        extensionPool[lowerExt] = lowerExt
        return lowerExt
    }

    public func internMimeType(_ mime: String) -> String {
        guard !mime.isEmpty else { return "application/octet-stream" }
        if let existing = mimeTypePool[mime] {
            return existing
        }
        mimeTypePool[mime] = mime
        return mime
    }

    public func internDirectoryPrefix(_ prefix: String) -> String {
        guard !prefix.isEmpty else { return "" }
        if let existing = directoryPrefixPool[prefix] {
            return existing
        }
        directoryPrefixPool[prefix] = prefix
        return prefix
    }

    public func detectMimeType(forPath path: String) -> String {
        let ext = (path as NSString).pathExtension.lowercased()
        if let mime = Self.predefinedMimeTypes[ext] {
            return internMimeType(mime)
        }
        return internMimeType("application/octet-stream")
    }

    public func extractAndInternDirectoryPrefix(fromPath path: String) -> String {
        let nsPath = path as NSString
        let dir = nsPath.deletingLastPathComponent
        guard !dir.isEmpty && dir != "." else { return "" }
        let prefix = dir.hasSuffix("/") ? dir : "\(dir)/"
        return internDirectoryPrefix(prefix)
    }

    public func clearPool() {
        clearPools()
    }

    public func clearPools() {
        pathPool.removeAll(keepingCapacity: false)
        extensionPool.removeAll(keepingCapacity: false)
        mimeTypePool.removeAll(keepingCapacity: false)
        directoryPrefixPool.removeAll(keepingCapacity: false)

        for (ext, mime) in Self.predefinedMimeTypes {
            extensionPool[ext] = ext
            mimeTypePool[mime] = mime
        }
    }

    public var poolCounts: (paths: Int, extensions: Int, mimeTypes: Int, prefixes: Int) {
        return (pathPool.count, extensionPool.count, mimeTypePool.count, directoryPrefixPool.count)
    }
}

public typealias ArchiveEntryFlyweightFactory = ArchiveEntryMetadataPool

public struct ArchiveEntryMetadataState: Sendable, Equatable {
    public let path: String
    public let extensionName: String
    public let mimeType: String
    public let directoryPrefix: String

    public init(path: String) {
        self.path = path
        let ext = (path as NSString).pathExtension.lowercased()
        self.extensionName = ext
        self.mimeType = ArchiveMimeMapper.mimeType(forExtension: ext)
        let nsPath = path as NSString
        let dir = nsPath.deletingLastPathComponent
        if !dir.isEmpty && dir != "." {
            self.directoryPrefix = dir.hasSuffix("/") ? dir : "\(dir)/"
        } else {
            self.directoryPrefix = ""
        }
    }

    public init(
        path: String,
        extensionName: String,
        mimeType: String,
        directoryPrefix: String
    ) {
        self.path = path
        self.extensionName = extensionName
        self.mimeType = mimeType
        self.directoryPrefix = directoryPrefix
    }
}

public typealias ArchiveEntryFlyweightState = ArchiveEntryMetadataState

/// Fast static lock-free MIME type mapper table.
public enum ArchiveMimeMapper {
    @usableFromInline
    static let staticMimeMap: [String: String] = [
        "swift": "text/x-swift", "js": "application/javascript", "json": "application/json",
        "html": "text/html", "css": "text/css", "png": "image/png", "jpg": "image/jpeg",
        "jpeg": "image/jpeg", "gif": "image/gif", "svg": "image/svg+xml", "pdf": "application/pdf",
        "zip": "application/zip", "7z": "application/x-7z-compressed", "tar": "application/x-tar",
        "gz": "application/gzip", "zst": "application/zstd", "txt": "text/plain", "md": "text/markdown",
        "c": "text/x-c", "cpp": "text/x-c++", "h": "text/x-chdr", "py": "text/x-python",
        "rs": "text/x-rust", "go": "text/x-go", "xml": "application/xml", "mp3": "audio/mpeg",
        "mp4": "video/mp4", "mov": "video/quicktime"
    ]
    
    @inlinable
    public static func mimeType(forExtension ext: String) -> String {
        staticMimeMap[ext] ?? "application/octet-stream"
    }
}
