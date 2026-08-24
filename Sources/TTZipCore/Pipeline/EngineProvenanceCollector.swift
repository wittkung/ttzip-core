// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation
import CTTZipBridge

public enum EngineProvenanceCollector {
    /// Captures actual engine dispatch provenance and timing across the FFI boundary.
    @inline(__always)
    public static func capture<T>(
        operation: () throws -> T
    ) rethrows -> (result: T, provenance: EngineDispatchProvenance) {
        let t0 = DispatchTime.now().uptimeNanoseconds
        let result = try operation()
        let t1 = DispatchTime.now().uptimeNanoseconds
        let totalNanos = t1 - t0

        var raw = TTZipExecutionProvenance()
        let ok = ttzip_rust_get_last_execution_provenance(&raw)

        let tag: EngineExecutionTag
        if ok {
            let cName = ttzip_rust_engine_tag_name(raw.engine_tag)
            let nameStr = cName != nil ? String(cString: cName!) : "Unknown"
            tag = EngineExecutionTag(rawValue: nameStr) ?? .unknown
        } else {
            tag = .unknown
        }

        let fallbackReason = withUnsafePointer(to: raw.fallback_reason) { ptr in
            ptr.withMemoryRebound(to: CChar.self, capacity: 128) { cStr in
                cStr.pointee != 0 ? String(cString: cStr) : nil
            }
        }

        let kernelNanos = raw.kernel_duration_nanos
        let ffiTax = totalNanos > kernelNanos ? (totalNanos - kernelNanos) : 0

        let provenance = EngineDispatchProvenance(
            engineTag: tag,
            threadCount: Int(raw.thread_count),
            uncompressedBytes: Int64(raw.uncompressed_bytes),
            compressedBytes: Int64(raw.compressed_bytes),
            kernelDurationNanos: kernelNanos,
            isFallback: raw.is_fallback,
            fallbackReason: fallbackReason,
            ffiBridgeOverheadNanos: ffiTax,
            totalE2EDurationNanos: totalNanos
        )

        return (result, provenance)
    }
}
