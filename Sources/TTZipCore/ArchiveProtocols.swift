// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// 3-Tier archive encryption classification topology.
public enum ArchiveEncryptionTier: String, Sendable, Codable, Equatable {
    /// Tier 0: Unencrypted archive.
    case none = "NONE"
    /// Tier 1: Entry payloads are encrypted, but metadata headers remain in cleartext (browsable without password).
    case dataOnly = "DATA_ONLY"
    /// Tier 2: Both metadata headers and entry payloads are encrypted (password required to list directory tree).
    case headerAndData = "HEADER_AND_DATA"
    /// Encryption status cannot be reliably determined or is unsupported.
    case unsupported = "UNSUPPORTED"
}

/// Core archive inspection and metadata discovery interface.
public protocol ArchiveReading: Sendable {
    /// Inspects archive contents and returns a flat array of archive entries.
    func inspect(archivePath: String) async throws -> [ArchiveEntry]
    
    /// Inspects archive contents with optional password or candidate password list.
    func inspect(archivePath: String, password: String?, candidatePasswords: [String]?) async throws -> [ArchiveEntry]
    
    /// Asynchronously inspects archive and returns a hierarchical directory tree (Composite Pattern).
    func inspectTree(archivePath: String, password: String?, candidatePasswords: [String]?) async throws -> ArchiveCompositeDirectory
    
    /// Fast zero-decompression probe of archive encryption tier.
    func probeEncryption(archivePath: String) async throws -> ArchiveEncryptionTier
}

extension ArchiveReading {
    /// Convenience facade method to list entries of an archive.
    @inline(__always)
    public func listEntries(archivePath: String, password: String? = nil) async throws -> [ArchiveEntry] {
        return try await inspect(archivePath: archivePath, password: password, candidatePasswords: nil)
    }
}

/// Core archive creation and compression engine interface.
public protocol ArchiveWriting: Sendable {
    func createArchive(_ request: ArchiveWriteRequest) async throws
    func createArchiveSync(_ request: ArchiveWriteRequest) throws
}

/// Core archive decompression and extraction engine interface.
public protocol ArchiveExtracting: Sendable {
    @discardableResult
    func extract(
        archivePath: String,
        destinationDir: String,
        options: ArchiveFilterOptions,
        password: String?,
        advancedOptions: ArchiveAdvancedOptions?,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)?,
        token: CancellationToken?
    ) async throws -> Int64

    func extractSingleFile(
        archivePath: String,
        entryPath: String,
        destinationDir: String,
        password: String?
    ) async throws
    
    @discardableResult
    func extractSync(
        archivePath: String,
        destinationDir: String,
        options: ArchiveFilterOptions,
        password: String?,
        advancedOptions: ArchiveAdvancedOptions?,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)?,
        token: CancellationToken?
    ) throws -> Int64

    func joinSplitVolumes(firstVolumePath: String, outputPath: String) -> Bool
}

extension ArchiveExtracting {
    @discardableResult
    public func extract(
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
            progressHandler: progressHandler,
            token: nil
        )
    }

    @discardableResult
    public func extractSync(
        archivePath: String,
        destinationDir: String,
        options: ArchiveFilterOptions = .defaultClean,
        password: String? = nil,
        advancedOptions: ArchiveAdvancedOptions? = nil,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) throws -> Int64 {
        return try extractSync(
            archivePath: archivePath,
            destinationDir: destinationDir,
            options: options,
            password: password,
            advancedOptions: advancedOptions,
            progressHandler: progressHandler,
            token: nil
        )
    }

    /// Convenience facade method to extract an archive.
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
            progressHandler: progressHandler,
            token: nil
        )
    }

    /// Convenience facade method to extract an archive without progress handler.
    @inline(__always)
    @discardableResult
    public func extract(
        archivePath: String,
        destinationDir: String,
        options: ArchiveFilterOptions = .defaultClean,
        password: String? = nil,
        advancedOptions: ArchiveAdvancedOptions? = nil
    ) async throws -> Int64 {
        let noProgress: (@Sendable (ArchiveProgress) -> Void)? = nil
        return try await extract(
            archivePath: archivePath,
            destinationDir: destinationDir,
            options: options,
            password: password,
            advancedOptions: advancedOptions,
            progressHandler: noProgress,
            token: nil
        )
    }

    /// Convenience facade method to synchronously extract an archive without progress handler.
    @inline(__always)
    @discardableResult
    public func extractSync(
        archivePath: String,
        destinationDir: String,
        options: ArchiveFilterOptions = .defaultClean,
        password: String? = nil,
        advancedOptions: ArchiveAdvancedOptions? = nil
    ) throws -> Int64 {
        let noProgress: (@Sendable (ArchiveProgress) -> Void)? = nil
        return try extractSync(
            archivePath: archivePath,
            destinationDir: destinationDir,
            options: options,
            password: password,
            advancedOptions: advancedOptions,
            progressHandler: noProgress,
            token: nil
        )
    }
}

