// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
import os
@testable import TTZipCore

final class TTZipZopfliOptimizationServiceTests: XCTestCase {

    var service: TTZipZopfliOptimizationService!

    override func setUp() {
        super.setUp()
        service = TTZipZopfliOptimizationService()
        service.clearHistory()
    }

    override func tearDown() {
        service.clearHistory()
        service = nil
        super.tearDown()
    }

    private func makeSampleData(text: String, count: Int) -> Data {
        var data = Data()
        let textData = Data(text.utf8)
        for _ in 0..<count {
            data.append(textData)
        }
        return data
    }

    // MARK: - 1. Preset Configuration Tests

    func testZopfliPresetsConfiguration() {
        let fastOpts = ZopfliPreset.fast.uniffiOptions
        XCTAssertEqual(fastOpts.iterationCount, 5)
        XCTAssertEqual(fastOpts.maximumBlockSplits, 5)
        XCTAssertTrue(fastOpts.blockSplitting)

        let balancedOpts = ZopfliPreset.balanced.uniffiOptions
        XCTAssertEqual(balancedOpts.iterationCount, 15)
        XCTAssertEqual(balancedOpts.maximumBlockSplits, 15)

        let maxOpts = ZopfliPreset.maximum.uniffiOptions
        XCTAssertEqual(maxOpts.iterationCount, 30)
        XCTAssertEqual(maxOpts.maximumBlockSplits, 30)

        let ultraOpts = ZopfliPreset.ultra.uniffiOptions
        XCTAssertEqual(ultraOpts.iterationCount, 100)
        XCTAssertEqual(ultraOpts.maximumBlockSplits, 50)

        let customPreset = ZopfliPreset.custom(
            iterationCount: 8,
            maximumBlockSplits: 4,
            iterationsWithoutImprovement: 3,
            blockSplitting: false
        )
        let customOpts = customPreset.uniffiOptions
        XCTAssertEqual(customOpts.iterationCount, 8)
        XCTAssertEqual(customOpts.maximumBlockSplits, 4)
        XCTAssertEqual(customOpts.iterationsWithoutImprovement, 3)
        XCTAssertFalse(customOpts.blockSplitting)
    }

    // MARK: - 2. Synchronous Compression & Roundtrip Tests

    func testZopfliSynchronousOptimizeAllFormats() throws {
        let sampleData = makeSampleData(
            text: "TTZip Google Zopfli Extreme Compression Service Test Payload 2026 - Deflate Zlib Gzip\n",
            count: 20
        )

        for format in ZopfliFormat.allCases {
            let (compressed, stats) = try service.optimize(
                data: sampleData,
                format: format,
                preset: .fast
            )

            XCTAssertFalse(compressed.isEmpty, "Compressed data for \(format.rawValue) must not be empty")
            XCTAssertEqual(stats.format, format)
            XCTAssertEqual(stats.uncompressedSize, Int64(sampleData.count))
            XCTAssertEqual(stats.compressedSize, Int64(compressed.count))
            XCTAssertLessThan(stats.compressionRatio, 100.0)
            XCTAssertGreaterThan(stats.throughputMBs, 0.0)

            // Decompress and verify roundtrip
            let decompressed = try service.decompress(data: compressed, format: format)
            XCTAssertEqual(decompressed, sampleData, "Decompressed data must match original input for \(format.rawValue)")

            // Verify with verifyRoundtrip helper
            let isLossless = try service.verifyRoundtrip(data: sampleData, format: format, preset: .fast)
            XCTAssertTrue(isLossless, "verifyRoundtrip must return true for \(format.rawValue)")
        }
    }

    // MARK: - 3. Asynchronous Background Compression Tests

