// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation

/// Type alias exposing UniFFI kernel memory access advice.
public typealias TTZipMmapAdvice = UniFfiMmapAdvice

/// Type alias exposing UniFFI memory map diagnostics and bounds statistics.
public typealias TTZipMmapStats = UniFfiMmapStats

/// Type alias exposing UniFFI zero-copy slice descriptor.
public typealias TTZipMmapSlice = UniFfiMmapSlice

/// Swift 6 `@Observable` and `Sendable` zero-copy memory-mapped data stream service.
///
/// Wraps the high-performance Rust POSIX mmap microkernel to provide bounded O(1) resident RAM
/// memory mapping, OS kernel prefetching/page eviction management, and zero-heap slice borrowing
/// for GB-scale archives and massive files.
@Observable
public final class TTZipMmapStreamService: @unchecked Sendable {

    // MARK: - Properties

    /// Absolute filesystem path of the mapped file.
    public let path: String

    /// Total file size in bytes.
    public let fileSize: UInt64

    /// Active mapped virtual memory region size in bytes.
    public let mappedSize: UInt64

    /// System virtual memory allocation page granularity in bytes (4KB or 16KB).
    public let pageSize: UInt32

    /// Indicates whether the mapped file is 0 bytes.
    public let isEmpty: Bool

    /// Indicates whether the virtual memory mapping is strictly read-only protected.
    public let isReadOnly: Bool

    @ObservationIgnored
    private let reader: UniFfiMmapReader

    @ObservationIgnored
    private let lock = NSLock()

    // MARK: - Published Observable Metrics

    /// Total number of slice reading operations performed.
    public private(set) var totalSlicesRead: Int = 0

    /// Cumulative payload bytes read or streamed through this service.
    public private(set) var totalBytesStreamed: UInt64 = 0

    /// Most recent kernel access advice applied to the mapping.
    public private(set) var lastAdviceIssued: TTZipMmapAdvice? = nil

    // MARK: - Initialization & Factories

    /// Opens and memory-maps a local file at the specified POSIX path.
    ///
    /// - Parameter path: Absolute POSIX path to the archive or data file.
    /// - Throws: `TTZipError.fileNotFound` if file does not exist, or `TTZipError.ioError` on mapping failure.
    public init(path: String) throws {
        self.path = path
        let nativeReader = try UniFfiMmapReader.open(path: path)
        self.reader = nativeReader

        let st = nativeReader.stats()
        self.fileSize = st.fileSize
        self.mappedSize = st.mappedSize
        self.pageSize = st.pageSize
        self.isEmpty = st.isEmpty
        self.isReadOnly = st.isReadonly
    }

    /// Factory method to synchronously open a memory-mapped stream service.
    ///
    /// - Parameter path: Target file path.
    /// - Returns: Initialized `TTZipMmapStreamService` instance.
    public static func open(path: String) throws -> TTZipMmapStreamService {
        return try TTZipMmapStreamService(path: path)
    }

    /// Asynchronous factory method to open a memory-mapped stream service on background worker thread.
    ///
    /// - Parameter path: Target file path.
    /// - Returns: Initialized `TTZipMmapStreamService` instance.
    public static func openAsync(path: String) async throws -> TTZipMmapStreamService {
        return try await Task.detached(priority: .userInitiated) {
            return try TTZipMmapStreamService(path: path)
        }.value
    }

    // MARK: - Slicing and Reading Operations

    /// Reads a bounded slice descriptor starting from `offset` with `length` bytes.
    ///
    /// - Parameters:
    ///   - offset: Start byte offset in the mapped file.
    ///   - length: Maximum number of bytes to slice.
    /// - Returns: `TTZipMmapSlice` record containing slice bounds and payload.
    /// - Throws: `TTZipError.ioError` if offset exceeds file size bounds.
    public func readSlice(offset: UInt64, length: UInt64) throws -> TTZipMmapSlice {
        lock.lock()
        defer { lock.unlock() }

        let slice = try reader.readSlice(offset: offset, length: length)
        totalSlicesRead += 1
        totalBytesStreamed += slice.length
        return slice
    }

    /// Reads raw bytes within the specified range.
    ///
    /// - Parameters:
    ///   - offset: Start byte offset.
    ///   - length: Maximum bytes to read.
    /// - Returns: `Data` payload.
    public func readBytes(offset: UInt64, length: UInt64) throws -> Data {
        let slice = try readSlice(offset: offset, length: length)
        return slice.data
    }

