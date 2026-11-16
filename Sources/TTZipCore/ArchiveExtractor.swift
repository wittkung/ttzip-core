// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import TTZipCoreBindings
import TTZipCoreCrypto
import TTZipCoreTypes
import Foundation

/// High-performance unified stream-based archive extraction engine (100% Pure Mozilla UniFFI Engine).
public final class ArchiveExtractor: ArchiveExtracting, Sendable {
    public let targetFormat: ArchiveCompressionFormat?

    public init(
        hardwareTuner: HardwareTunerProtocol? = nil,
        targetFormat: ArchiveCompressionFormat? = nil
    ) {
        self.targetFormat = targetFormat
    }

    /// Synchronously extracts an archive to the destination directory via UniFFI, returning extracted byte count.
    @inline(__always)
    @discardableResult
    public func extractSync(
        archivePath: String,
        destinationDir: String,
        options: ArchiveFilterOptions = .defaultClean,
        password: String? = nil,
        advancedOptions: ArchiveAdvancedOptions? = nil,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil,
        token: CancellationToken? = nil
    ) throws -> Int64 {
        try Self.prepareExtractionPaths(archivePath: archivePath, destinationDir: destinationDir)
        Self.logMissingQuarantineWorkerIfUntrusted(archivePath: archivePath)

        let spotlightCreated = Self.preventSpotlightIndexing(at: destinationDir)
        defer {
            Self.removeSpotlightPrevention(at: destinationDir, wasCreated: spotlightCreated)
        }

        var lastExtractionError: Error?
        do {
            let extractedBytes = try performUniFFIExtraction(
                archivePath: archivePath,
                destinationDir: destinationDir,
                password: password,
                progressHandler: progressHandler,
                token: token
            )
            Self.propagateQuarantine(from: archivePath, to: destinationDir)
            return extractedBytes.clampedToInt64
        } catch {
            lastExtractionError = error
        }

        if let vaultExtracted = attemptAutoVaultUnlockSync(
            archivePath: archivePath,
            destinationDir: destinationDir,
            initialPassword: password,
            lastError: &lastExtractionError,
            progressHandler: progressHandler,
            token: token
        ) {
            return vaultExtracted
        }

        if token?.isCancelled() == true || Task.isCancelled {
            throw ArchiveError.cancelled
        }
        if let err = lastExtractionError {
            throw ArchiveError.from(error: err)
        }
        throw ArchiveError.readFailed(code: -1)
    }

    private static func prepareExtractionPaths(archivePath: String, destinationDir: String) throws {
        let fileManager = FileManager.default
        guard fileManager.fileExists(atPath: archivePath) else {
            throw ArchiveError.fileNotFound
        }
        if !fileManager.fileExists(atPath: destinationDir) {
            try fileManager.createDirectory(atPath: destinationDir, withIntermediateDirectories: true)
        }
    }

    private static func logMissingQuarantineWorkerIfUntrusted(archivePath: String) {
        let isUntrusted = QuarantineWorkerClient.shared.shouldRouteToSandboxWorker(url: URL(fileURLWithPath: archivePath))
        if isUntrusted && QuarantineWorkerClient.shared.locateWorkerExecutable() == nil {
            TTLogger.info(
                "Quarantine worker executable not found for untrusted archive at \(archivePath), executing via native in-process engine.",
                category: .general
            )
        }
    }

