// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

public enum ArchiveEncryptionTier: String, Sendable, Codable, Equatable {
    case none = "NONE"
    /// Entry payloads are encrypted while headers remain cleartext for passwordless directory browsing.
    case dataOnly = "DATA_ONLY"
    /// Both headers and payloads are encrypted; listing entries requires the password upfront.
    case headerAndData = "HEADER_AND_DATA"
    case unsupported = "UNSUPPORTED"
}

public enum ArchiveOperationType: String, Sendable, Equatable, CaseIterable {
    case compress = "Compress"
    case extract = "Extract"
    case repair = "Repair"
    case batch = "Batch"
    case recover = "PasswordRecovery"
    case inspect = "Inspect"
}

public typealias ProgressSink = @Sendable (ArchiveProgress) -> Void

public struct ArchiveProgress: Sendable {
    public enum State: Sendable, Equatable {
        case idle
        case processing
        case completed
        case cancelled
        case failed(error: String)
    }

    public let state: State
    public let bytesProcessed: Int64
    public let totalBytes: Int64
    public let currentFileName: String
    public let throughputMBs: Double

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
        advancedOptions: ArchiveAdvancedOptions = ArchiveAdvancedOptions(cpuThreads: max(1, ProcessInfo.processInfo.activeProcessorCount)),
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
}

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