/// Archive data integrity and checksum verification interface.
public protocol ArchiveIntegrityChecking: Sendable {
    func computeCRC32(filePath: String) -> String
    func computeSHA256(filePath: String) async throws -> String
    @discardableResult
    func verifyExtractedDirectory(
        directoryPath: String,
        expectedOriginalBytes: Int64,
        sourceFilePath: String?,
        sourceCRC32: String?,
        label: String
    ) -> (isValid: Bool, totalExtractedBytes: Int64, crc32: String?)
}

/// Cryptographic hash calculation interface.
public protocol HashCalculating: Sendable {
    func computeHashSync(filePath: String, type: HashType) throws -> String
    func computeHash(filePath: String, type: HashType) async throws -> String
}


// MARK: - Progress

//
//


/// Archive operation type classification.
public enum ArchiveOperationType: String, Sendable, Equatable, CaseIterable {
    case compress = "Compress"
    case extract = "Extract"
    case repair = "Repair"
    case batch = "Batch"
    case recover = "PasswordRecovery"
    case inspect = "Inspect"
}

/// Real-time progress and telemetry metadata for archiving operations.
public struct ArchiveProgress: Sendable {
    /// Progress lifecycle states.
    public enum State: Sendable, Equatable {
        case idle
        case processing
        case completed
        case cancelled
        case failed(error: String)
    }
    
    /// Current execution state.
    public let state: State
    /// Number of bytes processed so far.
    public let bytesProcessed: Int64
    /// Total byte size of expected workload.
    public let totalBytes: Int64
    /// Name or path of file currently being compressed or extracted.
    public let currentFileName: String
    /// Monotonic throughput calculation in MB/s.
    public let throughputMBs: Double
    
    /// Normalized fraction completed (0.0 to 1.0).
    public var fractionCompleted: Double {
        guard totalBytes > 0 else { return 0.0 }
        return min(1.0, max(0.0, Double(bytesProcessed) / Double(totalBytes)))
    }
    
    public init(
        state: State = .idle,
        bytesProcessed: Int64 = 0,
        totalBytes: Int64 = 0,
        currentFileName: String = "",
        throughputMBs: Double = 0.0
    ) {
        self.state = state
        self.bytesProcessed = max(0, bytesProcessed)
        self.totalBytes = max(0, totalBytes)
        self.currentFileName = currentFileName
        self.throughputMBs = (throughputMBs.isNaN || throughputMBs.isInfinite || throughputMBs < 0) ? 0.0 : throughputMBs
    }
    
    public static let zero = ArchiveProgress()
}

/// Detailed progress data packet delivered to archive progress observers.
public struct ArchiveProgressInfo: Sendable, Equatable {
    public let state: ArchiveProgress.State
    public let bytesProcessed: Int64
    public let totalBytes: Int64
    public let currentFileName: String
    public let throughputMBs: Double
    public let estimatedTimeRemaining: TimeInterval?
    public let operationType: ArchiveOperationType
    
    public var fractionCompleted: Double {
        guard totalBytes > 0 else { return 0.0 }
        return min(1.0, max(0.0, Double(bytesProcessed) / Double(totalBytes)))
    }
    
    public init(
        state: ArchiveProgress.State = .processing,
        bytesProcessed: Int64 = 0,
        totalBytes: Int64 = 0,
        currentFileName: String = "",
        throughputMBs: Double = 0.0,
        estimatedTimeRemaining: TimeInterval? = nil,
        operationType: ArchiveOperationType = .compress
    ) {
        self.state = state
        self.bytesProcessed = max(0, bytesProcessed)
        self.totalBytes = max(0, totalBytes)
        self.currentFileName = currentFileName
        self.throughputMBs = (throughputMBs.isNaN || throughputMBs.isInfinite || throughputMBs < 0) ? 0.0 : throughputMBs
        if let eta = estimatedTimeRemaining, !eta.isNaN, !eta.isInfinite, eta >= 0 {
            self.estimatedTimeRemaining = eta
        } else {
            self.estimatedTimeRemaining = nil
        }
        self.operationType = operationType
    }
}

/// Progress data packet delivered to multi-file and batch task observers.
public struct BatchProgressInfo: Sendable, Equatable {
    public let completedTasks: Int
    public let totalTasks: Int
    public let currentTaskPath: String
    public let totalBytesProcessed: Int64
    public let totalBytesCount: Int64
    public let throughputMBs: Double
    public let estimatedTimeRemaining: TimeInterval?
    
    public var fractionCompleted: Double {
        guard totalTasks > 0 else { return 0.0 }
        return min(1.0, max(0.0, Double(completedTasks) / Double(totalTasks)))
    }
    
