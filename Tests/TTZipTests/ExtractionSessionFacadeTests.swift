// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import XCTest
@testable import TTZipCore

final class ExtractionSessionFacadeTests: XCTestCase {
    private var tempDir: URL!
    private var archivePath: String!
    private let testContent = "Facade Extraction Test Content: \(UUID().uuidString)"

    override func setUpWithError() throws {
        try super.setUpWithError()
        let fm = FileManager.default
        tempDir = fm.temporaryDirectory.appendingPathComponent("ttzip_facade_test_\(UUID().uuidString)")
        try fm.createDirectory(at: tempDir, withIntermediateDirectories: true)

        let sampleFile = tempDir.appendingPathComponent("sample.txt")
        try Data(testContent.utf8).write(to: sampleFile)

        archivePath = tempDir.appendingPathComponent("archive.zip").path
        _ = try createArchiveStream(
            sourcePaths: [sampleFile.path],
            outputPath: archivePath,
            format: ArchiveFormat.zip,
            level: 6,
            password: nil,
            progress: nil,
            token: nil
        )
    }

    override func tearDownWithError() throws {
        if let dir = tempDir {
            try? FileManager.default.removeItem(at: dir)
        }
        try super.tearDownWithError()
    }

    func testFacadeExtractPolicyKeepBoth() async throws {
        let destDir = tempDir.appendingPathComponent("dest_keep_both")
        try FileManager.default.createDirectory(at: destDir, withIntermediateDirectories: true)

        let existingFile = destDir.appendingPathComponent("sample.txt")
        try Data("Existing Content".utf8).write(to: existingFile)

        let request = ExtractionRequest(
            archivePath: archivePath,
            destinationRoot: destDir.path,
            password: nil,
            entryFilter: nil,
            rootAtCommonParent: false,
            policy: .keepBoth
        )

        let outcome = try await TTZipEngineFacade.shared.extract(
            request,
            handling: .policy(.keepBoth)
        )

        XCTAssertEqual(outcome.status, .completed)
        XCTAssertEqual(outcome.keptBoth, 1)

        let fm = FileManager.default
        XCTAssertTrue(fm.fileExists(atPath: existingFile.path))
        let keepBothFile = destDir.appendingPathComponent("sample 2.txt")
        XCTAssertTrue(fm.fileExists(atPath: keepBothFile.path))

        let keepBothContent = try String(contentsOf: keepBothFile, encoding: .utf8)
        XCTAssertEqual(keepBothContent, testContent)
    }

    func testFacadeExtractPolicyReplace() async throws {
        let destDir = tempDir.appendingPathComponent("dest_replace")
        try FileManager.default.createDirectory(at: destDir, withIntermediateDirectories: true)

        let targetFile = destDir.appendingPathComponent("sample.txt")
        try Data("OLD CONTENT".utf8).write(to: targetFile)

        let request = ExtractionRequest(
            archivePath: archivePath,
            destinationRoot: destDir.path,
            password: nil,
            entryFilter: nil,
            rootAtCommonParent: false,
            policy: .replace
        )

        let outcome = try await TTZipEngineFacade.shared.extract(
            request,
            handling: .policy(.replace)
        )

        XCTAssertEqual(outcome.status, .completed)
        XCTAssertEqual(outcome.replaced, 1)

        let currentContent = try String(contentsOf: targetFile, encoding: .utf8)
        XCTAssertEqual(currentContent, testContent)
    }

    func testFacadeExtractPolicySkip() async throws {
        let destDir = tempDir.appendingPathComponent("dest_skip")
        try FileManager.default.createDirectory(at: destDir, withIntermediateDirectories: true)

        let targetFile = destDir.appendingPathComponent("sample.txt")
        let originalContent = "ORIGINAL KEPT"
        try Data(originalContent.utf8).write(to: targetFile)

        let request = ExtractionRequest(
            archivePath: archivePath,
            destinationRoot: destDir.path,
            password: nil,
            entryFilter: nil,
            rootAtCommonParent: false,
            policy: .skip
        )

        let outcome = try await TTZipEngineFacade.shared.extract(
            request,
            handling: .policy(.skip)
        )

        XCTAssertEqual(outcome.status, .completed)
        XCTAssertEqual(outcome.skipped, 1)

        let currentContent = try String(contentsOf: targetFile, encoding: .utf8)
        XCTAssertEqual(currentContent, originalContent)
    }

    func testFacadeExtractPolicyStop() async throws {
        let destDir = tempDir.appendingPathComponent("dest_stop")
        try FileManager.default.createDirectory(at: destDir, withIntermediateDirectories: true)

        let targetFile = destDir.appendingPathComponent("sample.txt")
        let originalContent = "ORIGINAL IMMUTABLE"
        try Data(originalContent.utf8).write(to: targetFile)

        let request = ExtractionRequest(
            archivePath: archivePath,
            destinationRoot: destDir.path,
            password: nil,
            entryFilter: nil,
            rootAtCommonParent: false,
            policy: .stop
        )

        let outcome = try await TTZipEngineFacade.shared.extract(
            request,
            handling: .policy(.stop)
        )

        XCTAssertEqual(outcome.status, .stopped)
        let currentContent = try String(contentsOf: targetFile, encoding: .utf8)
        XCTAssertEqual(currentContent, originalContent)
    }

