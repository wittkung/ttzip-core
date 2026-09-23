// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import XCTest
import Foundation
@testable import TTZipCore

final class InPlaceArchiveEditTests: XCTestCase {
    private var tempDir: URL!
    
    override func setUpWithError() throws {
        try super.setUpWithError()
        tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("ttzip_inplace_edit_test_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }
    
    override func tearDownWithError() throws {
        InPlaceArchiveMutationEngine.shared.closeAllSessions()
        if let tempDir = tempDir {
            try? FileManager.default.removeItem(at: tempDir)
        }
        try super.tearDownWithError()
    }
    
    // MARK: - Lifecycle and RAII Rollback Tests
    
    /// Verifies that beginEditingSession cleans up temporary staging directory on extraction failure.
    func testBeginEditingSessionCleanupOnExtractionFailure() async throws {
        let zipPath = tempDir.appendingPathComponent("empty_test.zip").path
        let file1 = tempDir.appendingPathComponent("initial.txt")
        try "Initial Payload".write(to: file1, atomically: true, encoding: .utf8)
        
        let writer = ArchiveWriter()
        try await writer.createArchive(outputPath: zipPath, format: .zip, inputPaths: [file1.path])
        
        // Snapshot temporary directories before invocation
        let tempRoot = FileManager.default.temporaryDirectory
        let preDirs = (try? FileManager.default.contentsOfDirectory(atPath: tempRoot.path)) ?? []
        
        // Attempt to begin editing a non-existent entry in the archive
        do {
            _ = try await InPlaceArchiveMutationEngine.shared.beginEditingSession(
                archivePath: zipPath,
                entryPath: "non_existent_file.txt"
            )
            XCTFail("Should throw fileNotFound error for non-existent entry")
        } catch {
            // Expected error caught
            XCTAssertNotNil(error)
        }
        
        // Verify no orphaned TTZip_Edit_* directories are left behind
        let postDirs = (try? FileManager.default.contentsOfDirectory(atPath: tempRoot.path)) ?? []
        let newEditDirs = postDirs.filter { $0.hasPrefix("TTZip_Edit_") && !preDirs.contains($0) }
        XCTAssertTrue(newEditDirs.isEmpty, "Failed session must not leave orphaned staging directories")
    }
    
    /// Verifies that beginEditingSession successfully stages files and closeEditingSession cleans up completely.
    func testBeginEditingSessionSuccessAndCloseSessionCleanup() async throws {
        let zipPath = tempDir.appendingPathComponent("edit_lifecycle.zip").path
        let file1 = tempDir.appendingPathComponent("note.txt")
        let testContent = "Note content for live editing."
        try testContent.write(to: file1, atomically: true, encoding: .utf8)
        
        let writer = ArchiveWriter()
        try await writer.createArchive(outputPath: zipPath, format: .zip, inputPaths: [file1.path])
        
        let engine = InPlaceArchiveMutationEngine.shared
        let session = try await engine.beginEditingSession(
            archivePath: zipPath,
            entryPath: "note.txt"
        )
        
        XCTAssertEqual(session.state, .active)
        XCTAssertEqual(session.entryPath, "note.txt")
        XCTAssertTrue(FileManager.default.fileExists(atPath: session.stagedFilePath))
        XCTAssertTrue(FileManager.default.fileExists(atPath: session.stagedDirectoryPath))
        
        let stagedContent = try String(contentsOfFile: session.stagedFilePath, encoding: .utf8)
        XCTAssertEqual(stagedContent, testContent)
        
        // Retrieve session via sessionId
        let fetched = engine.getSession(sessionId: session.sessionId)
        XCTAssertNotNil(fetched)
        XCTAssertEqual(fetched?.sessionId, session.sessionId)
        
        // Close session
        engine.closeEditingSession(session: session)
        
        XCTAssertNil(engine.getSession(sessionId: session.sessionId))
        XCTAssertFalse(
            FileManager.default.fileExists(atPath: session.stagedDirectoryPath),
            "Staged directory must be physically removed after close"
        )
    }
    
    /// Verifies that discardEditingSession discards staging and frees resources.
    func testDiscardEditingSession() async throws {
        let zipPath = tempDir.appendingPathComponent("discard_test.zip").path
        let file1 = tempDir.appendingPathComponent("discard_me.txt")
        try "Discardable Data".write(to: file1, atomically: true, encoding: .utf8)
        
        let writer = ArchiveWriter()
        try await writer.createArchive(outputPath: zipPath, format: .zip, inputPaths: [file1.path])
        
        let engine = InPlaceArchiveMutationEngine.shared
        let session = try await engine.beginEditingSession(
            archivePath: zipPath,
            entryPath: "discard_me.txt"
        )
        
        let stagedDir = session.stagedDirectoryPath
        XCTAssertTrue(FileManager.default.fileExists(atPath: stagedDir))
        
        engine.discardEditingSession(sessionId: session.sessionId)
        
        XCTAssertNil(engine.getSession(sessionId: session.sessionId))
        XCTAssertFalse(FileManager.default.fileExists(atPath: stagedDir))
    }
    
    // MARK: - Mutation Diff Inspection and Direct Commit Tests
    
    /// Verifies diff inspection and committing live changes directly back into the archive.
    func testInspectSessionMutationAndCommitChangesDirectly() async throws {
        let zipPath = tempDir.appendingPathComponent("mutation_commit.zip").path
        let file1 = tempDir.appendingPathComponent("target.txt")
        let initialText = "Version 1.0 Initial Text"
        try initialText.write(to: file1, atomically: true, encoding: .utf8)
        
        let writer = ArchiveWriter()
        try await writer.createArchive(outputPath: zipPath, format: .zip, inputPaths: [file1.path])
        
        let engine = InPlaceArchiveMutationEngine.shared
        let session = try await engine.beginEditingSession(
            archivePath: zipPath,
            entryPath: "target.txt"
        )
        
        // 1. Initial inspection: unchanged
        let initialDiff = try await engine.inspectSessionMutationAsync(session: session)
        XCTAssertFalse(initialDiff.hasChanged, "Diff should be false when file is unmodified")
        
        // 2. Modify staged file externally
        let updatedText = "Version 2.0 Modified Content with New Lines\nAppended Data."
        try updatedText.write(toFile: session.stagedFilePath, atomically: true, encoding: .utf8)
        
        // 3. Inspection after edit: changed
        let updatedDiff = try await engine.inspectSessionMutationAsync(session: session)
        XCTAssertTrue(updatedDiff.hasChanged, "Diff should be true after external write")
        
        // 4. Commit changes directly into archive
        let committed = try await engine.commitChangesDirectly(session: session)
        XCTAssertTrue(committed, "commitChangesDirectly should return true on modifications")
        
        // 5. Verify the updated content inside the archive
        let extractedData = try await ArchiveSelectiveExtractor.shared.extractSingleEntryData(
            archivePath: zipPath,
            entryPath: "target.txt"
        )
        XCTAssertNotNil(extractedData)
        if let extractedData = extractedData {
            let extractedString = String(data: extractedData, encoding: .utf8)
            XCTAssertEqual(extractedString, updatedText)
        }
        
        engine.closeEditingSession(session: session)
    }
    
    // MARK: - Entry Rename Tests
    
    /// Verifies synchronous in-place entry renaming with automatic temporary directory cleanup.
    func testRenameEntrySyncAtomic() async throws {
        let zipPath = tempDir.appendingPathComponent("rename_sync.zip").path
        let file1 = tempDir.appendingPathComponent("document.txt")
        try "Original Document Text".write(to: file1, atomically: true, encoding: .utf8)
        
        let writer = ArchiveWriter()
        _ = try writer.createArchiveWithReport(
            outputPath: zipPath,
            format: .zip,
            inputPaths: [file1.path]
        )
        
        let engine = InPlaceArchiveMutationEngine.shared
        
        // Rename document.txt -> document_v2.txt
        try engine.renameEntrySync(
            archivePath: zipPath,
            oldEntryPath: "document.txt",
            newEntryPath: "document_v2.txt"
        )
        
        let reader = ArchiveReader()
        let entries = try await reader.listEntries(archivePath: zipPath)
        let entryNames = Set(entries.map(\.path))
        
        XCTAssertTrue(entryNames.contains("document_v2.txt"), "New entry name must exist in archive")
        XCTAssertFalse(entryNames.contains("document.txt"), "Old entry name must be deleted")
        
        // Rename with identical name must be a no-op
        try engine.renameEntrySync(
            archivePath: zipPath,
            oldEntryPath: "document_v2.txt",
            newEntryPath: "document_v2.txt"
        )
    }
    
    /// Verifies asynchronous in-place entry renaming.
    func testRenameEntryAsync() async throws {
        let zipPath = tempDir.appendingPathComponent("rename_async.zip").path
        let file1 = tempDir.appendingPathComponent("photo.png")
        let dummyData = Data([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
        try dummyData.write(to: file1)
        
        let writer = ArchiveWriter()
        try await writer.createArchive(outputPath: zipPath, format: .zip, inputPaths: [file1.path])
        
        let engine = InPlaceArchiveMutationEngine.shared
        
        try await engine.renameEntryAsync(
            archivePath: zipPath,
            oldEntryPath: "photo.png",
            newEntryPath: "images/photo_renamed.png"
        )
        
        let reader = ArchiveReader()
        let entries = try await reader.listEntries(archivePath: zipPath)
        let entryNames = Set(entries.map(\.path))
        
        XCTAssertTrue(entryNames.contains("images/photo_renamed.png"))
        XCTAssertFalse(entryNames.contains("photo.png"))
    }
    
    /// Verifies that active editing sessions have their entryPath updated when a rename occurs.
    func testRenameEntryUpdatesActiveSession() async throws {
        let zipPath = tempDir.appendingPathComponent("rename_session.zip").path
        let file1 = tempDir.appendingPathComponent("editable.md")
        try "# Markdown Document".write(to: file1, atomically: true, encoding: .utf8)
        
        let writer = ArchiveWriter()
        try await writer.createArchive(outputPath: zipPath, format: .zip, inputPaths: [file1.path])
        
        let engine = InPlaceArchiveMutationEngine.shared
        let session = try await engine.beginEditingSession(
            archivePath: zipPath,
            entryPath: "editable.md"
        )
        
        XCTAssertEqual(session.entryPath, "editable.md")
        
        // Perform rename
        try engine.renameEntrySync(
            archivePath: zipPath,
            oldEntryPath: "editable.md",
            newEntryPath: "renamed_editable.md"
        )
        
        let updatedSession = engine.getSession(sessionId: session.sessionId)
        XCTAssertEqual(updatedSession?.entryPath, "renamed_editable.md", "Session entryPath must be updated")
        
        engine.closeEditingSession(session: session)
    }
    
    // MARK: - Batch Session Cleanup Tests
    
    /// Verifies closeAllSessions closes and physically unlinks all staging directories.
    func testCloseAllSessionsCleansUpAllDirectories() async throws {
        let zipPath = tempDir.appendingPathComponent("batch_sessions.zip").path
        let file1 = tempDir.appendingPathComponent("f1.txt")
        let file2 = tempDir.appendingPathComponent("f2.txt")
        try "F1".write(to: file1, atomically: true, encoding: .utf8)
        try "F2".write(to: file2, atomically: true, encoding: .utf8)
        
        let writer = ArchiveWriter()
        try await writer.createArchive(outputPath: zipPath, format: .zip, inputPaths: [file1.path, file2.path])
        
        let engine = InPlaceArchiveMutationEngine.shared
        let s1 = try await engine.beginEditingSession(archivePath: zipPath, entryPath: "f1.txt")
        let s2 = try await engine.beginEditingSession(archivePath: zipPath, entryPath: "f2.txt")
        
        XCTAssertEqual(engine.allActiveSessions().count, 2)
        XCTAssertTrue(FileManager.default.fileExists(atPath: s1.stagedDirectoryPath))
        XCTAssertTrue(FileManager.default.fileExists(atPath: s2.stagedDirectoryPath))
        
        engine.closeAllSessions()
        
        XCTAssertEqual(engine.allActiveSessions().count, 0)
        XCTAssertFalse(FileManager.default.fileExists(atPath: s1.stagedDirectoryPath))
        XCTAssertFalse(FileManager.default.fileExists(atPath: s2.stagedDirectoryPath))
    }
}