    public init(
        completedTasks: Int,
        totalTasks: Int,
        currentTaskPath: String = "",
        totalBytesProcessed: Int64 = 0,
        totalBytesCount: Int64 = 0,
        throughputMBs: Double = 0.0,
        estimatedTimeRemaining: TimeInterval? = nil
    ) {
        self.completedTasks = max(0, completedTasks)
        self.totalTasks = max(0, totalTasks)
        self.currentTaskPath = currentTaskPath
        self.totalBytesProcessed = max(0, totalBytesProcessed)
        self.totalBytesCount = max(0, totalBytesCount)
        self.throughputMBs = (throughputMBs.isNaN || throughputMBs.isInfinite || throughputMBs < 0) ? 0.0 : throughputMBs
        if let eta = estimatedTimeRemaining, !eta.isNaN, !eta.isInfinite, eta >= 0 {
            self.estimatedTimeRemaining = eta
        } else {
            self.estimatedTimeRemaining = nil
        }
    }
}

// MARK: - Factory

/// Unified factory providing standard writers, extractors, and readers.
public enum ArchiveEngineFactory {
    
    /// Creates an archive writer.
    public static func makeWriter(for format: ArchiveCompressionFormat? = nil) -> ArchiveWriting {
        return ArchiveWriter()
    }
    
    /// Creates an archive extractor.
    public static func makeExtractor(for format: ArchiveCompressionFormat? = nil) -> ArchiveExtracting {
        return ArchiveExtractor()
    }
    
    /// Creates an archive reader.
    public static func makeReader(for format: ArchiveCompressionFormat? = nil) -> ArchiveReading {
        return ArchiveReader()
    }

    /// Creates an integrity checker engine instance.
    public static func makeIntegrityChecker() -> ArchiveIntegrityChecking {
        return ArchiveIntegrityChecker()
    }

    /// Creates a cryptographic hash calculator instance.
    public static func makeHashCalculator(hardwareTuner: HardwareTunerProtocol? = nil) -> HashCalculating {
        return HashCalculator(hardwareTuner: hardwareTuner ?? AppleSiliconTuner.shared)
    }
}

// MARK: - Conformances

//
//


// MARK: - ArchiveReading Default Implementations & Extensions

extension ArchiveReading {
    public func inspect(archivePath: String, password: String?) async throws -> [ArchiveEntry] {
        return try await inspect(archivePath: archivePath, password: password, candidatePasswords: nil)
    }
    
    public func inspectTree(archivePath: String, password: String? = nil, candidatePasswords: [String]? = nil) async throws -> ArchiveCompositeDirectory {
        let entries = try await inspect(archivePath: archivePath, password: password, candidatePasswords: candidatePasswords)
        return ArchiveComponentTreeBuilder.buildTree(from: entries)
    }
    
    public func probeEncryption(archivePath: String) async throws -> ArchiveEncryptionTier {
        do {
            let entries = try await inspect(archivePath: archivePath, password: nil, candidatePasswords: nil)
            if entries.isEmpty {
                return .none
            }
            let hasEncrypted = entries.contains { $0.isEncrypted }
            return hasEncrypted ? .dataOnly : .none
        } catch let error as ArchiveError {
            switch error {
            case .passwordRequired, .passwordRequiredDetailed(_, .headerAndData):
                return .headerAndData
            case .passwordRequiredDetailed(_, .dataOnly):
                return .dataOnly
            default:
                throw error
            }
        } catch {
            throw error
        }
    }
}

// MARK: - ArchiveWriteRequest

/// Encapsulates parameters for archive creation requests across async and sync writing operations.
public struct ArchiveWriteRequest: Sendable {
    public var outputPath: String
    public var format: ArchiveCompressionFormat
    public var level: ArchiveCompressionLevel
    public var inputPaths: [String]
    public var options: ArchiveFilterOptions
    public var splitVolumeSizeBytes: Int64?
    public var password: String?
    public var advancedOptions: ArchiveAdvancedOptions
    public var progressHandler: (@Sendable (ArchiveProgress) -> Void)?

    public init(
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        inputPaths: [String] = [],
        options: ArchiveFilterOptions = .defaultClean,
        splitVolumeSizeBytes: Int64? = nil,
        password: String? = nil,
        advancedOptions: ArchiveAdvancedOptions = .defaultOptions,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) {
        self.outputPath = outputPath
        self.format = format
        self.level = level
        self.inputPaths = inputPaths
        self.options = options
        self.splitVolumeSizeBytes = splitVolumeSizeBytes
        self.password = password
        self.advancedOptions = advancedOptions
        self.progressHandler = progressHandler
    }

