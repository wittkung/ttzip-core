// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation
import CTTZipBridge

/// Persistent Safe Rust VFS Tree session with cached lookups and zero per-keystroke allocations.
public final class RustVfsSession: @unchecked Sendable {
    private let handle: OpaquePointer
    private let entryMap: [String: ArchiveEntry]
    public let allEntries: [ArchiveEntry]
    
    public init?(entries: [ArchiveEntry], rootName: String = "") {
        guard !entries.isEmpty else { return nil }
        
        self.allEntries = entries
        var map: [String: ArchiveEntry] = [:]
        map.reserveCapacity(entries.count)
        
        for entry in entries {
            map[entry.path] = entry
        }
        self.entryMap = map
        
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
        
        let builtHandle: OpaquePointer? = rootName.withCString { rPtr in
            rawEntries.withUnsafeBufferPointer { ePtr in
                ttzip_rust_vfs_tree_build(ePtr.baseAddress, ePtr.count, rPtr)
            }
        }
        
        guard let validHandle = builtHandle else { return nil }
        self.handle = validHandle
    }
    
    deinit {
        ttzip_rust_vfs_tree_free(handle)
    }
    
    /// Fast read-only fuzzy search directly reusing persistent VFS Tree handle without reallocating nodes.
    public func fuzzySearch(query: String) -> [ArchiveEntry] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return allEntries
        }
        
        final class SearchAccumulator {
            var matchedPaths: [String] = []
        }
        let acc = SearchAccumulator()
        let accPtr = Unmanaged.passUnretained(acc).toOpaque()
        
        let queryCStr = trimmed.utf8CString
        _ = queryCStr.withUnsafeBufferPointer { qPtr in
            ttzip_rust_vfs_fuzzy_search(handle, qPtr.baseAddress, { resultPtr, ctx in
                guard let resultPtr = resultPtr, let ctx = ctx else { return false }
                let raw = resultPtr.pointee
                if let pathC = raw.path {
                    let path = String(cString: pathC)
                    let accumulator = Unmanaged<SearchAccumulator>.fromOpaque(ctx).takeUnretainedValue()
                    accumulator.matchedPaths.append(path)
                }
                return true
            }, accPtr)
        }
        
        return acc.matchedPaths.compactMap { entryMap[$0] }
    }
    
    /// Zero-heap allocation fuzzy search into fixed-capacity pre-allocated buffer.
    public func searchZeroAlloc(query: String, maxResults: Int = 64) -> [ArchiveEntry] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return allEntries }
        
        var buffer = [TTZipVfsMatchDto](repeating: TTZipVfsMatchDto(
            name: nil,
            name_len: 0,
            path: nil,
            path_len: 0,
            uncompressed_size: 0,
            compressed_size: 0,
            crc32: 0,
            score: 0,
            is_directory: false,
            is_encrypted: false
        ), count: maxResults)
        
        let count: Int32 = trimmed.withCString { qPtr in
            buffer.withUnsafeMutableBufferPointer { bPtr in
                ttzip_rust_vfs_search_zero_alloc(handle, qPtr, bPtr.baseAddress, Int32(bPtr.count))
            }
        }
        guard count > 0 else { return [] }
        
        var matches: [ArchiveEntry] = []
        matches.reserveCapacity(Int(count))
        for i in 0..<Int(count) {
            let item = buffer[i]
            if let p = item.path, item.path_len > 0 {
                let pathBytes = UnsafeRawBufferPointer(start: p, count: item.path_len)
                let pathStr = String(decoding: pathBytes, as: UTF8.self)
                if let entry = entryMap[pathStr] {
                    matches.append(entry)
                }
            }
        }
        return matches
    }
    
    /// Renders ASCII/Unicode tree from persistent VFS session.
    public func renderTree() -> String {
        var outPtr: UnsafeMutablePointer<CChar>? = nil
        let status = ttzip_rust_vfs_tree_render(handle, &outPtr)
        guard status == TTZIP_STATUS_OK, let ptr = outPtr else { return "" }
        defer { ttzip_rust_vfs_free_string(ptr) }
        return String(cString: ptr)
    }
    
    /// Returns aggregated statistics from persistent VFS tree.
    public func getStats() -> (totalFiles: UInt64, totalDirs: UInt64, totalSize: UInt64) {
        var totalFiles: UInt64 = 0
        var totalDirs: UInt64 = 0
        var totalSize: UInt64 = 0
        ttzip_rust_vfs_tree_get_stats(handle, &totalFiles, &totalDirs, &totalSize)
        return (totalFiles, totalDirs, totalSize)
    }
}
