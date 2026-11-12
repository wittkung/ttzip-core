// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import TTZipArchiveContract
import TTZipOperationContract

public protocol ArchiveReading: Sendable {
    func inspect(archivePath: String) async throws -> [ArchiveEntry]
    func inspect(archivePath: String, password: String?, candidatePasswords: [String]?) async throws -> [ArchiveEntry]
    func inspectTree(archivePath: String, password: String?, candidatePasswords: [String]?) async throws -> ArchiveCompositeDirectory
    func probeEncryption(archivePath: String) async throws -> ArchiveEncryptionTier
}

extension ArchiveReading {
    @inline(__always)
    public func listEntries(archivePath: String, password: String? = nil) async throws -> [ArchiveEntry] {
        try await inspect(archivePath: archivePath, password: password, candidatePasswords: nil)
    }
}

public protocol ArchiveWriting: Sendable {
    func createArchive(_ request: ArchiveWriteRequest) async throws
    func createArchiveSync(_ request: ArchiveWriteRequest) throws
}

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

public enum CollisionHandling: Sendable {
    case policy(CollisionPolicy)
    case interactive(CollisionDecisionProvider)
}

public protocol CollisionDecisionProvider: Sendable {
    /// Nil return signals Stop; policy-driven sessions never invoke this callback.
    @MainActor
    func decide(
        summary: ConflictSummary,
        fetch: @escaping @Sendable (UInt32, UInt32) async throws -> ConflictPage
    ) async -> ResolutionSet?
}

extension CollisionPolicy {
    public var defaultAction: CollisionAction? {
        switch self {
        case .keepBoth:
            return .keepBoth
        case .replace:
            return .replace
        case .skip:
            return .skip
        case .ask, .stop:
            return nil
        }
    }
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
        try await extract(
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
        try extractSync(
            archivePath: archivePath,
            destinationDir: destinationDir,
            options: options,
            password: password,
            advancedOptions: advancedOptions,
            progressHandler: progressHandler,
            token: nil
        )
    }

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
        try await extract(
            archivePath: archivePath,
            destinationDir: destinationDir,
            options: options,
            password: password,
            advancedOptions: advancedOptions,
            progressHandler: progressHandler,
            token: nil
        )
    }

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

public protocol HashCalculating: Sendable {
    func computeHashSync(filePath: String, type: HashType) throws -> String
    func computeHash(filePath: String, type: HashType) async throws -> String
}

public enum ArchiveEngineFactory {}

extension ArchiveReading {
    public func inspect(archivePath: String, password: String?) async throws -> [ArchiveEntry] {
        try await inspect(archivePath: archivePath, password: password, candidatePasswords: nil)
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

extension ArchiveWriteRequest {
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

extension ArchiveIntegrityChecking {
    @discardableResult
    public func verifyExtractedDirectory(
        directoryPath: String,
        expectedOriginalBytes: Int64,
        sourceFilePath: String? = nil,
        sourceCRC32: String? = nil,
        label: String = "Verification"
    ) -> (isValid: Bool, totalExtractedBytes: Int64, crc32: String?) {
        verifyExtractedDirectory(
            directoryPath: directoryPath,
            expectedOriginalBytes: expectedOriginalBytes,
            sourceFilePath: sourceFilePath,
            sourceCRC32: sourceCRC32,
            label: label
        )
    }
}

public typealias CompressionFormat = ArchiveCompressionFormat
public typealias CompressionLevel = ArchiveCompressionLevel
public typealias CompressionOptions = ArchiveAdvancedOptions
public typealias EntryMetadata = ArchiveEntryMetadata
public typealias UniFFITransactionDiff = UniFfiTransactionDiff
public typealias UniFFISmartExtractDecision = UniFfiSmartExtractDecision
