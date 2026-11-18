// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import TTZipCoreBindings
import TTZipCoreTypes
import Foundation
import os

public typealias UniFFICreateArchiveConfig = UniFfiCreateArchiveConfig

/// Keyed mutual exclusion lock pool for target archive paths to serialize concurrent writes and mutations.
/// Enforces deterministic FIFO ordering via continuation queues and guarantees that a path lock
/// remains held until the underlying worker and its cleanup finish.
public actor ArchiveTargetPathLockPool {
    public static let shared = ArchiveTargetPathLockPool()

    @TaskLocal
    private static var heldLocks: Set<String> = []

    private static let waiterEnqueuedHookStorage = OSAllocatedUnfairLock<(@Sendable (_ normalizedPath: String, _ waiterCount: Int) -> Void)?>(initialState: nil)
    private static let lockReleasedHookStorage = OSAllocatedUnfairLock<(@Sendable (_ normalizedPath: String) -> Void)?>(initialState: nil)

    package nonisolated static var _onWaiterEnqueuedHook: (@Sendable (_ normalizedPath: String, _ waiterCount: Int) -> Void)? {
        get { waiterEnqueuedHookStorage.withLock { $0 } }
        set { waiterEnqueuedHookStorage.withLock { $0 = newValue } }
    }

    package nonisolated static var _onLockReleasedHook: (@Sendable (_ normalizedPath: String) -> Void)? {
        get { lockReleasedHookStorage.withLock { $0 } }
        set { lockReleasedHookStorage.withLock { $0 = newValue } }
    }

    private final class Waiter: Sendable {
        let id: UUID
        let continuation: CheckedContinuation<Void, Error>
        private let isResumed = OSAllocatedUnfairLock(initialState: false)

        init(id: UUID, continuation: CheckedContinuation<Void, Error>) {
            self.id = id
            self.continuation = continuation
        }

        func resumeOnce(returning value: Void) {
            let shouldResume = isResumed.withLock { resumed in
                guard !resumed else { return false }
                resumed = true
                return true
            }
            if shouldResume {
                continuation.resume(returning: value)
            }
        }

        func resumeOnce(throwing error: Error) {
            let shouldResume = isResumed.withLock { resumed in
                guard !resumed else { return false }
                resumed = true
                return true
            }
            if shouldResume {
                continuation.resume(throwing: error)
            }
        }
    }

    private struct WaiterQueue {
        var waiters: [Waiter] = []
    }

    private var queues: [String: WaiterQueue] = [:]

    public init() {}

    public func withLock<T: Sendable>(_ path: String, operation: @Sendable () async throws -> T) async throws -> T {
        let normalizedKey = URL(fileURLWithPath: path).standardizedFileURL.path
        if Self.heldLocks.contains(normalizedKey) {
            return try await operation()
        }

        try await acquire(path: normalizedKey)
        defer {
            release(path: normalizedKey)
        }
        try Task.checkCancellation()
        return try await Self.$heldLocks.withValue(Self.heldLocks.union([normalizedKey])) {
            try await operation()
        }
    }

    private func acquire(path: String) async throws {
        try Task.checkCancellation()

        if queues[path] == nil {
            queues[path] = WaiterQueue(waiters: [])
            return
        }

        let waiterID = UUID()
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
                enqueueWaiter(path: path, id: waiterID, continuation: continuation)
            }
        } onCancel: {
            Task { [weak self] in
                await self?.cancelWaiter(path: path, id: waiterID)
            }
        }
    }

    private func enqueueWaiter(path: String, id: UUID, continuation: CheckedContinuation<Void, Error>) {
        if queues[path] == nil {
            queues[path] = WaiterQueue(waiters: [])
            continuation.resume(returning: ())
            return
        }
        queues[path]?.waiters.append(Waiter(id: id, continuation: continuation))
        let count = queues[path]?.waiters.count ?? 0
        Self._onWaiterEnqueuedHook?(path, count)
    }

    private func cancelWaiter(path: String, id: UUID) {
        guard var queue = queues[path] else { return }
        if let index = queue.waiters.firstIndex(where: { $0.id == id }) {
            let waiter = queue.waiters.remove(at: index)
            queues[path] = queue
            waiter.resumeOnce(throwing: CancellationError())
        }
    }

    private func release(path: String) {
        guard var queue = queues[path] else { return }
        Self._onLockReleasedHook?(path)
        if !queue.waiters.isEmpty {
            let next = queue.waiters.removeFirst()
            queues[path] = queue
            next.resumeOnce(returning: ())
        } else {
            queues.removeValue(forKey: path)
        }
    }
}

