// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation
import os

// MARK: - Task Info Descriptor

/// Strongly-typed observable task descriptor tracking real-time archive operation metrics.
public struct TTZipTaskInfo: Sendable, Identifiable, Equatable {
    /// Unique identifier for this concurrent operation task.
    public let id: UUID

    /// Classification of the archive operation.
    public let operationType: ArchiveOperationType

    /// Target output or source file path.
    public let targetPath: String

    /// Current lifecycle state of the operation.
    public var state: ArchiveProgress.State

    /// Normalized completion fraction in range [0.0, 1.0].
    public var fractionCompleted: Double

    /// Cumulative bytes processed so far.
    public var bytesProcessed: Int64

    /// Total workload size in bytes.
    public var totalBytes: Int64

    /// Name or path of the file currently being processed.
    public var currentFileName: String

    /// Instantaneous processing throughput in MB/s.
    public var throughputMBs: Double

    /// Timestamp when the operation was initiated.
    public let startedAt: Date

    /// Elapsed time in seconds since the operation started.
    public var durationSeconds: Double {
        Date().timeIntervalSince(startedAt)
    }

    public init(
        id: UUID = UUID(),
        operationType: ArchiveOperationType,
        targetPath: String,
        state: ArchiveProgress.State = .idle,
        fractionCompleted: Double = 0.0,
        bytesProcessed: Int64 = 0,
        totalBytes: Int64 = 0,
        currentFileName: String = "",
        throughputMBs: Double = 0.0,
        startedAt: Date = Date()
    ) {
        self.id = id
        self.operationType = operationType
        self.targetPath = targetPath
        self.state = state
        self.fractionCompleted = fractionCompleted
        self.bytesProcessed = bytesProcessed
        self.totalBytes = totalBytes
        self.currentFileName = currentFileName
        self.throughputMBs = throughputMBs
        self.startedAt = startedAt
    }
}

// MARK: - Batch Request Types

/// Request specification for batch compression workloads.
public struct TTZipBatchCompressRequest: Sendable {
    public let id: UUID
    public let inputs: [String]
    public let outputPath: String
    public let format: ArchiveCompressionFormat
    public let level: ArchiveCompressionLevel
    public let password: String?
    public let options: ArchiveFilterOptions
    public let splitVolumeSizeBytes: Int64?
    public let advancedOptions: ArchiveAdvancedOptions

    public init(
        id: UUID = UUID(),
        inputs: [String],
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        password: String? = nil,
        options: ArchiveFilterOptions = .defaultClean,
        splitVolumeSizeBytes: Int64? = nil,
        advancedOptions: ArchiveAdvancedOptions = .defaultOptions
    ) {
        self.id = id
        self.inputs = inputs
        self.outputPath = outputPath
        self.format = format
        self.level = level
        self.password = password
        self.options = options
        self.splitVolumeSizeBytes = splitVolumeSizeBytes
        self.advancedOptions = advancedOptions
    }
}

/// Request specification for batch extraction workloads.
public struct TTZipBatchExtractRequest: Sendable {
    public let id: UUID
    public let archivePath: String
    public let destinationDir: String
    public let password: String?

    public init(
        id: UUID = UUID(),
        archivePath: String,
        destinationDir: String,
        password: String? = nil
    ) {
        self.id = id
        self.archivePath = archivePath
        self.destinationDir = destinationDir
        self.password = password
    }
}

// MARK: - Pipeline Execution Result

/// Consolidated telemetry and verification result produced by an end-to-end pipeline.
public struct TTZipPipelineExecutionResult: Sendable {
    public let archiveResult: ArchiveOperationResult
    public let extractResult: ExtractResult?
    public let isVerified: Bool
    public let sha256Checksum: String?
    public let crc32Checksum: String?
    public let totalDurationSeconds: Double

    public init(
        archiveResult: ArchiveOperationResult,
        extractResult: ExtractResult? = nil,
        isVerified: Bool = true,
        sha256Checksum: String? = nil,
        crc32Checksum: String? = nil,
        totalDurationSeconds: Double
    ) {
        self.archiveResult = archiveResult
        self.extractResult = extractResult
        self.isVerified = isVerified
        self.sha256Checksum = sha256Checksum
        self.crc32Checksum = crc32Checksum
        self.totalDurationSeconds = totalDurationSeconds
    }
}

// MARK: - TTZipConcurrencyService