    func testZopfliAsyncBackgroundCompressionWithProgress() async throws {
        let sampleData = makeSampleData(
            text: "Asynchronous structured concurrency background Zopfli compression streaming test.\n",
            count: 30
        )

        let eventCount = OSAllocatedUnfairLock(initialState: 0)

        let (compressed, stats) = try await service.optimizeAsync(
            data: sampleData,
            format: .zlib,
            preset: .fast
        ) { _ in
            eventCount.withLock { $0 += 1 }
        }

        XCTAssertFalse(compressed.isEmpty)
        XCTAssertEqual(stats.format, ZopfliFormat.zlib)
        XCTAssertEqual(stats.uncompressedSize, Int64(sampleData.count))

        let decompressed = try service.decompress(data: compressed, format: .zlib)
        XCTAssertEqual(decompressed, sampleData)
    }

    // MARK: - 4. Cooperative Task Cancellation Tests

    func testZopfliAsyncCancellation() async throws {
        let sampleData = makeSampleData(
            text: "Long repetitive payload designed for testing cancellation during Zopfli optimization.\n",
            count: 100
        )

        let testService = self.service!
        let task = Task {
            try await testService.optimizeAsync(
                data: sampleData,
                format: .deflate,
                preset: .ultra
            )
        }

        // Cancel task
        task.cancel()

        do {
            _ = try await task.value
        } catch {
            // Task cancellation caught cleanly
            XCTAssertTrue(error is CancellationError || error is TtZipError)
        }
    }

    // MARK: - 5. Multi-Core Parallel Chunk Optimization Tests

    func testZopfliParallelChunkOptimization() async throws {
        let chunks: [Data] = (0..<4).map { idx in
            makeSampleData(
                text: "Parallel multi-core Zopfli chunk #\(idx) with distinct redundant pattern.\n",
                count: 25
            )
        }

        let completedCounter = OSAllocatedUnfairLock(initialState: 0)
        let results = try await service.optimizeParallelChunks(
            chunks: chunks,
            format: .gzip,
            preset: .fast,
            maxConcurrency: 4
        ) { _, _ in
            completedCounter.withLock { $0 += 1 }
        }

        XCTAssertEqual(results.count, chunks.count, "All parallel chunks must be processed")
        XCTAssertEqual(completedCounter.withLock { $0 }, chunks.count)

        for (idx, result) in results.enumerated() {
            let decompressed = try service.decompress(data: result.data, format: .gzip)
            XCTAssertEqual(decompressed, chunks[idx], "Chunk #\(idx) roundtrip decompression must match")
        }
    }

    // MARK: - 6. Benchmark & Diagnostic Telemetry Tests

    func testZopfliBenchmarkTelemetry() throws {
        let sampleData = makeSampleData(
            text: "Benchmarking telemetry metrics and ratio accounting for Zopfli optimizer.\n",
            count: 15
        )

        let stats = try service.benchmark(data: sampleData, format: .deflate, preset: .fast)

        XCTAssertEqual(stats.format, ZopfliFormat.deflate)
        XCTAssertEqual(stats.uncompressedSize, Int64(sampleData.count))
        XCTAssertGreaterThan(stats.compressedSize, 0)
        XCTAssertGreaterThan(stats.throughputMBs, 0.0)
        XCTAssertEqual(service.recentStats.count, 1)
        XCTAssertEqual(service.totalBytesInput, Int64(sampleData.count))
    }

    // MARK: - 7. State Management & Clear History Tests

    func testZopfliStateManagementAndHistoryClear() throws {
        let sampleData = makeSampleData(
            text: "State management and history reset test string.\n",
            count: 10
        )

        _ = try service.optimize(data: sampleData, format: .deflate, preset: .fast)
        _ = try service.optimize(data: sampleData, format: .zlib, preset: .fast)

        XCTAssertEqual(service.recentStats.count, 2)
        XCTAssertEqual(service.totalBytesInput, Int64(sampleData.count * 2))
        XCTAssertFalse(service.isOptimizing)
        XCTAssertEqual(service.activeTasksCount, 0)

        service.clearHistory()

        XCTAssertEqual(service.recentStats.count, 0)
        XCTAssertEqual(service.totalBytesInput, 0)
        XCTAssertEqual(service.totalBytesCompressed, 0)
        XCTAssertNil(service.latestError)
    }
}
