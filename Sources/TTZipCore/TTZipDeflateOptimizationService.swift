// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation
import os

// MARK: - Enums & Models

/// Dual Deflate execution engine options.
public enum DeflateEngine: String, Sendable, CaseIterable, Identifiable {
    /// Hardware SIMD vectorised C-libdeflate implementation for peak throughput.
    case libdeflateHardware = "LibdeflateHardware"
    /// Pure-Rust Near-Optimal DP OptParser implementation for peak compression ratio.
    case pureRustNearOptimalDp = "PureRustNearOptimalDp"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .libdeflateHardware:
            return "Hardware SIMD (Libdeflate)"
        case .pureRustNearOptimalDp:
            return "Near-Optimal DP (Pure-Rust)"
        }
    }

    internal var uniffiEngine: UniFfiDeflateEngine {
        switch self {
        case .libdeflateHardware:
            return .libdeflateHardware
        case .pureRustNearOptimalDp:
            return .pureRustNearOptimalDp
        }
    }

    internal init(from uniffi: UniFfiDeflateEngine) {
        switch uniffi {
        case .libdeflateHardware:
            self = .libdeflateHardware
        case .pureRustNearOptimalDp:
            self = .pureRustNearOptimalDp
        }
    }
}

/// Dynamic arbitration strategy for Deflate engine selection.
public enum DeflateArbitrationStrategy: String, Sendable, CaseIterable, Identifiable {
    /// Always choose hardware-accelerated engine for highest throughput.
    case speedFirst = "SpeedFirst"
    /// Always choose pure-Rust near-optimal DP engine for maximum compression.
    case ratioFirst = "RatioFirst"
    /// Balance speed and ratio based on payload size threshold (64KB boundary).
    case balanced = "Balanced"
    /// Dynamically inspect payload entropy and size to optimize trade-offs.
    case dynamicAdaptive = "DynamicAdaptive"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .speedFirst:
            return "Speed First (Maximum Throughput)"
        case .ratioFirst:
            return "Ratio First (Maximum Compression)"
        case .balanced:
            return "Balanced (Volume Thresholds)"
        case .dynamicAdaptive:
            return "Dynamic Adaptive (Entropy Aware)"
        }
    }

    internal var uniffiStrategy: UniFfiDeflateArbitrationStrategy {
        switch self {
        case .speedFirst:
            return .speedFirst
        case .ratioFirst:
            return .ratioFirst
        case .balanced:
            return .balanced
        case .dynamicAdaptive:
            return .dynamicAdaptive
        }
    }
}

/// Strongly typed compression level representation.
public enum DeflateLevel: Sendable, Equatable, Hashable {
    /// Level 0: Pure Store uncompressed blocks.
    case store
    /// Level 1: Fast greedy matching.
    case fast
    /// Level 6: Balanced standard Deflate.
    case normal
    /// Level 9: Maximum lazy evaluation.
    case maximum
    /// Level 12: Ultra Near-Optimal DP with EM refinement.
    case ultraDp
    /// Custom level in range 0..=12.
    case custom(Int32)

    public var rawLevel: Int32 {
        switch self {
        case .store:
            return 0
        case .fast:
            return 1
        case .normal:
            return 6
        case .maximum:
            return 9
        case .ultraDp:
            return 12
        case .custom(let val):
            return min(12, max(0, val))
        }
    }

    internal var uniffiLevel: UniFfiDeflateLevel {
        switch self {
        case .store:
            return .store
        case .fast:
            return .fast
        case .normal:
            return .defaultLevel
        case .maximum:
            return .maximum
        case .ultraDp:
            return .ultraDp
        case .custom(let val):
            return .custom(level: min(12, max(0, val)))
        }
    }
}

/// Performance telemetry snapshot for a single compression run.
public struct DeflateCompressionStats: Sendable, Identifiable, Equatable {
    public let id: UUID
    public let engine: DeflateEngine
    public let uncompressedSize: Int64
    public let compressedSize: Int64
    public let compressionRatio: Double
    public let durationNanoseconds: UInt64
    public let throughputMBs: Double
    public let timestamp: Date

    public var spaceSavingPercent: Double {
        max(0.0, 100.0 - compressionRatio)
    }

    public var durationMilliseconds: Double {
        Double(durationNanoseconds) / 1_000_000.0
    }

