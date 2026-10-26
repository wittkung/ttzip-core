// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// High-level result payload for archive extraction operations.
public struct ExtractResult: Sendable, Equatable {
    public let archivePath: String
    public let destinationDir: String
    public let durationSeconds: Double
    public let unlockedPassword: String?
    public let isVaultUnlocked: Bool
    public let outcome: ExtractionOutcome?

    public var status: OutcomeStatus {
        outcome?.status ?? .completed
    }
    
    public init(
        archivePath: String,
        destinationDir: String,
        durationSeconds: Double,
        unlockedPassword: String? = nil,
        isVaultUnlocked: Bool = false,
        outcome: ExtractionOutcome? = nil
    ) {
        self.archivePath = archivePath
        self.destinationDir = destinationDir
        self.durationSeconds = durationSeconds
        self.unlockedPassword = unlockedPassword
        self.isVaultUnlocked = isVaultUnlocked
        self.outcome = outcome
    }
}

/// High-level result payload for archive inspection and structural analysis.
public struct ArchiveInspectionResult: Sendable {
    public let archivePath: String
    public let entries: [ArchiveEntry]
    public let treeNode: ArchiveCompositeDirectory
    public let securityReport: SecurityReport
    public let unlockedPassword: String?
    
    public init(
        archivePath: String,
        entries: [ArchiveEntry],
        treeNode: ArchiveCompositeDirectory,
        securityReport: SecurityReport,
        unlockedPassword: String? = nil
    ) {
        self.archivePath = archivePath
        self.entries = entries
        self.treeNode = treeNode
        self.securityReport = securityReport
        self.unlockedPassword = unlockedPassword
    }
}

/// Cryptographic hash verification result payload.
public struct HashVerificationResult: Sendable, Equatable {
    public let filePath: String
    public let crc32: String
    public let sha256: String
    
    public init(filePath: String, crc32: String, sha256: String) {
        self.filePath = filePath
        self.crc32 = crc32
        self.sha256 = sha256
    }
}

/// Unified high-level engine facade protocol.
public protocol TTZipEngineFacading: Sendable {
    func quickCompress(
        inputs: [String],
        outputPath: String,
        format: ArchiveCompressionFormat,
        level: ArchiveCompressionLevel,
        password: String?,
        splitSize: Int64?,
        filterOptions: ArchiveFilterOptions,
        advancedOptions: ArchiveAdvancedOptions?,
        progress: (@Sendable (ArchiveProgress) -> Void)?
    ) async throws -> ArchiveOperationResult

    func quickCompress(
        inputs: [String],
        outputPath: String,
        format: ArchiveCompressionFormat,
        level: ArchiveCompressionLevel,
        password: String?,
        splitSize: Int64?,
        filterOptions: ArchiveFilterOptions,
        advancedOptions: ArchiveAdvancedOptions?,
        progress: (@Sendable (ArchiveProgress) -> Void)?,
        token: CancellationToken?
    ) async throws -> ArchiveOperationResult
    
    func quickExtract(
        archivePath: String,
        destinationDir: String,
        password: String?,
        autoVaultUnlock: Bool,
        progress: (@Sendable (ArchiveProgress) -> Void)?
    ) async throws -> ExtractResult

    func quickExtract(
        archivePath: String,
        destinationDir: String,
        password: String?,
        autoVaultUnlock: Bool,
        progress: (@Sendable (ArchiveProgress) -> Void)?,
        token: CancellationToken?
    ) async throws -> ExtractResult

    func quickExtract(
        archivePath: String,
        destinationDir: String,
        password: String?,
        autoVaultUnlock: Bool,
        handling: CollisionHandling,
        progress: (@Sendable (ArchiveProgress) -> Void)?,
        token: CancellationToken?
    ) async throws -> ExtractResult
    
    @discardableResult
    func extractSingleEntry(
        archivePath: String,
        entryPath: String,
        destinationDir: String,
        password: String?,
        handling: CollisionHandling
    ) async throws -> ExtractionOutcome
    
    /// Inspects an archive and reports the credential that worked.
    ///
    /// The inspection is a query: it may read the shared password cache to pick a candidate
    /// and it keeps the vault retry policy, but it must not publish, restore or remove a
    /// shared credential. Committing to the shared cache is an authorization act owned by the
    /// caller that accepts the result.
    func inspectArchive(
        archivePath: String,
        password: String?,
        autoVaultUnlock: Bool
    ) async throws -> ArchiveInspectionResult
    
    func verifyIntegrity(archivePath: String) async throws -> HashVerificationResult
    func repairArchive(damagedPath: String, outputPath: String) async throws -> Int
    func recoverPassword(archivePath: String, dictionary: [String]) async throws -> PasswordRecoveryResult
    
    // MARK: - Command Pattern: Execution and Undo/Redo Control
    /// Legacy raw snapshot API. Success is an owning `OpaquePointer` erased as `Any`;
    /// its caller must transfer or release the one reference through the snapshot C-ABI.
    /// Missing symbols are a bridge error; a null open is `ArchiveError.openFailed`,
    /// without a detailed file/password/format diagnosis from the legacy ABI.
    func openVfsSnapshot(archivePath: String, password: String?) async throws -> Any

