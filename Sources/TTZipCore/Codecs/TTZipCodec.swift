// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

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

    /// Maps algorithm to its corresponding UniFFI compression codec.
    @inlinable
    public func uniffiCodec(level: Int32 = 6) -> UniFfiCompressionCodec {
        switch self {
        case .deflate: return .deflateRaw
        case .zlib: return .zlib
        case .gzip: return .gzip
        case .zstd: return .zstd
        case .brotli: return .brotli
        case .lzma, .fastLzma2: return .fl2
        case .lz4: return level >= 9 ? .lz4Hc : .lz4Fast
        case .snappyBlock: return .snappyRaw
        case .snappyFramed: return .snappyFramed
        case .lzfse: return .lzfse
        case .bzip2: return .bzip2
        case .ppmd: return .ppmd
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
    @inlinable
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

/// Swift 6 strongly-typed facade for all native compression codecs via UniFFI bridge.
public struct TTZipCodec: Sendable {

    /// Calculates the maximum theoretical compressed output buffer size in bytes for a given input size.
    @inlinable
    public static func compressBound(
        uncompressedSize: Int,
        algorithm: TTZipCodecAlgorithm,
        level: TTZipCompressionLevel = .normal
    ) -> Int {
        let rawLvl = level.rawLevel(for: algorithm)
        switch algorithm {
        case .fastLzma2:
            return Int(uniffiFl2CompressBound(srcLen: UInt64(max(0, uncompressedSize))))
        default:
            let codec = algorithm.uniffiCodec(level: rawLvl)
            return Int(uniffiCompressBound(codec: codec, srcLen: UInt64(max(0, uncompressedSize)), level: rawLvl))
        }
    }

    /// Compresses in-memory byte buffer using the specified algorithm and level via UniFFI.
    @inlinable
    public static func compress(
        _ data: Data,
        algorithm: TTZipCodecAlgorithm,
        level: TTZipCompressionLevel = .normal
    ) throws -> Data {
        if data.isEmpty {
            return Data()
        }

        let rawLvl = level.rawLevel(for: algorithm)
        switch algorithm {
        case .fastLzma2:
            return try uniffiFl2Compress(src: data, level: rawLvl, nbThreads: 1)
        default:
            let codec = algorithm.uniffiCodec(level: rawLvl)
            let opts = UniFfiCompressionOptions(
                level: rawLvl,
                acceleration: algorithm == .lz4 ? 1 : nil,
                windowMb: nil,
                ppmdOrder: algorithm == .ppmd ? 6 : nil,
                ppmdMemMb: algorithm == .ppmd ? 16 : nil
            )
            return try uniffiCompressBuffer(codec: codec, src: data, options: opts)
        }
    }

    /// Decompresses an in-memory compressed byte buffer via UniFFI.
    @inlinable
    public static func decompress(
        _ data: Data,
        algorithm: TTZipCodecAlgorithm,
        expectedUncompressedSize: Int? = nil
    ) throws -> Data {
        if data.isEmpty {
            return Data()
        }

        let expSize: UInt64?
        if let expected = expectedUncompressedSize, expected > 0 {
            expSize = UInt64(expected)
        } else {
            switch algorithm {
            case .fastLzma2, .lzma:
                expSize = uniffiFl2FindDecompressedSize(src: data)
            default:
                expSize = nil
            }
        }

        let codec = algorithm.uniffiCodec()
        switch algorithm {
        case .fastLzma2:
            return try uniffiFl2Decompress(src: data, expectedUncompressedSize: expSize, nbThreads: 1)
        case .deflate, .zlib, .gzip, .lz4, .lzfse, .ppmd:
            if let exp = expSize {
                return try uniffiDecompressBuffer(codec: codec, src: data, expectedUncompressedSize: exp, options: nil)
            } else {
                var capacity = UInt64(max(data.count * 4, 65536))
                for _ in 0..<4 {
                    do {
                        return try uniffiDecompressBuffer(codec: codec, src: data, expectedUncompressedSize: capacity, options: nil)
                    } catch {
                        capacity *= 4
                    }
                }
                throw TTZipCodecError.decompressionFailed(status: -1)
            }
        default:
            return try uniffiDecompressBuffer(codec: codec, src: data, expectedUncompressedSize: expSize, options: nil)
        }
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
}