    public init(
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        components: [ArchiveComponentProtocol],
        options: ArchiveFilterOptions = .defaultClean,
        splitVolumeSizeBytes: Int64? = nil,
        password: String? = nil,
        advancedOptions: ArchiveAdvancedOptions = .defaultOptions,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) {
        self.init(
            outputPath: outputPath,
            format: format,
            level: level,
            inputPaths: components.map { $0.path },
            options: options,
            splitVolumeSizeBytes: splitVolumeSizeBytes,
            password: password,
            advancedOptions: advancedOptions,
            progressHandler: progressHandler
        )
    }
}

// MARK: - ArchiveWriting Default Implementations & Extensions

extension ArchiveWriting {
    public func createArchive(
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        inputPaths: [String],
        options: ArchiveFilterOptions = .defaultClean,
        splitVolumeSizeBytes: Int64? = nil,
        password: String? = nil,
        advancedOptions: ArchiveAdvancedOptions = .defaultOptions,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) async throws {
        let request = ArchiveWriteRequest(
            outputPath: outputPath,
            format: format,
            level: level,
            inputPaths: inputPaths,
            options: options,
            splitVolumeSizeBytes: splitVolumeSizeBytes,
            password: password,
            advancedOptions: advancedOptions,
            progressHandler: progressHandler
        )
        try await createArchive(request)
    }

    public func createArchiveSync(
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        inputPaths: [String],
        options: ArchiveFilterOptions = .defaultClean,
        password: String? = nil,
        splitVolumeSizeBytes: Int64? = nil,
        advancedOptions: ArchiveAdvancedOptions = .defaultOptions,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) throws {
        let request = ArchiveWriteRequest(
            outputPath: outputPath,
            format: format,
            level: level,
            inputPaths: inputPaths,
            options: options,
            splitVolumeSizeBytes: splitVolumeSizeBytes,
            password: password,
            advancedOptions: advancedOptions,
            progressHandler: progressHandler
        )
        try createArchiveSync(request)
    }

    public func createArchive(
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        components: [ArchiveComponentProtocol],
        options: ArchiveFilterOptions = .defaultClean,
        splitVolumeSizeBytes: Int64? = nil,
        password: String? = nil,
        advancedOptions: ArchiveAdvancedOptions = .defaultOptions,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) async throws {
        let request = ArchiveWriteRequest(
            outputPath: outputPath,
            format: format,
            level: level,
            components: components,
            options: options,
            splitVolumeSizeBytes: splitVolumeSizeBytes,
            password: password,
            advancedOptions: advancedOptions,
            progressHandler: progressHandler
        )
        try await createArchive(request)
    }

    public func createArchiveSync(
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        components: [ArchiveComponentProtocol],
        options: ArchiveFilterOptions = .defaultClean,
        password: String? = nil,
        splitVolumeSizeBytes: Int64? = nil,
        advancedOptions: ArchiveAdvancedOptions = .defaultOptions,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) throws {
        let request = ArchiveWriteRequest(
            outputPath: outputPath,
            format: format,
            level: level,
            components: components,
            options: options,
            splitVolumeSizeBytes: splitVolumeSizeBytes,
            password: password,
            advancedOptions: advancedOptions,
            progressHandler: progressHandler
        )
        try createArchiveSync(request)
    }
}

// MARK: - ArchiveIntegrityChecking Default Implementations & Extensions

extension ArchiveIntegrityChecking {
    @discardableResult
    public func verifyExtractedDirectory(
        directoryPath: String,
        expectedOriginalBytes: Int64,
        sourceFilePath: String? = nil,
        sourceCRC32: String? = nil,
        label: String = "Verification"
    ) -> (isValid: Bool, totalExtractedBytes: Int64, crc32: String?) {
        return verifyExtractedDirectory(
            directoryPath: directoryPath,
            expectedOriginalBytes: expectedOriginalBytes,
            sourceFilePath: sourceFilePath,
            sourceCRC32: sourceCRC32,
            label: label
        )
    }
}

// MARK: - Compression Types

//
//


// MARK: - Archive Compression Types Module Gateway
//
// This file serves as the unified types aggregation and compatibility gateway.
// Submodule definitions are decomposed across:
// - `Types/ArchiveCompressionFormat.swift`: Archive formats, extensions, MIME resolution.
// - `Types/ArchiveCompressionOptions.swift`: Compression levels, format options, advanced configurations.
// - `Types/ArchiveEntryMetadata.swift`: Structured entry-level metadata models.

/// Common typealiases for compression and archive specifications.
public typealias CompressionFormat = ArchiveCompressionFormat
public typealias CompressionLevel = ArchiveCompressionLevel
public typealias CompressionOptions = ArchiveAdvancedOptions
public typealias EntryMetadata = ArchiveEntryMetadata
public typealias UniFFITransactionDiff = UniFfiTransactionDiff
public typealias UniFFISmartExtractDecision = UniFfiSmartExtractDecision

