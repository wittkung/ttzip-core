// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// High-performance unified stream-based archive extraction engine (100% Pure Mozilla UniFFI Engine).
public final class ArchiveExtractor: ArchiveExtracting, Sendable {
    internal let hardwareTuner: HardwareTunerProtocol
    public let targetFormat: ArchiveCompressionFormat?

    public init(
        hardwareTuner: HardwareTunerProtocol = AppleSiliconTuner.shared,
        targetFormat: ArchiveCompressionFormat? = nil
    ) {
        self.hardwareTuner = hardwareTuner
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

        let dirExistedBefore = fileManager.fileExists(atPath: destinationDir)
        if !dirExistedBefore {
            try fileManager.createDirectory(atPath: destinationDir, withIntermediateDirectories: true)
        }

        let preExistingSubpaths = Set(fileManager.subpaths(atPath: destinationDir) ?? [])
        Self.preventSpotlightIndexing(at: destinationDir)
        defer { Self.cleanupQuarantineAttributes(at: destinationDir) }

        var extractedBytes: UInt64 = 0
        if dispatchUniFFIExtraction(
            archivePath: archivePath,
            destinationDir: destinationDir,
            password: password,
            progressHandler: progressHandler,
            outExtractedBytes: &extractedBytes,
            token: token
        ) {
            return Int64(extractedBytes)
        }

        if password == nil || password?.isEmpty == true {
            for vaultPwd in PasswordVaultManager.shared.candidatePasswordsForAutoUnlock() {
                if dispatchUniFFIExtraction(
                    archivePath: archivePath,
                    destinationDir: destinationDir,
                    password: vaultPwd,
                    progressHandler: progressHandler,
                    outExtractedBytes: &extractedBytes,
                    token: token
                ) {
                    return Int64(extractedBytes)
                }
            }
        }

        // 安全差集回滚：严格仅删除本次产生的新增文件，严禁删除既有文件
        let currentSubpaths = Set(fileManager.subpaths(atPath: destinationDir) ?? [])
        let newlyCreated = currentSubpaths.subtracting(preExistingSubpaths)
        let sortedToClean = newlyCreated.sorted {
            $0.components(separatedBy: "/").count > $1.components(separatedBy: "/").count
        }

        for relPath in sortedToClean {
            let fullPath = (destinationDir as NSString).appendingPathComponent(relPath)
            try? fileManager.removeItem(atPath: fullPath)
        }

        if !dirExistedBefore {
            let remaining = (try? fileManager.contentsOfDirectory(atPath: destinationDir)) ?? []
            if remaining.isEmpty || remaining == [".noindex"] {
                try? fileManager.removeItem(atPath: destinationDir)
            }
        }

        if token?.isCancelled() == true || Task.isCancelled {
            throw ArchiveError.cancelled
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

        Self.preventSpotlightIndexing(at: destinationDir)
        let capturedToken = token
        let capturedOptions = options
        let capturedAdvanced = advancedOptions
        let capturedProgress = progressHandler
        let capturedPassword = password

        let bytes = try await Task.detached(priority: .userInitiated) {
            try self.extractSync(
                archivePath: archivePath,
                destinationDir: destinationDir,
                options: capturedOptions,
                password: capturedPassword,
                advancedOptions: capturedAdvanced,
                progressHandler: capturedProgress,
                token: capturedToken
            )
        }.value

        Self.cleanupQuarantineAttributes(at: destinationDir)
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

    /// Synchronously extracts a single file from the archive without processing other entries.
    public func extractSingleFile(
        archivePath: String,
        entryPath: String,
        destinationDir: String,
        password: String? = nil
    ) async throws {
        try await Task.detached(priority: .userInitiated) {
            let fileManager = FileManager.default
            if !fileManager.fileExists(atPath: destinationDir) {
                try fileManager.createDirectory(atPath: destinationDir, withIntermediateDirectories: true)
            }

            _ = try extractSelectedEntries(
                archivePath: archivePath,
                targetEntries: [entryPath],
                destinationDir: destinationDir,
                password: password,
                progress: nil,
                token: nil
            )
        }.value

        Self.cleanupQuarantineAttributes(at: destinationDir)
    }

    /// Joins multi-volume split archive files into a continuous output file via UniFFI.
    public func joinSplitVolumes(firstVolumePath: String, outputPath: String) -> Bool {
        do {
            try joinSplitVolumeChain(firstVolumePath: firstVolumePath, outputPath: outputPath)
            return true
        } catch {
            return false
        }
    }

    // MARK: - Helpers

    internal static func cleanupQuarantineAttributes(at dirPath: String) {
        dirPath.withCString { pathPtr in
            let sz = getxattr(pathPtr, "com.apple.quarantine", nil, 0, 0, XATTR_NOFOLLOW)
            if sz > 0 {
                removexattr(pathPtr, "com.apple.quarantine", XATTR_NOFOLLOW)
            }
        }
    }

    private static func preventSpotlightIndexing(at dirPath: String) {
        let noIndexFilePath = (dirPath as NSString).appendingPathComponent(".noindex")
        if !FileManager.default.fileExists(atPath: noIndexFilePath) {
            FileManager.default.createFile(atPath: noIndexFilePath, contents: nil)
        }
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
            bytesProcessed: Int64(processedBytes),
            totalBytes: Int64(totalBytes),
            currentFileName: currentEntry ?? "",
            throughputMBs: throughput
        ))
        return true
    }
}

