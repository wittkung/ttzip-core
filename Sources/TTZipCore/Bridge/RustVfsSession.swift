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
        
        var packedUtf8: [UInt8] = []
        packedUtf8.reserveCapacity(entries.count * 32)
        var pathOffsets: [UInt32] = []
        pathOffsets.reserveCapacity(entries.count)
        var pathLens: [UInt32] = []
        pathLens.reserveCapacity(entries.count)
        var uncompressedSizes: [UInt64] = []
        uncompressedSizes.reserveCapacity(entries.count)
        var compressedSizes: [UInt64] = []
        compressedSizes.reserveCapacity(entries.count)
        var crc32s: [UInt32] = []
        crc32s.reserveCapacity(entries.count)
        var mtimes: [Int64] = []
        mtimes.reserveCapacity(entries.count)
        var modes: [UInt32] = []
        modes.reserveCapacity(entries.count)
        var flags: [UInt8] = []
        flags.reserveCapacity(entries.count)

        for entry in entries {
            map[entry.path] = entry
            
            let utf8 = Array(entry.path.utf8)
            let offset = UInt32(packedUtf8.count)
            let len = UInt32(utf8.count)
            packedUtf8.append(contentsOf: utf8)
            pathOffsets.append(offset)
            pathLens.append(len)
            
            uncompressedSizes.append(UInt64(max(0, entry.uncompressedSize)))
            compressedSizes.append(0)
            crc32s.append(0)
            let mtime = entry.modificationDate.map { Int64($0.timeIntervalSince1970) } ?? 0
            mtimes.append(mtime)
            modes.append(entry.isDirectory ? 0o755 : 0o644)
            var flag: UInt8 = 0
            if entry.isDirectory { flag |= 1 }
            if entry.isEncrypted { flag |= 2 }
            flags.append(flag)
        }
        self.entryMap = map
        
        let builtHandle: OpaquePointer? = rootName.withCString { rPtr in
            packedUtf8.withUnsafeBufferPointer { uPtr in
                pathOffsets.withUnsafeBufferPointer { offPtr in
                    pathLens.withUnsafeBufferPointer { lenPtr in
                        uncompressedSizes.withUnsafeBufferPointer { uSizePtr in
                            compressedSizes.withUnsafeBufferPointer { cSizePtr in
                                crc32s.withUnsafeBufferPointer { crcPtr in
                                    mtimes.withUnsafeBufferPointer { mtimePtr in
                                        modes.withUnsafeBufferPointer { modePtr in
                                            flags.withUnsafeBufferPointer { flagPtr in
                                                var packed = TTZipPackedEntryArray(
                                                    utf8_bytes: uPtr.baseAddress,
                                                    total_bytes_len: uPtr.count,
                                                    path_offsets: offPtr.baseAddress,
                                                    path_lens: lenPtr.baseAddress,
                                                    uncompressed_sizes: uSizePtr.baseAddress,
                                                    compressed_sizes: cSizePtr.baseAddress,
                                                    crc32s: crcPtr.baseAddress,
                                                    mtimes: mtimePtr.baseAddress,
                                                    modes: modePtr.baseAddress,
                                                    flags: flagPtr.baseAddress,
                                                    count: entries.count
                                                )
                                                return ttzip_rust_vfs_tree_build_packed(&packed, rPtr)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
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
    
    /// Retrieves a windowed slice of child nodes for interactive zero-copy UI directory paging.
    public func getChildren(dirNodeId: UInt32 = 0, offset: Int = 0, limit: Int = 100) -> (nodes: [TTZipVfsNodeSummary], total: Int) {
        var buffer = [TTZipVfsNodeSummary](repeating: TTZipVfsNodeSummary(
            node_id: 0,
            name_utf8: nil,
            name_len: 0,
            uncompressed_size: 0,
            compressed_size: 0,
            crc32: 0,
            mtime_epoch_secs: 0,
            mode: 0,
            is_directory: false,
            is_encrypted: false,
            has_children: false
        ), count: limit)
        
        var count: Int = 0
        var totalInDir: Int = 0
        
        let status = buffer.withUnsafeMutableBufferPointer { bPtr in
            ttzip_rust_vfs_get_children(handle, dirNodeId, offset, limit, bPtr.baseAddress, &count, &totalInDir)
        }
        
        guard status == TTZIP_STATUS_OK else { return ([], 0) }
        return (Array(buffer.prefix(count)), totalInDir)
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
