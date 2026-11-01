// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

public enum ArchiveResidualPathRole: String, Sendable, Equatable, Hashable, Codable {
    case stagedArtifact
    case stagedTempFile
    case unownedPathOccupant
    case transactionStageDirectory
    case preexistingBackupDirectory
    case finalDestinationTarget
}

public enum ArchiveCleanupDisposition: String, Sendable, Equatable, Hashable, Codable {
    case refusedOwnershipMismatch
    case ownershipOrLocationUnconfirmed
    case restoredDisplacedSentinel
    case restoreDisplacedSentinelFailed
    case unlinkOrRmdirIoError
    case stageDirectoryPreservedDueToInnerFailure
    case postCommitCleanupFailed
    case descriptorReleaseUnconfirmed
}

public enum ArchiveDescriptorReleaseStatus: String, Sendable, Equatable, Hashable, Codable {
    case confirmedReleased
    case releaseUnconfirmed
}

public struct ArchiveStageFileFinishDiagnostic: Sendable, Equatable, Hashable, Codable {
    public let role: ArchiveResidualPathRole
    public let diagnosticPath: String
    public let expectedDev: UInt64
    public let expectedIno: UInt64
    public let primaryStatusCode: Int32
    public let priorWriteOrEncoderError: String?
    public let userspaceFlushOsError: Int32?
    public let userspaceFlushDetail: String?
    public let syncOsError: Int32?
    public let syncDetail: String?
    public let closeOsError: Int32?
    public let closeDetail: String?
    public let releaseStatus: ArchiveDescriptorReleaseStatus

    public init(
        role: ArchiveResidualPathRole,
        diagnosticPath: String,
        expectedDev: UInt64,
        expectedIno: UInt64,
        primaryStatusCode: Int32,
        priorWriteOrEncoderError: String? = nil,
        userspaceFlushOsError: Int32? = nil,
        userspaceFlushDetail: String? = nil,
        syncOsError: Int32? = nil,
        syncDetail: String? = nil,
        closeOsError: Int32? = nil,
        closeDetail: String? = nil,
        releaseStatus: ArchiveDescriptorReleaseStatus
    ) {
        self.role = role
        self.diagnosticPath = diagnosticPath
        self.expectedDev = expectedDev
        self.expectedIno = expectedIno
        self.primaryStatusCode = primaryStatusCode
        self.priorWriteOrEncoderError = priorWriteOrEncoderError
        self.userspaceFlushOsError = userspaceFlushOsError
        self.userspaceFlushDetail = userspaceFlushDetail
        self.syncOsError = syncOsError
        self.syncDetail = syncDetail
        self.closeOsError = closeOsError
        self.closeDetail = closeDetail
        self.releaseStatus = releaseStatus
    }
}

public struct ArchiveStageCleanupFailureItem: Sendable, Equatable, Hashable, Codable {
    public let role: ArchiveResidualPathRole
    public let disposition: ArchiveCleanupDisposition
    public let path: String?
    public let originalStagedPath: String?
    public let osErrorCode: Int32?
    public let message: String

    public init(
        role: ArchiveResidualPathRole,
        disposition: ArchiveCleanupDisposition,
        path: String? = nil,
        originalStagedPath: String? = nil,
        osErrorCode: Int32? = nil,
        message: String
    ) {
        self.role = role
        self.disposition = disposition
        self.path = path
        self.originalStagedPath = originalStagedPath
        self.osErrorCode = osErrorCode
        self.message = message
    }
}

public enum ArchiveCreateBackupRecoveryState: String, Sendable, Equatable, Hashable, Codable {
    case notApplicable
    case restoredAndVerified
    case rollbackIncompletePreservedInBackup
}

public struct ArchiveBackupRecoveryEntryFailure: Sendable, Equatable, Hashable, Codable {
    public let backupSourcePath: String
    public let targetPath: String
    public let osErrorCode: Int32?
    public let message: String