    private func attemptAutoVaultUnlockSync(
        archivePath: String,
        destinationDir: String,
        initialPassword: String?,
        lastError: inout Error?,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)?,
        token: CancellationToken?
    ) -> Int64? {
        guard initialPassword == nil || initialPassword?.isEmpty == true,
              let err = lastError,
              Self.isPasswordError(err) else {
            return nil
        }
        for vaultPwd in PasswordVaultManager.shared.candidatePasswordsForAutoUnlock() {
            do {
                let extractedBytes = try performUniFFIExtraction(
                    archivePath: archivePath,
                    destinationDir: destinationDir,
                    password: vaultPwd,
                    progressHandler: progressHandler,
                    token: token
                )
                Self.propagateQuarantine(from: archivePath, to: destinationDir)
                return extractedBytes.clampedToInt64
            } catch {
                lastError = error
                if !Self.isPasswordError(error) {
                    break
                }
            }
        }
        return nil
    }

    /// Asynchronously extracts an archive with Task cancellation support, returning extracted byte count.
    @discardableResult
    public func extract(
        archivePath: String,
        destinationDir: String,
        options: ArchiveFilterOptions = .defaultClean,
        password: String? = nil,
        advancedOptions: ArchiveAdvancedOptions? = nil,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil,
        token: CancellationToken? = nil
    ) async throws -> Int64 {
        try Self.prepareExtractionPaths(archivePath: archivePath, destinationDir: destinationDir)

        if let quarantinedBytes = try await extractQuarantinedIfEligible(
            archivePath: archivePath,
            destinationDir: destinationDir,
            password: password,
            progressHandler: progressHandler,
            token: token
        ) {
            return quarantinedBytes
        }

        let capturedToken = token
        let capturedOptions = options
        let capturedAdvanced = advancedOptions
        let capturedProgress = progressHandler
        let capturedPassword = password

        let explicitHandle = token.map { TaskExecutionHandle(token: $0) }
        return try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated, cancellationHandle: explicitHandle) { (dispatchToken: CancellationToken) in
            try self.extractSync(
                archivePath: archivePath,
                destinationDir: destinationDir,
                options: capturedOptions,
                password: capturedPassword,
                advancedOptions: capturedAdvanced,
                progressHandler: capturedProgress,
                token: capturedToken ?? dispatchToken
            )
        }
    }

    private func extractQuarantinedIfEligible(
        archivePath: String,
        destinationDir: String,
        password: String?,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)?,
        token: CancellationToken?
    ) async throws -> Int64? {
        let archiveUrl = URL(fileURLWithPath: archivePath)
        let isUntrusted = QuarantineWorkerClient.shared.shouldRouteToSandboxWorker(url: archiveUrl)
        let hasPassword = password != nil && !(password?.isEmpty ?? true)
        guard isUntrusted && !hasPassword else {
            return nil
        }
        guard QuarantineWorkerClient.shared.locateWorkerExecutable() != nil else {
            TTLogger.info(
                "Quarantine worker executable not found, executing via native in-process DirJail engine.",
                category: .general
            )
            return nil
        }
        return try await runQuarantinedExtraction(
            archivePath: archivePath,
            archiveUrl: archiveUrl,
            destinationDir: destinationDir,
            progressHandler: progressHandler,
            token: token
        )
    }

    private func runQuarantinedExtraction(
        archivePath: String,
        archiveUrl: URL,
        destinationDir: String,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)?,
        token: CancellationToken?
    ) async throws -> Int64 {
        let destUrl = URL(fileURLWithPath: destinationDir)
        let spotlightCreated = Self.preventSpotlightIndexing(at: destinationDir)
        defer {
            Self.removeSpotlightPrevention(at: destinationDir, wasCreated: spotlightCreated)
        }
        do {
            if token?.isCancelled() == true || Task.isCancelled {
                throw ArchiveError.cancelled
            }
            let startTime = Date()
            let extractedBytes = try await QuarantineWorkerClient.shared.extractQuarantinedArchive(
                url: archiveUrl,
                destinationDirectory: destUrl
            )
            Self.propagateQuarantine(from: archivePath, to: destinationDir)
            let totalBytesInt64 = extractedBytes.clampedToInt64
            let duration = max(0.001, Date().timeIntervalSince(startTime))
            let throughput = (Double(extractedBytes) / (1024 * 1024)) / duration
            progressHandler?(ArchiveProgress(
                state: .completed,
                bytesProcessed: totalBytesInt64,
                totalBytes: totalBytesInt64,
                currentFileName: archiveUrl.lastPathComponent,
                throughputMBs: throughput
            ))
            return totalBytesInt64
        } catch is CancellationError {
            throw ArchiveError.cancelled
        } catch let error as ArchiveError {
            throw error
        } catch let workerError as QuarantineWorkerError {
            throw Self.mapQuarantineWorkerError(workerError)
        } catch {
            throw ArchiveError.from(error: error)
        }
    }

    /// Unified facade method to extract an archive to the destination directory.
    @inline(__always)
    @discardableResult
    public func extractArchive(
        archivePath: String,
        destinationDir: String,
        options: ArchiveFilterOptions = .defaultClean,
        password: String? = nil,
        advancedOptions: ArchiveAdvancedOptions? = nil,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil,
        token: CancellationToken? = nil
    ) async throws -> Int64 {
        return try await extract(
            archivePath: archivePath,
            destinationDir: destinationDir,
            options: options,
            password: password,
            advancedOptions: advancedOptions,
            progressHandler: progressHandler,
            token: token
        )
    }

    /// Joins multi-volume split archive files into a continuous output file via UniFFI (protocol conformance).
    public func joinSplitVolumes(firstVolumePath: String, outputPath: String) -> Bool {
        do {
            try joinSplitVolumesThrows(firstVolumePath: firstVolumePath, outputPath: outputPath)
            return true
        } catch {
            return false
        }
    }

    /// Joins multi-volume split archive files into a continuous output file with exact ArchiveError propagation.
    public func joinSplitVolumesThrows(firstVolumePath: String, outputPath: String) throws {
        do {
            try joinSplitVolumeChain(firstVolumePath: firstVolumePath, outputPath: outputPath)
        } catch {
            throw ArchiveError.from(error: error)
        }
    }

    // MARK: - Helpers

    /// Propagates com.apple.quarantine extended attribute from the parent archive to destinationDir and all extracted files.
    public static func propagateQuarantine(from sourceArchive: String, to destinationDir: String) {
        let qValue: [CChar]? = sourceArchive.withCString { srcPtr in
            let sz = getxattr(srcPtr, "com.apple.quarantine", nil, 0, 0, XATTR_NOFOLLOW)
            guard sz > 0 else { return nil }
            var buffer = [CChar](repeating: 0, count: sz)
            let readSz = getxattr(srcPtr, "com.apple.quarantine", &buffer, sz, 0, XATTR_NOFOLLOW)
            guard readSz > 0 else { return nil }
            return buffer
        }
        guard let quarantine = qValue, !quarantine.isEmpty else { return }

        // Apply to destinationDir itself
        destinationDir.withCString { dstPtr in
            var st = stat()
            if lstat(dstPtr, &st) == 0 && (st.st_mode & S_IFMT) == S_IFLNK {
                return
            }
            _ = setxattr(dstPtr, "com.apple.quarantine", quarantine, quarantine.count, 0, XATTR_NOFOLLOW)
        }

        // Apply recursively to all extracted items, actively skipping symbolic links
        let fm = FileManager.default
        guard let enumerator = fm.enumerator(atPath: destinationDir) else { return }
        while let relativePath = enumerator.nextObject() as? String {
            // Actively skip symbolic links to avoid propagating quarantine to external targets
            if let fileType = enumerator.fileAttributes?[.type] as? FileAttributeType, fileType == .typeSymbolicLink {
                continue
            }
            let fullPath = (destinationDir as NSString).appendingPathComponent(relativePath)
            fullPath.withCString { itemPtr in
                var st = stat()
                if lstat(itemPtr, &st) == 0 && (st.st_mode & S_IFMT) == S_IFLNK {
                    return
                }
                _ = setxattr(itemPtr, "com.apple.quarantine", quarantine, quarantine.count, 0, XATTR_NOFOLLOW)
            }
        }
    }

    private static let spotlightToken = Data("TTZIP_SPOTLIGHT_PREVENTION_TOKEN_V1".utf8)

    @discardableResult
    package static func preventSpotlightIndexing(at dirPath: String) -> Bool {
        let noIndexFilePath = (dirPath as NSString).appendingPathComponent(".noindex")
        let fm = FileManager.default
        guard !fm.fileExists(atPath: noIndexFilePath) else { return false }
        return fm.createFile(atPath: noIndexFilePath, contents: spotlightToken)
    }

    package static func removeSpotlightPrevention(at dirPath: String, wasCreated: Bool = true) {
        guard wasCreated else { return }
        let noIndexFilePath = (dirPath as NSString).appendingPathComponent(".noindex")
        let fm = FileManager.default

        var isDir: ObjCBool = false
        guard fm.fileExists(atPath: noIndexFilePath, isDirectory: &isDir), !isDir.boolValue else {
            return
        }

        guard let attributes = try? fm.attributesOfItem(atPath: noIndexFilePath),
              let size = attributes[.size] as? UInt64,
              size == UInt64(spotlightToken.count),
              let content = try? Data(contentsOf: URL(fileURLWithPath: noIndexFilePath)),
              content == spotlightToken else {
            return
        }

        try? fm.removeItem(atPath: noIndexFilePath)
    }
}

