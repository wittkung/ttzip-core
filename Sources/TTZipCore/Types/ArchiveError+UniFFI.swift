// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

extension ArchiveError {
    /// Maps a Rust Mozilla UniFFI `TtZipError` to its corresponding Swift `ArchiveError`.
    public static func from(uniffiError: TtZipError) -> ArchiveError {
        switch uniffiError {
        case .FileNotFound(_):
            return .fileNotFound
        case .InvalidPassword:
            return .passwordRequired
        case let .CorruptHeader(details, offset):
            return .corruptedData(archivePath: "offset_\(offset)", entryPath: details)
        case let .SecurityViolation(reason):
            return .engineFailure(code: -403, message: reason)
        case let .EngineError(code):
            switch code {
            case -2:
                return .fileNotFound
            case -7:
                return .passwordRequired
            case -9:
                return .corruptedData(archivePath: "code_\(code)", entryPath: "header")
            case -21:
                return .invalidFormat
            case -23:
                return .cancelled
            case -24:
                return .engineFailure(code: -403, message: "Security violation")
            default:
                return .readFailed(code: code)
            }
        case let .IoError(message):
            return .engineFailure(code: -500, message: message)
        case .Cancelled:
            return .cancelled
        }
    }

    /// Maps any generic Swift or UniFFI error to a strongly typed `ArchiveError`.
    public static func from(error: Error) -> ArchiveError {
        if let archiveError = error as? ArchiveError {
            return archiveError
        }
        if let uniffiError = error as? TtZipError {
            return from(uniffiError: uniffiError)
        }
        if error is CancellationError {
            return .cancelled
        }
        return .engineFailure(code: -1, message: error.localizedDescription)
    }
}

extension TtZipError {
    /// Converts this UniFFI error instance into the corresponding Swift `ArchiveError`.
    public func toArchiveError() -> ArchiveError {
        return ArchiveError.from(uniffiError: self)
    }
}
