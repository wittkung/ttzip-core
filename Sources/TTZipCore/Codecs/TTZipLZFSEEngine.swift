// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Compression

/// High-performance Swift 6 facade for Apple LZFSE (Lempel-Ziv Finite State Entropy) compression.
/// Utilizes Apple Silicon hardware-accelerated `libcompression` with dual-engine Rust microkernel fallback.
public struct TTZipLZFSEEngine: Sendable {

    /// Calculates theoretical worst-case bound for LZFSE compressed data buffer.
    public static func compressBound(uncompressedSize: Int) -> Int {
        Int(uniffiCompressBound(codec: .lzfse, srcLen: UInt64(max(0, uncompressedSize)), level: nil))
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
            guard let dstPtr = dstBuf.bindMemory(to: UInt8.self).baseAddress else { return 0 }
            return data.withUnsafeBytes { srcBuf -> Int in
                guard let srcPtr = srcBuf.bindMemory(to: UInt8.self).baseAddress else { return 0 }
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

        // 2. Microkernel Fallback via UniFFI
        return try uniffiLzfseCompress(src: data)
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
                guard let dstPtr = dstBuf.bindMemory(to: UInt8.self).baseAddress else { return 0 }
                return data.withUnsafeBytes { srcBuf -> Int in
                    guard let srcPtr = srcBuf.bindMemory(to: UInt8.self).baseAddress else { return 0 }
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

        // 2. Microkernel Fallback via UniFFI
        return try uniffiLzfseDecompress(src: data, expectedUncompressedSize: UInt64(capacity))
    }
}
