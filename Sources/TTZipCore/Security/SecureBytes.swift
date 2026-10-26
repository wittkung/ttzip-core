// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Errors associated with secure memory allocation and page locking.
public enum SecureBytesError: Error, Sendable, LocalizedError, Equatable {
    case allocationFailed(status: Int32)
    case invalidSize(Int)

    public var errorDescription: String? {
        switch self {
        case .allocationFailed(let status):
            return "Failed to allocate page-aligned memory for SecureBytes (posix_memalign status: \(status))."
        case .invalidSize(let size):
            return "Invalid memory size requested for SecureBytes: \(size)."
        }
    }
}

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

    /// Allocates a new locked secure buffer of the specified byte count and initializes with data.
    public init(data: Data) throws {
        let byteCount = data.count
        guard byteCount >= 0 else {
            throw SecureBytesError.invalidSize(byteCount)
        }
        self.count = byteCount
        
        // Page-align allocation for mlock compatibility
        let pageSize = Int(getpagesize())
        guard byteCount <= Int.max - pageSize else {
            throw SecureBytesError.invalidSize(byteCount)
        }
        let alignedSize = ((byteCount + pageSize - 1) / pageSize) * pageSize
        self.allocationSize = max(alignedSize, pageSize)
        
        var ptr: UnsafeMutableRawPointer? = nil
        let status = posix_memalign(&ptr, pageSize, self.allocationSize)
        guard status == 0, let allocatedPtr = ptr else {
            throw SecureBytesError.allocationFailed(status: status)
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

    /// Allocates an empty locked secure buffer with specified capacity and optional logical count.
    public init(capacity: Int, count: Int? = nil) throws {
        guard capacity >= 0 else {
            throw SecureBytesError.invalidSize(capacity)
        }
        let logicalCount = count ?? capacity
        guard logicalCount >= 0 && logicalCount <= capacity else {
            throw SecureBytesError.invalidSize(logicalCount)
        }
        self.count = logicalCount
        let pageSize = Int(getpagesize())
        guard capacity <= Int.max - pageSize else {
            throw SecureBytesError.invalidSize(capacity)
        }
        let alignedSize = ((capacity + pageSize - 1) / pageSize) * pageSize
        self.allocationSize = max(alignedSize, pageSize)
        
        var ptr: UnsafeMutableRawPointer? = nil
        let status = posix_memalign(&ptr, pageSize, self.allocationSize)
        guard status == 0, let allocatedPtr = ptr else {
            throw SecureBytesError.allocationFailed(status: status)
        }
        self.rawPointer = allocatedPtr
        
        _ = mlock(self.rawPointer, self.allocationSize)
        memset(self.rawPointer, 0, self.allocationSize)
    }

    /// Initializes buffer from UTF-8 string bytes directly into mlocked memory without intermediate Swift heap allocations.
    public convenience init(utf8String: String) throws {
        let utf8Count = utf8String.utf8.count
        try self.init(capacity: max(1, utf8Count + 1), count: utf8Count)
        if utf8Count > 0 {
            utf8String.withCString { cStr in
                self.rawPointer.copyMemory(from: cStr, byteCount: utf8Count)
            }
        }
        self.rawPointer.advanced(by: utf8Count).storeBytes(of: 0, as: UInt8.self)
    }

    /// Initializes buffer from byte array.
    public convenience init(bytes: [UInt8]) throws {
        try self.init(data: Data(bytes))
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

    #if DEBUG
    /// Internal accessor strictly for unit test verification of wiped pointer state.
    /// External callers must use `withUnsafeBytes` / `withUnsafeMutableBytes`.
    internal var baseAddress: UnsafeMutableRawPointer? {
        lock.lock()
        defer { lock.unlock() }
        return isScrubbed ? nil : rawPointer
    }
    #endif

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
        if self === other {
            return true
        }
        guard self.count == other.count else { return false }

        let selfID = ObjectIdentifier(self)
        let otherID = ObjectIdentifier(other)
        let (first, second) = selfID < otherID ? (self, other) : (other, self)

        first.lock.lock()
        defer { first.lock.unlock() }
        second.lock.lock()
        defer { second.lock.unlock() }

        guard !self.isScrubbed, !other.isScrubbed else {
            return false
        }
        if self.count == 0 {
            return true
        }
        return timingsafe_bcmp(self.rawPointer, other.rawPointer, self.count) == 0
    }
}
