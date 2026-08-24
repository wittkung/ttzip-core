// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation
import CTTZipBridge

/// High-performance unified stream-based archive extraction engine (Ultra-Thin Rust C-ABI Facade).
public final class ArchiveExtractor: ArchiveExtracting, @unchecked Sendable {
    internal let hardwareTuner: HardwareTunerProtocol
    public let targetFormat: ArchiveCompressionFormat?

    public init(
        hardwareTuner: HardwareTunerProtocol = AppleSiliconTuner.shared,
        targetFormat: ArchiveCompressionFormat? = nil
    ) {
        self.hardwareTuner = hardwareTuner
        self.targetFormat = targetFormat
    }

    /// Synchronously extracts an archive to the destination directory via Rust C-ABI, returning extracted byte count.
    @inline(__always)
    @discardableResult
    public func extractSync(
        archivePath: String,
        destinationDir: String,
        options: ArchiveFilterOptions = .defaultClean,
        password: String? = nil,
        advancedOptions: ArchiveAdvancedOptions? = nil,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) throws -> Int64 {
        let fileManager = FileManager.default
        guard fileManager.fileExists(atPath: archivePath) else {
            throw ArchiveError.fileNotFound
        }

        let dirExistedBefore = fileManager.fileExists(atPath: destinationDir)
        if !dirExistedBefore {
            try fileManager.createDirectory(atPath: destinationDir, withIntermediateDirectories: true)
        }

        // 记录解压前的既有文件集合快照
        let preExistingSubpaths = Set(fileManager.subpaths(atPath: destinationDir) ?? [])

        Self.preventSpotlightIndexing(at: destinationDir)
        defer { Self.cleanupQuarantineAttributes(at: destinationDir) }

        var extractedBytes: UInt64 = 0
        if dispatchFastExtraction(
            archivePath: archivePath,
            destinationDir: destinationDir,
            options: options,
            password: password,
            advancedOptions: advancedOptions,
            progressHandler: progressHandler,
            outExtractedBytes: &extractedBytes
        ) {
            return Int64(extractedBytes)
        }

        if password == nil || password?.isEmpty == true {
            for vaultPwd in PasswordVaultManager.shared.candidatePasswordsForAutoUnlock() {
                if dispatchFastExtraction(
                    archivePath: archivePath,
                    destinationDir: destinationDir,
                    options: options,
                    password: vaultPwd,
                    advancedOptions: advancedOptions,
                    progressHandler: progressHandler,
                    outExtractedBytes: &extractedBytes
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
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) async throws -> Int64 {
        let fileManager = FileManager.default
        guard fileManager.fileExists(atPath: archivePath) else {
            throw ArchiveError.fileNotFound
        }

        if !fileManager.fileExists(atPath: destinationDir) {
            try fileManager.createDirectory(atPath: destinationDir, withIntermediateDirectories: true)
        }

        Self.preventSpotlightIndexing(at: destinationDir)
        try Task.checkCancellation()

        let bytes = try await Task.detached(priority: .userInitiated) {
            try self.extractSync(
                archivePath: archivePath,
                destinationDir: destinationDir,
                options: options,
                password: password,
                advancedOptions: advancedOptions,
                progressHandler: progressHandler
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
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) async throws -> Int64 {
        return try await extract(
            archivePath: archivePath,
            destinationDir: destinationDir,
            options: options,
            password: password,
            advancedOptions: advancedOptions,
            progressHandler: progressHandler
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

            let pwd = (password != nil && !password!.isEmpty) ? password : nil
            let status = CUnsafeBufferAdapter.withCString(archivePath) { aPtr in
                CUnsafeBufferAdapter.withCString(destinationDir) { dPtr in
                    CUnsafeBufferAdapter.withCString(entryPath) { ePtr in
                        CUnsafeBufferAdapter.withCString(pwd) { pPtr in
                            guard let aPtr = aPtr, let dPtr = dPtr, let ePtr = ePtr else { return TTZIP_STATUS_ERR_INVALID_PARAM }
                            var targets: [UnsafePointer<CChar>?] = [ePtr]
                            var opt = TTZipExtractOptions(
                                destination_path: dPtr,
                                password: pPtr,
                                thread_budget: UInt32(ProcessInfo.processInfo.activeProcessorCount),
                                overwrite_existing: true,
                                preserve_permissions: true,
                                dry_run: false,
                                progress_callback: nil,
                                user_data: nil
                            )
                            var extractedCount: Int = 0
                            return targets.withUnsafeMutableBufferPointer { tPtr in
                                guard let base = tPtr.baseAddress else { return TTZIP_STATUS_ERR_INVALID_PARAM }
                                return ttzip_rust_archive_extract_selected(aPtr, base, 1, dPtr, &opt, &extractedCount)
                            }
                        }
                    }
                }
            }

            if status != TTZIP_STATUS_OK {
                throw ArchiveError.readFailed(code: status.rawValue)
            }
        }.value

        Self.cleanupQuarantineAttributes(at: destinationDir)
    }

    /// Joins multi-volume split archive files into a continuous output file via Rust C-ABI.
    public func joinSplitVolumes(firstVolumePath: String, outputPath: String) -> Bool {
        return CUnsafeBufferAdapter.withCString(firstVolumePath) { cFirst in
            CUnsafeBufferAdapter.withCString(outputPath) { cOut in
                guard let cFirst = cFirst, let cOut = cOut else { return false }
                return ttzip_rust_join_split_volumes(cFirst, cOut, nil, nil) == TTZIP_STATUS_OK
            }
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

//
//


extension ArchiveExtractor {
    /// Dispatches format-specific fast-path extraction pipelines directly via Rust microkernel C-ABI.
    internal func dispatchFastExtraction(
        archivePath: String,
        destinationDir: String,
        options: ArchiveFilterOptions,
        password: String?,
        advancedOptions: ArchiveAdvancedOptions? = nil,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil,
        outExtractedBytes: UnsafeMutablePointer<UInt64>? = nil
    ) -> Bool {
        let pwd = (password != nil && !password!.isEmpty) ? password : nil
        let threadBudget = (advancedOptions?.cpuThreads ?? 0) > 0 
            ? UInt32(advancedOptions!.cpuThreads) 
            : UInt32(ProcessInfo.processInfo.activeProcessorCount)
        let preservePerms = advancedOptions?.preservePosixAttributes ?? true
        let overwrite = true

        let bridgeCtx: ProgressBridgeContext? = progressHandler != nil ? ProgressBridgeContext(
            progressHandler: progressHandler,
            handle: nil,
            cancellationCheck: nil,
            totalExpectedBytes: 0
        ) : nil
        let ctxPtr = bridgeCtx != nil ? Unmanaged.passRetained(bridgeCtx!).toOpaque() : nil
        defer {
            if let ctxPtr = ctxPtr {
                Unmanaged<ProgressBridgeContext>.fromOpaque(ctxPtr).release()
            }
        }

        var extractedBytes: UInt64 = 0
        var errorInfo = TTZipErrorInfo()

        let status = CUnsafeBufferAdapter.withCString(archivePath) { aPtr in
            CUnsafeBufferAdapter.withCString(destinationDir) { dPtr in
                CUnsafeBufferAdapter.withCString(pwd) { pPtr in
                    guard let aPtr = aPtr, let dPtr = dPtr else { return TTZIP_STATUS_ERR_INVALID_PARAM }
                    var opt = TTZipExtractOptions(
                        destination_path: dPtr,
                        password: pPtr,
                        thread_budget: threadBudget,
                        overwrite_existing: overwrite,
                        preserve_permissions: preservePerms,
                        dry_run: false,
                        progress_callback: ctxPtr != nil ? ttzipProgressCallbackBridge : nil,
                        user_data: ctxPtr
                    )
                    return ttzip_rust_archive_extract_unified_v2(aPtr, dPtr, &opt, &extractedBytes, &errorInfo)
                }
            }
        }

        if status == TTZIP_STATUS_OK {
            outExtractedBytes?.pointee = extractedBytes
            Self.cleanupQuarantineAttributes(at: destinationDir)
            return true
        }

        return false
    }
}

// MARK: - Selective Extractor

//
//


/// High-performance selective archive extractor for targeted file subsets.
///
/// Bypasses full-archive decompression by extracting selected paths.
public final class ArchiveSelectiveExtractor: @unchecked Sendable {
    public static let shared = ArchiveSelectiveExtractor()
    
    private init() {}
    
    /// Selectively extracts a subset of files matching targetEntryPaths into destinationDir via single-pass C-ABI stream.
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
        let pwd = (password != nil && !password!.isEmpty) ? password : nil
        
        return try await Task.detached(priority: .userInitiated) {
            let (code, count) = CUnsafeBufferAdapter.withCString(archivePath) { aPtr -> (CTTZipBridge.TTZipStatus, Int) in
                CUnsafeBufferAdapter.withCString(destinationDir) { dPtr -> (CTTZipBridge.TTZipStatus, Int) in
                    CUnsafeBufferAdapter.withCString(pwd) { pPtr -> (CTTZipBridge.TTZipStatus, Int) in
                        guard let aPtr = aPtr, let dPtr = dPtr else { return (TTZIP_STATUS_ERR_INVALID_PARAM, 0) }
                        
                        let cPointers = targetsArray.map { strdup($0) }
                        defer {
                            for ptr in cPointers {
                                if let ptr = ptr { free(ptr) }
                            }
                        }
                        
                        var targets: [UnsafePointer<CChar>?] = cPointers.map { $0.map { UnsafePointer($0) } }
                        var opt = TTZipExtractOptions(
                            destination_path: dPtr,
                            password: pPtr,
                            thread_budget: UInt32(ProcessInfo.processInfo.activeProcessorCount),
                            overwrite_existing: true,
                            preserve_permissions: true,
                            dry_run: false,
                            progress_callback: nil,
                            user_data: nil
                        )
                        var extractedCount: Int = 0
                        let code = targets.withUnsafeMutableBufferPointer { tPtr -> CTTZipBridge.TTZipStatus in
                            guard let base = tPtr.baseAddress else { return TTZIP_STATUS_ERR_INVALID_PARAM }
                            return ttzip_rust_archive_extract_selected(aPtr, base, targetsArray.count, dPtr, &opt, &extractedCount)
                        }
                        return (code, extractedCount)
                    }
                }
            }
            
            if code != TTZIP_STATUS_OK {
                throw ArchiveError.readFailed(code: code.rawValue)
            }
            return count
        }.value
    }
    
    /// Extracts a single entry directly into memory for instant Space-bar Quick Look or Drag-and-Drop.
    public func extractSingleEntryData(
        archivePath: String,
        entryPath: String,
        password: String? = nil,
        maxAllowedBytes: Int = 256 * 1024 * 1024
    ) async throws -> Data? {
        // 0. VFS LZ4 Cache Pool Fast Path
        if let cached = VFSLz4CachePool.shared.getCachedEntry(archivePath: archivePath, entryPath: entryPath) {
            return cached
        }
        
        return await Task.detached(priority: .userInitiated) {
            // Stage 1: Probe exact uncompressed size with NULL buffer
            var probedLen: Int = 0
            let probeStatus = archivePath.withCString { cArch in
                entryPath.withCString { cEntry in
                    CUnsafeBufferAdapter.withCString(password) { cPwd in
                        ttzip_rust_archive_extract_single_entry_memory(
                            cArch,
                            cEntry,
                            -1,
                            cPwd,
                            nil,
                            0,
                            &probedLen
                        )
                    }
                }
            }
            
            guard probeStatus == TTZIP_STATUS_OK, probedLen > 0, probedLen <= maxAllowedBytes else {
                return nil
            }
            
            // Stage 2: Allocate EXACT buffer size (zero waste, zero truncation)
            var memBuffer = [UInt8](repeating: 0, count: probedLen)
            var actualExtractedLen: Int = 0
            
            let extractStatus = archivePath.withCString { cArch in
                entryPath.withCString { cEntry in
                    CUnsafeBufferAdapter.withCString(password) { cPwd in
                        memBuffer.withUnsafeMutableBufferPointer { bPtr in
                            guard let base = bPtr.baseAddress else { return TTZIP_STATUS_ERR_INVALID_PARAM }
                            return ttzip_rust_archive_extract_single_entry_memory(
                                cArch,
                                cEntry,
                                -1,
                                cPwd,
                                base,
                                probedLen,
                                &actualExtractedLen
                            )
                        }
                    }
                }
            }
            
            if extractStatus == TTZIP_STATUS_OK && actualExtractedLen == probedLen {
                let data = Data(memBuffer)
                VFSLz4CachePool.shared.cacheEntry(archivePath: archivePath, entryPath: entryPath, data: data)
                return data
            }
            
            return nil
        }.value
    }
}