    func testFacadeExtractInteractiveProviderDecide() async throws {
        let destDir = tempDir.appendingPathComponent("dest_interactive")
        try FileManager.default.createDirectory(at: destDir, withIntermediateDirectories: true)

        let targetFile = destDir.appendingPathComponent("sample.txt")
        try Data("PREEXISTING".utf8).write(to: targetFile)

        final class MockProvider: CollisionDecisionProvider, @unchecked Sendable {
            var decideCalled = false

            @MainActor
            func decide(
                summary: ConflictSummary,
                fetch: @escaping @Sendable (UInt32, UInt32) async throws -> ConflictPage
            ) async -> ResolutionSet? {
                decideCalled = true
                XCTAssertGreaterThan(summary.total, 0)
                let page = try? await fetch(0, 10)
                XCTAssertNotNil(page)
                return ResolutionSet(defaultAction: .keepBoth, overrides: [])
            }
        }

        let provider = MockProvider()
        let request = ExtractionRequest(
            archivePath: archivePath,
            destinationRoot: destDir.path,
            password: nil,
            entryFilter: nil,
            rootAtCommonParent: false,
            policy: .ask
        )

        let outcome = try await TTZipEngineFacade.shared.extract(
            request,
            handling: .interactive(provider)
        )

        XCTAssertEqual(outcome.status, .completed)
        XCTAssertEqual(outcome.keptBoth, 1)
        XCTAssertTrue(provider.decideCalled)

        let keepBothFile = destDir.appendingPathComponent("sample 2.txt")
        XCTAssertTrue(FileManager.default.fileExists(atPath: keepBothFile.path))
    }

    func testQuickExtractPopulatesOutcomeOnSuccess() async throws {
        let destDir = tempDir.appendingPathComponent("dest_quick_success")
        try FileManager.default.createDirectory(at: destDir, withIntermediateDirectories: true)

        let result = try await TTZipEngineFacade.shared.quickExtract(
            archivePath: archivePath,
            destinationDir: destDir.path,
            handling: .policy(.keepBoth)
        )

        XCTAssertNotNil(result.outcome)
        XCTAssertEqual(result.outcome?.status, OutcomeStatus.completed)
        XCTAssertEqual(result.outcome?.extracted, 1)
    }

    func testQuickExtractThrowsExtractionStoppedWhenPolicyStopsOnConflict() async throws {
        let destDir = tempDir.appendingPathComponent("dest_quick_stop")
        try FileManager.default.createDirectory(at: destDir, withIntermediateDirectories: true)

        let conflictingFile = destDir.appendingPathComponent("sample.txt")
        let originalContent = "EXISTING_DO_NOT_OVERWRITE"
        try Data(originalContent.utf8).write(to: conflictingFile)

        do {
            _ = try await TTZipEngineFacade.shared.quickExtract(
                archivePath: archivePath,
                destinationDir: destDir.path,
                handling: .policy(.stop)
            )
            XCTFail("quickExtract must throw ArchiveError.extractionStopped when outcome is .stopped")
        } catch let error as ArchiveError {
            guard case .extractionStopped = error else {
                XCTFail("Expected ArchiveError.extractionStopped, got \(error)")
                return
            }
        }

        let currentContent = try String(contentsOf: conflictingFile, encoding: .utf8)
        XCTAssertEqual(currentContent, originalContent)
    }

    func testPartialExtractionFailureErrorCarriesDetailedOutcome() {
        let failedItem = FailedItem(
            relativePath: "locked/file.txt",
            reason: .permissionDenied,
            errno: EACCES
        )
        let outcome = ExtractionOutcome(
            status: .completedWithFailures,
            extracted: 3,
            replaced: 0,
            keptBoth: 0,
            skipped: 0,
            failed: [failedItem]
        )
        let error = ArchiveError.partialExtractionFailure(outcome: outcome)
        XCTAssertNotNil(error.errorDescription)
        if case .partialExtractionFailure(let captured) = error {
            XCTAssertEqual(captured.extracted, 3)
            XCTAssertEqual(captured.failed.count, 1)
            XCTAssertEqual(captured.failed.first?.relativePath, "locked/file.txt")
        } else {
            XCTFail("Expected partialExtractionFailure pattern match")
        }
    }