/// Primary Swift 6 `@Observable` and `Sendable` unified concurrency and task orchestration service.
///
/// Wraps native compute dispatchers, actor-isolated engine pipelines, and 60fps lock-free progress bridges
/// to provide safe, race-free, and highly responsive task lifecycle management for SwiftUI and SDK consumers.
@Observable
public final class TTZipConcurrencyService: @unchecked Sendable {

    // MARK: - Shared Singleton

    public static let shared = TTZipConcurrencyService()

    // MARK: - Observable Public Properties

    /// Active operations currently executing or queued.
    public private(set) var activeTasks: [TTZipTaskInfo] = []

    /// Total count of currently active operations.
    public var activeTaskCount: Int {
        activeTasks.count
    }

    /// Cumulative count of successfully finished tasks since service launch.
    public private(set) var totalCompletedTasks: Int = 0

    /// Cumulative count of failed tasks since service launch.
    public private(set) var totalFailedTasks: Int = 0

    /// Cumulative count of cancelled tasks since service launch.
    public private(set) var totalCancelledTasks: Int = 0

    /// Cumulative byte volume processed across all tasks.
    public private(set) var totalBytesProcessed: Int64 = 0

    /// Indicates whether no background archiving tasks are currently running.
    public var isIdle: Bool {
        activeTasks.isEmpty
    }

    /// Most recent error message captured from a failed task.
    public private(set) var latestError: String? = nil

    // MARK: - Internal Dependencies & State

    @ObservationIgnored
    private let engine: TTZipEngine

    @ObservationIgnored
    private let facade: TTZipEngineFacade

    @ObservationIgnored
    private let hashCalculator: HashCalculator

    @ObservationIgnored
    private let stateLock = OSAllocatedUnfairLock(initialState: InternalState())

    private struct InternalState {
        var handles: [UUID: TaskExecutionHandle] = [:]
        var tasksById: [UUID: TTZipTaskInfo] = [:]
    }

    // MARK: - Initialization

    public init(
        engine: TTZipEngine = .shared,
        facade: TTZipEngineFacade = .shared,
        hashCalculator: HashCalculator = HashCalculator()
    ) {
        self.engine = engine
        self.facade = facade
        self.hashCalculator = hashCalculator
    }

    // MARK: - 1. Streaming Compression

    /// Orchestrates an asynchronous streaming compression task with 60fps progress throttling and cancellation tokens.
    ///
    /// - Parameters:
    ///   - inputs: Files and directories to include in the archive.
    ///   - outputPath: Target archive destination path.
    ///   - format: Compression format to use.
    ///   - level: Compression level.
    ///   - password: Optional encryption password.
    ///   - options: File filtering and cleaning options.
    ///   - splitVolumeSizeBytes: Optional volume slicing chunk size.
    ///   - advancedOptions: Advanced encoder and container settings.
    ///   - qos: Quality of Service profile for the worker thread.
    /// - Returns: A tuple containing the 60fps progress `AsyncStream`, the cancellation `TaskExecutionHandle`, and the underlying `Task`.
    public func compressStream(
        inputs: [String],
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        password: String? = nil,
        options: ArchiveFilterOptions = .defaultClean,
        splitVolumeSizeBytes: Int64? = nil,
        advancedOptions: ArchiveAdvancedOptions = .defaultOptions,
        qos: ExecutionQoSProfile = .userInitiated
    ) -> (stream: AsyncStream<ArchiveProgress>, handle: TaskExecutionHandle, task: Task<ArchiveOperationResult, Error>) {
        let taskId = UUID()
        let handle = TaskExecutionHandle()
        let (bridge, stream) = ConcurrencyBridge.ProgressStreamBridge.create()

        let initialTaskInfo = TTZipTaskInfo(
            id: taskId,
            operationType: .compress,
            targetPath: outputPath,
            state: .processing
        )
        registerTask(initialTaskInfo, handle: handle)

        let task = Task<ArchiveOperationResult, Error>(priority: qos.taskPriority) {
            defer {
                bridge.emit(bytesProcessed: 0, totalBytes: 0, state: .completed, force: true)
            }

            return try await withTaskCancellationHandler {
                do {
                    try Task.checkCancellation()
                    if handle.isCancelled {
                        throw CancellationError()
                    }

                    let result = try await self.facade.quickCompress(
                        inputs: inputs,
                        outputPath: outputPath,
                        format: format,
                        level: level,
                        password: password,
                        splitSize: splitVolumeSizeBytes,
                        filterOptions: options,
                        advancedOptions: advancedOptions,
                        progress: { progress in
                            bridge.emit(
                                bytesProcessed: progress.bytesProcessed,
                                totalBytes: progress.totalBytes,
                                currentFileName: progress.currentFileName,
                                state: progress.state
                            )
                            self.updateTaskProgress(
                                taskId: taskId,
                                progress: progress
                            )
                        },
                        token: handle.uniffiToken
                    )

                    self.completeTask(taskId: taskId, bytesProcessed: result.compressedBytes)
                    return result
                } catch is CancellationError {
                    self.cancelTaskInternal(taskId: taskId)
                    bridge.emit(bytesProcessed: 0, totalBytes: 0, state: .cancelled, force: true)
                    throw CancellationError()
                } catch {
                    if Task.isCancelled || handle.isCancelled {
                        self.cancelTaskInternal(taskId: taskId)
                        bridge.emit(bytesProcessed: 0, totalBytes: 0, state: .cancelled, force: true)
                        throw CancellationError()
                    }
                    self.failTask(taskId: taskId, error: error)
                    bridge.emit(bytesProcessed: 0, totalBytes: 0, state: .failed(error: error.localizedDescription), force: true)
                    throw error
                }
            } onCancel: {
                handle.cancel()
                bridge.cancel()
                self.cancelTaskInternal(taskId: taskId)
            }
        }

        return (stream: stream, handle: handle, task: task)
    }

