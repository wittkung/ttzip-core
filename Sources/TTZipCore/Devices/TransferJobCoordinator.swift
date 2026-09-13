// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// Supported transfer directions for files and streaming extraction.
public enum AndroidTransferDirection: String, Sendable, Hashable, Codable, CaseIterable, CustomStringConvertible {
    /// Upload payload from host macOS to Android device storage.
    case macToAndroid = "MacToAndroid"
    /// Download payload from Android device storage to host macOS.
    case androidToMac = "AndroidToMac"
    /// Directly decompress archive stream into Android storage without local disk staging.
    case directPipelineExtract = "DirectPipelineExtract"

    public var description: String {
        rawValue
    }
}

/// Execution status of a file transfer or streaming extraction job.
public enum AndroidTransferStatus: String, Sendable, Hashable, Codable, CaseIterable, CustomStringConvertible {
    /// Job is enqueued waiting for an available execution slot.
    case queued = "Queued"
    /// Active payload transmission in progress.
    case transferring = "Transferring"
    /// Transmission temporarily paused by user or transient network jitter.
    case paused = "Paused"
    /// Cancellation requested; cleaning up partial remote state.
    case cancelling = "Cancelling"
    /// Successfully transferred all bytes and finalized media scan.
    case completed = "Completed"
    /// Operation failed due to I/O error or pipe stall.
    case failed = "Failed"

    public var description: String {
        rawValue
    }

    /// Indicates whether the status represents an active operational state.
    public var isActive: Bool {
        self == .transferring || self == .cancelling
    }

    /// Indicates whether the status represents a final terminal state.
    public var isTerminal: Bool {
        self == .completed || self == .failed
    }
}

/// Thread-safe entity model representing an ongoing or completed transfer or direct pipeline job.
public struct AndroidTransferJob: Identifiable, Sendable, Hashable, Codable {
    /// UUID uniquely identifying the transfer task.
    public let jobId: String
    /// Transfer direction and pipeline mode.
    public let direction: AndroidTransferDirection
    /// Local or remote source path.
    public let sourcePath: String
    /// Local or remote destination path.
    public let destinationPath: String
    /// Total byte count of files to transfer.
    public let totalBytes: UInt64
    /// Number of bytes transferred or decompressed so far.
    public var transferredBytes: UInt64
    /// Smoothed rolling throughput in bytes per second.
    public var currentSpeedBps: UInt64
    /// Current lifecycle execution status.
    public var status: AndroidTransferStatus
    /// Optional failure description if status is .failed.
    public var errorMessage: String?

    /// Conformance to Identifiable using jobId.
    public var id: String {
        jobId
    }

    /// Normalized completion progress ratio between 0.0 and 1.0.
    public var progressRatio: Double {
        guard totalBytes > 0 else { return 0.0 }
        return min(max(Double(transferredBytes) / Double(totalBytes), 0.0), 1.0)
    }

    /// Estimated remaining duration in seconds based on smoothed currentSpeedBps.
    public var estimatedRemainingSeconds: TimeInterval? {
        guard currentSpeedBps > 0, totalBytes > transferredBytes else { return nil }
        let remainingBytes = totalBytes - transferredBytes
        return TimeInterval(remainingBytes) / TimeInterval(currentSpeedBps)
    }

    /// Memberwise initializer for AndroidTransferJob.
    public init(
        jobId: String = UUID().uuidString,
        direction: AndroidTransferDirection,
        sourcePath: String,
        destinationPath: String,
        totalBytes: UInt64,
        transferredBytes: UInt64 = 0,
        currentSpeedBps: UInt64 = 0,
        status: AndroidTransferStatus = .queued,
        errorMessage: String? = nil
    ) {
        self.jobId = jobId
        self.direction = direction
        self.sourcePath = sourcePath
        self.destinationPath = destinationPath
        self.totalBytes = totalBytes
        self.transferredBytes = transferredBytes
        self.currentSpeedBps = currentSpeedBps
        self.status = status
        self.errorMessage = errorMessage
    }
}

/// Internal bookkeeping state for an individual tracked transfer task.
private struct JobInternalContext {
    var job: AndroidTransferJob
    var lastSampleTimestamp: TimeInterval
    var lastSampleTransferredBytes: UInt64
    var cancellationTask: Task<Void, Never>?
}