/// High-performance multi-format archive compression engine (100% Pure Mozilla UniFFI Engine).
public final class ArchiveWriter: ArchiveWriting, Sendable {
    public typealias CreateArchiveExecutorOverride = @Sendable (
        _ config: UniFfiCreateArchiveConfig,
        _ extraOptions: UniFfiCreateArchiveExtraOptions,
        _ progress: ProgressHandler?,
        _ token: CancellationToken?
    ) throws -> CompressionReport

    internal let hardwareTuner: HardwareTunerProtocol
    public let targetFormat: ArchiveCompressionFormat?
    private let executorOverrideStorage = OSAllocatedUnfairLock<CreateArchiveExecutorOverride?>(initialState: nil)
    private let postFfiCommitBeforeDeliveryHookStorage = OSAllocatedUnfairLock<(@Sendable () -> Void)?>(initialState: nil)
    private let afterWorkerCleanupBeforeLockReleaseHookStorage = OSAllocatedUnfairLock<(@Sendable (_ outputPath: String) -> Void)?>(initialState: nil)
    private let onCancellationHandleCreatedHookStorage = OSAllocatedUnfairLock<(@Sendable (_ handle: TaskExecutionHandle) -> Void)?>(initialState: nil)
    private let lastCompressionReportStorage = OSAllocatedUnfairLock<CompressionReport?>(initialState: nil)

    package var _createArchiveExecutorOverride: CreateArchiveExecutorOverride? {
        get { executorOverrideStorage.withLock { $0 } }
        set { executorOverrideStorage.withLock { $0 = newValue } }
    }

    /// Optional test hook invoked on the worker thread immediately after the UniFFI `createArchiveAdvancedWithOptions`
    /// call has committed the target archive on disk and before the worker returns to `NativeComputeDispatcher`.
    package var _postFfiCommitBeforeDeliveryHook: (@Sendable () -> Void)? {
        get { postFfiCommitBeforeDeliveryHookStorage.withLock { $0 } }
        set { postFfiCommitBeforeDeliveryHookStorage.withLock { $0 = newValue } }
    }

    /// Optional test hook invoked on the worker thread after the synchronous worker body and its staging
    /// cleanup have completed, immediately before `ArchiveTargetPathLockPool` releases the path lock.
    package var _afterWorkerCleanupBeforeLockReleaseHook: (@Sendable (_ outputPath: String) -> Void)? {
        get { afterWorkerCleanupBeforeLockReleaseHookStorage.withLock { $0 } }
        set { afterWorkerCleanupBeforeLockReleaseHookStorage.withLock { $0 = newValue } }
    }

    /// Optional test hook invoked when `createArchiveWithOutcome` creates its internal `TaskExecutionHandle`.
    package var _onCancellationHandleCreatedHook: (@Sendable (_ handle: TaskExecutionHandle) -> Void)? {
        get { onCancellationHandleCreatedHookStorage.withLock { $0 } }
        set { onCancellationHandleCreatedHookStorage.withLock { $0 = newValue } }
    }

    /// Diagnostic snapshot of the most recently finished `CompressionReport` on this writer instance.
    /// Production callers must consume the per-operation `CompressionReport` returned by `createArchiveWithOutcome`.
    public var lastCompressionReport: CompressionReport? {
        lastCompressionReportStorage.withLock { $0 }
    }

    /// Diagnostic snapshot of the most recently finished `ExtractionOutcome` on this writer instance.
    /// Production callers must consume the per-operation outcome returned by `createArchiveWithOutcome`.
    public var lastCompressionOutcome: ExtractionOutcome? {
        lastCompressionReportStorage.withLock { $0?.outcome }
    }

    public init(
        hardwareTuner: HardwareTunerProtocol = AppleSiliconTuner.shared,
        targetFormat: ArchiveCompressionFormat? = nil
    ) {
        self.hardwareTuner = hardwareTuner
        self.targetFormat = targetFormat
    }

    /// Backward-compatible initializer accepting legacy engine parameters.
    public convenience init(
        zipEngine: Any? = nil,
        sevenZipEngine: Any? = nil,
        zstdEngine: Any? = nil,
        hardwareTuner: HardwareTunerProtocol = AppleSiliconTuner.shared,
        targetFormat: ArchiveCompressionFormat? = nil
    ) {
        self.init(hardwareTuner: hardwareTuner, targetFormat: targetFormat)
    }