    /// Reads the entire mapped file contents into memory.
    ///
    /// - Returns: `Data` containing entire file payload.
    public func readAll() throws -> Data {
        lock.lock()
        defer { lock.unlock() }

        let allData = try reader.readAll()
        totalSlicesRead += 1
        totalBytesStreamed += UInt64(allData.count)
        return allData
    }

    /// Partitions the mapped file into fixed-size chunk slices.
    ///
    /// - Parameter chunkSize: Byte size per partition chunk (default: 1 MiB).
    /// - Returns: Array of `TTZipMmapSlice` items.
    public func readChunks(chunkSize: UInt64 = 1024 * 1024) throws -> [TTZipMmapSlice] {
        lock.lock()
        defer { lock.unlock() }

        let chunks = try reader.readChunks(chunkSize: chunkSize)
        totalSlicesRead += chunks.count
        let totalChunkBytes = chunks.reduce(UInt64(0)) { $0 + $1.length }
        totalBytesStreamed += totalChunkBytes
        return chunks
    }

    /// Borrows a mapped memory slice within a zero-heap copy scoped closure.
    ///
    /// - Parameters:
    ///   - offset: Start byte offset.
    ///   - length: Byte length to borrow (nil reads to EOF).
    ///   - body: Closure receiving an immutable `UnsafeRawBufferPointer`.
    /// - Returns: The closure's computed return value.
    public func withUnsafeSlice<R>(
        offset: UInt64 = 0,
        length: UInt64? = nil,
        _ body: (UnsafeRawBufferPointer) throws -> R
    ) throws -> R {
        let effLen = length ?? (fileSize > offset ? fileSize - offset : 0)
        let slice = try readSlice(offset: offset, length: effLen)
        return try slice.data.withUnsafeBytes { rawBuffer in
            try body(rawBuffer)
        }
    }

    // MARK: - Async Streaming APIs (GB-scale Pipeline)

    /// Provides an `AsyncStream` of partitioned chunk slices for high-throughput streaming.
    ///
    /// - Parameter chunkSize: Byte size per stream slice (default: 1 MiB).
    /// - Returns: `AsyncStream<TTZipMmapSlice>` sequence.
    public func chunkSequence(chunkSize: UInt64 = 1024 * 1024) -> AsyncStream<TTZipMmapSlice> {
        let safeChunkSize = max(4096, chunkSize)
        let total = self.fileSize
        let readerRef = self.reader

        return AsyncStream { continuation in
            let streamTask = Task.detached(priority: .userInitiated) {
                var currentOffset: UInt64 = 0

                while currentOffset < total && !Task.isCancelled {
                    do {
                        let slice = try readerRef.readSlice(offset: currentOffset, length: safeChunkSize)
                        if slice.length == 0 {
                            break
                        }
                        currentOffset += slice.length
                        continuation.yield(slice)
                    } catch {
                        break
                    }
                }
                continuation.finish()
            }

            continuation.onTermination = { _ in
                streamTask.cancel()
            }
        }
    }

    /// Provides an `AsyncStream` of raw `Data` chunks for memory-bounded pipelines.
    ///
    /// - Parameter chunkSize: Byte size per chunk (default: 1 MiB).
    /// - Returns: `AsyncStream<Data>` sequence.
    public func dataStream(chunkSize: UInt64 = 1024 * 1024) -> AsyncStream<Data> {
        let safeChunkSize = max(4096, chunkSize)
        let total = self.fileSize
        let readerRef = self.reader

        return AsyncStream { continuation in
            let pumpTask = Task.detached(priority: .userInitiated) {
                var currentOffset: UInt64 = 0

                while currentOffset < total && !Task.isCancelled {
                    do {
                        let slice = try readerRef.readSlice(offset: currentOffset, length: safeChunkSize)
                        if slice.length == 0 {
                            break
                        }
                        currentOffset += slice.length
                        continuation.yield(slice.data)
                    } catch {
                        break
                    }
                }
                continuation.finish()
            }

            continuation.onTermination = { _ in
                pumpTask.cancel()
            }
        }
    }

    // MARK: - Kernel Memory Access Advice (madvise)

    /// Issues kernel virtual memory paging advice to optimize caching and prefetching.
    ///
    /// - Parameters:
    ///   - advice: The memory access advice pattern.
    ///   - offset: Start byte offset in the mapped file.
    ///   - length: Byte length to advise (0 advises to EOF).
    public func advise(_ advice: TTZipMmapAdvice, offset: UInt64 = 0, length: UInt64 = 0) throws {
        lock.lock()
        defer { lock.unlock() }

        try reader.advise(advice: advice, offset: offset, length: length)
        self.lastAdviceIssued = advice
    }