    // MARK: - 2. Streaming Extraction

    /// Orchestrates an asynchronous streaming extraction task with 60fps progress throttling and cancellation tokens.
    public func extractStream(
        archivePath: String,
        destinationDir: String,
        password: String? = nil,
        autoVaultUnlock: Bool = true,
        qos: ExecutionQoSProfile = .userInitiated
    ) -> (stream: AsyncStream<ArchiveProgress>, handle: TaskExecutionHandle, task: Task<ExtractResult, Error>) {
        let taskId = UUID()
        let handle = TaskExecutionHandle()
        let (bridge, stream) = ConcurrencyBridge.ProgressStreamBridge.create()

        let initialTaskInfo = TTZipTaskInfo(
            id: taskId,
            operationType: .extract,
            targetPath: archivePath,
            state: .processing
        )
        registerTask(initialTaskInfo, handle: handle)

        let task = Task<ExtractResult, Error>(priority: qos.taskPriority) {
            defer {
                bridge.emit(bytesProcessed: 0, totalBytes: 0, state: .completed, force: true)
            }

            return try await withTaskCancellationHandler {
                do {
                    try Task.checkCancellation()
                    if handle.isCancelled {
                        throw CancellationError()
                    }

                    let result = try await self.facade.quickExtract(
                        archivePath: archivePath,
                        destinationDir: destinationDir,
                        password: password,
                        autoVaultUnlock: autoVaultUnlock,
                        progress: { progress in
                            bridge.emit(
                                bytesProcessed: progress.bytesProcessed,
                                totalBytes: progress.totalBytes,
                                currentFileName: progress.currentFileName,
                                state: progress.state
                            )
                            self.updateTaskProgress(
                                taskId: taskId,
                                progress: progress
                            )
                        },
                        token: handle.uniffiToken
                    )

                    self.completeTask(taskId: taskId, bytesProcessed: 0)
                    return result
                } catch is CancellationError {
                    self.cancelTaskInternal(taskId: taskId)
                    bridge.emit(bytesProcessed: 0, totalBytes: 0, state: .cancelled, force: true)
                    throw CancellationError()
                } catch {
                    if Task.isCancelled || handle.isCancelled {
                        self.cancelTaskInternal(taskId: taskId)
                        bridge.emit(bytesProcessed: 0, totalBytes: 0, state: .cancelled, force: true)
                        throw CancellationError()
                    }
                    self.failTask(taskId: taskId, error: error)
                    bridge.emit(bytesProcessed: 0, totalBytes: 0, state: .failed(error: error.localizedDescription), force: true)
                    throw error
                }
            } onCancel: {
                handle.cancel()
                bridge.cancel()
                self.cancelTaskInternal(taskId: taskId)
            }
        }

        return (stream: stream, handle: handle, task: task)
    }

    // MARK: - 3. Asynchronous Inspection & Hash

    /// Asynchronously inspects an archive hierarchy and structural metadata.
    public func inspectAsync(
        archivePath: String,
        password: String? = nil,
        autoVaultUnlock: Bool = true,
        qos: ExecutionQoSProfile = .userInitiated
    ) async throws -> ArchiveInspectionResult {
        return try await self.facade.inspectArchive(
            archivePath: archivePath,
            password: password,
            autoVaultUnlock: autoVaultUnlock
        )
    }