    public init(
        id: UUID = UUID(),
        engine: DeflateEngine,
        uncompressedSize: Int64,
        compressedSize: Int64,
        compressionRatio: Double,
        durationNanoseconds: UInt64,
        throughputMBs: Double,
        timestamp: Date = Date()
    ) {
        self.id = id
        self.engine = engine
        self.uncompressedSize = uncompressedSize
        self.compressedSize = compressedSize
        self.compressionRatio = compressionRatio
        self.durationNanoseconds = durationNanoseconds
        self.throughputMBs = throughputMBs
        self.timestamp = timestamp
    }

    internal init(from uniffi: UniFfiDeflateStats) {
        self.id = UUID()
        self.engine = DeflateEngine(from: uniffi.engine)
        self.uncompressedSize = Int64(uniffi.uncompressedSize)
        self.compressedSize = Int64(uniffi.compressedSize)
        self.compressionRatio = uniffi.compressionRatio
        self.durationNanoseconds = uniffi.durationNanos
        self.throughputMBs = uniffi.throughputMbs
        self.timestamp = Date()
    }
}

/// 8 Representative mathematical synthetic corpus types.
public enum SyntheticCorpusType: String, Sendable, CaseIterable, Identifiable {
    case allZeros = "AllZeros"
    case textRedundant = "TextRedundant"
    case highlyRepetitive = "HighlyRepetitive"
    case uniformRandom = "UniformRandom"
    case lowEntropyNibbles = "LowEntropyNibbles"
    case asciiSourceCode = "AsciiSourceCode"
    case binaryExecutable = "BinaryExecutable"
    case exponentialDecay = "ExponentialDecay"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .allZeros:
            return "Zero Fill (Zero Entropy)"
        case .textRedundant:
            return "Structured Log Stream (Redundant)"
        case .highlyRepetitive:
            return "Repeated ASCII Phrases"
        case .uniformRandom:
            return "Uniform Random (~8.0 b/B)"
        case .lowEntropyNibbles:
            return "Low-Entropy Nibbles (~2.0 b/B)"
        case .asciiSourceCode:
            return "AST / JSON Source Code"
        case .binaryExecutable:
            return "Mach-O Executable Bytecode"
        case .exponentialDecay:
            return "Zipfian Power-Law Decay"
        }
    }

    internal var uniffiType: UniFfiSyntheticCorpusType {
        switch self {
        case .allZeros:
            return .allZeros
        case .textRedundant:
            return .textRedundant
        case .highlyRepetitive:
            return .highlyRepetitive
        case .uniformRandom:
            return .uniformRandom
        case .lowEntropyNibbles:
            return .lowEntropyNibbles
        case .asciiSourceCode:
            return .asciiSourceCode
        case .binaryExecutable:
            return .binaryExecutable
        case .exponentialDecay:
            return .exponentialDecay
        }
    }
}

/// Comparative dual-engine benchmark metrics container.
public struct DeflateBenchmarkComparison: Sendable, Identifiable, Equatable {
    public let id: UUID
    public let corpusType: SyntheticCorpusType?
    public let payloadSize: Int64
    public let hardwareStats: DeflateCompressionStats
    public let rustDpStats: DeflateCompressionStats

    public var speedupFactor: Double {
        if rustDpStats.durationNanoseconds > 0 {
            return Double(rustDpStats.durationNanoseconds) / Double(max(1, hardwareStats.durationNanoseconds))
        }
        return 1.0
    }

    public var compressionGainPercent: Double {
        if hardwareStats.compressedSize > 0 {
            let diff = Double(hardwareStats.compressedSize - rustDpStats.compressedSize)
            return (diff / Double(hardwareStats.compressedSize)) * 100.0
        }
        return 0.0
    }

    public var winningEngineForRatio: DeflateEngine {
        if rustDpStats.compressedSize <= hardwareStats.compressedSize {
            return .pureRustNearOptimalDp
        }
        return .libdeflateHardware
    }

    public var winningEngineForSpeed: DeflateEngine {
        if hardwareStats.throughputMBs >= rustDpStats.throughputMBs {
            return .libdeflateHardware
        }
        return .pureRustNearOptimalDp
    }

    public init(
        id: UUID = UUID(),
        corpusType: SyntheticCorpusType? = nil,
        payloadSize: Int64,
        hardwareStats: DeflateCompressionStats,
        rustDpStats: DeflateCompressionStats
    ) {
        self.id = id
        self.corpusType = corpusType
        self.payloadSize = payloadSize
        self.hardwareStats = hardwareStats
        self.rustDpStats = rustDpStats
    }
}

// MARK: - TTZipDeflateOptimizationService