extension ArchiveExtractor {
    internal func dispatchUniFFIExtraction(
        archivePath: String,
        destinationDir: String,
        password: String?,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil,
        outExtractedBytes: UnsafeMutablePointer<UInt64>? = nil,
        token: CancellationToken? = nil
    ) -> Bool {
        let startTime = Date()
        let relay = progressHandler.map { ExtractProgressRelay(startTime: startTime, handler: $0) }

        do {
            let report = try extractArchiveStream(
                archivePath: archivePath,
                destinationDir: destinationDir,
                password: password,
                progress: relay,
                token: token
            )
            outExtractedBytes?.pointee = report.uncompressedBytes
            Self.cleanupQuarantineAttributes(at: destinationDir)
            return true
        } catch {
            return false
        }
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
        password: String? = nil
    ) async throws -> Int {
        guard !targetEntryPaths.isEmpty else { return 0 }
        
        let fm = FileManager.default
        if !fm.fileExists(atPath: destinationDir) {
            try fm.createDirectory(atPath: destinationDir, withIntermediateDirectories: true)
        }
        
        let targetsArray = Array(targetEntryPaths)
        return try await Task.detached(priority: .userInitiated) {
            let count = try extractSelectedEntries(
                archivePath: archivePath,
                targetEntries: targetsArray,
                destinationDir: destinationDir,
                password: password,
                progress: nil,
                token: nil
            )
            return Int(count)
        }.value
    }
    
    /// Extracts a single entry directly into memory for instant Space-bar Quick Look or Drag-and-Drop.
    public func extractSingleEntryData(
        archivePath: String,
        entryPath: String,
        password: String? = nil,
        maxAllowedBytes: Int = 256 * 1024 * 1024
    ) async throws -> Data? {
        if let cached = VFSLz4CachePool.shared.getCachedEntry(archivePath: archivePath, entryPath: entryPath) {
            return cached
        }
        
        return await Task.detached(priority: .userInitiated) {
            if let bytes = try? extractSingleEntryByPath(archivePath: archivePath, entryPath: entryPath, password: password) {
                let data = Data(bytes)
                VFSLz4CachePool.shared.cacheEntry(archivePath: archivePath, entryPath: entryPath, data: data)
                return data
            }
            // Fallback for subpaths or index probing if path normalization differs
            let entries = (try? inspectArchiveEntries(archivePath: archivePath, password: password)) ?? []
            guard let idx = entries.firstIndex(where: {
                $0.path == entryPath || $0.path.hasSuffix("/" + entryPath) || ($0.path.contains("/") ? String($0.path.split(separator: "/").last!) == entryPath : false)
            }) else {
                return nil
            }
            if let bytes = try? extractSingleEntryStream(archivePath: archivePath, entryIndex: UInt64(idx), password: password) {
                let data = Data(bytes)
                VFSLz4CachePool.shared.cacheEntry(archivePath: archivePath, entryPath: entryPath, data: data)
                return data
            }
            return nil
        }.value
    }
}
