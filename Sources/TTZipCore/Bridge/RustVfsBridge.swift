// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation
import CTTZipBridge

/// High-performance Safe Rust Unified VFS Bridge.
public enum RustVfsBridge {
    
    /// Builds a native VFS tree handle from flat ArchiveEntry list.
    public static func withTreeHandle<R>(entries: [ArchiveEntry], rootName: String = "", block: (OpaquePointer) throws -> R) rethrows -> R? {
        let cPathPointers: [UnsafeMutablePointer<CChar>?] = entries.map { strdup($0.path) }
        defer {
            for ptr in cPathPointers {
                if let ptr = ptr { free(ptr) }
            }
        }
        
        var rawEntries: [TTZipEntryMetadata] = []
        rawEntries.reserveCapacity(entries.count)
        
        for (i, entry) in entries.enumerated() {
            let mtime = entry.modificationDate.map { Int64($0.timeIntervalSince1970) } ?? 0
            rawEntries.append(TTZipEntryMetadata(
                path: cPathPointers[i].map { UnsafePointer($0) },
                uncompressed_size: UInt64(max(0, entry.uncompressedSize)),
                compressed_size: 0,
                crc32: 0,
                mtime_epoch_secs: mtime,
                mode: entry.isDirectory ? 0o755 : 0o644,
                is_directory: entry.isDirectory,
                is_encrypted: entry.isEncrypted,
                compression_method: 0,
                detected_encoding: nil
            ))
        }
        
        let handle: OpaquePointer? = rootName.withCString { rPtr in
            rawEntries.withUnsafeBufferPointer { ePtr in
                ttzip_rust_vfs_tree_build(ePtr.baseAddress, ePtr.count, rPtr)
            }
        }
        
        guard let validHandle = handle else { return nil }
        defer { ttzip_rust_vfs_tree_free(validHandle) }
        return try block(validHandle)
    }
    
    /// Renders ASCII/Unicode hierarchical tree using Safe Rust VFS engine.
    public static func renderTree(from entries: [ArchiveEntry], rootName: String = "") -> String {
        return withTreeHandle(entries: entries, rootName: rootName) { handle in
            var outPtr: UnsafeMutablePointer<CChar>? = nil
            let status = ttzip_rust_vfs_tree_render(handle, &outPtr)
            guard status == TTZIP_STATUS_OK, let ptr = outPtr else { return "" }
            defer { ttzip_rust_vfs_free_string(ptr) }
            return String(cString: ptr)
        } ?? ""
    }
    
    /// Performs fast fuzzy search against archive entries using Safe Rust VFS engine.
    public static func fuzzySearch(in entries: [ArchiveEntry], query: String) -> [ArchiveEntry] {
        guard !query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return entries
        }
        guard let session = RustVfsSession(entries: entries) else { return entries }
        return session.fuzzySearch(query: query)
    }
    
    /// Retrieves aggregated VFS statistics (total files, directories, uncompressed size).
    public static func getStats(from entries: [ArchiveEntry], rootName: String = "") -> (totalFiles: UInt64, totalDirs: UInt64, totalSize: UInt64)? {
        return withTreeHandle(entries: entries, rootName: rootName) { handle in
            var totalFiles: UInt64 = 0
            var totalDirs: UInt64 = 0
            var totalSize: UInt64 = 0
            ttzip_rust_vfs_tree_get_stats(handle, &totalFiles, &totalDirs, &totalSize)
            return (totalFiles, totalDirs, totalSize)
        }
    }
}