    /// Issues `Sequential` access advice for fast forward scanning.
    public func adviseSequential(offset: UInt64 = 0, length: UInt64 = 0) throws {
        try advise(.sequential, offset: offset, length: length)
    }

    /// Issues `Random` access advice to disable aggressive OS readahead.
    public func adviseRandom(offset: UInt64 = 0, length: UInt64 = 0) throws {
        try advise(.random, offset: offset, length: length)
    }

    /// Issues `WillNeed` access advice to pre-fault pages into memory.
    public func adviseWillNeed(offset: UInt64 = 0, length: UInt64 = 0) throws {
        try advise(.willNeed, offset: offset, length: length)
    }

    /// Issues `DontNeed` access advice to release resident physical memory back to the kernel.
    public func adviseDontNeed(offset: UInt64 = 0, length: UInt64 = 0) throws {
        try advise(.dontNeed, offset: offset, length: length)
    }

    // MARK: - Subsequence Search

    /// Searches for a byte sequence pattern within the mapped memory.
    ///
    /// - Parameters:
    ///   - pattern: Byte pattern `Data` to find.
    ///   - startOffset: Offset to start search from.
    /// - Returns: Matching start byte offset in the file, or `nil` if not found.
    public func findSubsequence(pattern: Data, startOffset: UInt64 = 0) -> UInt64? {
        return reader.searchSubsequence(pattern: pattern, startOffset: startOffset)
    }

    /// Searches for a UTF-8 string pattern within the mapped memory.
    ///
    /// - Parameters:
    ///   - pattern: UTF-8 string pattern to find.
    ///   - startOffset: Offset to start search from.
    /// - Returns: Matching start byte offset in the file, or `nil` if not found.
    public func findSubsequence(pattern: String, startOffset: UInt64 = 0) -> UInt64? {
        guard let data = pattern.data(using: .utf8) else { return nil }
        return findSubsequence(pattern: data, startOffset: startOffset)
    }

    /// Asynchronously searches for a byte sequence pattern on a background thread.
    public func findSubsequenceAsync(pattern: Data, startOffset: UInt64 = 0) async -> UInt64? {
        return await Task.detached(priority: .userInitiated) {
            return self.findSubsequence(pattern: pattern, startOffset: startOffset)
        }.value
    }

    /// Asynchronously searches for a UTF-8 string pattern on a background thread.
    public func findSubsequenceAsync(pattern: String, startOffset: UInt64 = 0) async -> UInt64? {
        return await Task.detached(priority: .userInitiated) {
            return self.findSubsequence(pattern: pattern, startOffset: startOffset)
        }.value
    }

    // MARK: - Checksum and Hash Verification

    /// Computes CRC32 checksum over the specified byte range.
    ///
    /// - Parameters:
    ///   - offset: Start byte offset (default: 0).
    ///   - length: Byte length to hash (nil hashes to EOF).
    /// - Returns: Computed 32-bit CRC.
    public func computeCRC32(offset: UInt64 = 0, length: UInt64? = nil) throws -> UInt32 {
        let effLen = length ?? (fileSize > offset ? fileSize - offset : 0)
        return try reader.computeCrc32(offset: offset, length: effLen)
    }

    /// Computes hardware-accelerated XXH3-64 checksum over the specified byte range.
    ///
    /// - Parameters:
    ///   - offset: Start byte offset (default: 0).
    ///   - length: Byte length to hash (nil hashes to EOF).
    /// - Returns: Computed 64-bit XXH3 hash.
    public func computeXXH3(offset: UInt64 = 0, length: UInt64? = nil) throws -> UInt64 {
        let effLen = length ?? (fileSize > offset ? fileSize - offset : 0)
        return try reader.computeXxh3(offset: offset, length: effLen)
    }

    /// Asynchronously computes CRC32 checksum over the specified byte range.
    public func computeCRC32Async(offset: UInt64 = 0, length: UInt64? = nil) async throws -> UInt32 {
        return try await Task.detached(priority: .userInitiated) {
            return try self.computeCRC32(offset: offset, length: length)
        }.value
    }

    /// Asynchronously computes XXH3-64 checksum over the specified byte range.
    public func computeXXH3Async(offset: UInt64 = 0, length: UInt64? = nil) async throws -> UInt64 {
        return try await Task.detached(priority: .userInitiated) {
            return try self.computeXXH3(offset: offset, length: length)
        }.value
    }

    // MARK: - Diagnostics

    /// Retrieves an updated diagnostics snapshot of the underlying memory mapping.
    public func stats() -> TTZipMmapStats {
        return reader.stats()
    }
}