    public init(
        backupSourcePath: String,
        targetPath: String,
        osErrorCode: Int32? = nil,
        message: String
    ) {
        self.backupSourcePath = backupSourcePath
        self.targetPath = targetPath
        self.osErrorCode = osErrorCode
        self.message = message
    }
}

public struct ArchiveCreateTransactionFailureReport: Sendable, Equatable, Hashable, Codable {
    public let primaryStatusCode: Int32
    public let wasPrimaryCancelled: Bool
    public let isCancellationRequested: Bool
    public let stageDir: String
    public let stageDirPreserved: Bool
    public let unconfirmedReleaseCount: UInt32
    public let cleanupFailures: [ArchiveStageCleanupFailureItem]
    public let finishDiagnostics: [ArchiveStageFileFinishDiagnostic]
    public let recoveryState: ArchiveCreateBackupRecoveryState
    public let backupDir: String?
    public let restoredTargets: [String]
    public let rollbackFailedEntries: [ArchiveBackupRecoveryEntryFailure]
    public let backupDirCleanupError: String?
    public let summaryMessage: String

    public var hasUnconfirmedDescriptorRelease: Bool {
        unconfirmedReleaseCount > 0
    }

    public var hasCleanupOrRecoveryIssues: Bool {
        stageDirPreserved
            || unconfirmedReleaseCount > 0
            || !cleanupFailures.isEmpty
            || !finishDiagnostics.isEmpty
            || !rollbackFailedEntries.isEmpty
            || backupDirCleanupError != nil
            || recoveryState == .rollbackIncompletePreservedInBackup
    }

    public init(
        primaryStatusCode: Int32,
        wasPrimaryCancelled: Bool,
        isCancellationRequested: Bool,
        stageDir: String,
        stageDirPreserved: Bool,
        unconfirmedReleaseCount: UInt32 = 0,
        cleanupFailures: [ArchiveStageCleanupFailureItem] = [],
        finishDiagnostics: [ArchiveStageFileFinishDiagnostic] = [],
        recoveryState: ArchiveCreateBackupRecoveryState = .notApplicable,
        backupDir: String? = nil,
        restoredTargets: [String] = [],
        rollbackFailedEntries: [ArchiveBackupRecoveryEntryFailure] = [],
        backupDirCleanupError: String? = nil,
        summaryMessage: String
    ) {
        self.primaryStatusCode = primaryStatusCode
        self.wasPrimaryCancelled = wasPrimaryCancelled
        self.isCancellationRequested = isCancellationRequested
        self.stageDir = stageDir
        self.stageDirPreserved = stageDirPreserved
        self.unconfirmedReleaseCount = unconfirmedReleaseCount
        self.cleanupFailures = cleanupFailures
        self.finishDiagnostics = finishDiagnostics
        self.recoveryState = recoveryState
        self.backupDir = backupDir
        self.restoredTargets = restoredTargets
        self.rollbackFailedEntries = rollbackFailedEntries
        self.backupDirCleanupError = backupDirCleanupError
        self.summaryMessage = summaryMessage
    }
}

public enum ArchiveError: Error, LocalizedError, Equatable {
    case fileNotFound
    case readFailed(code: Int32)
    case invalidFormat
    case passwordRequired
    case passwordRequiredDetailed(archivePath: String, tier: ArchiveEncryptionTier)
    case wrongPassword(archivePath: String)
    case unsupportedEncryptionMethod(archivePath: String, method: String)
    case corruptedData(archivePath: String, entryPath: String)
    case cancelled
    case extractionStopped
    case partialExtractionFailure(outcome: ExtractionOutcome)
    case createTransactionIncomplete(ArchiveCreateTransactionFailureReport)
    case invalidState
    case engineFailure(code: Int32, message: String)
    case openFailed
    case corruptHeader
    
    public var errorDescription: String? {
        localizedDescription()
    }

    public var isPasswordRelated: Bool {
        switch self {
        case .passwordRequired, .passwordRequiredDetailed, .wrongPassword:
            return true
        default:
            return false
        }
    }
}
