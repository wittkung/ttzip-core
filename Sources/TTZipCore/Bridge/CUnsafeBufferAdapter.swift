// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation

/// Memory-safe buffer and pointer utilities for high-performance memory operations.
public enum CUnsafeBufferAdapter {
    
    /// Safely converts an optional Swift `String` to a temporary `const char*` pointer.
    @inline(__always)
    public static func withCString<R>(_ string: String?, _ body: (UnsafePointer<CChar>?) throws -> R) rethrows -> R {
        guard let string = string else {
            return try body(nil)
        }
        return try string.withCString { cStr in
            try body(cStr)
        }
    }

    private static let emptySentinel: UInt8 = 0

    /// Safely provides raw byte pointer and count representation of Swift `Data`.
    @inline(__always)
    public static func withBufferPointer<R>(_ data: Data, _ body: (UnsafeRawPointer, Int) throws -> R) rethrows -> R {
        if data.isEmpty {
            return try withUnsafePointer(to: emptySentinel) { sentinelPtr in
                try body(sentinelPtr, 0)
            }
        }
        return try data.withUnsafeBytes { rawBuffer in
            if let baseAddress = rawBuffer.baseAddress {
                return try body(baseAddress, data.count)
            } else {
                return try withUnsafePointer(to: emptySentinel) { sentinelPtr in
                    try body(sentinelPtr, 0)
                }
            }
        }
    }

    /// Safely provides mutable raw byte pointer and capacity representation of Swift `Data`.
    @inline(__always)
    public static func withMutableBufferPointer<R>(_ data: inout Data, _ body: (UnsafeMutableRawPointer, Int) throws -> R) rethrows -> R {
        let count = data.count
        if count == 0 {
            var dummy: UInt8 = 0
            return try body(&dummy, 0)
        }
        return try data.withUnsafeMutableBytes { rawBuffer in
            if let baseAddress = rawBuffer.baseAddress {
                return try body(baseAddress, count)
            } else {
                var dummy: UInt8 = 0
                return try body(&dummy, 0)
            }
        }
    }

    /// Allocates an Apple Silicon 16KB hardware page-aligned memory buffer.
    @inline(__always)
    public static func allocateAlignedBuffer(capacity: Int) -> UnsafeMutableRawPointer? {
        guard capacity > 0 else { return nil }
        return UnsafeMutableRawPointer.allocate(byteCount: capacity, alignment: 16384)
    }

    /// Deallocates a 16KB hardware page-aligned memory buffer.
    @inline(__always)
    public static func deallocateAlignedBuffer(_ pointer: UnsafeMutableRawPointer) {
        pointer.deallocate()
    }
}
