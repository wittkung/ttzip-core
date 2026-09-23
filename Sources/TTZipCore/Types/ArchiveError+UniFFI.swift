// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

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
            case RustTTZipStatusCode.errFileNotFound.rawValue:
                return .fileNotFound
            case RustTTZipStatusCode.errCorruptHeader.rawValue:
                return .corruptHeader
            case RustTTZipStatusCode.errOpenFailed.rawValue:
                return .openFailed
            case RustTTZipStatusCode.errInvalidPassword.rawValue:
                return .passwordRequired
            case RustTTZipStatusCode.errOutOfMemory.rawValue:
                return .engineFailure(code: -501, message: "Out of memory: memory allocation failed in microkernel")
            case -21:
                return .invalidFormat
            case -23:
                return .cancelled
            case RustTTZipStatusCode.errSolidBudgetExceeded.rawValue,
                 RustTTZipStatusCode.errSecurityViolation.rawValue:
                return .engineFailure(code: -403, message: "Security violation")
            default:
                return .readFailed(code: code)
            }
        case let .IoError(message):
            return .engineFailure(code: -500, message: message)
        case .Cancelled:
            return .cancelled
        case let .OutOfMemory(details):
            return .engineFailure(code: -501, message: "Out of memory: \(details)")
        case let .DecompressionBombDetected(reason):
            return .engineFailure(code: -403, message: "Decompression bomb detected: \(reason)")
        case let .ChecksumMismatch(expected, actual):
            return .corruptedData(
                archivePath: "checksum",
                entryPath: String(format: "Expected 0x%08x, got 0x%08x", expected, actual)
            )
        }
    }
}

/// Strongly typed status codes exported by Rust `ttzip-engine` FFI ABI.
public enum RustTTZipStatusCode: Int32, Sendable {
    case ok = 0
    case eof = 1
    case cancelled = 2
    case errInvalidParam = -1
    case errFileNotFound = -2
    case errMmapFailed = -3
    case errCorruptHeader = -4
    case errInvalidOffset = -5
    case errArchiveInitFailed = -6
    case errOpenFailed = -7
    case errPathTooLong = -8
    case errOutOfMemory = -9
    case errInvalidPassword = -10
    case errExtractionFailed = -11
    case errCompressionFailed = -12
    case errUnsupportedFeature = -13
    case errSolidBudgetExceeded = -24
    case errSecurityViolation = -30
    case errPanicCaught = -99
}

extension ArchiveError {
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