    func testCancelWhileWaitingForDecisionPreventsCommitAndPreservesExistingTarget() async throws {
        let fm = FileManager.default
        let destDir = tempDir.appendingPathComponent("dest_cancel_during_decide")
        try fm.createDirectory(at: destDir, withIntermediateDirectories: true)

        let existingFile = destDir.appendingPathComponent("sample.txt")
        let originalContent = "Original precious file before cancel"
        try originalContent.write(to: existingFile, atomically: true, encoding: .utf8)

        let externalToken = CancellationToken()
        let provider = ClosureDecisionProvider { _, _ in
            // Simulate user cancelling via external token while collision modal is awaiting decision,
            // followed by the modal returning a Replace resolution.
            externalToken.cancel()
            return ResolutionSet(
                defaultAction: .replace,
                overrides: []
            )
        }

        do {
            _ = try await TTZipEngineFacade.shared.quickExtract(
                archivePath: archivePath,
                destinationDir: destDir.path,
                handling: .interactive(provider),
                token: externalToken
            )
            XCTFail("quickExtract must throw cancelled when external token is cancelled during decide")
        } catch let error as ArchiveError {
            guard case .cancelled = error else {
                XCTFail("Expected ArchiveError.cancelled, got \(error)")
                return
            }
        }

        let postCancelContent = try String(contentsOf: existingFile, encoding: .utf8)
        XCTAssertEqual(
            postCancelContent,
            originalContent,
            "Commit must not execute when cancellation occurs before session.commit"
        )
    }

    func testQuickExtractRealPartialFailureThrowsPartialExtractionFailureThroughFacade() async throws {
        let fm = FileManager.default
        let srcRoot = tempDir.appendingPathComponent("partial_src")
        let srcLockedSub = srcRoot.appendingPathComponent("locked_sub")
        try fm.createDirectory(at: srcLockedSub, withIntermediateDirectories: true)

        let okSrc = srcRoot.appendingPathComponent("ok.txt")
        let blockedSrc = srcLockedSub.appendingPathComponent("blocked.txt")
        try "Sibling file that commits cleanly".write(to: okSrc, atomically: true, encoding: .utf8)
        try "File blocked by read-only parent dir during commit".write(
            to: blockedSrc,
            atomically: true,
            encoding: .utf8
        )

        let partialArchivePath = tempDir.appendingPathComponent("partial.zip").path
        _ = try createArchiveStream(
            sourcePaths: [okSrc.path, srcLockedSub.path],
            outputPath: partialArchivePath,
            format: ArchiveFormat.zip,
            level: 6,
            password: nil,
            progress: nil,
            token: nil
        )

        let destDir = tempDir.appendingPathComponent("dest_partial")
        let lockedSubdir = destDir.appendingPathComponent("locked_sub")
        try fm.createDirectory(at: lockedSubdir, withIntermediateDirectories: true)
        let existingProtected = lockedSubdir.appendingPathComponent("existing.txt")
        try "Protected content".write(to: existingProtected, atomically: true, encoding: .utf8)

        defer {
            try? fm.setAttributes([.posixPermissions: 0o755], ofItemAtPath: lockedSubdir.path)
        }
        // Lock `dest/locked_sub` (0o555) while keeping `dest` writable (0o755) so staging succeeds
        // at root and commit fails only inside `locked_sub` with EACCES.
        try fm.setAttributes([.posixPermissions: 0o555], ofItemAtPath: lockedSubdir.path)

        do {
            _ = try await TTZipEngineFacade.shared.quickExtract(
                archivePath: partialArchivePath,
                destinationDir: destDir.path,
                handling: .policy(.replace)
            )
            XCTFail("quickExtract must throw partialExtractionFailure when a commit item fails")
        } catch let error as ArchiveError {
            guard case .partialExtractionFailure(let outcome) = error else {
                XCTFail("Expected ArchiveError.partialExtractionFailure, got \(error)")
                return
            }
            XCTAssertEqual(outcome.status, OutcomeStatus.completedWithFailures)
            XCTAssertGreaterThanOrEqual(outcome.extracted, 1)
            XCTAssertFalse(outcome.failed.isEmpty, "Failed items must be surfaced in outcome.failed")
        }

        XCTAssertEqual(
            try String(contentsOf: destDir.appendingPathComponent("ok.txt"), encoding: .utf8),
            "Sibling file that commits cleanly"
        )
        XCTAssertEqual(
            try String(contentsOf: existingProtected, encoding: .utf8),
            "Protected content"
        )
    }
}

private final class ClosureDecisionProvider: CollisionDecisionProvider, @unchecked Sendable {
    private let handler: (
        ConflictSummary,
        @escaping @Sendable (UInt32, UInt32) async throws -> ConflictPage
    ) async -> ResolutionSet?

    init(
        handler: @escaping (
            ConflictSummary,
            @escaping @Sendable (UInt32, UInt32) async throws -> ConflictPage
        ) async -> ResolutionSet?
    ) {
        self.handler = handler
    }

    @MainActor
    func decide(
        summary: ConflictSummary,
        fetch: @escaping @Sendable (UInt32, UInt32) async throws -> ConflictPage
    ) async -> ResolutionSet? {
        await handler(summary, fetch)
    }
}
