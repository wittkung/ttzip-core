// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

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

        let fallbackReason = withUnsafeBytes(of: raw.fallback_reason) { rawBuf -> String? in
            guard let ptr = rawBuf.baseAddress?.assumingMemoryBound(to: CChar.self), ptr.pointee != 0 else {
                return nil
            }
            return String(cString: ptr)
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
