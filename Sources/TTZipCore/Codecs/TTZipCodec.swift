// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import CTTZipBridge

/// Supported single-stream compression codecs across TTZip engine.
public enum TTZipCodecAlgorithm: String, Sendable, CaseIterable {
    case deflate
    case zlib
    case gzip
    case zstd
    case brotli
    case lzma
    case fastLzma2
    case lz4
    case snappyBlock
    case snappyFramed
    case lzfse
    case bzip2
    case ppmd

    /// Formal human-readable display name for the codec.
    public var displayName: String {
        switch self {
        case .deflate: return "DEFLATE"
        case .zlib: return "Zlib"
        case .gzip: return "Gzip"
        case .zstd: return "Zstandard (Zstd)"
        case .brotli: return "Brotli"
        case .lzma: return "LZMA"
        case .fastLzma2: return "Fast-LZMA2"
        case .lz4: return "LZ4"
        case .snappyBlock: return "Snappy (Raw Block)"
        case .snappyFramed: return "Snappy (Framed)"
        case .lzfse: return "Apple LZFSE"
        case .bzip2: return "Bzip2"
        case .ppmd: return "PPMd (Model H)"
        }
    }
}

/// Compression effort level specification.
public enum TTZipCompressionLevel: Sendable, Hashable {
    case store
    case fastest
    case normal
    case maximum
    case ultra
    case custom(Int32)

    /// Translates semantic level to algorithm-specific integer level.
    public func rawLevel(for algorithm: TTZipCodecAlgorithm) -> Int32 {
        switch self {
        case .store:
            return 0
        case .fastest:
            switch algorithm {
            case .zstd: return 1
            case .brotli: return 1
            case .bzip2: return 1
            case .fastLzma2: return 1
            case .lzma: return 1
            default: return 1
            }
        case .normal:
            switch algorithm {
            case .zstd: return 3
            case .brotli: return 6
            case .bzip2: return 6
            case .fastLzma2: return 3
            case .lzma: return 5
            default: return 6
            }
        case .maximum:
            switch algorithm {
            case .zstd: return 9
            case .brotli: return 9
            case .bzip2: return 9
            case .fastLzma2: return 6
            case .lzma: return 7
            default: return 9
            }
        case .ultra:
            switch algorithm {
            case .zstd: return 19
            case .brotli: return 11
            case .bzip2: return 9
            case .fastLzma2: return 9
            case .lzma: return 9
            default: return 12
            }
        case .custom(let val):
            return val
        }
    }
}

/// Errors thrown by the unified codec engine.
public enum TTZipCodecError: Error, Sendable, LocalizedError {
    case invalidParameter
    case compressionFailed(status: Int32)
    case decompressionFailed(status: Int32)
    case bufferTooSmall
    case unsupportedCodec(TTZipCodecAlgorithm)
    case memoryAllocationFailed

    public var errorDescription: String? {
        switch self {
        case .invalidParameter:
            return "Invalid parameter provided to TTZip codec operation."
        case .compressionFailed(let status):
            return "Compression failed with status code \(status)."
        case .decompressionFailed(let status):
            return "Decompression failed with status code \(status)."
        case .bufferTooSmall:
            return "Destination buffer is too small for codec output."
        case .unsupportedCodec(let alg):
            return "Codec '\(alg.displayName)' is not supported for the requested operation."
        case .memoryAllocationFailed:
            return "Failed to allocate memory buffer for codec operation."
        }
    }
}

/// Swift 6 strongly-typed facade for all 13 native compression codecs.
public struct TTZipCodec: Sendable {

    /// Calculates the maximum theoretical compressed output buffer size in bytes for a given input size.
    public static func compressBound(
        uncompressedSize: Int,
        algorithm: TTZipCodecAlgorithm,
        level: TTZipCompressionLevel = .normal
    ) -> Int {
        let rawLvl = level.rawLevel(for: algorithm)
        switch algorithm {
        case .deflate, .zlib, .gzip:
            return Int(ttzip_rust_deflate_compress_bound(uncompressedSize, rawLvl))
        case .zstd:
            return Int(ttzip_rust_zstd_compress_bound(uncompressedSize))
        case .brotli:
            return Int(ttzip_rust_brotli_compress_bound(uncompressedSize))
        case .lz4:
            return Int(ttzip_rust_lz4_compress_bound(uncompressedSize))
        case .snappyBlock:
            return Int(ttzip_rust_snappy_max_compressed_length(uncompressedSize))
        case .snappyFramed:
            return Int(ttzip_rust_snappy_frame_max_encoded_length(uncompressedSize))
        case .lzfse:
            return Int(ttzip_rust_lzfse_compress_bound(uncompressedSize))
        case .fastLzma2:
            return Int(ttzip_rust_fl2_compress_bound(uncompressedSize))
        case .bzip2:
            return Int(ttzip_rust_bzip2_compress_bound(uncompressedSize))
        case .lzma, .ppmd:
            return max(uncompressedSize + 1024, uncompressedSize * 2)
        }
    }

