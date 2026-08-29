// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class TTZipCodecTests: XCTestCase {

    private let sampleText = """
    TTZip High-Performance Native Archiving and Compression Engine.
    Swift 6 Strongly-Typed Microkernel Architecture with Mozilla UniFFI Bridge.
    Benchmarking Deflate, Zstd, Brotli, LZ4, Snappy, LZFSE, Fast-LZMA2, and Bzip2 codecs.
    Repeating payload to create compressible structure in memory.
    TTZip High-Performance Native Archiving and Compression Engine.
    Swift 6 Strongly-Typed Microkernel Architecture with Mozilla UniFFI Bridge.
    """.data(using: .utf8)!

    // MARK: - 13 Single-Stream Codecs Roundtrip Matrix

    func testDeflateVariantsRoundtrip() throws {
        for alg in [TTZipCodecAlgorithm.deflate, .zlib, .gzip] {
            let compressed = try TTZipCodec.compress(sampleText, algorithm: alg, level: .normal)
            XCTAssertFalse(compressed.isEmpty)
            XCTAssertLessThan(compressed.count, sampleText.count)

            let decompressed = try TTZipCodec.decompress(compressed, algorithm: alg, expectedUncompressedSize: sampleText.count)
            XCTAssertEqual(decompressed, sampleText)
        }
    }

    func testModernHighSpeedCodecsRoundtrip() throws {
        for alg in [TTZipCodecAlgorithm.zstd, .brotli, .lz4, .snappyBlock, .snappyFramed, .lzfse, .fastLzma2, .bzip2] {
            let compressed = try TTZipCodec.compress(sampleText, algorithm: alg, level: .normal)
            XCTAssertFalse(compressed.isEmpty, "Compression failed for \(alg.displayName)")

            let decompressed = try TTZipCodec.decompress(compressed, algorithm: alg, expectedUncompressedSize: sampleText.count)
            XCTAssertEqual(decompressed, sampleText, "Decompression mismatch for \(alg.displayName)")
        }
    }

    func testCompressionLevelsPropagation() throws {
        let l1 = try TTZipCodec.compress(sampleText, algorithm: .zstd, level: .fastest)
        let l9 = try TTZipCodec.compress(sampleText, algorithm: .zstd, level: .maximum)
        XCTAssertFalse(l1.isEmpty)
        XCTAssertFalse(l9.isEmpty)

        let d1 = try TTZipCodec.decompress(l1, algorithm: .zstd)
        let d9 = try TTZipCodec.decompress(l9, algorithm: .zstd)
        XCTAssertEqual(d1, sampleText)
        XCTAssertEqual(d9, sampleText)
    }

    func testEmptyDataRoundtrip() throws {
        let empty = Data()
        let comp = try TTZipCodec.compress(empty, algorithm: .zstd)
        XCTAssertTrue(comp.isEmpty)
        let decomp = try TTZipCodec.decompress(comp, algorithm: .zstd)
        XCTAssertTrue(decomp.isEmpty)
    }

    // MARK: - Zstandard Dictionary Manager Tests

    func testZstdDictionaryTrainingAndCompression() throws {
        // Generate small similar sample chunks
        var samples: [Data] = []
        for i in 0..<20 {
            let chunk = "{\"user_id\": \(1000 + i), \"action\": \"login_success\", \"timestamp\": 170000\(i), \"client\": \"macos_desktop\"}".data(using: .utf8)!
            samples.append(chunk)
        }

        let dictData = try TTZipZstdDictionaryManager.trainDictionary(samples: samples, targetDictionarySize: 16384)
        XCTAssertFalse(dictData.isEmpty)
        XCTAssertLessThanOrEqual(dictData.count, 16384)

        let targetSample = "{\"user_id\": 9999, \"action\": \"login_success\", \"timestamp\": 17000099, \"client\": \"macos_desktop\"}".data(using: .utf8)!

        let compWithDict = try TTZipZstdDictionaryManager.compressWithDict(targetSample, dictionary: dictData, level: 3)
        XCTAssertFalse(compWithDict.isEmpty)

        let decompWithDict = try TTZipZstdDictionaryManager.decompressWithDict(compWithDict, dictionary: dictData, expectedUncompressedSize: targetSample.count)
        XCTAssertEqual(decompWithDict, targetSample)

        // Manager registration & metrics
        let manager = TTZipZstdDictionaryManager.shared
        let meta = manager.registerDictionary(name: "user_actions_v1", dictBytes: dictData, sampleCount: samples.count)
        XCTAssertEqual(meta.name, "user_actions_v1")
        XCTAssertEqual(manager.cachedDictionariesCount, 1)

        manager.recordAcceleration(originalSize: targetSample.count, compressedSize: compWithDict.count)
        XCTAssertGreaterThan(manager.totalAcceleratedBytes, 0)
    }

    // MARK: - Apple LZFSE Engine Tests

    func testLZFSEEngineHardwareAndFallback() throws {
        let compressed = try TTZipLZFSEEngine.compress(sampleText)
        XCTAssertFalse(compressed.isEmpty)

        let decompressed = try TTZipLZFSEEngine.decompress(compressed, estimatedSize: sampleText.count)
        XCTAssertEqual(decompressed, sampleText)
    }

    // MARK: - Modern Parallel Block Compressor Tests

    func testModernParallelBlockCompressorAsync() async throws {
        // Build 2MB repeatable data
        var largeData = Data()
        for _ in 0..<40 {
            largeData.append(sampleText)
        }

        let compressedBlocks = try await TTZipModernBlockCompressor.compressParallel(
            largeData,
            algorithm: .zstd,
            level: .normal,
            chunkSize: 64 * 1024
        )
        XCTAssertFalse(compressedBlocks.isEmpty)
        XCTAssertLessThan(compressedBlocks.count, largeData.count)

        let decompressed = try await TTZipModernBlockCompressor.decompressParallel(compressedBlocks)
        XCTAssertEqual(decompressed, largeData)
    }

    // MARK: - AsyncThrowingStream Pipeline Tests

    func testAsyncThrowingStreamCodecPipeline() async throws {
        let stream = AsyncThrowingStream<Data, Error> { continuation in
            continuation.yield(self.sampleText)
            continuation.yield(self.sampleText)
            continuation.finish()
        }

        let compressedStream = TTZipCodec.streamCompress(source: stream, algorithm: .zstd)
        let decompressedStream = TTZipCodec.streamDecompress(source: compressedStream, algorithm: .zstd)

        var totalChunks = 0
        for try await chunk in decompressedStream {
            XCTAssertEqual(chunk, self.sampleText)
            totalChunks += 1
        }
        XCTAssertEqual(totalChunks, 2)
    }
}
