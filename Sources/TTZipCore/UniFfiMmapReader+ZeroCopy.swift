// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation

/// Thread-safe weak-keyed cache preserving a single `PlatformMmapResult` per `UniFfiMmapReader` instance.
/// Eliminates repeated `open`/`mmap` system calls and VMA exhaustion during chunked streaming or slicing.
final class UniFfiMmapCache: @unchecked Sendable {
    static let shared = UniFfiMmapCache()

    private let lock = NSLock()
    private let table = NSMapTable<AnyObject, PlatformMmapResult>.weakToStrongObjects()

    private init() {}

    func mmapResult(for reader: UniFfiMmapReader) throws -> PlatformMmapResult {
        lock.lock()
        defer { lock.unlock() }

        if let cached = table.object(forKey: reader) {
            return cached
        }

        let result = try PlatformMemory.mapFileReadOnly(filePath: reader.path())
        table.setObject(result, forKey: reader)
        return result
    }

    func purge(for reader: UniFfiMmapReader) {
        lock.lock()
        defer { lock.unlock() }
        table.removeObject(forKey: reader)
    }
}

extension UniFfiMmapReader {

    /// Executes a closure with a true zero-copy read-only byte buffer borrowed directly from mapped memory.
    ///
    /// - Parameters:
    ///   - offset: Start byte offset in the mapped file.
    ///   - length: Maximum byte count to slice (nil borrows to EOF).
    ///   - body: Scoped closure receiving an immutable `UnsafeRawBufferPointer`.
    /// - Returns: The closure's computed return value.
    /// - Throws: `TtZipError.IoError` if offset exceeds mapped file bounds.
    public func withZeroCopyBytes<R: Sendable>(
        offset: UInt64 = 0,
        length: UInt64? = nil,
        _ body: @Sendable (UnsafeRawBufferPointer) throws -> R
    ) throws -> R {
        let fileSize = self.len()
        guard offset <= fileSize else {
            throw TtZipError.IoError(message: "Offset \(offset) out of bounds for size \(fileSize)")
        }
        let available = fileSize - offset
        let count = min(length ?? available, available)
        guard count > 0 else {
            return try body(UnsafeRawBufferPointer(start: nil, count: 0))
        }

        let mmapResult: PlatformMmapResult
        do {
            mmapResult = try UniFfiMmapCache.shared.mmapResult(for: self)
        } catch {
            throw TtZipError.IoError(message: "Failed to memory-map file at \(self.path()): \(error.localizedDescription)")
        }

        return try mmapResult.withUnsafeBytes { rawBuffer in
            guard let baseAddr = rawBuffer.baseAddress else {
                return try body(UnsafeRawBufferPointer(start: nil, count: 0))
            }
            let sliceStart = baseAddr.advanced(by: Int(offset))
            let sliceBuf = UnsafeRawBufferPointer(start: sliceStart, count: Int(count))
            return try body(sliceBuf)
        }
    }

    /// Returns a true zero-copy `Data` instance mapped directly over the virtual address space
    /// backed by `PlatformMmapResult` ARC lifecycle and active borrower tracking without intermediate heap copying.
    ///
    /// - Parameters:
    ///   - offset: Start byte offset in the mapped file.
    ///   - length: Maximum byte count to slice.
    /// - Returns: True zero-copy `Data` view over the requested byte range.
    /// - Throws: `TtZipError.IoError` if offset is out of bounds or the file cannot be mapped.
    public func dataNoCopy(offset: UInt64, length: UInt64) throws -> Data {
        let fileSize = self.len()
        guard offset <= fileSize else {
            throw TtZipError.IoError(message: "Offset \(offset) out of bounds for size \(fileSize)")
        }
        guard length > 0, offset < fileSize else {
            return Data()
        }

        let mmapResult: PlatformMmapResult
        do {
            mmapResult = try UniFfiMmapCache.shared.mmapResult(for: self)
        } catch {
            throw TtZipError.IoError(message: "Failed to memory-map file at \(self.path()): \(error.localizedDescription)")
        }

        let available = fileSize - offset
        let count = min(length, available)

        let borrowedPtr: UnsafeRawPointer
        do {
            borrowedPtr = try mmapResult.retainBorrow()
        } catch {
            throw TtZipError.IoError(message: "Failed to borrow memory from mapping: \(error.localizedDescription)")
        }

        let mutBase = UnsafeMutableRawPointer(mutating: borrowedPtr)
        let startPtr = mutBase.advanced(by: Int(offset))

        return Data(bytesNoCopy: startPtr, count: Int(count), deallocator: .custom { [mmapResult] _, _ in
            mmapResult.releaseBorrow()
        })
    }
}