// MARK: - Extractor Dispatch

private final class ExtractProgressRelay: ProgressHandler, @unchecked Sendable {
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
            bytesProcessed: processedBytes.clampedToInt64,
            totalBytes: totalBytes.clampedToInt64,
            currentFileName: currentEntry ?? "",
            throughputMBs: throughput
        ))
        return true
    }
}

extension ArchiveExtractor {
    internal static func isPasswordError(_ error: Error) -> Bool {
        if let ttErr = error as? TtZipError {
            if case .InvalidPassword = ttErr {
                return true
            }
            return false
        }
        if let archErr = error as? ArchiveError {
            switch archErr {
            case .passwordRequired, .wrongPassword:
                return true
            default:
                return false
            }
        }
        return false
    }

    internal static func mapQuarantineWorkerError(_ error: QuarantineWorkerError) -> ArchiveError {
        switch error {
        case .fileNotFound:
            return .fileNotFound
        case .zipSlipDetected(let msg):
            return .engineFailure(
                code: RustTTZipStatusCode.errSecurityViolation.rawValue,
                message: msg
            )
        case .executionFailed(let code, let msg):
            let lower = msg.lowercased()
            if code == -30
                || code == -403
                || lower.contains("zip slip")
                || lower.contains("security")
                || lower.contains("symlink")
                || lower.contains("traversal")
            {
                return .engineFailure(
                    code: RustTTZipStatusCode.errSecurityViolation.rawValue,
                    message: msg
                )
            }
            return .engineFailure(
                code: RustTTZipStatusCode.errExtractionFailed.rawValue,
                message: msg
            )
        case .workerCrashed(let exitCode, let reason):
            return .engineFailure(code: exitCode, message: reason)
        case .communicationFailure(let reason):
            return .engineFailure(
                code: RustTTZipStatusCode.errExtractionFailed.rawValue,
                message: reason
            )
        case .workerNotFound:
            return .engineFailure(
                code: RustTTZipStatusCode.errSecurityViolation.rawValue,
                message: error.localizedDescription
            )
        }
    }

