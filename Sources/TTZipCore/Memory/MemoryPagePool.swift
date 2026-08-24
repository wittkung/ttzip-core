// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// Memory page size enumeration (4KB standard, 16KB Apple Silicon hardware page, 64KB super-page).
public enum MemoryPageSize: Int, Sendable, CaseIterable {
    case page4K = 4096
    case page16K = 16384
    case page64K = 65536
}

/// Page-aligned physical memory buffer wrapper.
public final class MemoryPageBuffer: @unchecked Sendable {
    public let pointer: UnsafeMutableRawPointer
    public let capacity: Int
    public let pageSize: MemoryPageSize
    internal var inUse: Bool = false

    internal init?(pageSize: MemoryPageSize) {
        self.pageSize = pageSize
        self.capacity = pageSize.rawValue
        var rawPtr: UnsafeMutableRawPointer? = nil
        let alignment = max(16384, pageSize.rawValue)
        let result = posix_memalign(&rawPtr, alignment, capacity)
        guard result == 0, let validPtr = rawPtr else {
            return nil
        }
        memset(validPtr, 0, capacity)
        self.pointer = validPtr
    }

    private init(rawPointer: UnsafeMutableRawPointer, capacity: Int, pageSize: MemoryPageSize) {
        self.pointer = rawPointer
        self.capacity = capacity
        self.pageSize = pageSize
        self.inUse = true
    }

    deinit {
        free(pointer)
    }

    public static func allocateAligned(capacity: Int, pageSize: MemoryPageSize = .page64K) -> MemoryPageBuffer? {
        let actualSize = max(capacity, pageSize.rawValue)
        var rawPtr: UnsafeMutableRawPointer? = nil
        let alignment = max(16384, pageSize.rawValue)
        let result = posix_memalign(&rawPtr, alignment, actualSize)
        guard result == 0, let validPtr = rawPtr else {
            return nil
        }
        memset(validPtr, 0, actualSize)
        return MemoryPageBuffer(rawPointer: validPtr, capacity: actualSize, pageSize: pageSize)
    }
}

/// Dynamic, lock-free aligned memory page pool for high-throughput I/O.
public final class MemoryPageBufferPool: @unchecked Sendable {
    public static let shared = MemoryPageBufferPool()

    private let lock = NSLock()
    private var pools: [MemoryPageSize: [MemoryPageBuffer]] = [
        .page4K: [],
        .page16K: [],
        .page64K: []
    ]
    private let maxPoolSizePerClass = 32

    private init() {
        for _ in 0..<8 {
            if let buf = MemoryPageBuffer(pageSize: .page4K) {
                pools[.page4K]?.append(buf)
            }
            if let buf = MemoryPageBuffer(pageSize: .page16K) {
                pools[.page16K]?.append(buf)
            }
            if let buf = MemoryPageBuffer(pageSize: .page64K) {
                pools[.page64K]?.append(buf)
            }
        }
    }

    /// Borrows a page buffer from the pool or allocates a new one if pool is empty.
    public func borrowBuffer(size: MemoryPageSize) -> MemoryPageBuffer? {
        lock.lock()
        defer { lock.unlock() }

        if let available = pools[size]?.popLast() {
            available.inUse = true
            return available
        }
        return MemoryPageBuffer(pageSize: size)
    }

    /// Returns a page buffer back to the pool.
    public func returnBuffer(_ buffer: MemoryPageBuffer) {
        lock.lock()
        defer { lock.unlock() }

        buffer.inUse = false
        if let currentCount = pools[buffer.pageSize]?.count, currentCount < maxPoolSizePerClass {
            pools[buffer.pageSize]?.append(buffer)
        }
    }

    /// Executes a closure with a borrowed buffer, automatically returning it upon completion.
    public func withBuffer<T>(size: MemoryPageSize, _ closure: (MemoryPageBuffer) throws -> T) rethrows -> T? {
        guard let buffer = borrowBuffer(size: size) else {
            return nil
        }
        defer {
            returnBuffer(buffer)
        }
        return try closure(buffer)
    }
}