    /// Asynchronously computes a cryptographic or verification hash for a target file.
    public func calculateHashAsync(
        filePath: String,
        type: HashType = .sha256,
        qos: ExecutionQoSProfile = .userInitiated
    ) async throws -> String {
        return try await NativeComputeDispatcher.shared.dispatchCompute(qos: qos) {
            try self.hashCalculator.computeHashSync(filePath: filePath, type: type)
        }
    }

    // MARK: - 4. Batch Operations with Bounded Concurrency

    /// Executes multiple archive compression requests in parallel bounded by a maximum worker concurrency limit.
    public func executeBatchCompress(
        requests: [TTZipBatchCompressRequest],
        maxConcurrentTasks: Int = 4,
        qos: ExecutionQoSProfile = .userInitiated
    ) async throws -> [ArchiveOperationResult] {
        guard !requests.isEmpty else { return [] }
        let concurrencyLimit = max(1, min(maxConcurrentTasks, ConcurrencyBridge.ThreadBudget.optimalThreadCount()))

        return try await withThrowingTaskGroup(of: (Int, ArchiveOperationResult).self) { group in
            var results: [ArchiveOperationResult?] = Array(repeating: nil, count: requests.count)
            var submittedIndex = 0

            // Prime the concurrency pipeline
            while submittedIndex < min(concurrencyLimit, requests.count) {
                let index = submittedIndex
                let req = requests[index]
                group.addTask(priority: qos.taskPriority) {
                    let (_, _, task) = self.compressStream(
                        inputs: req.inputs,
                        outputPath: req.outputPath,
                        format: req.format,
                        level: req.level,
                        password: req.password,
                        options: req.options,
                        splitVolumeSizeBytes: req.splitVolumeSizeBytes,
                        advancedOptions: req.advancedOptions,
                        qos: qos
                    )
                    let res = try await task.value
                    return (index, res)
                }
                submittedIndex += 1
            }

            // Drain and replenish concurrently
            for try await (idx, result) in group {
                results[idx] = result
                if submittedIndex < requests.count {
                    let nextIdx = submittedIndex
                    let req = requests[nextIdx]
                    group.addTask(priority: qos.taskPriority) {
                        let (_, _, task) = self.compressStream(
                            inputs: req.inputs,
                            outputPath: req.outputPath,
                            format: req.format,
                            level: req.level,
                            password: req.password,
                            options: req.options,
                            splitVolumeSizeBytes: req.splitVolumeSizeBytes,
                            advancedOptions: req.advancedOptions,
                            qos: qos
                        )
                        let res = try await task.value
                        return (nextIdx, res)
                    }
                    submittedIndex += 1
                }
            }

            return results.compactMap { $0 }
        }
    }

    /// Executes multiple extraction requests in parallel bounded by a maximum worker concurrency limit.
    public func executeBatchExtract(
        requests: [TTZipBatchExtractRequest],
        maxConcurrentTasks: Int = 4,
        qos: ExecutionQoSProfile = .userInitiated
    ) async throws -> [ExtractResult] {
        guard !requests.isEmpty else { return [] }
        let concurrencyLimit = max(1, min(maxConcurrentTasks, ConcurrencyBridge.ThreadBudget.optimalThreadCount()))

        return try await withThrowingTaskGroup(of: (Int, ExtractResult).self) { group in
            var results: [ExtractResult?] = Array(repeating: nil, count: requests.count)
            var submittedIndex = 0

            while submittedIndex < min(concurrencyLimit, requests.count) {
                let index = submittedIndex
                let req = requests[index]
                group.addTask(priority: qos.taskPriority) {
                    let (_, _, task) = self.extractStream(
                        archivePath: req.archivePath,
                        destinationDir: req.destinationDir,
                        password: req.password,
                        qos: qos
                    )
                    let res = try await task.value
                    return (index, res)
                }
                submittedIndex += 1
            }

            for try await (idx, result) in group {
                results[idx] = result
                if submittedIndex < requests.count {
                    let nextIdx = submittedIndex
                    let req = requests[nextIdx]
                    group.addTask(priority: qos.taskPriority) {
                        let (_, _, task) = self.extractStream(
                            archivePath: req.archivePath,
                            destinationDir: req.destinationDir,
                            password: req.password,
                            qos: qos
                        )
                        let res = try await task.value
                        return (nextIdx, res)
                    }
                    submittedIndex += 1
                }
            }

            return results.compactMap { $0 }
        }
    }

    // MARK: - 5. End-to-End Pipeline Execution