    /// Compresses in-memory byte buffer using the specified algorithm and level.
    public static func compress(
        _ data: Data,
        algorithm: TTZipCodecAlgorithm,
        level: TTZipCompressionLevel = .normal
    ) throws -> Data {
        if data.isEmpty {
            return Data()
        }

        let rawLvl = level.rawLevel(for: algorithm)

        if algorithm == .ppmd || algorithm == .lzma {
            return try compressUnifiedBuffer(data: data, algorithm: algorithm, level: rawLvl)
        }

        let bound = compressBound(uncompressedSize: data.count, algorithm: algorithm, level: level)
        var destination = Data(count: bound)

        var written: Int = 0

        let status: Int32 = try data.withUnsafeBytes { srcBuf in
            guard let srcPtr = srcBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                throw TTZipCodecError.invalidParameter
            }
            return try destination.withUnsafeMutableBytes { dstBuf in
                guard let dstPtr = dstBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                    throw TTZipCodecError.invalidParameter
                }
                var outLen: Int = 0

                let code: Int32
                switch algorithm {
                case .deflate:
                    code = ttzip_rust_deflate_compress(srcPtr, data.count, dstPtr, bound, rawLvl, &outLen)
                case .zlib:
                    code = ttzip_rust_zlib_compress(srcPtr, data.count, dstPtr, bound, rawLvl, &outLen)
                case .gzip:
                    code = ttzip_rust_gzip_compress(srcPtr, data.count, dstPtr, bound, rawLvl, &outLen)
                case .zstd:
                    code = ttzip_rust_zstd_compress(srcPtr, data.count, dstPtr, bound, rawLvl, &outLen)
                case .brotli:
                    code = ttzip_rust_brotli_compress(srcPtr, data.count, dstPtr, bound, UInt32(max(0, rawLvl)), 22, &outLen)
                case .lz4:
                    code = ttzip_rust_lz4_compress(srcPtr, data.count, dstPtr, bound, &outLen)
                case .snappyBlock:
                    code = ttzip_rust_snappy_compress(srcPtr, data.count, dstPtr, bound, &outLen)
                case .snappyFramed:
                    code = ttzip_rust_snappy_frame_encode(srcPtr, data.count, dstPtr, bound, &outLen)
                case .lzfse:
                    code = ttzip_rust_lzfse_compress(srcPtr, data.count, dstPtr, bound, &outLen)
                case .fastLzma2:
                    code = ttzip_rust_fl2_compress(srcPtr, data.count, dstPtr, bound, rawLvl, 1, &outLen)
                case .bzip2:
                    code = ttzip_rust_bzip2_compress(srcPtr, data.count, dstPtr, bound, rawLvl, &outLen)
                case .lzma, .ppmd:
                    code = 0
                }

                written = outLen
                return code
            }
        }

        guard status == 0 else {
            throw TTZipCodecError.compressionFailed(status: status)
        }

        destination.count = written
        return destination
    }

    /// Decompresses an in-memory compressed byte buffer.
    public static func decompress(
        _ data: Data,
        algorithm: TTZipCodecAlgorithm,
        expectedUncompressedSize: Int? = nil
    ) throws -> Data {
        if data.isEmpty {
            return Data()
        }

        if algorithm == .ppmd || algorithm == .lzma {
            return try decompressUnifiedBuffer(data: data, algorithm: algorithm, expectedUncompressedSize: expectedUncompressedSize)
        }

        // Determine destination capacity
        var capacity = expectedUncompressedSize ?? 0
        if capacity <= 0 {
            switch algorithm {
            case .zstd:
                let detected = data.withUnsafeBytes { buf in
                    ttzip_rust_zstd_get_decompressed_size(buf.baseAddress?.assumingMemoryBound(to: UInt8.self), data.count)
                }
                capacity = (detected > 0 && detected < 0x7FFFFFFF) ? Int(detected) : (data.count * 4 + 16384)
            case .snappyBlock:
                var uncompLen: Int = 0
                let st = data.withUnsafeBytes { buf in
                    ttzip_rust_snappy_uncompressed_length(buf.baseAddress?.assumingMemoryBound(to: UInt8.self), data.count, &uncompLen)
                }
                capacity = (st == 0 && uncompLen > 0) ? uncompLen : (data.count * 4 + 16384)
            case .fastLzma2:
                let detected = data.withUnsafeBytes { buf in
                    ttzip_rust_fl2_find_decompressed_size(buf.baseAddress?.assumingMemoryBound(to: UInt8.self), data.count)
                }
                capacity = (detected > 0 && detected < 0x7FFFFFFF) ? Int(detected) : (data.count * 4 + 16384)
            default:
                capacity = max(data.count * 4, 65536)
            }
        }

        var destination = Data(count: capacity)
        var written: Int = 0

        var currentCapacity = capacity
        var retryCount = 0

        while retryCount < 4 {
            let status: Int32 = try data.withUnsafeBytes { srcBuf in
                guard let srcPtr = srcBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                    throw TTZipCodecError.invalidParameter
                }
                return try destination.withUnsafeMutableBytes { dstBuf in
                    guard let dstPtr = dstBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                        throw TTZipCodecError.invalidParameter
                    }
                    var outLen: Int = 0

                    let code: Int32
                    switch algorithm {
                    case .deflate:
                        code = ttzip_rust_deflate_decompress(srcPtr, data.count, dstPtr, currentCapacity, &outLen)
                    case .zlib:
                        code = ttzip_rust_zlib_decompress(srcPtr, data.count, dstPtr, currentCapacity, &outLen)
                    case .gzip:
                        code = ttzip_rust_gzip_decompress(srcPtr, data.count, dstPtr, currentCapacity, &outLen)
                    case .zstd:
                        code = ttzip_rust_zstd_decompress(srcPtr, data.count, dstPtr, currentCapacity, &outLen)
                    case .brotli:
                        code = ttzip_rust_brotli_decompress(srcPtr, data.count, dstPtr, currentCapacity, &outLen)
                    case .lz4:
                        code = ttzip_rust_lz4_decompress(srcPtr, data.count, dstPtr, currentCapacity, &outLen)
                    case .snappyBlock:
                        code = ttzip_rust_snappy_decompress(srcPtr, data.count, dstPtr, currentCapacity, &outLen)
                    case .snappyFramed:
                        code = ttzip_rust_snappy_frame_decode(srcPtr, data.count, dstPtr, currentCapacity, &outLen)
                    case .lzfse:
                        code = ttzip_rust_lzfse_decompress(srcPtr, data.count, dstPtr, currentCapacity, &outLen)
                    case .fastLzma2:
                        code = ttzip_rust_fl2_decompress(srcPtr, data.count, dstPtr, currentCapacity, 1, &outLen)
                    case .bzip2:
                        code = ttzip_rust_bzip2_decompress(srcPtr, data.count, dstPtr, currentCapacity, &outLen)
                    case .lzma, .ppmd:
                        code = 0
                    }

                    written = outLen
                    return code
                }
            }

            if status == 0 {
                destination.count = written
                return destination
            }

            // Expand buffer if possibly truncated
            currentCapacity *= 4
            destination = Data(count: currentCapacity)
            retryCount += 1
        }

        throw TTZipCodecError.decompressionFailed(status: -1)
    }

    /// Asynchronously streams compressed chunks through an `AsyncThrowingStream` pipeline.
    public static func streamCompress(
        source: AsyncThrowingStream<Data, Error>,
        algorithm: TTZipCodecAlgorithm,
        level: TTZipCompressionLevel = .normal
    ) -> AsyncThrowingStream<Data, Error> {
        AsyncThrowingStream { continuation in
            Task {
                do {
                    for try await chunk in source {
                        if Task.isCancelled { break }
                        let compressedChunk = try compress(chunk, algorithm: algorithm, level: level)
                        continuation.yield(compressedChunk)
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
        }
    }

    /// Asynchronously streams decompressed chunks through an `AsyncThrowingStream` pipeline.
    public static func streamDecompress(
        source: AsyncThrowingStream<Data, Error>,
        algorithm: TTZipCodecAlgorithm
    ) -> AsyncThrowingStream<Data, Error> {
        AsyncThrowingStream { continuation in
            Task {
                do {
                    for try await chunk in source {
                        if Task.isCancelled { break }
                        let decompressedChunk = try decompress(chunk, algorithm: algorithm)
                        continuation.yield(decompressedChunk)
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
        }
    }

    // MARK: - Internal Unified Microkernel Bridges

    private static func compressUnifiedBuffer(
        data: Data,
        algorithm: TTZipCodecAlgorithm,
        level: Int32
    ) throws -> Data {
        let codec: UniFfiCompressionCodec
        switch algorithm {
        case .ppmd: codec = .ppmd
        default: codec = .deflateRaw
        }
        let opts = UniFfiCompressionOptions(
            level: level,
            acceleration: nil,
            windowMb: nil,
            ppmdOrder: 6,
            ppmdMemMb: 16
        )
        return try uniffiCompressBuffer(codec: codec, src: data, options: opts)
    }

    private static func decompressUnifiedBuffer(
        data: Data,
        algorithm: TTZipCodecAlgorithm,
        expectedUncompressedSize: Int?
    ) throws -> Data {
        let codec: UniFfiCompressionCodec
        switch algorithm {
        case .ppmd: codec = .ppmd
        default: codec = .deflateRaw
        }
        let sizeUInt64 = expectedUncompressedSize.map { UInt64($0) }
        return try uniffiDecompressBuffer(codec: codec, src: data, expectedUncompressedSize: sizeUInt64, options: nil)
    }
}
