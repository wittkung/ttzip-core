// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class TTZipDeflateOptimizationServiceTests: XCTestCase {

    var service: TTZipDeflateOptimizationService!

    override func setUp() {
        super.setUp()
        service = TTZipDeflateOptimizationService()
        service.clearHistory()
    }

    override func tearDown() {
        service.clearHistory()
        service = nil
        super.tearDown()
    }

    // MARK: - 1. Synthetic Corpus Generation Tests

    func testSyntheticCorpusGenerationAll8Types() {
        let requestedSize: Int64 = 8192

        for corpusType in SyntheticCorpusType.allCases {
            let corpusData = service.generateSyntheticCorpus(
                type: corpusType,
                sizeBytes: requestedSize,
                seed: 0x1234_5678
            )

            XCTAssertEqual(
                Int64(corpusData.count),
                requestedSize,
                "Generated corpus for \(corpusType.rawValue) must have exact requested size"
            )

            // Verify determinism with identical seed
            let identicalData = service.generateSyntheticCorpus(
                type: corpusType,
                sizeBytes: requestedSize,
                seed: 0x1234_5678
            )
            XCTAssertEqual(
                corpusData,
                identicalData,
                "Synthetic generation must be 100% deterministic for identical seeds"
            )
        }
    }

    // MARK: - 2. Dual-Engine Compression & Decompression Roundtrip Tests

    func testDualEngineCompressionAndDecompressionRoundtrip() throws {
        let sampleData = service.generateSyntheticCorpus(
            type: .textRedundant,
            sizeBytes: 16384,
            seed: 0xABCD_EF01
        )

        let levels: [DeflateLevel] = [.store, .fast, .normal, .maximum, .ultraDp]

        for engine in DeflateEngine.allCases {
            for level in levels {
                let (compressed, stats) = try service.compress(
                    sampleData,
                    engine: engine,
                    level: level
                )

                XCTAssertFalse(compressed.isEmpty, "Compressed payload must not be empty for engine \(engine.rawValue)")
                XCTAssertEqual(stats.engine, engine)
                XCTAssertEqual(stats.uncompressedSize, Int64(sampleData.count))
                XCTAssertEqual(stats.compressedSize, Int64(compressed.count))
                XCTAssertGreaterThan(stats.throughputMBs, 0.0)

                // Decompress and verify roundtrip integrity
                let decompressed = try service.decompress(
                    compressed,
                    expectedSize: Int64(sampleData.count),
                    engine: engine
                )

                XCTAssertEqual(
                    decompressed,
                    sampleData,
                    "Decompressed data must match original exactly for \(engine.rawValue) at level \(level.rawLevel)"
                )
            }
        }
    }

    // MARK: - 3. Arbitration Strategy Dispatch Tests

    func testArbitrationStrategyDispatch() {
        let smallData = service.generateSyntheticCorpus(type: .asciiSourceCode, sizeBytes: 1024)
        let largeData = service.generateSyntheticCorpus(type: .asciiSourceCode, sizeBytes: 1024 * 1024)

        // SpeedFirst must always pick Hardware SIMD
        let speedChoice = service.arbitrateEngine(for: smallData, strategy: .speedFirst)
        XCTAssertEqual(speedChoice, .libdeflateHardware)

        // RatioFirst must always pick PureRust Near-Optimal DP
        let ratioChoice = service.arbitrateEngine(for: smallData, strategy: .ratioFirst)
        XCTAssertEqual(ratioChoice, .pureRustNearOptimalDp)

        // Balanced: small payload selects PureRust DP, large payload selects Hardware
        let balancedSmall = service.arbitrateEngine(for: smallData, strategy: .balanced)
        XCTAssertEqual(balancedSmall, .pureRustNearOptimalDp)

        let balancedLarge = service.arbitrateEngine(for: largeData, strategy: .balanced)
        XCTAssertEqual(balancedLarge, .libdeflateHardware)
    }

    // MARK: - 4. Head-to-Head Benchmark Comparison Tests

    func testHeadToHeadBenchmarkComparison() throws {
        let testPayload = service.generateSyntheticCorpus(
            type: .asciiSourceCode,
            sizeBytes: 32768,
            seed: 0x9988_7766
        )

        let comparison = try service.benchmark(
            data: testPayload,
            level: .normal,
            corpusType: .asciiSourceCode
        )

        XCTAssertEqual(comparison.payloadSize, 32768)
        XCTAssertEqual(comparison.corpusType, .asciiSourceCode)
        XCTAssertGreaterThan(comparison.hardwareStats.compressedSize, 0)
        XCTAssertGreaterThan(comparison.rustDpStats.compressedSize, 0)
        XCTAssertGreaterThan(comparison.hardwareStats.throughputMBs, 0.0)
        XCTAssertGreaterThan(comparison.rustDpStats.throughputMBs, 0.0)
        XCTAssertGreaterThan(comparison.speedupFactor, 0.0)

        XCTAssertEqual(service.recentComparisons.count, 1)
        XCTAssertEqual(service.recentComparisons.first?.id, comparison.id)
    }

    // MARK: - 5. Full Synthetic Matrix Benchmark Async Tests

    func testFullSyntheticMatrixBenchmarkAsync() async throws {
        let results = try await service.runSyntheticMatrixBenchmark(
            sizePerCorpus: 8192,
            level: .normal
        )

        XCTAssertEqual(
            results.count,
            SyntheticCorpusType.allCases.count,
            "Matrix benchmark must generate comparisons for all 8 synthetic corpus types"
        )

        for comparison in results {
            XCTAssertNotNil(comparison.corpusType)
            XCTAssertEqual(comparison.payloadSize, 8192)
            XCTAssertGreaterThan(comparison.hardwareStats.compressedSize, 0)
            XCTAssertGreaterThan(comparison.rustDpStats.compressedSize, 0)
        }
    }

    // MARK: - 6. Roundtrip Verification API Tests

    func testRoundtripVerificationAPI() throws {
        let payload = service.generateSyntheticCorpus(
            type: .highlyRepetitive,
            sizeBytes: 16384,
            seed: 0x5555_AAAA
        )

        let isValid = try service.verifyRoundtrip(payload, level: .normal)
        XCTAssertTrue(isValid, "Lossless roundtrip verification must succeed across both engines")
    }

    // MARK: - 7. Service State Tracking & History Clearing Tests

    func testServiceStateTrackingAndClearHistory() throws {
        let payload = service.generateSyntheticCorpus(
            type: .allZeros,
            sizeBytes: 4096
        )

        XCTAssertEqual(service.totalOperationsCount, 0)
        XCTAssertEqual(service.totalBytesCompressed, 0)
        XCTAssertEqual(service.totalBytesDecompressed, 0)

        let (compressed, _) = try service.compress(payload, engine: .libdeflateHardware, level: .fast)
        XCTAssertEqual(service.totalOperationsCount, 1)
        XCTAssertEqual(service.totalBytesCompressed, 4096)
        XCTAssertEqual(service.recentStats.count, 1)

        _ = try service.decompress(compressed, expectedSize: 4096, engine: .libdeflateHardware)
        XCTAssertEqual(service.totalOperationsCount, 2)
        XCTAssertEqual(service.totalBytesDecompressed, 4096)

        service.clearHistory()
        XCTAssertEqual(service.recentStats.count, 0)
        XCTAssertEqual(service.recentComparisons.count, 0)
    }
}
