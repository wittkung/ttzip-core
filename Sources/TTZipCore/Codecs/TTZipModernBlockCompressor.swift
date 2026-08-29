// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import CTTZipBridge

/// Magic 4-byte header identifying TTZip Framed Parallel Block Archive format (`TTZB`).
private let TTZIP_BLOCK_MAGIC: UInt32 = 0x54545A42

/// High-performance multi-threaded block chunk compressor.
/// Slices large memory buffers into independent chunk blocks, compressing and hashing them concurrently.
public final class TTZipModernBlockCompressor: Sendable {

    public init() {}

    /// Compresses large data buffer in parallel chunks across available CPU cores.
    public static func compressParallel(
        _ data: Data,
        algorithm: TTZipCodecAlgorithm = .zstd,
        level: TTZipCompressionLevel = .normal,
        chunkSize: Int = 1024 * 1024,
        threadBudget: Int = ProcessInfo.processInfo.activeProcessorCount
    ) async throws -> Data {
        if data.isEmpty {
            return Data()
        }

        let actualChunkSize = max(64 * 1024, chunkSize)
        let totalChunks = (data.count + actualChunkSize - 1) / actualChunkSize

        // Split data into contiguous chunks
        var chunks: [(index: Int, range: Range<Int>)] = []
        chunks.reserveCapacity(totalChunks)
        for i in 0..<totalChunks {
            let start = i * actualChunkSize
            let end = min(data.count, start + actualChunkSize)
            chunks.append((index: i, range: start..<end))
        }

        // Parallel compression task group
        let compressedChunks: [(index: Int, uncompSize: UInt32, crc32: UInt32, payload: Data)] = try await withThrowingTaskGroup(
            of: (index: Int, uncompSize: UInt32, crc32: UInt32, payload: Data).self
        ) { group in
            for chunk in chunks {
                let chunkData = data.subdata(in: chunk.range)
                group.addTask {
                    let crc = chunkData.withUnsafeBytes { buf in
                        ttzip_rust_crc32(0, buf.baseAddress?.assumingMemoryBound(to: UInt8.self), chunkData.count)
                    }
                    let compressed = try TTZipCodec.compress(chunkData, algorithm: algorithm, level: level)
                    return (
                        index: chunk.index,
                        uncompSize: UInt32(chunkData.count),
                        crc32: crc,
                        payload: compressed
                    )
                }
            }

            var results: [(index: Int, uncompSize: UInt32, crc32: UInt32, payload: Data)] = []
            results.reserveCapacity(totalChunks)
            for try await result in group {
                results.append(result)
            }
            return results.sorted { $0.index < $1.index }
        }

        // Assemble framed binary payload
        var output = Data()
        output.reserveCapacity(data.count / 2 + 1024)

        // Header: Magic (4B), Version (2B), Algorithm Code (1B), Reserved (1B), Total Chunks (4B), Original Size (8B)
        var magic = TTZIP_BLOCK_MAGIC.littleEndian
        var version = UInt16(1).littleEndian
        var algCode = algorithmCode(for: algorithm)
        var reserved = UInt8(0)
        var countField = UInt32(totalChunks).littleEndian
        var totalSizeField = UInt64(data.count).littleEndian

        output.append(Data(bytes: &magic, count: 4))
        output.append(Data(bytes: &version, count: 2))
        output.append(Data(bytes: &algCode, count: 1))
        output.append(Data(bytes: &reserved, count: 1))
        output.append(Data(bytes: &countField, count: 4))
        output.append(Data(bytes: &totalSizeField, count: 8))

        // Write Chunks
        for chunk in compressedChunks {
            var uSize = chunk.uncompSize.littleEndian
            var cSize = UInt32(chunk.payload.count).littleEndian
            var crc = chunk.crc32.littleEndian

            output.append(Data(bytes: &uSize, count: 4))
            output.append(Data(bytes: &cSize, count: 4))
            output.append(Data(bytes: &crc, count: 4))
            output.append(chunk.payload)
        }

        return output
    }

