// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation
import Observation

/// Type alias exposing UniFFI kernel memory access advice.
public typealias TTZipMmapAdvice = UniFfiMmapAdvice

/// Type alias exposing UniFFI memory map diagnostics and bounds statistics.
public typealias TTZipMmapStats = UniFfiMmapStats

/// Type alias exposing UniFFI slice descriptor.
public typealias TTZipMmapSlice = UniFfiMmapSlice

/// Swift 6 `@Observable` and `Sendable` zero-copy memory-mapped data stream service.
///
/// Wraps the high-performance Rust POSIX mmap microkernel (`UniFfiMmapReader`) to provide bounded O(1) resident RAM
/// memory mapping, OS kernel prefetching/page eviction management, and safe scoped memory access
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

    /// Opens and memory-maps a local file at the specified POSIX path via the Rust microkernel.
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
    /// The returned `TTZipMmapSlice` contains memory-managed payload data safely copied from the microkernel,
    /// preventing dangling pointers upon service deallocation.
    ///
    /// - Parameters:
    ///   - offset: Start byte offset in the mapped file.
    ///   - length: Maximum number of bytes to slice.
    /// - Returns: `TTZipMmapSlice` record containing slice bounds and payload data.
    /// - Throws: `TTZipError.ioError` if offset exceeds file size bounds.
    public func readSlice(offset: UInt64, length: UInt64) throws -> TTZipMmapSlice {
        lock.lock()
        defer { lock.unlock() }

        guard offset <= fileSize else {
            throw TtZipError.IoError(message: "Offset \(offset) out of bounds for size \(fileSize)")
        }

        let slice = try reader.readSlice(offset: offset, length: length)
        totalSlicesRead += 1
        totalBytesStreamed += slice.length
        return slice
    }

    /// Reads a bounded data payload starting from signed `offset` with integer `length` bytes.
    ///
    /// Returns safe, memory-managed `Data` payload backed by ARC storage, eliminating wild pointer risks.
    ///
    /// - Parameters:
    ///   - offset: Start byte offset in the mapped file.
    ///   - length: Number of bytes to slice.
    /// - Returns: Safe `Data` payload.
    /// - Throws: `TTZipError.ioError` if offset is negative, out of bounds, or on I/O failure.
    public func readSlice(at offset: Int64, length: Int) throws -> Data {
        guard offset >= 0 && length >= 0 else {
            throw TtZipError.IoError(message: "Offset and length must be non-negative")
        }
        let slice = try readSlice(offset: UInt64(offset), length: UInt64(length))
        return slice.data
    }

    /// Reads a true zero-copy data view over the specified memory-mapped range.
    ///
    /// Holds a strong reference to the underlying `UniFfiMmapReader` inside the `Data` deallocator,
    /// guaranteeing safe zero-heap-allocation memory mapping without any buffer copying across the FFI boundary.
    ///
    /// - Parameters:
    ///   - offset: Start byte offset in the mapped file.
    ///   - length: Maximum byte count to slice.
    /// - Returns: True zero-copy `Data` view over the virtual memory region.
    /// - Throws: `TtZipError.IoError` if offset exceeds file bounds or virtual address is invalid.
    public func readZeroCopy(offset: UInt64, length: UInt64) throws -> Data {
        lock.lock()
        defer { lock.unlock() }

        guard offset <= fileSize else {
            throw TtZipError.IoError(message: "Offset \(offset) out of bounds for size \(fileSize)")
        }

        let data = try reader.dataNoCopy(offset: offset, length: length)
        totalSlicesRead += 1
        totalBytesStreamed += UInt64(data.count)
        return data
    }

    /// Reads raw bytes within the specified range.
    ///
    /// - Parameters:
    ///   - offset: Start byte offset.
    ///   - length: Maximum bytes to read.
    /// - Returns: Safe `Data` payload.
    public func readBytes(offset: UInt64, length: UInt64) throws -> Data {
        lock.lock()
        defer { lock.unlock() }

        guard offset <= fileSize else {
            throw TtZipError.IoError(message: "Offset \(offset) out of bounds for size \(fileSize)")
        }

        let data = try reader.readBytes(offset: offset, length: length)
        totalSlicesRead += 1
        totalBytesStreamed += UInt64(data.count)
        return data
    }

    /// Reads the entire mapped file contents into memory.
    ///
    /// - Returns: `Data` containing entire file payload.
    public func readAll() throws -> Data {
        lock.lock()
        defer { lock.unlock() }

        let data = try reader.readAll()
        totalSlicesRead += 1
        totalBytesStreamed += UInt64(data.count)
        return data
    }

    /// Partitions the mapped file into fixed-size chunk slices.
    ///
    /// - Parameter chunkSize: Byte size per partition chunk (default: 1 MiB).
    /// - Returns: Array of `TTZipMmapSlice` items.
    public func readChunks(chunkSize: UInt64 = 1024 * 1024) throws -> [TTZipMmapSlice] {
        lock.lock()
        defer { lock.unlock() }

        guard chunkSize > 0 else {
            throw TtZipError.IoError(message: "Chunk size must be greater than zero")
        }

        let chunks = try reader.readChunks(chunkSize: chunkSize)
        totalSlicesRead += chunks.count
        totalBytesStreamed += chunks.reduce(0) { $0 + $1.length }
        return chunks
    }

    /// Borrows a mapped memory slice within a scoped closure.
    ///
    /// Guarantees that the underlying buffer pointer remains 100% valid and bounds-checked
    /// for the exact duration of the closure execution.
    ///
    /// - Parameters:
    ///   - offset: Start byte offset in the mapped file.
    ///   - length: Byte length to borrow.
    ///   - body: Closure receiving an immutable `UnsafeRawBufferPointer`.
    /// - Returns: The closure's computed return value.
    public func withSlice<R>(
        at offset: Int64,
        length: Int,
        _ body: (UnsafeRawBufferPointer) throws -> R
    ) throws -> R {
        guard offset >= 0 && length >= 0 else {
            throw TtZipError.IoError(message: "Offset and length must be non-negative")
        }
        return try withUnsafeSlice(offset: UInt64(offset), length: UInt64(length), body)
    }

    /// Borrows a mapped memory slice within a scoped closure.
    ///
    /// - Parameters:
    ///   - offset: Start byte offset (default: 0).
    ///   - length: Byte length to borrow (nil reads to EOF).
    ///   - body: Closure receiving an immutable `UnsafeRawBufferPointer`.
    /// - Returns: The closure's computed return value.
    public func withUnsafeSlice<R>(
        offset: UInt64 = 0,
        length: UInt64? = nil,
        _ body: (UnsafeRawBufferPointer) throws -> R
    ) throws -> R {
        guard offset <= fileSize else {
            throw TtZipError.IoError(message: "Offset \(offset) out of bounds for size \(fileSize)")
        }

        let effLen = min(length ?? (fileSize - offset), fileSize - offset)
        if effLen == 0 {
            return try body(UnsafeRawBufferPointer(start: nil, count: 0))
        }

        let data = try readBytes(offset: offset, length: effLen)
        return try data.withUnsafeBytes { rawBuffer in
            try body(rawBuffer)
        }
    }

    // MARK: - Async Streaming APIs (GB-scale Pipeline)

    private final class MmapStreamCursor: @unchecked Sendable {
        private let lock = NSLock()
        private var offset: UInt64 = 0
        private let total: UInt64
        private let chunkSize: UInt64

        init(total: UInt64, chunkSize: UInt64) {
            self.total = total
            self.chunkSize = Swift.max(4096, chunkSize)
        }

        func nextRange() -> (offset: UInt64, length: UInt64)? {
            lock.lock()
            defer { lock.unlock() }
            guard offset < total else { return nil }
            let current = offset
            let length = min(chunkSize, total - current)
            offset += length
            return (offset: current, length: length)
        }
    }

    /// Provides a pull-based `AsyncThrowingStream` of slice chunks for memory-bounded streaming pipelines.
    ///
    /// Chunks are produced strictly on demand when the consumer requests the next element,
    /// eliminating unbounded producer-consumer buffering overflow and propagating I/O errors immediately.
    ///
    /// - Parameter chunkSize: Byte size per stream slice (default: 1 MiB).
    /// - Returns: `AsyncThrowingStream<TTZipMmapSlice, Error>` sequence.
    public func streamChunks(chunkSize: UInt64 = 1024 * 1024) -> AsyncThrowingStream<TTZipMmapSlice, Error> {
        let cursor = MmapStreamCursor(total: self.fileSize, chunkSize: chunkSize)

        return AsyncThrowingStream {
            guard !Task.isCancelled else {
                throw CancellationError()
            }
            guard let range = cursor.nextRange() else {
                return nil
            }
            let slice = try self.readSlice(offset: range.offset, length: range.length)
            guard slice.length > 0 else {
                return nil
            }
            return slice
        }
    }

    /// Provides a pull-based `AsyncStream` of partitioned chunk slices for high-throughput streaming.
    ///
    /// Elements are fetched strictly on demand when consumed, guaranteeing bounded resident memory.
    ///
    /// - Parameter chunkSize: Byte size per stream slice (default: 1 MiB).
    /// - Returns: `AsyncStream<TTZipMmapSlice>` sequence.
    public func chunkSequence(chunkSize: UInt64 = 1024 * 1024) -> AsyncStream<TTZipMmapSlice> {
        let cursor = MmapStreamCursor(total: self.fileSize, chunkSize: chunkSize)

        return AsyncStream {
            guard !Task.isCancelled else {
                return nil
            }
            guard let range = cursor.nextRange() else {
                return nil
            }
            do {
                let slice = try self.readSlice(offset: range.offset, length: range.length)
                guard slice.length > 0 else {
                    return nil
                }
                return slice
            } catch {
                return nil
            }
        }
    }

    /// Provides a pull-based `AsyncStream` of raw `Data` chunks for memory-bounded pipelines.
    ///
    /// Pull-based on-demand generation eliminates producer-consumer buffering overflow.
    ///
    /// - Parameter chunkSize: Byte size per chunk (default: 1 MiB).
    /// - Returns: `AsyncStream<Data>` sequence.
    public func dataStream(chunkSize: UInt64 = 1024 * 1024) -> AsyncStream<Data> {
        let cursor = MmapStreamCursor(total: self.fileSize, chunkSize: chunkSize)

        return AsyncStream {
            guard !Task.isCancelled else {
                return nil
            }
            guard let range = cursor.nextRange() else {
                return nil
            }
            do {
                let slice = try self.readSlice(offset: range.offset, length: range.length)
                guard slice.length > 0 else {
                    return nil
                }
                return slice.data
            } catch {
                return nil
            }
        }
    }

    /// Returns a pull-based `AsyncSequence` conforming type that yields slices on demand with strict backpressure and error propagation.
    public func pullChunks(chunkSize: UInt64 = 1024 * 1024) -> PullChunkSequence {
        PullChunkSequence(service: self, chunkSize: chunkSize)
    }

    // MARK: - Kernel Memory Access Advice (madvise)

    /// Issues kernel virtual memory paging advice to optimize caching and prefetching via the Rust microkernel.
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