/// Primary Swift 6 `@Observable` and `Sendable` dual-engine optimization and benchmark coordinator.
///
/// Orchestrates dynamic arbitration between C-libdeflate hardware vectorisation and pure-Rust
/// near-optimal DP parsing, providing reactive telemetry tracking and synthetic benchmark capabilities.
@Observable
public final class TTZipDeflateOptimizationService: @unchecked Sendable {

    // MARK: - Shared Singleton

    public static let shared = TTZipDeflateOptimizationService()

    // MARK: - Observable Public Properties

    /// Active arbitration strategy.
    public private(set) var defaultStrategy: DeflateArbitrationStrategy = .dynamicAdaptive

    /// Recent compression execution performance records.
    public private(set) var recentStats: [DeflateCompressionStats] = []

    /// Recent comparative benchmark results.
    public private(set) var recentComparisons: [DeflateBenchmarkComparison] = []

    /// Total number of operations processed.
    public private(set) var totalOperationsCount: Int = 0

    /// Cumulative uncompressed bytes compressed.
    public private(set) var totalBytesCompressed: Int64 = 0

    /// Cumulative bytes decompressed.
    public private(set) var totalBytesDecompressed: Int64 = 0

    /// Indicates whether a synthetic matrix benchmark is actively running.
    public private(set) var isBenchmarking: Bool = false

    /// Most recent error captured during execution.
    public private(set) var latestError: String? = nil

    // MARK: - Internal Synchronized State

    @ObservationIgnored
    private let stateLock = OSAllocatedUnfairLock(initialState: InternalState())

    private struct InternalState {
        var recentStats: [DeflateCompressionStats] = []
        var recentComparisons: [DeflateBenchmarkComparison] = []
        var totalOperations: Int = 0
        var totalCompressed: Int64 = 0
        var totalDecompressed: Int64 = 0
    }

    // MARK: - Initialization

    public init() {}

    // MARK: - Strategy Management

    /// Updates the default dynamic arbitration strategy.
    public func setStrategy(_ strategy: DeflateArbitrationStrategy) {
        self.defaultStrategy = strategy
    }

    /// Selects optimal Deflate engine according to current or overridden strategy.
    public func arbitrateEngine(
        for data: Data,
        strategy: DeflateArbitrationStrategy? = nil
    ) -> DeflateEngine {
        let activeStrategy = strategy ?? defaultStrategy
        let entropy = estimateShannonEntropy(data: data)
        let uniffiChoice = uniffiDeflateDualArbitrate(
            strategy: activeStrategy.uniffiStrategy,
            uncompressedSize: UInt64(data.count),
            estimatedEntropy: entropy
        )
        return DeflateEngine(from: uniffiChoice)
    }

    // MARK: - Compression & Decompression Operations