    /// Asynchronously creates an archive using a structured `ArchiveWriteRequest`.
    public func createArchive(_ request: ArchiveWriteRequest) async throws {
        _ = try await createArchiveWithOutcome(request)
    }

    /// Asynchronously creates an archive using a structured `ArchiveWriteRequest` and returns its per-operation `CompressionReport`.
    @discardableResult
    public func createArchiveWithOutcome(
        _ request: ArchiveWriteRequest,
        token: CancellationToken? = nil
    ) async throws -> CompressionReport {
        try await createArchiveWithOutcome(
            outputPath: request.outputPath,
            format: request.format,
            level: request.level,
            inputPaths: request.inputPaths,
            options: request.options,
            splitVolumeSizeBytes: request.splitVolumeSizeBytes,
            password: request.password,
            advancedOptions: request.advancedOptions,
            progressHandler: request.progressHandler,
            token: token
        )
    }

    /// Synchronously creates an archive using a structured `ArchiveWriteRequest`.
    public func createArchiveSync(_ request: ArchiveWriteRequest) throws {
        _ = try createArchiveSyncWithOutcome(request)
    }

    /// Synchronously creates an archive using a structured `ArchiveWriteRequest` and returns its per-operation `CompressionReport`.
    @discardableResult
    public func createArchiveSyncWithOutcome(
        _ request: ArchiveWriteRequest,
        token: CancellationToken? = nil
    ) throws -> CompressionReport {
        try createArchiveSyncWithOutcome(
            outputPath: request.outputPath,
            format: request.format,
            level: request.level,
            inputPaths: request.inputPaths,
            options: request.options,
            password: request.password,
            splitVolumeSizeBytes: request.splitVolumeSizeBytes,
            advancedOptions: request.advancedOptions,
            progressHandler: request.progressHandler,
            token: token
        )
    }

    /// Asynchronously compresses files and directories into an archive with validation and progress tracking.
    public func createArchive(
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        inputPaths: [String],
        options: ArchiveFilterOptions = .defaultClean,
        splitVolumeSizeBytes: Int64? = nil,
        password: String? = nil,
        advancedOptions: ArchiveAdvancedOptions = .defaultOptions,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil,
        token: CancellationToken? = nil
    ) async throws {
        _ = try await createArchiveWithOutcome(
            outputPath: outputPath,
            format: format,
            level: level,
            inputPaths: inputPaths,
            options: options,
            splitVolumeSizeBytes: splitVolumeSizeBytes,
            password: password,
            advancedOptions: advancedOptions,
            progressHandler: progressHandler,
            token: token
        )
    }

    /// Asynchronously compresses files and directories into an archive and returns its per-operation `CompressionReport`
    /// (including `outcome` when post-commit `.ttzip_create_txn_*` or `.ttzip_create_bak_*` cleanup reports warnings).
    @discardableResult
    public func createArchiveWithOutcome(
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        inputPaths: [String],
        options: ArchiveFilterOptions = .defaultClean,
        splitVolumeSizeBytes: Int64? = nil,
        password: String? = nil,
        advancedOptions: ArchiveAdvancedOptions = .defaultOptions,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil,
        token: CancellationToken? = nil
    ) async throws -> CompressionReport {
        guard !inputPaths.isEmpty else {
            throw ArchiveError.readFailed(code: -10)
        }

        try Task.checkCancellation()

        if token?.isCancelled() == true {
            throw ArchiveError.cancelled
        }

        let boundToken = token ?? CancellationToken()
        let handle = TaskExecutionHandle(token: boundToken)
        self._onCancellationHandleCreatedHook?(handle)

        do {
            return try await ArchiveTargetPathLockPool.shared.withLock(outputPath) {
                try await NativeComputeDispatcher.shared.dispatchCompute(
                    qos: .userInitiated,
                    cancellationHandle: handle,
                    cancellationPolicy: .waitForWorkerCompletion
                ) { [weak self] (_: CancellationToken) -> CompressionReport in
                    guard let self = self else {
                        throw ArchiveError.cancelled
                    }
                    defer {
                        self._afterWorkerCleanupBeforeLockReleaseHook?(outputPath)
                    }
                    return try self.createArchiveSyncWithOutcome(
                        outputPath: outputPath,
                        format: format,
                        level: level,
                        inputPaths: inputPaths,
                        options: options,
                        password: password,
                        splitVolumeSizeBytes: splitVolumeSizeBytes,
                        advancedOptions: advancedOptions,
                        progressHandler: progressHandler,
                        token: boundToken
                    )
                }
            }
        } catch let archiveError as ArchiveError {
            throw archiveError
        } catch is CancellationError {
            throw ArchiveError.cancelled
        }
    }

