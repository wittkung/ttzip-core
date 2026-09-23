// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import XCTest
import Foundation
@testable import TTZipCore

final class UniFFIErrorMappingTests: XCTestCase {

    // MARK: - 1. Core UniFFI TtZipError Enum Cases Mapping

    func testUniFFIErrorEnumCasesMapping() {
        // 1. FileNotFound
        let errFileNotFound = TtZipError.FileNotFound(path: "/nonexistent")
        XCTAssertEqual(errFileNotFound.toArchiveError(), ArchiveError.fileNotFound)
        XCTAssertEqual(ArchiveError.from(uniffiError: errFileNotFound), ArchiveError.fileNotFound)

        // 2. InvalidPassword
        let errInvalidPassword = TtZipError.InvalidPassword
        XCTAssertEqual(errInvalidPassword.toArchiveError(), ArchiveError.passwordRequired)

        // 3. CorruptHeader
        let errCorruptHeader = TtZipError.CorruptHeader(details: "Invalid magic 0x1234", offset: 1024)
        let mappedCorrupt = errCorruptHeader.toArchiveError()
        if case let .corruptedData(archivePath, entryPath) = mappedCorrupt {
            XCTAssertEqual(archivePath, "offset_1024")
            XCTAssertEqual(entryPath, "Invalid magic 0x1234")
        } else {
            XCTFail("CorruptHeader must map to ArchiveError.corruptedData")
        }

        // 4. SecurityViolation
        let errSecViolation = TtZipError.SecurityViolation(reason: "Zip Slip path traversal attempt")
        let mappedSec = errSecViolation.toArchiveError()
        if case let .engineFailure(code, message) = mappedSec {
            XCTAssertEqual(code, -403)
            XCTAssertTrue(message.contains("Zip Slip"))
        } else {
            XCTFail("SecurityViolation must map to ArchiveError.engineFailure(code: -403)")
        }

        // 5. IoError
        let errIo = TtZipError.IoError(message: "Disk read timeout on sector 42")
        let mappedIo = errIo.toArchiveError()
        if case let .engineFailure(code, message) = mappedIo {
            XCTAssertEqual(code, -500)
            XCTAssertTrue(message.contains("Disk read timeout"))
        } else {
            XCTFail("IoError must map to ArchiveError.engineFailure(code: -500)")
        }

        // 6. Cancelled
        let errCancelled = TtZipError.Cancelled
        XCTAssertEqual(errCancelled.toArchiveError(), ArchiveError.cancelled)

        // 7. OutOfMemory
        let errOom = TtZipError.OutOfMemory(details: "Allocation failed")
        let mappedOom = errOom.toArchiveError()
        if case let .engineFailure(code, message) = mappedOom {
            XCTAssertEqual(code, -501)
            XCTAssertTrue(message.contains("Allocation failed"))
        } else {
            XCTFail("OutOfMemory must map to ArchiveError.engineFailure(code: -501)")
        }

        // 8. DecompressionBombDetected
        let errBomb = TtZipError.DecompressionBombDetected(reason: "Ratio too high")
        let mappedBomb = errBomb.toArchiveError()
        if case let .engineFailure(code, message) = mappedBomb {
            XCTAssertEqual(code, -403)
            XCTAssertTrue(message.contains("Ratio too high"))
        } else {
            XCTFail("DecompressionBombDetected must map to ArchiveError.engineFailure(code: -403)")
        }

        // 9. ChecksumMismatch
        let errCrc = TtZipError.ChecksumMismatch(expected: 0x12345678, actual: 0x87654321)
        let mappedCrc = errCrc.toArchiveError()
        if case let .corruptedData(archivePath, entryPath) = mappedCrc {
            XCTAssertEqual(archivePath, "checksum")
            XCTAssertTrue(entryPath.contains("0x12345678"))
        } else {
            XCTFail("ChecksumMismatch must map to ArchiveError.corruptedData")
        }
    }

    // MARK: - 2. Rust EngineError Status Code Mappings

    func testEngineErrorCodeMappings() {
        // Specific known code overrides
        XCTAssertEqual(TtZipError.EngineError(code: -2).toArchiveError(), ArchiveError.fileNotFound)
        XCTAssertEqual(TtZipError.EngineError(code: -4).toArchiveError(), ArchiveError.corruptHeader)
        XCTAssertEqual(TtZipError.EngineError(code: -7).toArchiveError(), ArchiveError.openFailed)
        XCTAssertEqual(TtZipError.EngineError(code: -10).toArchiveError(), ArchiveError.passwordRequired)
        XCTAssertEqual(TtZipError.EngineError(code: -21).toArchiveError(), ArchiveError.invalidFormat)
        XCTAssertEqual(TtZipError.EngineError(code: -23).toArchiveError(), ArchiveError.cancelled)

        // General status codes
        XCTAssertEqual(TtZipError.EngineError(code: -100).toArchiveError(), ArchiveError.readFailed(code: -100))
    }

    // MARK: - 3. Boundary & Extreme Value Safety

    func testBoundaryStatusCodeSafety() {
        let boundaryCodes: [Int32] = [
            Int32.min,
            -10000,
            -404,
            -1,
            0,
            1,
            404,
            10000,
            Int32.max
        ]

        for code in boundaryCodes {
            let mapped = TtZipError.EngineError(code: code).toArchiveError()
            XCTAssertNotNil(mapped, "Mapping status code \(code) must not crash or produce nil")
        }
    }

    // MARK: - 4. Swift Generic Error Bridging via ArchiveError.from(error:)

    func testGenericSwiftErrorBridging() {
        // Swift CancellationError
        let cancelError = CancellationError()
        XCTAssertEqual(ArchiveError.from(error: cancelError), ArchiveError.cancelled)

        // Native ArchiveError passthrough
        let existingArchiveError = ArchiveError.fileNotFound
        XCTAssertEqual(ArchiveError.from(error: existingArchiveError), ArchiveError.fileNotFound)

        // TtZipError passed as generic Error
        let uniffiError: Error = TtZipError.Cancelled
        XCTAssertEqual(ArchiveError.from(error: uniffiError), ArchiveError.cancelled)

        // Arbitrary Cocoa / POSIX NSError
        let customError = NSError(
            domain: "POSIXErrorDomain",
            code: 28,
            userInfo: [NSLocalizedDescriptionKey: "No space left on device"]
        )
        let mappedCustom = ArchiveError.from(error: customError)
        if case let .engineFailure(code, message) = mappedCustom {
            XCTAssertEqual(code, -1)
            XCTAssertTrue(message.contains("No space left on device"))
        } else {
            XCTFail("Arbitrary error must map to ArchiveError.engineFailure")
        }
    }
}