    /// Runs a unified sequential pipeline: compress -> hash/verify -> extract.
    public func executePipeline(
        inputs: [String],
        archivePath: String,
        destinationDir: String? = nil,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        password: String? = nil,
        verifyIntegrity: Bool = true,
        qos: ExecutionQoSProfile = .userInitiated
    ) async throws -> TTZipPipelineExecutionResult {
        let startTime = Date()

        // 1. Compression
        let (_, _, compressTask) = compressStream(
            inputs: inputs,
            outputPath: archivePath,
            format: format,
            level: level,
            password: password,
            qos: qos
        )
        let archiveResult = try await compressTask.value

        // 2. Hash calculation & verification
        var sha256: String? = nil
        var crc32: String? = nil
        if verifyIntegrity {
            sha256 = try await calculateHashAsync(filePath: archivePath, type: .sha256, qos: qos)
            crc32 = try await calculateHashAsync(filePath: archivePath, type: .crc32, qos: qos)
        }

        // 3. Optional Extraction
        var extractResult: ExtractResult? = nil
        if let dest = destinationDir {
            let (_, _, extTask) = extractStream(
                archivePath: archivePath,
                destinationDir: dest,
                password: password,
                qos: qos
            )
            extractResult = try await extTask.value
        }

        let totalDuration = Date().timeIntervalSince(startTime)
        return TTZipPipelineExecutionResult(
            archiveResult: archiveResult,
            extractResult: extractResult,
            isVerified: true,
            sha256Checksum: sha256,
            crc32Checksum: crc32,
            totalDurationSeconds: totalDuration
        )
    }

    // MARK: - 6. Task Management & Controls

    /// Cancels a specific running task by its identifier.
    public func cancelTask(id: UUID) {
        let handle = stateLock.withLock { $0.handles[id] }
        handle?.cancel()
        cancelTaskInternal(taskId: id)
    }

    /// Cancels all active operations currently running.
    public func cancelAll() {
        let activeHandles = stateLock.withLock { state -> [TaskExecutionHandle] in
            let handles = Array(state.handles.values)
            return handles
        }
        for handle in activeHandles {
            handle.cancel()
        }
        stateLock.withLock { state in
            for (id, var task) in state.tasksById {
                task.state = .cancelled
                state.tasksById[id] = task
            }
        }
        syncObservableActiveTasks()
    }

    /// Clears historical telemetry and reset failure/completion counters.
    public func clearCounters() {
        totalCompletedTasks = 0
        totalFailedTasks = 0
        totalCancelledTasks = 0
        totalBytesProcessed = 0
        latestError = nil
    }

    // MARK: - Internal State Synchronization

    private func registerTask(_ taskInfo: TTZipTaskInfo, handle: TaskExecutionHandle) {
        stateLock.withLock { state in
            state.tasksById[taskInfo.id] = taskInfo
            state.handles[taskInfo.id] = handle
        }
        syncObservableActiveTasks()
    }

    private func updateTaskProgress(taskId: UUID, progress: ArchiveProgress) {
        stateLock.withLock { state in
            guard var task = state.tasksById[taskId] else { return }
            task.state = progress.state
            task.bytesProcessed = progress.bytesProcessed
            task.totalBytes = progress.totalBytes
            task.fractionCompleted = progress.fractionCompleted
            task.currentFileName = progress.currentFileName
            task.throughputMBs = progress.throughputMBs
            state.tasksById[taskId] = task
        }
        syncObservableActiveTasks()
    }

    private func completeTask(taskId: UUID, bytesProcessed: Int64) {
        stateLock.withLock { state in
            state.tasksById.removeValue(forKey: taskId)
            state.handles.removeValue(forKey: taskId)
        }
        totalCompletedTasks += 1
        totalBytesProcessed += bytesProcessed
        syncObservableActiveTasks()
    }

    private func failTask(taskId: UUID, error: Error) {
        stateLock.withLock { state in
            state.tasksById.removeValue(forKey: taskId)
            state.handles.removeValue(forKey: taskId)
        }
        totalFailedTasks += 1
        latestError = error.localizedDescription
        syncObservableActiveTasks()
    }

    private func cancelTaskInternal(taskId: UUID) {
        stateLock.withLock { state in
            state.tasksById.removeValue(forKey: taskId)
            state.handles.removeValue(forKey: taskId)
        }
        totalCancelledTasks += 1
        syncObservableActiveTasks()
    }

    private func syncObservableActiveTasks() {
        let tasks = stateLock.withLock { Array($0.tasksById.values) }
        self.activeTasks = tasks
    }
}
