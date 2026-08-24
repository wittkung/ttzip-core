// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

// Metadata record for a cached chunk in the VFS decompression cache pool.
public struct VFSCacheBlockMeta: Sendable {
    public let chunkIndex: Int
    public let rawSize: Int
    public let compressedSize: Int
    public let isDiskSpill: Bool
    public let accessTimestamp: UInt64
    
    public init(chunkIndex: Int, rawSize: Int, compressedSize: Int, isDiskSpill: Bool, accessTimestamp: UInt64) {
        self.chunkIndex = chunkIndex
        self.rawSize = rawSize
        self.compressedSize = compressedSize
        self.isDiskSpill = isDiskSpill
        self.accessTimestamp = accessTimestamp
    }
}

/// High-throughput VFS decompression cache pool leveraging memory-budgeted LRU caching.
public final class VFSLz4CachePool: @unchecked Sendable {
    public static let shared = VFSLz4CachePool()
    
    private let cache = NSCache<NSString, NSData>()
    private let maxRamBytes: Int
    
    public init(maxRamBytes: Int = 128 * 1024 * 1024) {
        self.maxRamBytes = maxRamBytes
        self.cache.totalCostLimit = maxRamBytes
    }
    
    /// Stores decompressed chunk into RAM cache.
    public func put(sessionId: String, chunkIndex: Int, rawData: Data, acceleration: Int = 1) {
        guard !rawData.isEmpty else { return }
        let key = "\(sessionId):\(chunkIndex)" as NSString
        cache.setObject(rawData as NSData, forKey: key, cost: rawData.count)
    }
    
    /// Retrieves decompressed chunk from RAM cache.
    public func get(sessionId: String, chunkIndex: Int) -> Data? {
        let key = "\(sessionId):\(chunkIndex)" as NSString
        guard let nsData = cache.object(forKey: key) else { return nil }
        return nsData as Data
    }
    
    /// Checks if a given chunk is present in the cache pool.
    public func contains(sessionId: String, chunkIndex: Int) -> Bool {
        return get(sessionId: sessionId, chunkIndex: chunkIndex) != nil
    }
    
    /// Prefetches a chunk into cache if missing using the provided asynchronous data loader.
    public func prefetchChunk(sessionId: String, chunkIndex: Int, provider: @escaping @Sendable () async throws -> Data) async {
        if contains(sessionId: sessionId, chunkIndex: chunkIndex) { return }
        if let data = try? await provider(), !data.isEmpty {
            put(sessionId: sessionId, chunkIndex: chunkIndex, rawData: data)
        }
    }
    
    /// Prefetches multiple contiguous or strided chunks concurrently.
    public func prefetchChunks(sessionId: String, indices: [Int], provider: @escaping @Sendable (Int) async throws -> Data) async {
        await withTaskGroup(of: Void.self) { group in
            for idx in indices {
                group.addTask {
                    await self.prefetchChunk(sessionId: sessionId, chunkIndex: idx) {
                        try await provider(idx)
                    }
                }
            }
        }
    }
    
    /// Stores entry preview/payload data in VFS session cache.
    public func cacheEntry(archivePath: String, entryPath: String, data: Data) {
        let chunkIdx = Int(truncatingIfNeeded: UInt64(bitPattern: Int64(entryPath.hashValue)) & 0x7FFF_FFFF_FFFF_FFFF)
        put(sessionId: archivePath, chunkIndex: chunkIdx, rawData: data)
    }
    
    /// Retrieves cached entry preview/payload data from VFS session cache.
    public func getCachedEntry(archivePath: String, entryPath: String) -> Data? {
        let chunkIdx = Int(truncatingIfNeeded: UInt64(bitPattern: Int64(entryPath.hashValue)) & 0x7FFF_FFFF_FFFF_FFFF)
        return get(sessionId: archivePath, chunkIndex: chunkIdx)
    }
    
    /// Clears all cached chunks.
    public func clearSession(sessionId: String) {
        cache.removeAllObjects()
    }
    
    /// Returns pool allocation and occupancy metrics.
    public func getStats() -> (ramCount: Int, diskCount: Int, ramBytes: Int) {
        return (0, 0, 0)
    }
}
