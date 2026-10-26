// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import os

/// High-throughput, low-contention progress coalescer and 60FPS rate-limiter.
///
/// Designed to decouple millions of fine-grained worker thread progress emissions (e.g. compression,
/// extraction, hashing) from UI rendering layers by coalescing updates into OSAllocatedUnfairLock
/// snapshots and throttling emissions to target display refresh rates (default: 60 FPS / ~16.6ms).
public final class AtomicProgressCoalescer: @unchecked Sendable {

    /// Coalesced progress snapshot at a discrete point in time.
    public struct Snapshot: Sendable, Equatable {
        public let processedBytes: Int64
        public let totalBytes: Int64
        public let fractionCompleted: Double
        public let currentEntry: String?
        public let throughputMBs: Double
        public let timestampNanos: UInt64
        public let isFinished: Bool

        public init(
            processedBytes: Int64 = 0,
            totalBytes: Int64 = 0,
            currentEntry: String? = nil,
            throughputMBs: Double = 0.0,
            timestampNanos: UInt64 = 0,
            isFinished: Bool = false
        ) {
            self.processedBytes = max(0, processedBytes)
            self.totalBytes = max(0, totalBytes)
            self.fractionCompleted = totalBytes > 0
                ? min(1.0, max(0.0, Double(processedBytes) / Double(totalBytes)))
                : 0.0
            self.currentEntry = currentEntry
            self.throughputMBs = (throughputMBs.isNaN || throughputMBs.isInfinite || throughputMBs < 0)
                ? 0.0
                : throughputMBs
            self.timestampNanos = timestampNanos
            self.isFinished = isFinished
        }

        public static let zero = Snapshot()
    }

    private struct State: Sendable {
        var latest: Snapshot
        var lastEmittedSnapshot: Snapshot?
        var lastEmittedNanos: UInt64
        var hasPendingUpdate: Bool
        var updateCount: UInt64
        var initialNanos: UInt64
    }

    /// Target refresh frame rate in Hertz (default: 60.0 FPS).
    public let targetFrameRate: Double

    /// Minimum duration between emitted progress events in nanoseconds (~16,666,666 ns for 60 FPS).
    public let minIntervalNanos: UInt64

    private let stateLock: OSAllocatedUnfairLock<State>
    private let onEmitHandler: (@Sendable (Snapshot) -> Void)?
    private let continuationLock: OSAllocatedUnfairLock<[UUID: AsyncStream<Snapshot>.Continuation]>

    /// Initializes a coalescer with a specified target frame rate and optional emission handler.
    ///
    /// - Parameters:
    ///   - targetFrameRate: Maximum emission frequency in FPS (default: 60.0).
    ///   - onEmit: Optional callback invoked on emission boundaries when updates occur.
    public init(
        targetFrameRate: Double = 60.0,
        onEmit: (@Sendable (Snapshot) -> Void)? = nil
    ) {
        let fps = max(1.0, min(240.0, targetFrameRate))
        self.targetFrameRate = fps
        self.minIntervalNanos = UInt64(1_000_000_000.0 / fps)
        self.onEmitHandler = onEmit

        let now = clock_gettime_nsec_np(CLOCK_UPTIME_RAW)
        self.stateLock = OSAllocatedUnfairLock(initialState: State(
            latest: .zero,
            lastEmittedSnapshot: nil,
            lastEmittedNanos: 0,
            hasPendingUpdate: false,
            updateCount: 0,
            initialNanos: now
        ))
        self.continuationLock = OSAllocatedUnfairLock(initialState: [:])
    }

    /// Submits a high-frequency progress update non-blockingly.
    /// Throttles execution such that downstream handlers receive at most `targetFrameRate` events per second,
    /// while guaranteeing that completion (`isFinished == true`) is emitted immediately.
    ///
    /// - Parameters:
    ///   - processedBytes: Number of bytes processed so far.
    ///   - totalBytes: Total expected workload in bytes.
    ///   - currentEntry: Optional name or path of current file entry.
    ///   - isFinished: Set to true if the workload has completed or terminated.
    public func update(
        processedBytes: Int64,
        totalBytes: Int64,
        currentEntry: String? = nil,
        isFinished: Bool = false
    ) {
        let now = clock_gettime_nsec_np(CLOCK_UPTIME_RAW)

        let snapshotToEmit: Snapshot? = stateLock.withLock { state in
            state.updateCount += 1

            // Compute throughput based on elapsed time since first submission
            let elapsedNanos = now > state.initialNanos ? (now - state.initialNanos) : 1
            let elapsedSec = Double(elapsedNanos) / 1_000_000_000.0
            let throughput = elapsedSec > 0.001
                ? (Double(processedBytes) / (1024.0 * 1024.0)) / elapsedSec
                : 0.0

            let snapshot = Snapshot(
                processedBytes: processedBytes,
                totalBytes: totalBytes,
                currentEntry: currentEntry ?? state.latest.currentEntry,
                throughputMBs: throughput,
                timestampNanos: now,
                isFinished: isFinished
            )

            state.latest = snapshot
            state.hasPendingUpdate = true

            // Determine if emission condition is met
            let elapsedSinceEmission = now >= state.lastEmittedNanos ? (now - state.lastEmittedNanos) : minIntervalNanos
            if isFinished || elapsedSinceEmission >= minIntervalNanos {
                state.lastEmittedNanos = now
                state.lastEmittedSnapshot = snapshot
                state.hasPendingUpdate = false
                return snapshot
            }
            return nil
        }

        if let snapshot = snapshotToEmit {
            dispatchSnapshot(snapshot)
        }
    }

