// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

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

        try await Task.detached(priority: .userInitiated) { [weak self] in
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
                token: token
            )
        }.value
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
        let totalBytes = inputPaths.reduce(Int64(0)) { $0 + Self.recursivePathSize(at: $1) }

        try createArchiveInternal(
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
            totalBytes: totalBytes,
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
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) throws -> EngineDispatchProvenance {
        let (_, provenance) = try EngineProvenanceCollector.capture {
            try self.createArchiveSync(
                outputPath: outputPath,
                format: format,
                level: level,
                inputPaths: inputPaths,
                options: options,
                password: password,
                splitVolumeSizeBytes: splitVolumeSizeBytes,
                advancedOptions: advancedOptions,
                progressHandler: progressHandler
            )
        }
        return provenance
    }

    // MARK: - Format Mappings

    internal static func mapUniFFIFormat(_ format: ArchiveCompressionFormat) -> ArchiveFormat {
        switch format {
        case .sevenZip: return .sevenZip
        case .zip: return .zip
        case .tar: return .tar
        case .tarGz, .gz: return .tarGz
        case .tarBz2, .bz2: return .tarBz2
        case .tarXz, .xz: return .tarXz
        case .tarZst, .zst: return .tarZstd
        case .dmg: return .dmg
        case .snappy: return .snappy
        case .aar: return .lzfse
        default: return .zip
        }
    }

    internal static func mapUniFFILevel(_ level: ArchiveCompressionLevel) -> Int32 {
        switch level {
        case .store: return 0
        case .fastest: return 1
        case .fast: return 2
        case .normal: return 5
        case .maximum: return 7
        case .ultra: return 9
        default: return 5
        }
    }
}

// MARK: - Internal Dispatch

private final class ProgressRelay: ProgressHandler, @unchecked Sendable {
    let handler: (@Sendable (ArchiveProgress) -> Void)?
    let totalBytes: Int64
    let startTime: Date

    init(totalBytes: Int64, startTime: Date, handler: (@Sendable (ArchiveProgress) -> Void)?) {
        self.totalBytes = totalBytes
        self.startTime = startTime
        self.handler = handler
    }

    func onProgress(processedBytes: UInt64, totalBytes: UInt64, currentEntry: String?) -> Bool {
        let duration = max(0.001, Date().timeIntervalSince(startTime))
        let throughput = (Double(processedBytes) / (1024 * 1024)) / duration
        handler?(ArchiveProgress(
            state: .processing,
            bytesProcessed: Int64(processedBytes),
            totalBytes: Int64(totalBytes > 0 ? totalBytes : UInt64(self.totalBytes)),
            currentFileName: currentEntry ?? "",
            throughputMBs: throughput
        ))
        return true
    }
}

extension ArchiveWriter {
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
        totalBytes: Int64,
        token: CancellationToken? = nil
    ) throws {
        let targetFmt = self.targetFormat ?? format
        let uniffiFmt = ArchiveWriter.mapUniFFIFormat(targetFmt)
        let uniffiLvl = ArchiveWriter.mapUniFFILevel(level)

        let relay = progressHandler.map {
            ProgressRelay(totalBytes: totalBytes, startTime: startTime, handler: $0)
        }

        do {
            _ = try createArchiveStream(
                sourcePaths: inputPaths,
                outputPath: outputPath,
                format: uniffiFmt,
                level: uniffiLvl,
                password: password,
                progress: relay,
                token: token
            )

            let duration = max(0.001, Date().timeIntervalSince(startTime))
            let throughput = (Double(totalBytes) / (1024 * 1024)) / duration
            progressHandler?(ArchiveProgress(
                state: .completed,
                bytesProcessed: totalBytes,
                totalBytes: totalBytes,
                currentFileName: "Archive created",
                throughputMBs: throughput
            ))
        } catch {
            if token?.isCancelled() == true || Task.isCancelled {
                throw ArchiveError.cancelled
            }
            throw ArchiveError.from(error: error)
        }
    }

    /// Calculates physical directory byte size recursively.
    static func recursivePathSize(at path: String) -> Int64 {
        var isDir: ObjCBool = false
        guard FileManager.default.fileExists(atPath: path, isDirectory: &isDir) else { return 0 }
        if !isDir.boolValue {
            let attr = try? FileManager.default.attributesOfItem(atPath: path)
            return (attr?[.size] as? Int64) ?? 0
        }
        var total: Int64 = 0
        if let enumerator = FileManager.default.enumerator(atPath: path) {
            while let sub = enumerator.nextObject() as? String {
                let full = (path as NSString).appendingPathComponent(sub)
                let attr = try? FileManager.default.attributesOfItem(atPath: full)
                total += (attr?[.size] as? Int64) ?? 0
            }
        }
        return total
    }
}
