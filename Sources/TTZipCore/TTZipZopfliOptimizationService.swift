// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation
import os

// MARK: - Enums & Configuration Models

/// Target container format for Zopfli extreme compression.
public enum ZopfliFormat: String, Sendable, CaseIterable, Identifiable {
    /// Raw RFC 1951 Deflate byte stream without headers or checksums.
    case deflate = "Deflate"
    /// RFC 1950 Zlib container with Adler-32 verification checksum.
    case zlib = "Zlib"
    /// RFC 1952 Gzip container with CRC-32 checksum and timestamp headers.
    case gzip = "Gzip"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .deflate:
            return "Raw DEFLATE (RFC 1951)"
        case .zlib:
            return "Zlib Container (RFC 1950)"
        case .gzip:
            return "Gzip Stream (RFC 1952)"
        }
    }

    internal var uniffiFormat: UniFfiZopfliFormat {
        switch self {
        case .deflate:
            return .deflate
        case .zlib:
            return .zlib
        case .gzip:
            return .gzip
        }
    }

    internal init(from uniffi: UniFfiZopfliFormat) {
        switch uniffi {
        case .deflate:
            self = .deflate
        case .zlib:
            self = .zlib
        case .gzip:
            self = .gzip
        }
    }
}

/// Predefined optimization profiles balancing CPU compression time vs extreme ratio.
public enum ZopfliPreset: Sendable, Equatable, Hashable {
    /// Fast pass: 5 iterations, 5 block splits.
    case fast
    /// Balanced standard pass: 15 iterations, 15 block splits.
    case balanced
    /// Maximum density pass: 30 iterations, 30 block splits.
    case maximum
    /// Ultra extreme pass: 100 iterations, 50 block splits.
    case ultra
    /// Custom parameter tuning configuration.
    case custom(iterationCount: UInt64, maximumBlockSplits: UInt16, iterationsWithoutImprovement: UInt64, blockSplitting: Bool)

    public var displayName: String {
        switch self {
        case .fast:
            return "Fast (5 iterations)"
        case .balanced:
            return "Balanced (15 iterations)"
        case .maximum:
            return "Maximum (30 iterations)"
        case .ultra:
            return "Ultra (100 iterations)"
        case .custom(let iter, _, _, _):
            return "Custom (\(iter) iterations)"
        }
    }

    public var uniffiOptions: UniFfiZopfliOptions {
        switch self {
        case .fast:
            return uniffiZopfliOptionsForPreset(preset: .fast)
        case .balanced:
            return uniffiZopfliOptionsForPreset(preset: .balanced)
        case .maximum:
            return uniffiZopfliOptionsForPreset(preset: .maximum)
        case .ultra:
            return uniffiZopfliOptionsForPreset(preset: .ultra)
        case .custom(let iterations, let splits, let noImprovement, let splitting):
            return UniFfiZopfliOptions(
                iterationCount: iterations,
                iterationsWithoutImprovement: noImprovement,
                maximumBlockSplits: splits,
                blockSplitting: splitting
            )
        }
    }
}

/// Compression performance telemetry and analytical metrics for Zopfli runs.
public struct ZopfliCompressionStats: Sendable, Identifiable, Equatable {
    public var id: UUID
    public var format: ZopfliFormat
    public var uncompressedSize: Int64
    public var compressedSize: Int64
    public var compressionRatio: Double
    public var durationNanoseconds: UInt64
    public var throughputMBs: Double
    public var iterations: UInt64
    public var timestamp: Date

    public init(
        id: UUID = UUID(),
        format: ZopfliFormat,
        uncompressedSize: Int64,
        compressedSize: Int64,
        compressionRatio: Double,
        durationNanoseconds: UInt64,
        throughputMBs: Double,
        iterations: UInt64,
        timestamp: Date = Date()
    ) {
        self.id = id
        self.format = format
        self.uncompressedSize = uncompressedSize
        self.compressedSize = compressedSize
        self.compressionRatio = compressionRatio
        self.durationNanoseconds = durationNanoseconds
        self.throughputMBs = throughputMBs
        self.iterations = iterations
        self.timestamp = timestamp
    }

