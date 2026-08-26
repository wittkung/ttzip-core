// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// Primary Swift 6 Actor-based unified archiving engine for macOS.
///
/// Provides thread-safe, isolated coordination for compression, extraction,
/// inspection, and streaming telemetry.
public actor TTZipEngine {
    public static let shared = TTZipEngine()

    private let facade: TTZipEngineFacade
    private let reader: ArchiveReading
    private let writer: ArchiveWriting
    private let extractor: ArchiveExtracting

    public init(
        facade: TTZipEngineFacade = .shared,
        reader: ArchiveReading = ArchiveEngineFactory.makeReader(),
        writer: ArchiveWriting = ArchiveEngineFactory.makeWriter(),
        extractor: ArchiveExtracting = ArchiveEngineFactory.makeExtractor()
    ) {
        self.facade = facade
        self.reader = reader
        self.writer = writer
        self.extractor = extractor
    }

    /// Opens an existing archive file and returns a `TTZipArchive` model.
    public func open(at path: String, password: String? = nil) async throws -> TTZipArchive {
        guard !path.isEmpty, FileManager.default.fileExists(atPath: path) else {
            throw ArchiveError.fileNotFound
        }
        let ext = (path as NSString).pathExtension
        let format = ArchiveCompressionFormat.from(extensionOrName: ext) ?? .zip
        let tier = try await reader.probeEncryption(archivePath: path)
        return TTZipArchive(
            path: path,
            format: format,
            encryptionTier: tier,
            defaultPassword: password
        )
    }

    /// Compresses input items into an archive with an asynchronous progress stream.
    public func compress(
        inputs: [String],
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        password: String? = nil,
        options: ArchiveFilterOptions = .defaultClean,
        splitVolumeSizeBytes: Int64? = nil,
        advancedOptions: ArchiveAdvancedOptions = .defaultOptions
    ) -> (progress: AsyncStream<ArchiveProgress>, task: Task<ArchiveOperationResult, Error>) {
        let (stream, continuation) = AsyncStream.makeStream(of: ArchiveProgress.self)
        let token = CancellationToken()

        let task = Task<ArchiveOperationResult, Error> {
            defer { continuation.finish() }
            if Task.isCancelled || token.isCancelled() {
                continuation.yield(ArchiveProgress(state: .cancelled))
                throw CancellationError()
            }
            return try await withTaskCancellationHandler {
                do {
                    let res = try await self.facade.quickCompress(
                        inputs: inputs,
                        outputPath: outputPath,
                        format: format,
                        level: level,
                        password: password,
                        splitSize: splitVolumeSizeBytes,
                        filterOptions: options,
                        advancedOptions: advancedOptions,
                        progress: { progress in
                            continuation.yield(progress)
                        },
                        token: token
                    )
                    return res
                } catch {
                    if Task.isCancelled || token.isCancelled() {
                        continuation.yield(ArchiveProgress(state: .cancelled))
                        throw CancellationError()
                    }
                    throw error
                }
            } onCancel: {
                token.cancel()
                continuation.yield(ArchiveProgress(state: .cancelled))
                continuation.finish()
            }
        }

        return (stream, task)
    }

    /// Direct async compression of items without explicit stream handling.
    public func compressDirect(
        inputs: [String],
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        password: String? = nil,
        options: ArchiveFilterOptions = .defaultClean,
        splitVolumeSizeBytes: Int64? = nil,
        advancedOptions: ArchiveAdvancedOptions = .defaultOptions,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) async throws -> ArchiveOperationResult {
        return try await facade.quickCompress(
            inputs: inputs,
            outputPath: outputPath,
            format: format,
            level: level,
            password: password,
            splitSize: splitVolumeSizeBytes,
            filterOptions: options,
            advancedOptions: advancedOptions,
            progress: progressHandler
        )
    }

    /// Extracts an archive to a destination directory with an asynchronous progress stream.
    public func extract(
        archivePath: String,
        destinationDir: String,
        password: String? = nil,
        options: ArchiveFilterOptions = .defaultClean,
        autoVaultUnlock: Bool = true
    ) -> (progress: AsyncStream<ArchiveProgress>, task: Task<ExtractResult, Error>) {
        let (stream, continuation) = AsyncStream.makeStream(of: ArchiveProgress.self)
        let token = CancellationToken()

        let task = Task<ExtractResult, Error> {
            defer { continuation.finish() }
            if Task.isCancelled || token.isCancelled() {
                continuation.yield(ArchiveProgress(state: .cancelled))
                throw CancellationError()
            }
            return try await withTaskCancellationHandler {
                do {
                    let res = try await self.facade.quickExtract(
                        archivePath: archivePath,
                        destinationDir: destinationDir,
                        password: password,
                        autoVaultUnlock: autoVaultUnlock,
                        progress: { progress in
                            continuation.yield(progress)
                        },
                        token: token
                    )
                    return res
                } catch {
                    if Task.isCancelled || token.isCancelled() {
                        continuation.yield(ArchiveProgress(state: .cancelled))
                        throw CancellationError()
                    }
                    throw error
                }
            } onCancel: {
                token.cancel()
                continuation.yield(ArchiveProgress(state: .cancelled))
                continuation.finish()
            }
        }

        return (stream, task)
    }

    /// Extracts an archive directly to a destination directory.
    public func extractDirect(
        archivePath: String,
        destinationDir: String,
        password: String? = nil,
        autoVaultUnlock: Bool = true,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) async throws -> ExtractResult {
        return try await facade.quickExtract(
            archivePath: archivePath,
            destinationDir: destinationDir,
            password: password,
            autoVaultUnlock: autoVaultUnlock,
            progress: progressHandler
        )
    }

    /// Inspects archive contents and returns a flat list of entries.
    public func inspect(archivePath: String, password: String? = nil) async throws -> [ArchiveEntry] {
        return try await reader.inspect(archivePath: archivePath, password: password, candidatePasswords: nil)
    }

    /// Inspects archive hierarchy and returns a composite directory tree.
    public func inspectTree(archivePath: String, password: String? = nil) async throws -> ArchiveCompositeDirectory {
        return try await reader.inspectTree(archivePath: archivePath, password: password, candidatePasswords: nil)
    }

    /// Verifies the cryptographic hash and integrity of an archive.
    public func verifyIntegrity(archivePath: String) async throws -> HashVerificationResult {
        return try await facade.verifyIntegrity(archivePath: archivePath)
    }

    /// Repairs a damaged archive file.
    public func repairArchive(damagedPath: String, outputPath: String) async throws -> Int {
        return try await facade.repairArchive(damagedPath: damagedPath, outputPath: outputPath)
    }

    /// Performs dictionary-based password recovery against an encrypted archive.
    public func recoverPassword(archivePath: String, dictionary: [String]) async throws -> PasswordRecoveryResult {
        return try await facade.recoverPassword(archivePath: archivePath, dictionary: dictionary)
    }
}

