// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

extension UniFfiMmapReader {

    /// Executes a closure with a true zero-copy read-only byte buffer borrowed directly from
    /// the underlying Rust memory-mapped allocation without secondary POSIX `open`/`mmap` system calls.
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

        let address = self.memoryAddress()
        guard address != 0, let baseAddr = UnsafeRawPointer(bitPattern: UInt(address)) else {
            return try body(UnsafeRawBufferPointer(start: nil, count: 0))
        }

        let sliceStart = baseAddr.advanced(by: Int(offset))
        let sliceBuf = UnsafeRawBufferPointer(start: sliceStart, count: Int(count))
        return try withExtendedLifetime(self) {
            try body(sliceBuf)
        }
    }

    /// Returns a true zero-copy `Data` instance mapped directly over the virtual address space
    /// backed by Rust `UniFfiMmapReader` ARC lifecycle without secondary POSIX file descriptors or heap copies.
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

        let available = fileSize - offset
        let count = min(length, available)

        let address = self.memoryAddress()
        guard address != 0, let baseAddr = UnsafeRawPointer(bitPattern: UInt(address)) else {
            return Data()
        }

        let mutBase = UnsafeMutableRawPointer(mutating: baseAddr)
        let startPtr = mutBase.advanced(by: Int(offset))

        return Data(bytesNoCopy: startPtr, count: Int(count), deallocator: .custom { [self] _, _ in
            _ = self
        })
    }
}