    /// Decompresses framed multi-chunk buffer in parallel across available CPU cores.
    public static func decompressParallel(
        _ compressedData: Data,
        threadBudget: Int = ProcessInfo.processInfo.activeProcessorCount
    ) async throws -> Data {
        if compressedData.isEmpty {
            return Data()
        }

        guard compressedData.count >= 20 else {
            throw TTZipCodecError.decompressionFailed(status: -1)
        }

        // Parse Frame Header
        let magic = compressedData.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 0, as: UInt32.self).littleEndian }
        guard magic == TTZIP_BLOCK_MAGIC else {
            throw TTZipCodecError.decompressionFailed(status: -2)
        }

        let rawAlg = compressedData[6]
        let algorithm = algorithm(fromCode: rawAlg)
        let totalChunks = Int(compressedData.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 8, as: UInt32.self).littleEndian })
        let totalOriginalSize = Int(compressedData.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 12, as: UInt64.self).littleEndian })

        var offset = 20
        var chunkDescriptors: [(index: Int, uncompSize: Int, crc32: UInt32, payload: Data)] = []
        chunkDescriptors.reserveCapacity(totalChunks)

        for i in 0..<totalChunks {
            guard offset + 12 <= compressedData.count else {
                throw TTZipCodecError.decompressionFailed(status: -3)
            }
            let uSize = Int(compressedData.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: offset, as: UInt32.self).littleEndian })
            let cSize = Int(compressedData.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: offset + 4, as: UInt32.self).littleEndian })
            let crc = compressedData.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: offset + 8, as: UInt32.self).littleEndian }
            offset += 12

            guard offset + cSize <= compressedData.count else {
                throw TTZipCodecError.decompressionFailed(status: -4)
            }
            let chunkPayload = compressedData.subdata(in: offset..<offset + cSize)
            offset += cSize

            chunkDescriptors.append((index: i, uncompSize: uSize, crc32: crc, payload: chunkPayload))
        }

        // Parallel decompression task group
        let decompressedChunks: [(index: Int, data: Data)] = try await withThrowingTaskGroup(
            of: (index: Int, data: Data).self
        ) { group in
            for descriptor in chunkDescriptors {
                group.addTask {
                    let decomp = try TTZipCodec.decompress(
                        descriptor.payload,
                        algorithm: algorithm,
                        expectedUncompressedSize: descriptor.uncompSize
                    )
                    guard decomp.count == descriptor.uncompSize else {
                        throw TTZipCodecError.decompressionFailed(status: -5)
                    }

                    // Verify CRC32
                    let actualCrc = decomp.withUnsafeBytes { buf in
                        ttzip_rust_crc32(0, buf.baseAddress?.assumingMemoryBound(to: UInt8.self), decomp.count)
                    }
                    guard actualCrc == descriptor.crc32 else {
                        throw TTZipCodecError.decompressionFailed(status: -6)
                    }

                    return (index: descriptor.index, data: decomp)
                }
            }

            var results: [(index: Int, data: Data)] = []
            results.reserveCapacity(totalChunks)
            for try await result in group {
                results.append(result)
            }
            return results.sorted { $0.index < $1.index }
        }

        var output = Data()
        output.reserveCapacity(totalOriginalSize)
        for chunk in decompressedChunks {
            output.append(chunk.data)
        }

        return output
    }

    // MARK: - Code Mapping Helpers

    private static func algorithmCode(for algorithm: TTZipCodecAlgorithm) -> UInt8 {
        switch algorithm {
        case .deflate: return 1
        case .zlib: return 2
        case .gzip: return 3
        case .zstd: return 4
        case .brotli: return 5
        case .lzma: return 6
        case .fastLzma2: return 7
        case .lz4: return 8
        case .snappyBlock: return 9
        case .snappyFramed: return 10
        case .lzfse: return 11
        case .bzip2: return 12
        case .ppmd: return 13
        }
    }

    private static func algorithm(fromCode code: UInt8) -> TTZipCodecAlgorithm {
        switch code {
        case 1: return .deflate
        case 2: return .zlib
        case 3: return .gzip
        case 4: return .zstd
        case 5: return .brotli
        case 6: return .lzma
        case 7: return .fastLzma2
        case 8: return .lz4
        case 9: return .snappyBlock
        case 10: return .snappyFramed
        case 11: return .lzfse
        case 12: return .bzip2
        case 13: return .ppmd
        default: return .zstd
        }
    }
}
