// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

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
        let fileManager = FileManager.default
        guard fileManager.fileExists(atPath: archivePath) else {
            throw ArchiveError.fileNotFound
        }

        if !fileManager.fileExists(atPath: destinationDir) {
            try fileManager.createDirectory(atPath: destinationDir, withIntermediateDirectories: true)
        }

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

        if password == nil || password?.isEmpty == true {
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
                    lastExtractionError = error
                }
            }
        }

        if token?.isCancelled() == true || Task.isCancelled {
            throw ArchiveError.cancelled
        }
        if let err = lastExtractionError {
            throw ArchiveError.from(error: err)
        }
        throw ArchiveError.readFailed(code: -1)
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
        let fileManager = FileManager.default
        guard fileManager.fileExists(atPath: archivePath) else {
            throw ArchiveError.fileNotFound
        }

        if !fileManager.fileExists(atPath: destinationDir) {
            try fileManager.createDirectory(atPath: destinationDir, withIntermediateDirectories: true)
        }


        let capturedToken = token
        let capturedOptions = options
        let capturedAdvanced = advancedOptions
        let capturedProgress = progressHandler
        let capturedPassword = password

        let explicitHandle = token.map { TaskExecutionHandle(token: $0) }
        let bytes = try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated, cancellationHandle: explicitHandle) { (dispatchToken: CancellationToken) in
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

        return bytes
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

    /// Extracts a single file matching the ArchiveExtracting protocol requirement.
    public func extractSingleFile(
        archivePath: String,
        entryPath: String,
        destinationDir: String,
        password: String?
    ) async throws {
        try await extractSingleFile(
            archivePath: archivePath,
            entryPath: entryPath,
            destinationDir: destinationDir,
            password: password,
            token: nil
        )
    }

    /// Asynchronously extracts a single file from the archive without processing other entries.
    public func extractSingleFile(
        archivePath: String,
        entryPath: String,
        destinationDir: String,
        password: String?,
        token: CancellationToken?
    ) async throws {
        let explicitHandle = token.map { TaskExecutionHandle(token: $0) }
        try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated, cancellationHandle: explicitHandle) { (dispatchToken: CancellationToken) in
            let fileManager = FileManager.default
            if !fileManager.fileExists(atPath: destinationDir) {
                try fileManager.createDirectory(atPath: destinationDir, withIntermediateDirectories: true)
            }

            let spotlightCreated = Self.preventSpotlightIndexing(at: destinationDir)
            defer {
                Self.removeSpotlightPrevention(at: destinationDir, wasCreated: spotlightCreated)
            }

            _ = try extractSelectedEntries(
                archivePath: archivePath,
                targetEntries: [entryPath],
                destinationDir: destinationDir,
                password: password,
                progress: nil,
                token: token ?? dispatchToken
            )
            Self.propagateQuarantine(from: archivePath, to: destinationDir)
        }
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
    internal static func propagateQuarantine(from sourceArchive: String, to destinationDir: String) {
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

    private static let spotlightToken = "TTZIP_SPOTLIGHT_PREVENTION_TOKEN_V1".data(using: .utf8)!

    @discardableResult
    internal static func preventSpotlightIndexing(at dirPath: String) -> Bool {
        let noIndexFilePath = (dirPath as NSString).appendingPathComponent(".noindex")
        let fm = FileManager.default
        guard !fm.fileExists(atPath: noIndexFilePath) else { return false }
        return fm.createFile(atPath: noIndexFilePath, contents: spotlightToken)
    }

    internal static func removeSpotlightPrevention(at dirPath: String, wasCreated: Bool = true) {
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
    
    /// Selectively extracts a subset of files matching targetEntryPaths into destinationDir via single-pass UniFFI stream.
    public func extractSelected(
        archivePath: String,
        targetEntryPaths: Set<String>,
        destinationDir: String,
        password: String? = nil,
        token: CancellationToken? = nil
    ) async throws -> Int {
        guard !targetEntryPaths.isEmpty else { return 0 }
        
        let fm = FileManager.default
        if !fm.fileExists(atPath: destinationDir) {
            try fm.createDirectory(atPath: destinationDir, withIntermediateDirectories: true)
        }
        
        let targetsArray = Array(targetEntryPaths)
        let explicitHandle = token.map { TaskExecutionHandle(token: $0) }
        return try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated, cancellationHandle: explicitHandle) { (dispatchToken: CancellationToken) in
            let count = try extractSelectedEntries(
                archivePath: archivePath,
                targetEntries: targetsArray,
                destinationDir: destinationDir,
                password: password,
                progress: nil,
                token: token ?? dispatchToken
            )
            return Int(count)
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
            if cached.count > maxAllowedBytes {
                throw ArchiveError.engineFailure(
                    code: RustTTZipStatusCode.errOutOfMemory.rawValue,
                    message: "Cached entry exceeds memory limit of \(maxAllowedBytes) bytes"
                )
            }
            return cached
        }
        
        return try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated) {
            // Fast-path: extract directly by path without parsing entire archive catalog
            if let data = try? extractSingleEntryByPath(archivePath: archivePath, entryPath: entryPath, password: password) {
                if data.count > maxAllowedBytes {
                    throw ArchiveError.engineFailure(
                        code: RustTTZipStatusCode.errOutOfMemory.rawValue,
                        message: "Extracted entry exceeds memory limit of \(maxAllowedBytes) bytes"
                    )
                }
                VFSLz4CachePool.shared.cacheEntry(archivePath: archivePath, entryPath: entryPath, data: data)
                return data
            }

            // Fallback for subpaths or index probing only if direct path lookup fails
            let entries = (try? inspectArchiveEntries(archivePath: archivePath, password: password)) ?? []
            guard let idx = entries.firstIndex(where: {
                $0.path == entryPath || $0.path.hasSuffix("/" + entryPath) || ($0.path.contains("/") ? String($0.path.split(separator: "/").last!) == entryPath : false)
            }) else {
                return nil
            }
            let entry = entries[idx]
            if entry.uncompressedSize > UInt64(maxAllowedBytes) {
                throw ArchiveError.engineFailure(
                    code: RustTTZipStatusCode.errOutOfMemory.rawValue,
                    message: "Entry uncompressed size \(entry.uncompressedSize) exceeds memory limit of \(maxAllowedBytes) bytes"
                )
            }
            if let data = try? extractSingleEntryStream(archivePath: archivePath, entryIndex: UInt64(idx), password: password) {
                if data.count > maxAllowedBytes {
                    throw ArchiveError.engineFailure(
                        code: RustTTZipStatusCode.errOutOfMemory.rawValue,
                        message: "Extracted stream exceeds memory limit of \(maxAllowedBytes) bytes"
                    )
                }
                VFSLz4CachePool.shared.cacheEntry(archivePath: archivePath, entryPath: entryPath, data: data)
                return data
            }
            return nil
        }
    }
}
