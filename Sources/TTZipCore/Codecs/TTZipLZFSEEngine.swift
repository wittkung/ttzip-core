// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Compression
import CTTZipBridge

/// High-performance Swift 6 facade for Apple LZFSE (Lempel-Ziv Finite State Entropy) compression.
/// Utilizes Apple Silicon hardware-accelerated `libcompression` with dual-engine Rust microkernel fallback.
public struct TTZipLZFSEEngine: Sendable {

    /// Calculates theoretical worst-case bound for LZFSE compressed data buffer.
    public static func compressBound(uncompressedSize: Int) -> Int {
        Int(ttzip_rust_lzfse_compress_bound(uncompressedSize))
    }

    /// Compresses in-memory buffer using Apple Silicon native hardware LZFSE engine.
    public static func compress(_ data: Data) throws -> Data {
        if data.isEmpty {
            return Data()
        }

        let bound = compressBound(uncompressedSize: data.count)
        var destination = Data(count: bound)

        // 1. Try Apple Native libcompression
        let nativeWritten = destination.withUnsafeMutableBytes { dstBuf -> Int in
            guard let dstPtr = dstBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return 0 }
            return data.withUnsafeBytes { srcBuf -> Int in
                guard let srcPtr = srcBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return 0 }
                return compression_encode_buffer(
                    dstPtr,
                    bound,
                    srcPtr,
                    data.count,
                    nil,
                    COMPRESSION_LZFSE
                )
            }
        }

        if nativeWritten > 0 {
            destination.count = nativeWritten
            return destination
        }

        // 2. Microkernel Fallback
        var fallbackLen: Int = 0
        let status = data.withUnsafeBytes { srcBuf -> Int32 in
            guard let srcPtr = srcBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
            return destination.withUnsafeMutableBytes { dstBuf -> Int32 in
                guard let dstPtr = dstBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
                return ttzip_rust_lzfse_compress(srcPtr, data.count, dstPtr, bound, &fallbackLen)
            }
        }

        guard status == 0 && fallbackLen > 0 else {
            throw TTZipCodecError.compressionFailed(status: status)
        }

        destination.count = fallbackLen
        return destination
    }

    /// Decompresses an LZFSE compressed buffer.
    public static func decompress(_ data: Data, estimatedSize: Int? = nil) throws -> Data {
        if data.isEmpty {
            return Data()
        }

        var capacity = estimatedSize ?? max(data.count * 4, 65536)
        var destination = Data(count: capacity)

        // 1. Try Apple Native libcompression
        var retry = 0
        while retry < 4 {
            let currentCap = capacity
            let nativeWritten = destination.withUnsafeMutableBytes { dstBuf -> Int in
                guard let dstPtr = dstBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return 0 }
                return data.withUnsafeBytes { srcBuf -> Int in
                    guard let srcPtr = srcBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return 0 }
                    return compression_decode_buffer(
                        dstPtr,
                        currentCap,
                        srcPtr,
                        data.count,
                        nil,
                        COMPRESSION_LZFSE
                    )
                }
            }

            if nativeWritten > 0 {
                destination.count = nativeWritten
                return destination
            }

            capacity *= 4
            destination = Data(count: capacity)
            retry += 1
        }

        // 2. Microkernel Fallback
        var fallbackLen: Int = 0
        let status = data.withUnsafeBytes { srcBuf -> Int32 in
            guard let srcPtr = srcBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
            return destination.withUnsafeMutableBytes { dstBuf -> Int32 in
                guard let dstPtr = dstBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
                return ttzip_rust_lzfse_decompress(srcPtr, data.count, dstPtr, capacity, &fallbackLen)
            }
        }

        guard status == 0 && fallbackLen > 0 else {
            throw TTZipCodecError.decompressionFailed(status: status)
        }

        destination.count = fallbackLen
        return destination
    }
}