// MARK: - PullChunkSequence

/// Pull-based on-demand asynchronous sequence of memory-mapped slices.
public struct PullChunkSequence: AsyncSequence, Sendable {
    public typealias Element = TTZipMmapSlice

    public struct AsyncIterator: AsyncIteratorProtocol {
        private let service: TTZipMmapStreamService
        private let chunkSize: UInt64
        private var currentOffset: UInt64 = 0

        init(service: TTZipMmapStreamService, chunkSize: UInt64) {
            self.service = service
            self.chunkSize = chunkSize
        }

        public mutating func next() async throws -> TTZipMmapSlice? {
            guard !Task.isCancelled else {
                throw CancellationError()
            }
            guard currentOffset < service.fileSize else {
                return nil
            }
            let slice = try service.readSlice(offset: currentOffset, length: chunkSize)
            guard slice.length > 0 else {
                return nil
            }
            currentOffset += slice.length
            return slice
        }
    }

    private let service: TTZipMmapStreamService
    private let chunkSize: UInt64

    public init(service: TTZipMmapStreamService, chunkSize: UInt64) {
        self.service = service
        self.chunkSize = Swift.max(4096, chunkSize)
    }

    public func makeAsyncIterator() -> AsyncIterator {
        AsyncIterator(service: service, chunkSize: chunkSize)
    }
}

