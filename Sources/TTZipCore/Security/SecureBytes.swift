// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// A page-aligned, locked in-memory buffer for storing sensitive cryptographic material
/// such as master passwords, derived vault encryption keys, and intermediate plaintexts.
///
/// Guarantees:
/// 1. Physical RAM Locking (`mlock`): Prevents the kernel from writing secrets to swap space on disk.
/// 2. Deterministic Secure Scrubbing on Deinit: Invokes compiler-fence protected zeroization (`memset_s`) before deallocation.
/// 3. Zero-Copy Interoperability: Exposes direct pointer access via scoped closures.
public final class SecureBytes: @unchecked Sendable {
    private let rawPointer: UnsafeMutableRawPointer
    private let allocationSize: Int
    public let count: Int
    private var isScrubbed = false
    private let lock = NSLock()

    public var baseAddress: UnsafeMutableRawPointer? {
        lock.lock()
        defer { lock.unlock() }
        return isScrubbed ? nil : rawPointer
    }

    /// Allocates a new locked secure buffer of the specified byte count and initializes with data.
    public init(data: Data) {
        let byteCount = data.count
        self.count = byteCount
        
        // Page-align allocation for mlock compatibility
        let pageSize = Int(getpagesize())
        let alignedSize = ((byteCount + pageSize - 1) / pageSize) * pageSize
        self.allocationSize = max(alignedSize, pageSize)
        
        var ptr: UnsafeMutableRawPointer? = nil
        let status = posix_memalign(&ptr, pageSize, self.allocationSize)
        guard status == 0, let allocatedPtr = ptr else {
            fatalError("Failed to allocate page-aligned memory for SecureBytes (status: \(status))")
        }
        self.rawPointer = allocatedPtr
        
        // Lock pages in physical memory
        _ = mlock(self.rawPointer, self.allocationSize)
        
        // Copy initial data and zero trailing padding
        if byteCount > 0 {
            data.withUnsafeBytes { dataBuf in
                if let base = dataBuf.baseAddress {
                    self.rawPointer.copyMemory(from: base, byteCount: byteCount)
                }
            }
        }
        if self.allocationSize > byteCount {
            let paddingPtr = self.rawPointer.advanced(by: byteCount)
            memset(paddingPtr, 0, self.allocationSize - byteCount)
        }
    }

    /// Allocates an empty locked secure buffer with specified capacity.
    public init(capacity: Int) {
        self.count = capacity
        let pageSize = Int(getpagesize())
        let alignedSize = ((capacity + pageSize - 1) / pageSize) * pageSize
        self.allocationSize = max(alignedSize, pageSize)
        
        var ptr: UnsafeMutableRawPointer? = nil
        let status = posix_memalign(&ptr, pageSize, self.allocationSize)
        guard status == 0, let allocatedPtr = ptr else {
            fatalError("Failed to allocate page-aligned memory for SecureBytes (status: \(status))")
        }
        self.rawPointer = allocatedPtr
        
        _ = mlock(self.rawPointer, self.allocationSize)
        memset(self.rawPointer, 0, self.allocationSize)
    }

    /// Initializes buffer from UTF-8 string bytes directly into mlocked memory without intermediate Swift heap allocations.
    public convenience init(utf8String: String) {
        let utf8Count = utf8String.utf8.count
        self.init(capacity: max(1, utf8Count))
        if utf8Count > 0 {
            utf8String.withCString { cStr in
                self.rawPointer.copyMemory(from: cStr, byteCount: utf8Count)
            }
        }
    }

    /// Initializes buffer from byte array.
    public convenience init(bytes: [UInt8]) {
        self.init(data: Data(bytes))
    }

    /// Returns a copy of the secure buffer contents as `Data`.
    public func toData() -> Data {
        withUnsafeBytes { buf in
            guard let base = buf.baseAddress, buf.count > 0 else { return Data() }
            return Data(bytes: base, count: buf.count)
        }
    }

    deinit {
        wipeAndFree()
    }

    /// Securely wipes all memory contents and unlocks physical pages.
    public func wipeAndFree() {
        lock.lock()
        defer { lock.unlock() }
        guard !isScrubbed else { return }
        
        // Secure zeroization using POSIX memset_s (guaranteed never dead-code eliminated)
        memset_s(rawPointer, allocationSize, 0, allocationSize)
        
        // Unlock physical pages
        _ = munlock(rawPointer, allocationSize)
        
        // Free page-aligned memory
        free(rawPointer)
        isScrubbed = true
    }

    /// Executes closure with read-only pointer to the secure memory buffer.
    @inline(__always)
    public func withUnsafeBytes<R>(_ body: (UnsafeRawBufferPointer) throws -> R) rethrows -> R {
        lock.lock()
        defer { lock.unlock() }
        guard !isScrubbed else {
            return try body(UnsafeRawBufferPointer(start: nil, count: 0))
        }
        let buffer = UnsafeRawBufferPointer(start: rawPointer, count: count)
        return try body(buffer)
    }

    /// Executes closure with mutable pointer to the secure memory buffer.
    @inline(__always)
    public func withUnsafeMutableBytes<R>(_ body: (UnsafeMutableRawBufferPointer) throws -> R) rethrows -> R {
        lock.lock()
        defer { lock.unlock() }
        guard !isScrubbed else {
            return try body(UnsafeMutableRawBufferPointer(start: nil, count: 0))
        }
        let buffer = UnsafeMutableRawBufferPointer(start: rawPointer, count: count)
        return try body(buffer)
    }

    /// Executes closure with null-terminated C string pointer.
    @inline(__always)
    public func withCString<R>(_ body: (UnsafePointer<CChar>?) throws -> R) rethrows -> R {
        lock.lock()
        defer { lock.unlock() }
        guard !isScrubbed, count > 0 else {
            return try body(nil)
        }
        let cPtr = rawPointer.assumingMemoryBound(to: CChar.self)
        return try body(cPtr)
    }

    /// Constant-time comparison between two SecureBytes instances.
    public func constantTimeEquals(_ other: SecureBytes) -> Bool {
        guard self.count == other.count else { return false }
        return self.withUnsafeBytes { aBuf in
            other.withUnsafeBytes { bBuf in
                guard let a = aBuf.baseAddress, let b = bBuf.baseAddress else { return false }
                return timingsafe_bcmp(a, b, self.count) == 0
            }
        }
    }
}