    var historyManager: CommandHistoryManager { get }
    var canUndoCommand: Bool { get async }
    var canRedoCommand: Bool { get async }
    func executeCommand(_ command: ArchiveCommandProtocol) async throws -> CommandResult
    func undoCommand() async throws -> CommandResult?
    func redoCommand() async throws -> CommandResult?
    func undoLastCommand() async throws -> CommandResult?
    func redoLastCommand() async throws -> CommandResult?
}

extension TTZipEngineFacading {
    public func quickCompress(
        inputs: [String],
        outputPath: String,
        format: ArchiveCompressionFormat,
        level: ArchiveCompressionLevel,
        password: String?,
        splitSize: Int64?,
        filterOptions: ArchiveFilterOptions,
        advancedOptions: ArchiveAdvancedOptions?,
        progress: (@Sendable (ArchiveProgress) -> Void)?
    ) async throws -> ArchiveOperationResult {
        return try await quickCompress(
            inputs: inputs,
            outputPath: outputPath,
            format: format,
            level: level,
            password: password,
            splitSize: splitSize,
            filterOptions: filterOptions,
            advancedOptions: advancedOptions,
            progress: progress,
            token: nil
        )
    }

    public func quickExtract(
        archivePath: String,
        destinationDir: String,
        password: String?,
        autoVaultUnlock: Bool,
        progress: (@Sendable (ArchiveProgress) -> Void)?
    ) async throws -> ExtractResult {
        return try await quickExtract(
            archivePath: archivePath,
            destinationDir: destinationDir,
            password: password,
            autoVaultUnlock: autoVaultUnlock,
            handling: .policy(.keepBoth),
            progress: progress,
            token: nil
        )
    }

    public func quickExtract(
        archivePath: String,
        destinationDir: String,
        password: String?,
        autoVaultUnlock: Bool,
        progress: (@Sendable (ArchiveProgress) -> Void)?,
        token: CancellationToken?
    ) async throws -> ExtractResult {
        return try await quickExtract(
            archivePath: archivePath,
            destinationDir: destinationDir,
            password: password,
            autoVaultUnlock: autoVaultUnlock,
            handling: .policy(.keepBoth),
            progress: progress,
            token: token
        )
    }

    public func quickExtract(
        archivePath: String,
        destinationDir: String,
        password: String?,
        autoVaultUnlock: Bool,
        handling: CollisionHandling,
        progress: (@Sendable (ArchiveProgress) -> Void)?
    ) async throws -> ExtractResult {
        return try await quickExtract(
            archivePath: archivePath,
            destinationDir: destinationDir,
            password: password,
            autoVaultUnlock: autoVaultUnlock,
            handling: handling,
            progress: progress,
            token: nil
        )
    }

    public func quickCompress(
        inputs: [String],
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        password: String? = nil,
        splitSize: Int64? = nil,
        progress: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) async throws -> ArchiveOperationResult {
        return try await quickCompress(
            inputs: inputs,
            outputPath: outputPath,
            format: format,
            level: level,
            password: password,
            splitSize: splitSize,
            filterOptions: .defaultClean,
            advancedOptions: nil,
            progress: progress
        )
    }
    
    public func quickExtract(
        archivePath: String,
        destinationDir: String,
        password: String? = nil,
        progress: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) async throws -> ExtractResult {
        return try await quickExtract(
            archivePath: archivePath,
            destinationDir: destinationDir,
            password: password,
            autoVaultUnlock: true,
            progress: progress
        )
    }
    
    public func inspectArchive(
        archivePath: String,
        password: String? = nil
    ) async throws -> ArchiveInspectionResult {
        return try await inspectArchive(
            archivePath: archivePath,
            password: password,
            autoVaultUnlock: true
        )
    }
    
    public var historyManager: CommandHistoryManager {
        return CommandHistoryManager.shared
    }
    
    /// Initializes native microkernel subsystems.
    public static func initializeSubsystems() {
        // Subsystems initialized
    }
    
    public var canUndoCommand: Bool {
        get async {
            await historyManager.canUndo
        }
    }
    
    public var canRedoCommand: Bool {
        get async {
            await historyManager.canRedo
        }
    }
    
    public func executeCommand(_ command: ArchiveCommandProtocol) async throws -> CommandResult {
        return try await historyManager.execute(command: command)
    }
    
    public func undoCommand() async throws -> CommandResult? {
        return try await historyManager.undo()
    }
    
    public func redoCommand() async throws -> CommandResult? {
        return try await historyManager.redo()
    }
    
    public func undoLastCommand() async throws -> CommandResult? {
        return try await undoCommand()
    }
    
    public func redoLastCommand() async throws -> CommandResult? {
        return try await redoCommand()
    }
    
    public func recoverPassword(archivePath: String, dictionary: [String]) async throws -> PasswordRecoveryResult {
        let engine = PasswordRecoveryEngine()
        return try await engine.recoverPassword(archivePath: archivePath, dictionary: dictionary)
    }
}