/// Represents an opened archive instance offering ergonomic queries, extraction, and VFS sessions.
public struct TTZipArchive: Sendable, Identifiable {
    public var id: String { path }
    public let path: String
    public let format: ArchiveCompressionFormat
    public let encryptionTier: ArchiveEncryptionTier
    public let defaultPassword: String?

    public init(
        path: String,
        format: ArchiveCompressionFormat,
        encryptionTier: ArchiveEncryptionTier = .none,
        defaultPassword: String? = nil
    ) {
        self.path = path
        self.format = format
        self.encryptionTier = encryptionTier
        self.defaultPassword = defaultPassword
    }

    /// Discovers all entries contained in the archive.
    public func entries(password: String? = nil) async throws -> [ArchiveEntry] {
        let effectivePassword = password ?? defaultPassword
        let reader = ArchiveEngineFactory.makeReader()
        return try await reader.inspect(archivePath: path, password: effectivePassword, candidatePasswords: nil)
    }

    /// Builds a hierarchical directory tree of archive entries.
    public func tree(password: String? = nil) async throws -> ArchiveCompositeDirectory {
        let effectivePassword = password ?? defaultPassword
        let reader = ArchiveEngineFactory.makeReader()
        return try await reader.inspectTree(archivePath: path, password: effectivePassword, candidatePasswords: nil)
    }

    /// Extracts all archive contents to target directory with an `AsyncStream<ArchiveProgress>`.
    public func extract(
        to destinationDir: String,
        password: String? = nil,
        autoVaultUnlock: Bool = true
    ) -> (progress: AsyncStream<ArchiveProgress>, task: Task<ExtractResult, Error>) {
        let effectivePassword = password ?? defaultPassword
        let (stream, continuation) = AsyncStream.makeStream(of: ArchiveProgress.self)
        let token = CancellationToken()

        let task = Task<ExtractResult, Error> {
            defer { continuation.finish() }
            if Task.isCancelled || token.isCancelled() {
                continuation.yield(ArchiveProgress(state: .cancelled))
                throw CancellationError()
            }
            return try await withTaskCancellationHandler {
                do {
                    let res = try await TTZipEngineFacade.shared.quickExtract(
                        archivePath: self.path,
                        destinationDir: destinationDir,
                        password: effectivePassword,
                        autoVaultUnlock: autoVaultUnlock,
                        progress: { progress in
                            continuation.yield(progress)
                        },
                        token: token
                    )
                    return res
                } catch {
                    if Task.isCancelled || token.isCancelled() {
                        continuation.yield(ArchiveProgress(state: .cancelled))
                        throw CancellationError()
                    }
                    throw error
                }
            } onCancel: {
                token.cancel()
                continuation.yield(ArchiveProgress(state: .cancelled))
                continuation.finish()
            }
        }

        return (stream, task)
    }

    /// Extracts all archive contents directly without managing an async stream.
    public func extractDirect(
        to destinationDir: String,
        password: String? = nil,
        autoVaultUnlock: Bool = true,
        progressHandler: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) async throws -> ExtractResult {
        let effectivePassword = password ?? defaultPassword
        return try await TTZipEngineFacade.shared.quickExtract(
            archivePath: self.path,
            destinationDir: destinationDir,
            password: effectivePassword,
            autoVaultUnlock: autoVaultUnlock,
            progress: progressHandler
        )
    }

    /// Extracts a single entry from the archive.
    public func extractEntry(
        _ entryPath: String,
        to destinationDir: String,
        password: String? = nil
    ) async throws {
        let effectivePassword = password ?? defaultPassword
        try await TTZipEngineFacade.shared.extractSingleEntry(
            archivePath: path,
            entryPath: entryPath,
            destinationDir: destinationDir,
            password: effectivePassword
        )
    }

    /// Verifies cryptographic integrity of this archive.
    public func verifyIntegrity() async throws -> HashVerificationResult {
        return try await TTZipEngineFacade.shared.verifyIntegrity(archivePath: path)
    }

    /// Spawns a high-performance persistent Rust VFS session for instant search and pagination.
    public func openVfsSession(password: String? = nil) async throws -> RustVfsSession? {
        let effectivePassword = password ?? defaultPassword
        let entriesList = try await self.entries(password: effectivePassword)
        return RustVfsSession(entries: entriesList, rootName: (path as NSString).lastPathComponent)
    }
}