    internal init(from uniffi: UniFfiZopfliStats) {
        self.id = UUID()
        self.format = ZopfliFormat(from: uniffi.format)
        self.uncompressedSize = Int64(uniffi.uncompressedSize)
        self.compressedSize = Int64(uniffi.compressedSize)
        self.compressionRatio = uniffi.compressionRatio
        self.durationNanoseconds = uniffi.durationNanos
        self.throughputMBs = uniffi.throughputMbs
        self.iterations = uniffi.iterations
        self.timestamp = Date()
    }
}

/// Progress event emitted during fine-grained or long-running Zopfli jobs.
public struct ZopfliProgressEvent: Sendable {
    public let processedBytes: Int64
    public let totalBytes: Int64
    public let fractionCompleted: Double
    public let currentEntry: String?

    public init(processedBytes: Int64, totalBytes: Int64, currentEntry: String? = nil) {
        self.processedBytes = processedBytes
        self.totalBytes = totalBytes
        self.fractionCompleted = totalBytes > 0 ? Double(processedBytes) / Double(totalBytes) : 1.0
        self.currentEntry = currentEntry
    }
}

extension UniFfiCancellationToken: @unchecked Sendable {}

// MARK: - Internal Callback Adapter

private final class ProgressCallbackBridge: UniFfiProgressCallback, @unchecked Sendable {
    private let handler: @Sendable (Int64, Int64, String?) -> Bool

    init(handler: @escaping @Sendable (Int64, Int64, String?) -> Bool) {
        self.handler = handler
    }

    func onProgress(processedBytes: UInt64, totalBytes: UInt64, currentEntry: String?) -> Bool {
        handler(Int64(processedBytes), Int64(totalBytes), currentEntry)
    }
}

// MARK: - TTZipZopfliOptimizationService

/// High-level Swift 6 service for scheduling and running Google Zopfli extreme compression pipelines.
@Observable
public final class TTZipZopfliOptimizationService: @unchecked Sendable {
    // MARK: - Observable UI Properties

    /// Indicates whether one or more optimization pipelines are actively executing.
    public private(set) var isOptimizing: Bool = false

    /// Number of concurrent optimization tasks currently in flight.
    public private(set) var activeTasksCount: Int = 0

    /// Total cumulative uncompressed bytes processed by this service instance.
    public private(set) var totalBytesInput: Int64 = 0

    /// Total cumulative compressed bytes emitted by this service instance.
    public private(set) var totalBytesCompressed: Int64 = 0

    /// Recent compression telemetry history (up to 50 entries).
    public private(set) var recentStats: [ZopfliCompressionStats] = []

    /// Most recent localized error encountered.
    public private(set) var latestError: String? = nil

    // MARK: - Thread-Safe State Lock

    private struct InternalState {
        var activeCount: Int = 0
        var totalInput: Int64 = 0
        var totalOutput: Int64 = 0
        var statsHistory: [ZopfliCompressionStats] = []
    }

    private let stateLock = OSAllocatedUnfairLock(initialState: InternalState())

    // MARK: - Initialization

    public init() {}

    // MARK: - Synchronous Compression Pipeline