    /// Synchronously creates an archive bypassing Task queue context-switches.
    @inline(__always)
    public func createArchiveSync(
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        inputPaths: [String],
        options: ArchiveFilterOptions = .defaultClean,
        password: String? = nil,
        splitVolumeSizeBytes: Int64? = nil,
        advancedOptions: ArchiveAdvancedOptions = .defaultOptions,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil,
        token: CancellationToken? = nil
    ) throws {
        _ = try createArchiveSyncWithOutcome(
            outputPath: outputPath,
            format: format,
            level: level,
            inputPaths: inputPaths,
            options: options,
            password: password,
            splitVolumeSizeBytes: splitVolumeSizeBytes,
            advancedOptions: advancedOptions,
            progressHandler: progressHandler,
            token: token
        )
    }

    /// Synchronously creates an archive and returns its per-operation `CompressionReport`.
    @discardableResult
    public func createArchiveSyncWithOutcome(
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        inputPaths: [String],
        options: ArchiveFilterOptions = .defaultClean,
        password: String? = nil,
        splitVolumeSizeBytes: Int64? = nil,
        advancedOptions: ArchiveAdvancedOptions = .defaultOptions,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil,
        token: CancellationToken? = nil
    ) throws -> CompressionReport {
        guard !inputPaths.isEmpty else {
            throw ArchiveError.readFailed(code: -10)
        }

        let startTime = Date()

        return try createArchiveInternal(
            outputPath: outputPath,
            format: format,
            level: level,
            inputPaths: inputPaths,
            options: options,
            splitVolumeSizeBytes: splitVolumeSizeBytes,
            password: password,
            advancedOptions: advancedOptions,
            progressHandler: progressHandler,
            startTime: startTime,
            token: token
        )
    }

    /// Creates an archive and returns non-forgeable engine execution provenance telemetry.
    @discardableResult
    public func createArchiveWithReport(
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        inputPaths: [String],
        options: ArchiveFilterOptions = .defaultClean,
        password: String? = nil,
        splitVolumeSizeBytes: Int64? = nil,
        advancedOptions: ArchiveAdvancedOptions = .defaultOptions,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil,
        token: CancellationToken? = nil
    ) throws -> EngineDispatchProvenance {
        guard !inputPaths.isEmpty else {
            throw ArchiveError.readFailed(code: -10)
        }

        var compressionReport: CompressionReport?
        let startTime = Date()
        let (_, provenance) = try EngineProvenanceCollector.capture(
            expectedEngine: format == .sevenZip ? .rustZeroCopy7zDecoder : .rustStreamingParallelZip,
            uncompressedBytes: 0
        ) {
            compressionReport = try self.createArchiveInternal(
                outputPath: outputPath,
                format: format,
                level: level,
                inputPaths: inputPaths,
                options: options,
                splitVolumeSizeBytes: splitVolumeSizeBytes,
                password: password,
                advancedOptions: advancedOptions,
                progressHandler: progressHandler,
                startTime: startTime,
                token: token
            )
        }
        let uncompressedBytes = compressionReport.map { Int64($0.uncompressedBytes) } ?? provenance.uncompressedBytes
        let outputBytes = compressionReport.map { Int64($0.compressedBytes) } ?? ((try? FileManager.default.attributesOfItem(atPath: outputPath)[.size] as? Int64) ?? provenance.compressedBytes)
        let kernelDuration = compressionReport?.elapsedNanos ?? provenance.kernelDurationNanos

        return EngineDispatchProvenance(
            engineTag: provenance.engineTag,
            threadCount: provenance.threadCount,
            uncompressedBytes: max(1, uncompressedBytes),
            compressedBytes: max(1, outputBytes),
            kernelDurationNanos: kernelDuration,
            isFallback: provenance.isFallback,
            fallbackReason: provenance.fallbackReason,
            ffiBridgeOverheadNanos: provenance.ffiBridgeOverheadNanos,
            totalE2EDurationNanos: provenance.totalE2EDurationNanos
        )
    }


    // MARK: - Format Mappings