/// Swift 6 Actor coordinating concurrent Android transfer jobs, live speed smoothing, and cancellation.
public actor TransferJobCoordinator {
    /// Shared singleton instance for host-wide transfer coordination.
    public static let shared = TransferJobCoordinator()

    /// Maximum number of concurrently transferring jobs (default 2).
    public let maxConcurrentJobs: Int

    /// Smoothing alpha factor for Exponential Moving Average (EMA) rate calculations.
    private let emaAlpha: Double = 0.25

    /// Active tracked job registry keyed by jobId.
    private var jobContexts: [String: JobInternalContext] = [:]

    /// Active state stream subscriber continuations keyed by subscription UUID.
    private var continuations: [UUID: AsyncStream<[AndroidTransferJob]>.Continuation] = [:]

    /// Creates a new `TransferJobCoordinator` actor instance with configurable concurrency limit.
    /// - Parameter maxConcurrentJobs: Maximum allowed simultaneous active jobs (minimum 1).
    public init(maxConcurrentJobs: Int = 2) {
        self.maxConcurrentJobs = max(1, maxConcurrentJobs)
    }

    /// Enqueues a new transfer or streaming extraction job.
    /// - Parameter job: The initial transfer job model to enqueue.
    /// - Returns: The registered transfer job.
    @discardableResult
    public func enqueueJob(_ job: AndroidTransferJob) -> AndroidTransferJob {
        let now = Date().timeIntervalSince1970
        let context = JobInternalContext(
            job: job,
            lastSampleTimestamp: now,
            lastSampleTransferredBytes: job.transferredBytes,
            cancellationTask: nil
        )
        jobContexts[job.jobId] = context
        scheduleNextJobs()
        broadcastUpdate()
        return job
    }

    /// Updates incremental transfer progress and recalculates smoothed throughput via EMA.
    /// - Parameters:
    ///   - jobId: Unique identifier of the target job.
    ///   - transferredBytes: Total cumulative bytes transferred up to this moment.
    ///   - totalBytes: Optional updated total byte count if size became known dynamically.
    public func updateProgress(jobId: String, transferredBytes: UInt64, totalBytes: UInt64? = nil) {
        guard var context = jobContexts[jobId] else { return }

        let now = Date().timeIntervalSince1970
        let timeDelta = max(0.001, now - context.lastSampleTimestamp)
        let bytesDelta = transferredBytes >= context.lastSampleTransferredBytes
            ? transferredBytes - context.lastSampleTransferredBytes
            : 0

        let instantSpeedBps = Double(bytesDelta) / timeDelta

        // Apply Exponential Moving Average (EMA) smoothing to eliminate speed oscillation
        let currentEma = Double(context.job.currentSpeedBps)
        let smoothedSpeed: Double
        if currentEma <= 0.0 {
            smoothedSpeed = instantSpeedBps
        } else {
            smoothedSpeed = (emaAlpha * instantSpeedBps) + ((1.0 - emaAlpha) * currentEma)
        }

        context.job.transferredBytes = transferredBytes
        if let total = totalBytes {
            context.job = AndroidTransferJob(
                jobId: context.job.jobId,
                direction: context.job.direction,
                sourcePath: context.job.sourcePath,
                destinationPath: context.job.destinationPath,
                totalBytes: total,
                transferredBytes: transferredBytes,
                currentSpeedBps: UInt64(smoothedSpeed.rounded()),
                status: context.job.status,
                errorMessage: context.job.errorMessage
            )
        } else {
            context.job.currentSpeedBps = UInt64(smoothedSpeed.rounded())
        }

        context.lastSampleTimestamp = now
        context.lastSampleTransferredBytes = transferredBytes
        jobContexts[jobId] = context

        broadcastUpdate()
    }

    /// Transitions an existing job into a terminal completed or failed state.
    /// - Parameters:
    ///   - jobId: Unique identifier of the job.
    ///   - status: Completed or Failed status.
    ///   - errorMessage: Optional error description if failed.
    public func finalizeJob(jobId: String, status: AndroidTransferStatus, errorMessage: String? = nil) {
        guard var context = jobContexts[jobId] else { return }

        context.job.status = status
        context.job.currentSpeedBps = 0
        context.job.errorMessage = errorMessage
        if status == .completed {
            context.job.transferredBytes = context.job.totalBytes
        }
        context.cancellationTask?.cancel()
        context.cancellationTask = nil
        jobContexts[jobId] = context

        scheduleNextJobs()
        broadcastUpdate()
    }

    /// Requests cooperative cancellation of an active or queued transfer job.
    /// - Parameter jobId: Unique identifier of the job to cancel.
    /// - Returns: True if the job was located and marked for cancellation, false otherwise.
    @discardableResult
    public func cancelJob(jobId: String) -> Bool {
        guard var context = jobContexts[jobId] else { return false }

        if context.job.status.isTerminal {
            return false
        }

        context.job.status = .cancelling
        context.job.currentSpeedBps = 0
        context.cancellationTask?.cancel()
        context.cancellationTask = nil
        jobContexts[jobId] = context

        broadcastUpdate()

        // Transition to terminal failed status with cancellation marker
        finalizeJob(jobId: jobId, status: .failed, errorMessage: "Cancelled by user")
        return true
    }

    /// Retrieves an immutable snapshot array of all currently tracked transfer jobs.
    /// - Returns: Array of AndroidTransferJob instances.
    public func getJobs() -> [AndroidTransferJob] {
        jobContexts.values.map(\.job).sorted { $0.jobId < $1.jobId }
    }

    /// Retrieves a specific transfer job by its unique identifier.
    /// - Parameter jobId: The unique job identifier.
    /// - Returns: The matching AndroidTransferJob if present, nil otherwise.
    public func getJob(jobId: String) -> AndroidTransferJob? {
        jobContexts[jobId]?.job
    }

    /// Clears completed or failed jobs from the coordinator registry.
    public func clearFinishedJobs() {
        jobContexts = jobContexts.filter { !$0.value.job.status.isTerminal }
        broadcastUpdate()
    }

    /// Asynchronous stream broadcasting all active and updated transfer jobs to UI subscribers.
    public var jobUpdates: AsyncStream<[AndroidTransferJob]> {
        let (stream, continuation) = AsyncStream<[AndroidTransferJob]>.makeStream()
        let id = UUID()
        continuations[id] = continuation
        continuation.yield(getJobs())
        continuation.onTermination = { @Sendable [weak self] _ in
            Task { [weak self] in
                await self?.removeContinuation(id: id)
            }
        }
        return stream
    }

    /// Internal helper scheduling pending queued jobs up to maxConcurrentJobs.
    private func scheduleNextJobs() {
        let activeCount = jobContexts.values.filter { $0.job.status == .transferring }.count
        var availableSlots = max(0, maxConcurrentJobs - activeCount)

        for (id, var context) in jobContexts where context.job.status == .queued && availableSlots > 0 {
            context.job.status = .transferring
            jobContexts[id] = context
            availableSlots -= 1
        }
    }

    /// Removes a terminated stream continuation observer.
    private func removeContinuation(id: UUID) {
        continuations.removeValue(forKey: id)
    }

    /// Broadcasts the current jobs snapshot array to all registered stream observers.
    private func broadcastUpdate() {
        let snapshot = getJobs()
        for continuation in continuations.values {
            continuation.yield(snapshot)
        }
    }

    // MARK: - Native UniFFI Pipeline Execution

    /// Executes a direct pipeline extraction task via background UniFFI driver.
    public func executeDirectExtraction(
        jobId: String,
        archivePath: String,
        destinationDeviceId: String,
        destinationDir: String
    ) {
        let task = Task.detached { [weak self] in
            do {
                let _ = try uniffiExtractToDevice(
                    archivePath: archivePath,
                    destinationDeviceId: destinationDeviceId,
                    destinationDir: destinationDir
                )
                await self?.finalizeJob(jobId: jobId, status: .completed)
            } catch {
                await self?.finalizeJob(
                    jobId: jobId,
                    status: .failed,
                    errorMessage: error.localizedDescription
                )
            }
        }
        if var ctx = jobContexts[jobId] {
            ctx.cancellationTask = Task {
                task.cancel()
            }
            jobContexts[jobId] = ctx
        }
    }

    /// Executes a file download task via background UniFFI driver.
    public func executeDownload(
        jobId: String,
        deviceId: String,
        remotePath: String,
        localPath: String
    ) {
        let task = Task.detached { [weak self] in
            do {
                let _ = try uniffiDownloadFile(
                    deviceId: deviceId,
                    remotePath: remotePath,
                    localPath: localPath
                )
                await self?.finalizeJob(jobId: jobId, status: .completed)
            } catch {
                await self?.finalizeJob(
                    jobId: jobId,
                    status: .failed,
                    errorMessage: error.localizedDescription
                )
            }
        }
        if var ctx = jobContexts[jobId] {
            ctx.cancellationTask = Task {
                task.cancel()
            }
            jobContexts[jobId] = ctx
        }
    }

    /// Executes a file upload task via background UniFFI driver.
    public func executeUpload(
        jobId: String,
        deviceId: String,
        localPath: String,
        remoteDir: String
    ) {
        let task = Task.detached { [weak self] in
            do {
                let _ = try uniffiUploadFile(
                    deviceId: deviceId,
                    localPath: localPath,
                    remoteDir: remoteDir
                )
                await self?.finalizeJob(jobId: jobId, status: .completed)
            } catch {
                await self?.finalizeJob(
                    jobId: jobId,
                    status: .failed,
                    errorMessage: error.localizedDescription
                )
            }
        }
        if var ctx = jobContexts[jobId] {
            ctx.cancellationTask = Task {
                task.cancel()
            }
            jobContexts[jobId] = ctx
        }
    }
}

// MARK: - UniFFI Type Conversions

extension AndroidTransferDirection {
    public init(_ ffi: UniFfiTransferDirection) {
        switch ffi {
        case .macToAndroid: self = .macToAndroid
        case .androidToMac: self = .androidToMac
        case .directPipelineExtract: self = .directPipelineExtract
        }
    }
}

extension AndroidTransferStatus {
    public init(_ ffi: UniFfiTransferStatus) {
        switch ffi {
        case .queued: self = .queued
        case .transferring: self = .transferring
        case .paused: self = .paused
        case .cancelling: self = .cancelling
        case .completed: self = .completed
        case .failed: self = .failed
        }
    }
}

extension AndroidTransferJob {
    public init(_ ffi: UniFfiTransferJob) {
        self.init(
            jobId: ffi.jobId,
            direction: AndroidTransferDirection(ffi.direction),
            sourcePath: ffi.sourcePath,
            destinationPath: ffi.destinationPath,
            totalBytes: ffi.totalBytes,
            transferredBytes: ffi.transferredBytes,
            currentSpeedBps: ffi.currentSpeedBps,
            status: AndroidTransferStatus(ffi.status),
            errorMessage: ffi.errorMessage
        )
    }
}
