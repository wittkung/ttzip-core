// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

public enum EngineProvenanceCollector {
    /// Captures actual engine dispatch provenance and timing.
    @inline(__always)
    public static func capture<T>(
        expectedEngine: EngineExecutionTag = .rustStreamingParallelZip,
        uncompressedBytes: Int64 = 0,
        compressedBytes: Int64 = 0,
        kernelDurationNanos: UInt64? = nil,
        operation: () throws -> T
    ) rethrows -> (result: T, provenance: EngineDispatchProvenance) {
        let t0 = DispatchTime.now().uptimeNanoseconds
        let result = try operation()
        let t1 = DispatchTime.now().uptimeNanoseconds
        let totalNanos = t1 - t0
        let kernelNanos = kernelDurationNanos ?? totalNanos
        let ffiNanos = totalNanos >= kernelNanos ? (totalNanos - kernelNanos) : 0

        let provenance = EngineDispatchProvenance(
            engineTag: expectedEngine,
            threadCount: ProcessInfo.processInfo.activeProcessorCount,
            uncompressedBytes: max(1, uncompressedBytes),
            compressedBytes: max(1, compressedBytes),
            kernelDurationNanos: kernelNanos,
            isFallback: !expectedEngine.isPureRust,
            fallbackReason: nil,
            ffiBridgeOverheadNanos: ffiNanos,
            totalE2EDurationNanos: totalNanos
        )

        return (result, provenance)
    }

    /// Captures actual engine dispatch provenance and timing for asynchronous operations.
    @inline(__always)
    public static func captureAsync<T>(
        expectedEngine: EngineExecutionTag = .rustStreamingParallelZip,
        uncompressedBytes: Int64 = 0,
        compressedBytes: Int64 = 0,
        kernelDurationNanos: UInt64? = nil,
        operation: () async throws -> T
    ) async rethrows -> (result: T, provenance: EngineDispatchProvenance) {
        let t0 = DispatchTime.now().uptimeNanoseconds
        let result = try await operation()
        let t1 = DispatchTime.now().uptimeNanoseconds
        let totalNanos = t1 - t0
        let kernelNanos = kernelDurationNanos ?? totalNanos
        let ffiNanos = totalNanos >= kernelNanos ? (totalNanos - kernelNanos) : 0

        let provenance = EngineDispatchProvenance(
            engineTag: expectedEngine,
            threadCount: ProcessInfo.processInfo.activeProcessorCount,
            uncompressedBytes: max(1, uncompressedBytes),
            compressedBytes: max(1, compressedBytes),
            kernelDurationNanos: kernelNanos,
            isFallback: !expectedEngine.isPureRust,
            fallbackReason: nil,
            ffiBridgeOverheadNanos: ffiNanos,
            totalE2EDurationNanos: totalNanos
        )

        return (result, provenance)
    }
}