    private static let uniffiFormatMap: [ArchiveCompressionFormat: TtZipArchiveFormat] = [
        .sevenZip: .sevenZip, .zip: .zip, .tar: .tar,
        .tarGz: .tarGz, .gz: .gzip, .tarBz2: .tarBz2, .bz2: .bzip2,
        .tarXz: .tarXz, .xz: .xz, .tarZst: .tarZstd, .zst: .zstd,
        .tarLz4: .tarLz4, .lz4: .lz4, .tarBrotli: .tarBrotli, .brotli: .brotli,
        .tarLzip: .tarLzip, .lzip: .lzip, .tarLrzip: .tarLrzip, .lrzip: .lrzip,
        .dmg: .dmg, .iso: .iso, .cab: .cab, .wim: .wim, .aar: .aar,
        .cpio: .cpio, .ar: .ar, .deb: .deb, .rpm: .rpm, .xar: .xar,
        .snappy: .snappy, .lzfse: .lzfse, .squashfs: .squashfs,
        .lzh: .lzh, .rar: .rar
    ]

    internal static func mapUniFFIFormat(_ format: ArchiveCompressionFormat) -> TtZipArchiveFormat {
        uniffiFormatMap[format] ?? .zip
    }

    internal static func mapUniFFILevel(_ level: ArchiveCompressionLevel) -> Int32 {
        if level.rawValue < 0 {
            return 1
        }
        return Int32(max(0, min(22, level.rawValue)))
    }
}

// MARK: - Internal Dispatch

private final class ProgressRelay: ProgressHandler, @unchecked Sendable {
    let handler: (@Sendable (ArchiveProgress) -> Void)?
    let startTime: Date

    init(startTime: Date, handler: (@Sendable (ArchiveProgress) -> Void)?) {
        self.startTime = startTime
        self.handler = handler
    }

    func onProgress(processedBytes: UInt64, totalBytes: UInt64, currentEntry: String?) -> Bool {
        let duration = max(0.001, Date().timeIntervalSince(startTime))
        let throughput = (Double(processedBytes) / (1024 * 1024)) / duration
        handler?(ArchiveProgress(
            state: .processing,
            bytesProcessed: Int64(processedBytes),
            totalBytes: Int64(totalBytes),
            currentFileName: currentEntry ?? "",
            throughputMBs: throughput
        ))
        return true
    }
}

extension ArchiveWriter {
    internal static let inlineTuningTag: Int32 = 1 << 30

    internal static func encodeInlineCreateConfig(
        _ config: UniFfiCreateArchiveConfig,
        advancedOptions: ArchiveAdvancedOptions
    ) -> UniFfiCreateArchiveConfig {
        let baseLevel = max(0, min(22, config.level)) & 0x3F
        let zstdLevel = Int32(max(0, min(22, advancedOptions.zstdLevel))) & 0x3F
        let recoveryPct = Int32(max(0, min(255, advancedOptions.recoveryRecordRedundancyPercent))) & 0xFF
        let solidBit: Int32 = advancedOptions.enableSolidArchive ? 1 : 0
        let useZipCrypto =
            advancedOptions.zipEncryptionMethod.caseInsensitiveCompare("ZipCrypto") == .orderedSame
        let zipCryptoBit: Int32 = useZipCrypto ? 1 : 0
        let ldmBit: Int32 = advancedOptions.zstdEnableLDM ? 1 : 0

        let packedLevel =
            inlineTuningTag
            | baseLevel
            | (zstdLevel << 6)
            | (recoveryPct << 12)
            | (solidBit << 20)
            | (zipCryptoBit << 21)
            | (ldmBit << 22)

        let cpuThreads = min(config.threadBudget, 0xFFFF)
        let dictMB = min(UInt32(max(0, advancedOptions.dictionarySizeMB)), 0xFFFF)
        let packedThreadBudget = cpuThreads | (dictMB << 16)

        var encoded = config
        encoded.level = packedLevel
        encoded.threadBudget = packedThreadBudget
        return encoded
    }

    private static func validateRecoveryRecordRedundancy(
        targetFmt: ArchiveCompressionFormat,
        splitVolumeSizeBytes: Int64?,
        advancedOptions: ArchiveAdvancedOptions
    ) throws {
        let recPct = advancedOptions.recoveryRecordRedundancyPercent
        if recPct < 0 || recPct > 50 {
            throw ArchiveError.invalidArgument(reason: "Recovery record redundancy must be between 0% and 50%.")
        }
        if recPct > 0 && (!targetFmt.supportsRecoveryRecord || (splitVolumeSizeBytes ?? 0) > 0) {
            throw ArchiveError.unsupportedOperation(
                reason: "Recovery records are only supported for single-volume ZIP, 7Z, and TAR archives."
            )
        }
    }

