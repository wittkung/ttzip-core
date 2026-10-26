// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

public typealias UniFFICreateArchiveConfig = UniFfiCreateArchiveConfig

/// High-performance multi-format archive compression engine (100% Pure Mozilla UniFFI Engine).
public final class ArchiveWriter: ArchiveWriting, Sendable {
    internal let hardwareTuner: HardwareTunerProtocol
    public let targetFormat: ArchiveCompressionFormat?

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
        try await createArchive(
            outputPath: request.outputPath,
            format: request.format,
            level: request.level,
            inputPaths: request.inputPaths,
            options: request.options,
            splitVolumeSizeBytes: request.splitVolumeSizeBytes,
            password: request.password,
            advancedOptions: request.advancedOptions,
            progressHandler: request.progressHandler
        )
    }

    /// Synchronously creates an archive using a structured `ArchiveWriteRequest`.
    public func createArchiveSync(_ request: ArchiveWriteRequest) throws {
        try createArchiveSync(
            outputPath: request.outputPath,
            format: request.format,
            level: request.level,
            inputPaths: request.inputPaths,
            options: request.options,
            password: request.password,
            splitVolumeSizeBytes: request.splitVolumeSizeBytes,
            advancedOptions: request.advancedOptions,
            progressHandler: request.progressHandler
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
        guard !inputPaths.isEmpty else {
            throw ArchiveError.readFailed(code: -10)
        }

        try Task.checkCancellation()

        if token?.isCancelled() == true {
            throw ArchiveError.cancelled
        }

        let boundToken = token ?? CancellationToken()
        let handle = TaskExecutionHandle(token: boundToken)

        do {
            try await NativeComputeDispatcher.shared.dispatchCompute(
                qos: .userInitiated,
                cancellationHandle: handle
            ) { [weak self] (_: CancellationToken) in
                guard let self = self else { return }
                try self.createArchiveSync(
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
        guard !inputPaths.isEmpty else {
            throw ArchiveError.readFailed(code: -10)
        }

        let startTime = Date()

        _ = try createArchiveInternal(
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

    internal static func mapUniFFIFormat(_ format: ArchiveCompressionFormat) -> TtZipArchiveFormat {
        switch format {
        case .sevenZip: return .sevenZip
        case .zip: return .zip
        case .tar: return .tar
        case .tarGz: return .tarGz
        case .gz: return .gzip
        case .tarBz2: return .tarBz2
        case .bz2: return .bzip2
        case .tarXz: return .tarXz
        case .xz: return .xz
        case .tarZst: return .tarZstd
        case .zst: return .zstd
        case .tarLz4: return .tarLz4
        case .lz4: return .lz4
        case .tarBrotli: return .tarBrotli
        case .brotli: return .brotli
        case .tarLzip: return .tarLzip
        case .lzip: return .lzip
        case .tarLrzip: return .tarLrzip
        case .lrzip: return .lrzip
        case .dmg: return .dmg
        case .iso: return .iso
        case .cab: return .cab
        case .wim: return .wim
        case .aar: return .aar
        case .cpio: return .cpio
        case .ar: return .ar
        case .deb: return .deb
        case .rpm: return .rpm
        case .xar: return .xar
        case .snappy: return .snappy
        case .lzfse: return .lzfse
        case .squashfs: return .squashfs
        case .lzh: return .lzh
        case .rar: return .rar
        }
    }

    internal static func mapUniFFILevel(_ level: ArchiveCompressionLevel) -> Int32 {
        Int32(max(0, min(22, level.rawValue)))
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
    private static func validateAndBuildExtraOptions(
        outputPath: String,
        targetFmt: ArchiveCompressionFormat,
        password: String?,
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
            advancedOptions: advancedOptions
        )

        let uniffiFmt = ArchiveWriter.mapUniFFIFormat(targetFmt)
        let uniffiLvl = ArchiveWriter.mapUniFFILevel(level)
        let splitBytes: UInt64? = splitVolumeSizeBytes.flatMap { $0 > 0 ? UInt64($0) : nil }
        let threadBudget = UInt32(max(0, advancedOptions.cpuThreads))

        let config = UniFfiCreateArchiveConfig(
            sourcePaths: inputPaths,
            outputPath: outputPath,
            format: uniffiFmt,
            level: uniffiLvl,
            password: password,
            splitVolumeSizeBytes: splitBytes,
            excludePatterns: options.excludePatterns,
            includePatterns: options.includePatterns,
            excludeVcs: options.excludeVCS,
            noMacMetadata: options.noMacMetadata,
            threadBudget: threadBudget
        )

        let relay = progressHandler.map {
            ProgressRelay(startTime: startTime, handler: $0)
        }

        do {
            let report = try createArchiveAdvancedWithOptions(
                config: config,
                extraOptions: extraOptions,
                progress: relay,
                token: token
            )

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
            if token?.isCancelled() == true || Task.isCancelled {
                throw ArchiveError.cancelled
            }
            throw ArchiveError.from(error: error)
        }
    }
}
