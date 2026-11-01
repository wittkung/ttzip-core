// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import XCTest
import Foundation
import os
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

    // MARK: - 5. U1 CreateTransactionIncomplete Mapping & Cancellation Cross-Check

    func testCreateTransactionIncompleteMappingAndWasPrimaryCancelledVerification() {
        let cleanupItem = UniFfiStageCleanupFailureItem(
            role: .stagedArtifact,
            disposition: .descriptorReleaseUnconfirmed,
            path: nil,
            originalStagedPath: "/tmp/.ttzip_create_txn_42/archive.7z",
            osErrorCode: Int32(EINTR),
            message: "close(2) returned EINTR (descriptor release unconfirmed)"
        )
        let finishDiag = UniFfiStageFileFinishDiagnostic(
            role: .stagedArtifact,
            diagnosticPath: "/tmp/.ttzip_create_txn_42/archive.7z",
            expectedDev: 16,
            expectedIno: 99,
            primaryStatusCode: RustTTZipStatusCode.cancelled.rawValue,
            priorWriteOrEncoderError: "Cancelled",
            userspaceFlushOsError: nil,
            userspaceFlushDetail: nil,
            syncOsError: nil,
            syncDetail: nil,
            closeOsError: Int32(EINTR),
            closeDetail: "Interrupted system call",
            releaseStatus: .releaseUnconfirmed
        )
        let rollbackFailure = UniFfiBackupRecoveryEntryFailure(
            backupSourcePath: "/tmp/.ttzip_create_bak_42/vol.7z.002",
            targetPath: "/tmp/vol.7z.002",
            osErrorCode: Int32(EACCES),
            message: "Permission denied"
        )
        let uniffiReport = UniFfiCreateTransactionFailureReport(
            primaryStatusCode: RustTTZipStatusCode.cancelled.rawValue,
            wasPrimaryCancelled: true,
            isCancellationRequested: true,
            stageDir: "/tmp/.ttzip_create_txn_42",
            stageDirPreserved: true,
            unconfirmedReleaseCount: 1,
            cleanupFailures: [cleanupItem],
            finishDiagnostics: [finishDiag],
            recoveryState: .rollbackIncompletePreservedInBackup,
            backupDir: "/tmp/.ttzip_create_bak_42",
            restoredTargets: ["/tmp/vol.7z.001"],
            rollbackFailedEntries: [rollbackFailure],
            backupDirCleanupError: "Backup preserved",
            summaryMessage: "Archive creation transaction incomplete"
        )

        let uniffiErr = TtZipError.CreateTransactionIncomplete(
            report: uniffiReport,
            summaryMessage: "Archive creation transaction incomplete"
        )
        let mapped = uniffiErr.toArchiveError()
        guard case let .createTransactionIncomplete(report) = mapped else {
            XCTFail("Expected .createTransactionIncomplete, got: \(mapped)")
            return
        }

        XCTAssertEqual(report.primaryStatusCode, RustTTZipStatusCode.cancelled.rawValue)
        XCTAssertTrue(report.wasPrimaryCancelled)
        XCTAssertTrue(report.isCancellationRequested)
        XCTAssertEqual(report.stageDir, "/tmp/.ttzip_create_txn_42")
        XCTAssertTrue(report.stageDirPreserved)
        XCTAssertEqual(report.unconfirmedReleaseCount, 1)
        XCTAssertTrue(report.hasUnconfirmedDescriptorRelease)
        XCTAssertTrue(report.hasCleanupOrRecoveryIssues)
        XCTAssertEqual(report.cleanupFailures.count, 1)
        XCTAssertEqual(report.cleanupFailures[0].role, .stagedArtifact)
        XCTAssertEqual(report.cleanupFailures[0].disposition, .descriptorReleaseUnconfirmed)
        XCTAssertEqual(report.cleanupFailures[0].osErrorCode, Int32(EINTR))
        XCTAssertEqual(report.finishDiagnostics.count, 1)
        XCTAssertEqual(report.finishDiagnostics[0].releaseStatus, .releaseUnconfirmed)
        XCTAssertEqual(report.finishDiagnostics[0].closeOsError, Int32(EINTR))
        XCTAssertEqual(report.recoveryState, .rollbackIncompletePreservedInBackup)
        XCTAssertEqual(report.backupDir, "/tmp/.ttzip_create_bak_42")
        XCTAssertEqual(report.restoredTargets, ["/tmp/vol.7z.001"])
        XCTAssertEqual(report.rollbackFailedEntries.count, 1)
        XCTAssertEqual(report.rollbackFailedEntries[0].targetPath, "/tmp/vol.7z.002")
        XCTAssertEqual(report.backupDirCleanupError, "Backup preserved")

        // Verify cross-check: if primaryStatusCode is NOT .cancelled (2), wasPrimaryCancelled must be false
        var nonCancelReport = uniffiReport
        nonCancelReport.primaryStatusCode = RustTTZipStatusCode.errCompressionFailed.rawValue
        nonCancelReport.wasPrimaryCancelled = true
        let mappedNonCancel = TtZipError.CreateTransactionIncomplete(
            report: nonCancelReport,
            summaryMessage: "Non-cancel primary"
        ).toArchiveError()
        if case let .createTransactionIncomplete(r2) = mappedNonCancel {
            XCTAssertFalse(r2.wasPrimaryCancelled, "wasPrimaryCancelled must require primaryStatusCode == .cancelled")
            XCTAssertTrue(r2.isCancellationRequested)
        } else {
            XCTFail("Expected .createTransactionIncomplete")
        }
    }

    func test_c2a_second_write_to_same_target_waits_for_cancelled_first_write_cleanup() async throws {
        let sandbox = try IsolatedTempSandbox(prefix: "c2a_same_target")
        defer { sandbox.cleanup() }
        let writer = ArchiveWriter()
        let timeline = EventTimeline()
        let firstStarted = XCTestExpectation(description: "First write started")
        let secondEnteredExecutor = XCTestExpectation(description: "Second write entered executor")
        secondEnteredExecutor.isInverted = true
        let firstCleanupGate = BlockingGate()
        defer { firstCleanupGate.open() }

        let inputFile = sandbox.fileURL(named: "c2a_input.txt")
        try "C2A payload".write(to: inputFile, atomically: true, encoding: .utf8)
        let targetZip = sandbox.fileURL(named: "c2a_same_target.zip")

        let invocationCounter = OSAllocatedUnfairLock(initialState: 0)
        writer._createArchiveExecutorOverride = { config, _, _, token in
            let callIndex = invocationCounter.withLock { count -> Int in
                count += 1
                return count
            }
            if callIndex == 1 {
                timeline.record("write1Started")
                firstStarted.fulfill()
                firstCleanupGate.wait()
                timeline.record("write1CleanupFinished")
                if token?.isCancelled() == true { throw TtZipError.Cancelled }
                return CompressionReport(
                    uncompressedBytes: 11, compressedBytes: 11, elapsedNanos: 1_000,
                    throughputMbs: 10.0, spaceSavingsPct: 0.0, engineProvenance: "c2a-test", outcome: nil
                )
            }
            timeline.record("write2Started")
            secondEnteredExecutor.fulfill()
            try Data("SECOND_ARCHIVE".utf8).write(to: URL(fileURLWithPath: config.outputPath))
            timeline.record("write2Finished")
            return CompressionReport(
                uncompressedBytes: 14, compressedBytes: 14, elapsedNanos: 1_000,
                throughputMbs: 10.0, spaceSavingsPct: 0.0, engineProvenance: "c2a-test", outcome: nil
            )
        }

        let token1 = CancellationToken()
        let task1 = Task {
            try await writer.createArchive(
                outputPath: targetZip.path, format: .zip, inputPaths: [inputFile.path], token: token1
            )
        }
        await fulfillment(of: [firstStarted], timeout: 5.0)

        token1.cancel()
        task1.cancel()
        let task2 = Task {
            try await writer.createArchive(
                outputPath: targetZip.path, format: .zip, inputPaths: [inputFile.path]
            )
        }

        await fulfillment(of: [secondEnteredExecutor], timeout: 0.08)

        firstCleanupGate.open()
        do {
            try await task1.value
            XCTFail("First write must report ArchiveError.cancelled")
        } catch let error as ArchiveError {
            XCTAssertEqual(error, .cancelled)
        }
        try await task2.value

        XCTAssertEqual(
            timeline.events,
            ["write1Started", "write1CleanupFinished", "write2Started", "write2Finished"]
        )
        XCTAssertEqual(try Data(contentsOf: targetZip), Data("SECOND_ARCHIVE".utf8))
    }

    func test_c2a_completion_cancellation_race_preserves_late_commit_and_cleanup_io_error() async throws {
        let sandbox = try IsolatedTempSandbox(prefix: "c2a_race")
        defer { sandbox.cleanup() }
        let writer = ArchiveWriter()
        let inputFile = sandbox.fileURL(named: "c2a_race_input.txt")
        try "C2A race payload".write(to: inputFile, atomically: true, encoding: .utf8)

        let lateCommitZip = sandbox.fileURL(named: "c2a_late_commit.zip")
        let lateToken = CancellationToken()
        writer._createArchiveExecutorOverride = { config, _, _, _ in
            try Data("COMMITTED_ARCHIVE".utf8).write(to: URL(fileURLWithPath: config.outputPath))
            lateToken.cancel()
            return CompressionReport(
                uncompressedBytes: 17, compressedBytes: 17, elapsedNanos: 1_000,
                throughputMbs: 10.0, spaceSavingsPct: 0.0, engineProvenance: "c2a-test", outcome: nil
            )
        }
        try await writer.createArchive(
            outputPath: lateCommitZip.path, format: .zip, inputPaths: [inputFile.path], token: lateToken
        )
        XCTAssertEqual(try Data(contentsOf: lateCommitZip), Data("COMMITTED_ARCHIVE".utf8))

        let cleanupFailZip = sandbox.fileURL(named: "c2a_cleanup_fail.zip")
        let failToken = CancellationToken()
        writer._createArchiveExecutorOverride = { _, _, _, _ in
            failToken.cancel()
            throw TtZipError.IoError(message: "RollbackIncomplete: failed to remove staging directory /tmp/txn_01 (EACCES)")
        }
        do {
            try await writer.createArchive(
                outputPath: cleanupFailZip.path, format: .zip, inputPaths: [inputFile.path], token: failToken
            )
            XCTFail("Cleanup failure must not succeed")
        } catch let error as ArchiveError {
            guard case let .engineFailure(code, message) = error else {
                XCTFail("Expected .engineFailure(-500) preserving cleanup IoError, got \(error)")
                return
            }
            XCTAssertEqual(code, -500)
            XCTAssertTrue(message.contains("RollbackIncomplete"))
        }
    }

    // MARK: - C2-B4: Real Swift -> UniFFI -> Rust ArchiveWriter Cancellation & Post-Commit Preservation

    func test_c2b_real_ffi_archive_writer_cancellation_and_post_commit_preservation() async throws {
        let sandbox = try IsolatedTempSandbox(prefix: "c2b_real_ffi")
        defer { sandbox.cleanup() }
        let writer = ArchiveWriter()
        let srcDir = sandbox.fileURL(named: "c2b_real_ffi_src")
        try FileManager.default.createDirectory(at: srcDir, withIntermediateDirectories: true)
        for i in 0..<8 {
            let fileURL = srcDir.appendingPathComponent("part_\(i).txt")
            try String(repeating: "Payload_\(i)_", count: 2048)
                .write(to: fileURL, atomically: true, encoding: .utf8)
        }

        let outDir = sandbox.fileURL(named: "c2b_real_ffi_out")
        try FileManager.default.createDirectory(at: outDir, withIntermediateDirectories: true)
        let targetZip = outDir.appendingPathComponent("real_target.zip")
        let sentinelFile = outDir.appendingPathComponent("sentinel.txt")
        try Data("SENTINEL_INTACT".utf8).write(to: sentinelFile)

        let commitToken = CancellationToken()
        try await writer.createArchive(
            outputPath: targetZip.path, format: .zip, inputPaths: [srcDir.path], token: commitToken
        )
        commitToken.cancel()
        let committedBytes = try Data(contentsOf: targetZip)
        XCTAssertGreaterThan(committedBytes.count, 0)

        let cancelToken = CancellationToken()
        do {
            try await writer.createArchive(
                outputPath: targetZip.path,
                format: .zip,
                inputPaths: [srcDir.path],
                progressHandler: { _ in cancelToken.cancel() },
                token: cancelToken
            )
            XCTFail("Expected ArchiveError.cancelled from real FFI createArchive")
        } catch let error as ArchiveError {
            XCTAssertEqual(error, .cancelled)
        }

        XCTAssertEqual(try Data(contentsOf: targetZip), committedBytes)
        XCTAssertEqual(try Data(contentsOf: sentinelFile), Data("SENTINEL_INTACT".utf8))
        let outEntries = try FileManager.default.contentsOfDirectory(atPath: outDir.path)
        XCTAssertFalse(
            outEntries.contains(where: { $0.hasPrefix(".ttzip_create_txn_") }),
            "No lingering .ttzip_create_txn_ directory allowed after real FFI cancellation: \(outEntries)"
        )
    }
}

private final class BlockingGate: @unchecked Sendable {
    private let semaphore = DispatchSemaphore(value: 0)
    private let state = OSAllocatedUnfairLock(initialState: false)

    func wait() {
        if !state.withLock({ $0 }) { semaphore.wait() }
    }

    func open() {
        let shouldSignal = state.withLock { isOpen -> Bool in
            guard !isOpen else { return false }
            isOpen = true
            return true
        }
        if shouldSignal {
            for _ in 0..<16 { semaphore.signal() }
        }
    }
}

private final class EventTimeline: @unchecked Sendable {
    private let lock = OSAllocatedUnfairLock(initialState: [String]())

    func record(_ event: String) {
        lock.withLock { $0.append(event) }
    }

    var events: [String] {
        lock.withLock { $0 }
    }
}