    private static func validateAndBuildExtraOptions(
        outputPath: String,
        targetFmt: ArchiveCompressionFormat,
        password: String?,
        splitVolumeSizeBytes: Int64?,
        advancedOptions: ArchiveAdvancedOptions
    ) throws -> UniFfiCreateArchiveExtraOptions {
        if let pwd = password, pwd.isEmpty {
            throw ArchiveError.passwordRequired
        }
        let hasNonEmptyPassword = (password?.isEmpty == false)
        if advancedOptions.encryptFileNames && !hasNonEmptyPassword {
            throw ArchiveError.passwordRequired
        }
        if advancedOptions.encryptFileNames && targetFmt != .sevenZip {
            throw ArchiveError.unsupportedEncryptionMethod(
                archivePath: outputPath,
                method: "7z-HeaderEncryption"
            )
        }
        try validateRecoveryRecordRedundancy(
            targetFmt: targetFmt,
            splitVolumeSizeBytes: splitVolumeSizeBytes,
            advancedOptions: advancedOptions
        )
        let encryptFileNames: Bool? = advancedOptions.encryptFileNames
            ? true
            : ((targetFmt == .sevenZip && hasNonEmptyPassword) ? false : nil)
        return UniFfiCreateArchiveExtraOptions(encryptFileNames: encryptFileNames)
    }

    @discardableResult
    internal func createArchiveInternal(
        outputPath: String,
        format: ArchiveCompressionFormat,
        level: ArchiveCompressionLevel,
        inputPaths: [String],
        options: ArchiveFilterOptions,
        splitVolumeSizeBytes: Int64?,
        password: String?,
        advancedOptions: ArchiveAdvancedOptions,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)?,
        startTime: Date,
        token: CancellationToken? = nil
    ) throws -> CompressionReport {
        let targetFmt = self.targetFormat ?? format
        let extraOptions = try Self.validateAndBuildExtraOptions(
            outputPath: outputPath,
            targetFmt: targetFmt,
            password: password,
            splitVolumeSizeBytes: splitVolumeSizeBytes,
            advancedOptions: advancedOptions
        )
        let splitBytes: UInt64? = splitVolumeSizeBytes.flatMap { $0 > 0 ? UInt64($0) : nil }
        let config = UniFfiCreateArchiveConfig(
            sourcePaths: inputPaths,
            outputPath: outputPath,
            format: ArchiveWriter.mapUniFFIFormat(targetFmt),
            level: ArchiveWriter.mapUniFFILevel(level),
            password: password,
            splitVolumeSizeBytes: splitBytes,
            excludePatterns: options.excludePatterns,
            includePatterns: options.includePatterns,
            excludeVcs: options.excludeVCS,
            noMacMetadata: options.noMacMetadata,
            threadBudget: UInt32(max(0, advancedOptions.cpuThreads))
        )
        let relay = progressHandler.map { ProgressRelay(startTime: startTime, handler: $0) }

        do {
            lastCompressionReportStorage.withLock { $0 = nil }
            let report: CompressionReport
            if let override = self._createArchiveExecutorOverride {
                report = try override(config, extraOptions, relay, token)
            } else {
                let encodedConfig = Self.encodeInlineCreateConfig(
                    config,
                    advancedOptions: advancedOptions
                )
                report = try createArchiveAdvancedWithOptions(
                    config: encodedConfig,
                    extraOptions: extraOptions,
                    progress: relay,
                    token: token
                )
            }
            lastCompressionReportStorage.withLock { $0 = report }
            self._postFfiCommitBeforeDeliveryHook?()
            let duration = max(0.001, Date().timeIntervalSince(startTime))
            let throughput = report.throughputMbs > 0
                ? report.throughputMbs
                : ((Double(report.uncompressedBytes) / (1024 * 1024)) / duration)
            progressHandler?(ArchiveProgress(
                state: .completed,
                bytesProcessed: Int64(report.uncompressedBytes),
                totalBytes: Int64(report.uncompressedBytes),
                currentFileName: "Archive created",
                throughputMBs: throughput
            ))
            return report
        } catch {
            throw ArchiveError.from(error: error)
        }
    }
}