    internal func performUniFFIExtraction(
        archivePath: String,
        destinationDir: String,
        password: String?,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil,
        token: CancellationToken? = nil
    ) throws -> UInt64 {
        let startTime = Date()
        let relay = progressHandler.map { ExtractProgressRelay(startTime: startTime, handler: $0) }

        let report = try extractArchiveStream(
            archivePath: archivePath,
            destinationDir: destinationDir,
            policy: .keepBoth,
            password: password,
            progress: relay,
            token: token
        )
        return report.uncompressedBytes
    }
}

// MARK: - Selective Extractor

/// High-performance selective archive extractor for targeted file subsets (100% Pure Mozilla UniFFI Engine).
public final class ArchiveSelectiveExtractor: Sendable {
    public static let shared = ArchiveSelectiveExtractor()
    
    private init() {}
    
    /// Performs one opt-in bounded read of a plaintext solid LZMA2 entry. Any failure is terminal.
    public func extractSolidLzma2PreviewData(
        archivePath: String,
        entryPath: String,
        token: CancellationToken? = nil
    ) async throws -> Data {
        do {
            let data = try await NativeComputeDispatcher.shared.dispatchCompute(
                cancellationHandle: token.map { TaskExecutionHandle(token: $0) },
                cancellationPolicy: .waitForWorkerCompletion
            ) { (workerToken: CancellationToken) in
                try extractSolidLzma2Preview(
                    archivePath: archivePath,
                    entryPath: entryPath,
                    token: workerToken
                )
            }
            try Task.checkCancellation()
            return data
        } catch is CancellationError {
            throw SolidLzma2PreviewError.Cancelled
        }
    }