    /// Compresses buffer using chosen or arbitrated Deflate engine.
    @discardableResult
    public func compress(
        _ data: Data,
        engine: DeflateEngine? = nil,
        level: DeflateLevel = .normal,
        strategy: DeflateArbitrationStrategy? = nil
    ) throws -> (data: Data, stats: DeflateCompressionStats) {
        let selectedEngine = engine ?? arbitrateEngine(for: data, strategy: strategy)
        let startTime = DispatchTime.now()

        do {
            let compressed = try uniffiDeflateDualCompress(
                engine: selectedEngine.uniffiEngine,
                src: data,
                level: level.uniffiLevel
            )
            let endTime = DispatchTime.now()
            let nanos = endTime.uptimeNanoseconds - startTime.uptimeNanoseconds

            let ratio = data.isEmpty ? 100.0 : (Double(compressed.count) / Double(data.count)) * 100.0
            let secs = Double(nanos) / 1_000_000_000.0
            let throughput = secs > 0.0 ? (Double(data.count) / (1024.0 * 1024.0)) / secs : 0.0

            let stat = DeflateCompressionStats(
                engine: selectedEngine,
                uncompressedSize: Int64(data.count),
                compressedSize: Int64(compressed.count),
                compressionRatio: ratio,
                durationNanoseconds: nanos,
                throughputMBs: throughput
            )

            recordOperation(compressedBytes: Int64(data.count), stat: stat)
            return (data: compressed, stats: stat)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Decompresses raw Deflate payload into expected uncompressed buffer.
    public func decompress(
        _ data: Data,
        expectedSize: Int64,
        engine: DeflateEngine = .libdeflateHardware
    ) throws -> Data {
        do {
            let decompressed = try uniffiDeflateDualDecompress(
                engine: engine.uniffiEngine,
                src: data,
                expectedUncompressedSize: UInt64(max(0, expectedSize))
            )
            recordDecompression(decompressedBytes: Int64(decompressed.count))
            return decompressed
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Performs lossless roundtrip verification.
    public func verifyRoundtrip(
        _ data: Data,
        level: DeflateLevel = .normal
    ) throws -> Bool {
        try uniffiDeflateDualVerifyRoundtrip(src: data, level: level.uniffiLevel)
    }

    // MARK: - Benchmarking & Synthetic Corpora

    /// Generates deterministic mathematical synthetic benchmark payload.
    public func generateSyntheticCorpus(
        type: SyntheticCorpusType,
        sizeBytes: Int64,
        seed: UInt64? = nil
    ) -> Data {
        uniffiGenerateSyntheticCorpus(
            corpusType: type.uniffiType,
            sizeBytes: UInt64(max(0, sizeBytes)),
            seed: seed
        )
    }

    /// Executes head-to-head dual-engine benchmark on supplied payload.
    public func benchmark(
        data: Data,
        level: DeflateLevel = .normal,
        corpusType: SyntheticCorpusType? = nil
    ) throws -> DeflateBenchmarkComparison {
        let uniffiStats = try uniffiDeflateDualBenchmark(src: data, level: level.uniffiLevel)

        guard let hwUniffi = uniffiStats.first(where: { $0.engine == .libdeflateHardware }),
              let dpUniffi = uniffiStats.first(where: { $0.engine == .pureRustNearOptimalDp }) else {
            throw TtZipError.EngineError(code: -1)
        }

        let comparison = DeflateBenchmarkComparison(
            corpusType: corpusType,
            payloadSize: Int64(data.count),
            hardwareStats: DeflateCompressionStats(from: hwUniffi),
            rustDpStats: DeflateCompressionStats(from: dpUniffi)
        )

        recordComparison(comparison)
        return comparison
    }

    /// Runs all 8 synthetic benchmark corpus types and returns comparative matrices.
    public func runSyntheticMatrixBenchmark(
        sizePerCorpus: Int64 = 65536,
        level: DeflateLevel = .normal
    ) async throws -> [DeflateBenchmarkComparison] {
        self.isBenchmarking = true
        defer { self.isBenchmarking = false }

        var results: [DeflateBenchmarkComparison] = []
        for corpusType in SyntheticCorpusType.allCases {
            let corpusData = generateSyntheticCorpus(
                type: corpusType,
                sizeBytes: sizePerCorpus,
                seed: 0xCAFE_BABE_1234_5678
            )
            let comp = try benchmark(data: corpusData, level: level, corpusType: corpusType)
            results.append(comp)
        }
        return results
    }

    /// Resets captured performance history.
    public func clearHistory() {
        stateLock.withLock { state in
            state.recentStats.removeAll()
            state.recentComparisons.removeAll()
        }
        self.recentStats.removeAll()
        self.recentComparisons.removeAll()
        self.latestError = nil
    }

    // MARK: - Private State Sync

    private func recordOperation(compressedBytes: Int64, stat: DeflateCompressionStats) {
        let (allStats, count, totalComp) = stateLock.withLock { state -> ([DeflateCompressionStats], Int, Int64) in
            state.recentStats.insert(stat, at: 0)
            if state.recentStats.count > 50 {
                state.recentStats.removeLast()
            }
            state.totalOperations += 1
            state.totalCompressed += compressedBytes
            return (state.recentStats, state.totalOperations, state.totalCompressed)
        }

        self.recentStats = allStats
        self.totalOperationsCount = count
        self.totalBytesCompressed = totalComp
    }

    private func recordDecompression(decompressedBytes: Int64) {
        let (count, totalDecomp) = stateLock.withLock { state -> (Int, Int64) in
            state.totalOperations += 1
            state.totalDecompressed += decompressedBytes
            return (state.totalOperations, state.totalDecompressed)
        }
        self.totalOperationsCount = count
        self.totalBytesDecompressed = totalDecomp
    }

    private func recordComparison(_ comp: DeflateBenchmarkComparison) {
        let allComparisons = stateLock.withLock { state -> [DeflateBenchmarkComparison] in
            state.recentComparisons.insert(comp, at: 0)
            if state.recentComparisons.count > 50 {
                state.recentComparisons.removeLast()
            }
            return state.recentComparisons
        }
        self.recentComparisons = allComparisons
    }
}
