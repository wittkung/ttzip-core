// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

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
}