    /// Extracts a single entry directly into memory for instant Space-bar Quick Look or Drag-and-Drop.
    /// Memory preview limit strictly defaults to 64MB aligning with microkernel bounds.
    public func extractSingleEntryData(
        archivePath: String,
        entryPath: String,
        password: String? = nil,
        maxAllowedBytes: Int = 64 * 1024 * 1024
    ) async throws -> Data? {
        if let cached = VFSLz4CachePool.shared.getCachedEntry(archivePath: archivePath, entryPath: entryPath) {
            try Self.enforceSizeLimit(cached.count, maxAllowedBytes: maxAllowedBytes, label: "Cached entry")
            return cached
        }
        
        return try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated) {
            try Self.extractEntrySync(
                archivePath: archivePath,
                entryPath: entryPath,
                password: password,
                maxAllowedBytes: maxAllowedBytes
            )
        }
    }

    private static func extractEntrySync(
        archivePath: String,
        entryPath: String,
        password: String?,
        maxAllowedBytes: Int
    ) throws -> Data? {
        if let fastPathData = try extractEntryFastPath(
            archivePath: archivePath,
            entryPath: entryPath,
            password: password,
            maxAllowedBytes: maxAllowedBytes
        ) {
            return fastPathData
        }
        return try extractEntryViaCatalogFallback(
            archivePath: archivePath,
            entryPath: entryPath,
            password: password,
            maxAllowedBytes: maxAllowedBytes
        )
    }

    private static func extractEntryFastPath(
        archivePath: String,
        entryPath: String,
        password: String?,
        maxAllowedBytes: Int
    ) throws -> Data? {
        do {
            let data = try extractSingleEntryByPath(archivePath: archivePath, entryPath: entryPath, password: password)
            try enforceSizeLimit(data.count, maxAllowedBytes: maxAllowedBytes, label: "Extracted entry")
            VFSLz4CachePool.shared.cacheEntry(archivePath: archivePath, entryPath: entryPath, data: data)
            return data
        } catch let error as TtZipError {
            try rethrowFatalSelectiveError(error, archivePath: archivePath, password: password)
            return nil
        } catch let error as ArchiveError {
            throw error
        } catch {
            return nil
        }
    }

    private static func extractEntryViaCatalogFallback(
        archivePath: String,
        entryPath: String,
        password: String?,
        maxAllowedBytes: Int
    ) throws -> Data? {
        let entries: [UniFfiEntryMetadata]
        do {
            entries = try inspectArchiveEntries(archivePath: archivePath, password: password)
        } catch let error as TtZipError {
            try rethrowFatalSelectiveError(error, archivePath: archivePath, password: password)
            return nil
        } catch {
            return nil
        }

        guard let idx = entries.firstIndex(where: { matchesEntryPath($0.path, target: entryPath) }) else {
            return nil
        }
        let entry = entries[idx]
        if entry.uncompressedSize > UInt64(maxAllowedBytes) {
            throw ArchiveError.engineFailure(
                code: RustTTZipStatusCode.errOutOfMemory.rawValue,
                message: "Entry uncompressed size \(entry.uncompressedSize) exceeds memory limit of \(maxAllowedBytes) bytes"
            )
        }

        do {
            let data = try extractSingleEntryStream(archivePath: archivePath, entryIndex: UInt64(idx), password: password)
            try enforceSizeLimit(data.count, maxAllowedBytes: maxAllowedBytes, label: "Extracted stream")
            VFSLz4CachePool.shared.cacheEntry(archivePath: archivePath, entryPath: entryPath, data: data)
            return data
        } catch let error as TtZipError {
            try rethrowFatalSelectiveError(error, archivePath: archivePath, password: password)
            return nil
        } catch let error as ArchiveError {
            throw error
        } catch {
            return nil
        }
    }

    private static func matchesEntryPath(_ candidate: String, target: String) -> Bool {
        if candidate == target || candidate.hasSuffix("/" + target) {
            return true
        }
        guard candidate.contains("/") else {
            return false
        }
        return (candidate as NSString).lastPathComponent == target
    }

    private static func enforceSizeLimit(_ count: Int, maxAllowedBytes: Int, label: String) throws {
        if count > maxAllowedBytes {
            throw ArchiveError.engineFailure(
                code: RustTTZipStatusCode.errOutOfMemory.rawValue,
                message: "\(label) exceeds memory limit of \(maxAllowedBytes) bytes"
            )
        }
    }

    private static func rethrowFatalSelectiveError(
        _ error: TtZipError,
        archivePath: String,
        password: String?
    ) throws {
        switch error {
        case .InvalidPassword:
            if password == nil {
                throw ArchiveError.passwordRequired
            }
            throw ArchiveError.wrongPassword(archivePath: archivePath)
        case .DecompressionBombDetected(let reason), .SecurityViolation(let reason):
            throw ArchiveError.engineFailure(
                code: RustTTZipStatusCode.errSecurityViolation.rawValue,
                message: reason
            )
        case .CorruptHeader:
            throw ArchiveError.corruptHeader
        default:
            break
        }
    }
}
