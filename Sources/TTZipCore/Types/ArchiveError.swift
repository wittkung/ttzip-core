// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import TTZipCoreBindings
import Foundation
import TTZipOperationContract

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

    public static func invalidArgument(reason: String) -> ArchiveError {
        .engineFailure(code: -400, message: reason)
    }

    public static func unsupportedOperation(reason: String) -> ArchiveError {
        .engineFailure(code: -13, message: reason)
    }
}