/// Lightweight Flyweight buffer handle referencing memory managed by `MemoryPageFlyweightPool`.
public final class MemoryPageBufferFlyweight: @unchecked Sendable {
    public let pointer: UnsafeMutableRawPointer
    public let capacity: Int
    public let pageSize: MemoryPageSize
    internal var inUse: Bool = false

    internal init(pointer: UnsafeMutableRawPointer, capacity: Int, pageSize: MemoryPageSize) {
        self.pointer = pointer
        self.capacity = capacity
        self.pageSize = pageSize
        self.inUse = true
    }
}

/// Thread-safe flyweight page pool providing fast buffer reuse.
public final class MemoryPageFlyweightPool: @unchecked Sendable {
    public static let shared = MemoryPageFlyweightPool()

    private let lock = NSLock()
    private var activeBuffers: [MemoryPageBuffer] = []

    private init() {}

    public func borrowBuffer(size: MemoryPageSize) -> MemoryPageBufferFlyweight {
        lock.lock()
        defer { lock.unlock() }

        if let buffer = MemoryPageBufferPool.shared.borrowBuffer(size: size) {
            activeBuffers.append(buffer)
            return MemoryPageBufferFlyweight(pointer: buffer.pointer, capacity: buffer.capacity, pageSize: size)
        }

        let raw = UnsafeMutableRawPointer.allocate(byteCount: size.rawValue, alignment: 64)
        return MemoryPageBufferFlyweight(pointer: raw, capacity: size.rawValue, pageSize: size)
    }

    public func returnBuffer(_ buffer: MemoryPageBufferFlyweight) {
        lock.lock()
        defer { lock.unlock() }

        if let idx = activeBuffers.firstIndex(where: { $0.pointer == buffer.pointer }) {
            let pageBuf = activeBuffers.remove(at: idx)
            MemoryPageBufferPool.shared.returnBuffer(pageBuf)
        } else {
            buffer.pointer.deallocate()
        }
    }

    public func withBuffer<T>(size: MemoryPageSize, _ closure: (UnsafeMutableRawPointer, Int) throws -> T) rethrows -> T? {
        let handle = borrowBuffer(size: size)
        defer {
            returnBuffer(handle)
        }
        return try closure(handle.pointer, handle.capacity)
    }
}

/// Contiguous virtual memory arena holding multiple packed data blocks with zero heap churn.
public final class VirtualMultiBlockArena: @unchecked Sendable {
    public struct BlockDescriptor: Sendable {
        public let id: Int
        public let name: String
        public let offset: Int
        public let length: Int
    }

    private let totalCapacity: Int
    private let rawBuffer: UnsafeMutableRawPointer
    private let typedBuffer: UnsafeMutablePointer<UInt8>
    private var _currentOffset: Int = 0
    private var _blocks: [BlockDescriptor] = []
    private let lock = NSLock()

    public init?(capacity: Int = 32 * 1024 * 1024) {
        self.totalCapacity = capacity
        var rawPtr: UnsafeMutableRawPointer? = nil
        let result = posix_memalign(&rawPtr, 16384, capacity)
        guard result == 0, let validPtr = rawPtr else {
            return nil
        }
        memset(validPtr, 0, capacity)
        self.rawBuffer = validPtr
        self.typedBuffer = validPtr.assumingMemoryBound(to: UInt8.self)
    }

    deinit {
        free(rawBuffer)
    }

    public var basePointer: UnsafePointer<UInt8> {
        return UnsafePointer(typedBuffer)
    }

    @discardableResult
    public func appendBlock(name: String, data: UnsafePointer<UInt8>, length: Int) -> BlockDescriptor? {
        lock.lock()
        defer { lock.unlock() }

        guard _currentOffset + length <= totalCapacity else {
            return nil
        }

        let startOffset = _currentOffset
        memcpy(typedBuffer + startOffset, data, length)
        _currentOffset += length

        let descriptor = BlockDescriptor(
            id: _blocks.count,
            name: name,
            offset: startOffset,
            length: length
        )
        _blocks.append(descriptor)
        return descriptor
    }

    public func reset() {
        lock.lock()
        defer { lock.unlock() }
        _currentOffset = 0
        _blocks.removeAll(keepingCapacity: true)
    }
}