    /// Submits an `ArchiveProgress` update structure non-blockingly.
    public func update(_ progress: ArchiveProgress) {
        let isDone: Bool
        switch progress.state {
        case .completed, .cancelled, .failed:
            isDone = true
        case .idle, .processing:
            isDone = false
        }
        update(
            processedBytes: progress.bytesProcessed,
            totalBytes: progress.totalBytes,
            currentEntry: progress.currentFileName.isEmpty ? nil : progress.currentFileName,
            isFinished: isDone
        )
    }

    /// Polls and samples the current progress state at 60FPS display sync rate.
    /// Returns the latest snapshot if new data has arrived since the last sample, or `nil` if idle.
    ///
    /// - Parameter force: If true, returns the latest snapshot even if no new update was submitted.
    /// - Returns: Coalesced snapshot if updated or forced; otherwise nil.
    public func sample(force: Bool = false) -> Snapshot? {
        let now = clock_gettime_nsec_np(CLOCK_UPTIME_RAW)

        let sampled: Snapshot? = stateLock.withLock { state in
            guard state.hasPendingUpdate || force else {
                return nil
            }
            state.hasPendingUpdate = false
            state.lastEmittedNanos = now
            state.lastEmittedSnapshot = state.latest
            return state.latest
        }

        if let snapshot = sampled {
            dispatchToStreams(snapshot)
        }
        return sampled
    }

    /// Flushes any pending un-emitted progress updates immediately.
    ///
    /// - Parameter force: If true, emits the current snapshot regardless of pending flag.
    /// - Returns: The emitted snapshot if any.
    @discardableResult
    public func flush(force: Bool = false) -> Snapshot? {
        let now = clock_gettime_nsec_np(CLOCK_UPTIME_RAW)

        let flushed: Snapshot? = stateLock.withLock { state in
            guard state.hasPendingUpdate || force else {
                return nil
            }
            state.hasPendingUpdate = false
            state.lastEmittedNanos = now
            state.lastEmittedSnapshot = state.latest
            return state.latest
        }

        if let snapshot = flushed {
            dispatchSnapshot(snapshot)
        }
        return flushed
    }

    /// Returns an atomic snapshot of the current progress without clearing pending flags.
    public var currentSnapshot: Snapshot {
        stateLock.withLock { $0.latest }
    }

    /// Returns the cumulative update count submitted to this coalescer.
    public var updateCount: UInt64 {
        stateLock.withLock { $0.updateCount }
    }

    /// Resets the coalescer to its initial zero state.
    public func reset() {
        let now = clock_gettime_nsec_np(CLOCK_UPTIME_RAW)
        stateLock.withLock { state in
            state.latest = .zero
            state.lastEmittedSnapshot = nil
            state.lastEmittedNanos = 0
            state.hasPendingUpdate = false
            state.updateCount = 0
            state.initialNanos = now
        }
    }

    /// Creates an `AsyncStream` that receives throttled progress snapshots.
    public func makeAsyncStream() -> AsyncStream<Snapshot> {
        let id = UUID()
        return AsyncStream { continuation in
            continuationLock.withLock { continuations in
                continuations[id] = continuation
            }
            continuation.onTermination = { [weak self] _ in
                _ = self?.continuationLock.withLock { continuations in
                    continuations.removeValue(forKey: id)
                }
            }
        }
    }

    private func dispatchSnapshot(_ snapshot: Snapshot) {
        onEmitHandler?(snapshot)
        dispatchToStreams(snapshot)
    }

    private func dispatchToStreams(_ snapshot: Snapshot) {
        let continuations = continuationLock.withLock { Array($0.values) }
        for continuation in continuations {
            continuation.yield(snapshot)
            if snapshot.isFinished {
                continuation.finish()
            }
        }
    }
}