    /// Compresses a buffer synchronously with Zopfli using specified format and preset profile.
    public func optimize(
        data: Data,
        format: ZopfliFormat = .deflate,
        preset: ZopfliPreset = .balanced
    ) throws -> (data: Data, stats: ZopfliCompressionStats) {
        let uncompressedSize = Int64(data.count)
        let opts = preset.uniffiOptions
        let startNanos = DispatchTime.now().uptimeNanoseconds

        updateActiveTaskCount(delta: 1)
        defer { updateActiveTaskCount(delta: -1) }

        do {
            let compressed = try uniffiZopfliCompress(
                format: format.uniffiFormat,
                data: data,
                options: opts
            )
            let endNanos = DispatchTime.now().uptimeNanoseconds
            let durationNanos = endNanos - startNanos
            let compressedSize = Int64(compressed.count)

            let ratio = uncompressedSize > 0 ? (Double(compressedSize) / Double(uncompressedSize)) * 100.0 : 100.0
            let secs = Double(durationNanos) / 1_000_000_000.0
            let throughput = secs > 0.0 ? (Double(uncompressedSize) / (1024.0 * 1024.0)) / secs : 0.0

            let stat = ZopfliCompressionStats(
                format: format,
                uncompressedSize: uncompressedSize,
                compressedSize: compressedSize,
                compressionRatio: ratio,
                durationNanoseconds: durationNanos,
                throughputMBs: throughput,
                iterations: opts.iterationCount
            )

            recordSuccess(inputBytes: uncompressedSize, outputBytes: compressedSize, stat: stat)
            return (data: compressed, stats: stat)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    // MARK: - Asynchronous Background Compression Pipeline

    /// Compresses a buffer in a background cooperative task with optional progress updates and cancellation.
    public func optimizeAsync(
        data: Data,
        format: ZopfliFormat = .deflate,
        preset: ZopfliPreset = .balanced,
        onProgress: (@Sendable (ZopfliProgressEvent) -> Void)? = nil
    ) async throws -> (data: Data, stats: ZopfliCompressionStats) {
        let cancellationToken = UniFfiCancellationToken()

        return try await withTaskCancellationHandler {
            try Task.checkCancellation()

            let uncompressedSize = Int64(data.count)
            let opts = preset.uniffiOptions
            let startNanos = DispatchTime.now().uptimeNanoseconds

            self.updateActiveTaskCount(delta: 1)
            defer { self.updateActiveTaskCount(delta: -1) }

            let callback: UniFfiProgressCallback? = onProgress.map { handler in
                ProgressCallbackBridge { processed, total, entry in
                    handler(ZopfliProgressEvent(processedBytes: processed, totalBytes: total, currentEntry: entry))
                    return !Task.isCancelled
                }
            }

            let compressed = try uniffiZopfliCompressWithProgress(
                format: format.uniffiFormat,
                data: data,
                options: opts,
                callback: callback,
                cancellationToken: cancellationToken
            )

            let endNanos = DispatchTime.now().uptimeNanoseconds
            let durationNanos = endNanos - startNanos
            let compressedSize = Int64(compressed.count)

            let ratio = uncompressedSize > 0 ? (Double(compressedSize) / Double(uncompressedSize)) * 100.0 : 100.0
            let secs = Double(durationNanos) / 1_000_000_000.0
            let throughput = secs > 0.0 ? (Double(uncompressedSize) / (1024.0 * 1024.0)) / secs : 0.0

            let stat = ZopfliCompressionStats(
                format: format,
                uncompressedSize: uncompressedSize,
                compressedSize: compressedSize,
                compressionRatio: ratio,
                durationNanoseconds: durationNanos,
                throughputMBs: throughput,
                iterations: opts.iterationCount
            )

            self.recordSuccess(inputBytes: uncompressedSize, outputBytes: compressedSize, stat: stat)
            return (data: compressed, stats: stat)
        } onCancel: {
            cancellationToken.cancel()
        }
    }

    // MARK: - Multi-Core Parallel Chunk Compression

    /// Compresses multiple independent payload chunks in parallel using structured Swift Concurrency.
    public func optimizeParallelChunks(
        chunks: [Data],
        format: ZopfliFormat = .deflate,
        preset: ZopfliPreset = .balanced,
        maxConcurrency: Int = ProcessInfo.processInfo.activeProcessorCount,
        onChunkCompleted: (@Sendable (Int, ZopfliCompressionStats) -> Void)? = nil
    ) async throws -> [(data: Data, stats: ZopfliCompressionStats)] {
        guard !chunks.isEmpty else { return [] }

        let effectiveConcurrency = max(1, min(maxConcurrency, chunks.count))

        return try await withThrowingTaskGroup(
            of: (index: Int, result: (data: Data, stats: ZopfliCompressionStats)).self
        ) { group in
            var results = [(data: Data, stats: ZopfliCompressionStats)?](repeating: nil, count: chunks.count)
            var nextIndex = 0

            // Seed worker pool
            for _ in 0..<effectiveConcurrency {
                if nextIndex < chunks.count {
                    let idx = nextIndex
                    let chunk = chunks[idx]
                    nextIndex += 1

                    group.addTask {
                        let res = try self.optimize(data: chunk, format: format, preset: preset)
                        return (index: idx, result: res)
                    }
                }
            }

            // Drain and spawn remaining tasks
            while let item = try await group.next() {
                results[item.index] = item.result
                onChunkCompleted?(item.index, item.result.stats)

                if nextIndex < chunks.count {
                    let idx = nextIndex
                    let chunk = chunks[idx]
                    nextIndex += 1

                    group.addTask {
                        let res = try self.optimize(data: chunk, format: format, preset: preset)
                        return (index: idx, result: res)
                    }
                }
            }

            return results.compactMap { $0 }
        }
    }

    // MARK: - Decompression & Verification

    /// Decompresses a Zopfli-compressed byte stream back into uncompressed data.
    public func decompress(
        data: Data,
        format: ZopfliFormat = .deflate
    ) throws -> Data {
        do {
            return try uniffiZopfliDecompress(format: format.uniffiFormat, compressed: data)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Performs lossless roundtrip verification (compress + decompress check).
    public func verifyRoundtrip(
        data: Data,
        format: ZopfliFormat = .deflate,
        preset: ZopfliPreset = .fast
    ) throws -> Bool {
        try uniffiZopfliVerifyRoundtrip(
            format: format.uniffiFormat,
            data: data,
            options: preset.uniffiOptions
        )
    }

    /// Benchmarks Zopfli compression on the given payload without returning compressed payload.
    public func benchmark(
        data: Data,
        format: ZopfliFormat = .deflate,
        preset: ZopfliPreset = .fast
    ) throws -> ZopfliCompressionStats {
        let uniffiStats = try uniffiZopfliBenchmark(
            data: data,
            options: preset.uniffiOptions,
            format: format.uniffiFormat
        )
        let stat = ZopfliCompressionStats(from: uniffiStats)
        recordSuccess(inputBytes: Int64(data.count), outputBytes: stat.compressedSize, stat: stat)
        return stat
    }

    // MARK: - History & Diagnostics

    /// Clears the recorded telemetry statistics and reset diagnostic metrics.
    public func clearHistory() {
        stateLock.withLock { state in
            state.statsHistory.removeAll()
            state.totalInput = 0
            state.totalOutput = 0
        }
        self.recentStats.removeAll()
        self.totalBytesInput = 0
        self.totalBytesCompressed = 0
        self.latestError = nil
    }

    // MARK: - Private State Synchronization

    private func updateActiveTaskCount(delta: Int) {
        let (count, isRunning) = stateLock.withLock { state -> (Int, Bool) in
            state.activeCount = max(0, state.activeCount + delta)
            return (state.activeCount, state.activeCount > 0)
        }
        self.activeTasksCount = count
        self.isOptimizing = isRunning
    }

    private func recordSuccess(inputBytes: Int64, outputBytes: Int64, stat: ZopfliCompressionStats) {
        let (history, inTotal, outTotal) = stateLock.withLock { state -> ([ZopfliCompressionStats], Int64, Int64) in
            state.totalInput += inputBytes
            state.totalOutput += outputBytes
            state.statsHistory.insert(stat, at: 0)
            if state.statsHistory.count > 50 {
                state.statsHistory.removeLast()
            }
            return (state.statsHistory, state.totalInput, state.totalOutput)
        }

        self.recentStats = history
        self.totalBytesInput = inTotal
        self.totalBytesCompressed = outTotal
    }
}
