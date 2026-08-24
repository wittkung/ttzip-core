// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation
import CTTZipBridge

public enum EngineExecutionTag: String, Sendable, Equatable, CaseIterable {
    case rustRayonParallelZip = "RustRayonParallelZip"
    case rustStreamingParallelZip = "RustStreamingParallelZip"
    case rustZeroCopy7zDecoder = "RustZeroCopy7zDecoder"
    case rustPure7zEncoder = "RustPure7zEncoder"
    case rustTarStreamEngine = "RustTarStreamEngine"
    case rustInPlaceZip = "RustInPlaceZip"
    case rustInPlaceSevenZip = "RustInPlaceSevenZip"
    case rustVfsParallelScanner = "RustVfsParallelScanner"
    case libarchiveLegacy = "LibarchiveLegacy"
    case cli7zFallback = "Cli7zFallback"
    case systemTarFallback = "SystemTarFallback"
    case unknown = "Unknown"

    public var isPureRust: Bool {
        switch self {
        case .rustRayonParallelZip, .rustStreamingParallelZip,
             .rustZeroCopy7zDecoder, .rustPure7zEncoder,
             .rustTarStreamEngine, .rustInPlaceZip,
             .rustInPlaceSevenZip, .rustVfsParallelScanner:
            return true
        default:
            return false
        }
    }
}

/// End-to-end non-forgeable engine execution provenance telemetry.
public struct EngineDispatchProvenance: Sendable, Equatable {
    public let engineTag: EngineExecutionTag
    public let threadCount: Int
    public let uncompressedBytes: Int64
    public let compressedBytes: Int64
    public let kernelDurationNanos: UInt64
    public let isFallback: Bool
    public let fallbackReason: String?
    public let ffiBridgeOverheadNanos: UInt64
    public let totalE2EDurationNanos: UInt64

    public init(
        engineTag: EngineExecutionTag,
        threadCount: Int,
        uncompressedBytes: Int64,
        compressedBytes: Int64,
        kernelDurationNanos: UInt64,
        isFallback: Bool,
        fallbackReason: String?,
        ffiBridgeOverheadNanos: UInt64,
        totalE2EDurationNanos: UInt64
    ) {
        self.engineTag = engineTag
        self.threadCount = threadCount
        self.uncompressedBytes = uncompressedBytes
        self.compressedBytes = compressedBytes
        self.kernelDurationNanos = kernelDurationNanos
        self.isFallback = isFallback
        self.fallbackReason = fallbackReason
        self.ffiBridgeOverheadNanos = ffiBridgeOverheadNanos
        self.totalE2EDurationNanos = totalE2EDurationNanos
    }

    public var compressionRatio: Double {
        guard uncompressedBytes > 0 else { return 1.0 }
        return Double(compressedBytes) / Double(uncompressedBytes)
    }

    public var throughputMBs: Double {
        let seconds = Double(totalE2EDurationNanos) / 1_000_000_000.0
        guard seconds > 0 else { return 0.0 }
        return (Double(uncompressedBytes) / (1024 * 1024)) / seconds
    }
}
