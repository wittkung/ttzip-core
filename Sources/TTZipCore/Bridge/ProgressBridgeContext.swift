// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation
import CTTZipBridge

/// Thread-safe context object passed as `user_data` pointer to C-ABI callbacks.
/// Provides nanosecond-precision monotonic clock throttling (<= 60Hz) to prevent UI thread saturation.
public final class ProgressBridgeContext: @unchecked Sendable {
    public let progressHandler: (@Sendable (ArchiveProgress) -> Void)?
    public let handle: TaskExecutionHandle?
    public let startTime: CFAbsoluteTime
    public let totalExpectedBytes: Int64
    private var lastEmitTime: UInt64 = 0
    private let minIntervalNs: UInt64 = 16_666_667 // ~60 Hz (16.6 ms)
    private let lock = NSLock()

    public init(
        progressHandler: (@Sendable (ArchiveProgress) -> Void)?,
        handle: TaskExecutionHandle?,
        totalExpectedBytes: Int64
    ) {
        self.progressHandler = progressHandler
        self.handle = handle
        self.totalExpectedBytes = max(1, totalExpectedBytes)
        self.startTime = CFAbsoluteTimeGetCurrent()
    }

    /// Evaluates monotonic time gate to determine if progress should be dispatched.
    public func shouldEmit() -> Bool {
        let now = clock_gettime_nsec_np(CLOCK_MONOTONIC_RAW)
        lock.lock()
        defer { lock.unlock() }
        if now - lastEmitTime >= minIntervalNs {
            lastEmitTime = now
            return true
        }
        return false
    }
}

/// Standard C-ABI progress bridge callback conforming to `TTZipProgressCallback`.
public func ttzipProgressCallbackBridge(
    processedBytes: UInt64,
    totalBytes: UInt64,
    currentEntry: UnsafePointer<CChar>?,
    userData: UnsafeMutableRawPointer?
) -> Bool {
    guard let userData = userData else { return true }
    let ctx = Unmanaged<ProgressBridgeContext>.fromOpaque(userData).takeUnretainedValue()

    // 1. Cancellation check: Returning false immediately signals Rust kernel loop to abort
    if ctx.handle?.isCancelled == true || Task.isCancelled {
        return false
    }

    guard let handler = ctx.progressHandler else { return true }

    let effectiveTotal = totalBytes > 0 ? Int64(totalBytes) : ctx.totalExpectedBytes
    let processed = Int64(processedBytes)
    let isKeyFrame = (processedBytes == 0 || processedBytes >= UInt64(effectiveTotal))

    // 2. 60Hz Monotonic time gate or keyframe
    if isKeyFrame || ctx.shouldEmit() {
        let now = CFAbsoluteTimeGetCurrent()
        let elapsed = max(0.001, now - ctx.startTime)
        let throughput = (Double(processed) / (1024.0 * 1024.0)) / elapsed
        let fileName = currentEntry != nil ? String(cString: currentEntry!) : ""
        let progress = ArchiveProgress(
            state: isKeyFrame && processedBytes >= UInt64(effectiveTotal) ? .completed : .processing,
            bytesProcessed: processed,
            totalBytes: effectiveTotal,
            currentFileName: fileName,
            throughputMBs: throughput
        )
        handler(progress)
    }

    return true
}
