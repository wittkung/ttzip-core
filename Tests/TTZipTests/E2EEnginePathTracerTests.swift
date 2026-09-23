// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import XCTest
@testable import TTZipCore

final class E2EEnginePathTracerTests: XCTestCase {
    private var tempDir: URL!

    override func setUpWithError() throws {
        try super.setUpWithError()
        tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("ttzip_tracer_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: tempDir)
        try super.tearDownWithError()
    }

    /// Verifies that ZIP parallel compression executes pure Rust streaming engine and returns provenance.
    func testZipCompressionStrictEngineAssertion() throws {
        let sampleFile = tempDir.appendingPathComponent("payload.bin")
        let outputZip = tempDir.appendingPathComponent("output.zip")
        let data = Data(repeating: 0x41, count: 1024 * 1024) // 1MB
        try data.write(to: sampleFile)

        let writer = ArchiveWriter()
        let report = try writer.createArchiveWithReport(
            outputPath: outputZip.path,
            format: .zip,
            level: .normal,
            inputPaths: [sampleFile.path]
        )

        TTZipAssertions.assertEngineExecution(report, expected: .rustStreamingParallelZip)
        TTZipAssertions.assertNoFallback(report)
        XCTAssertGreaterThan(report.uncompressedBytes, 0)
        XCTAssertGreaterThan(report.compressedBytes, 0)
        print("✓ ZIP Compression Strict Provenance Verified: \(report.engineTag.rawValue), E2E Duration: \(report.totalE2EDurationNanos / 1_000_000)ms")
    }

    /// Verifies that in-place archive mutation returns non-forgeable provenance.
    func testInPlaceArchiveMutationEngineAssertion() throws {
        let originalZip = tempDir.appendingPathComponent("source.zip")
        let file1 = tempDir.appendingPathComponent("file1.txt")
        let file2 = tempDir.appendingPathComponent("file2.txt")
        try "Content 1".write(to: file1, atomically: true, encoding: .utf8)
        try "Content 2".write(to: file2, atomically: true, encoding: .utf8)

        let writer = ArchiveWriter()
        _ = try writer.createArchiveWithReport(
            outputPath: originalZip.path,
            format: .zip,
            inputPaths: [file1.path]
        )

        let (_, provenance) = try EngineProvenanceCollector.capture(expectedEngine: .rustInPlaceZip) {
            try InPlaceArchiveMutationEngine.shared.addFilesToArchiveSync(
                archivePath: originalZip.path,
                sourceFilePaths: [file2.path]
            )
        }

        TTZipAssertions.assertEngineExecution(provenance, expected: .rustInPlaceZip)
        TTZipAssertions.assertNoFallback(provenance)
        print("✓ In-Place Mutation Provenance Verified: \(provenance.engineTag.rawValue)")
    }

    /// Verifies that async mutation methods execute on background thread without blocking caller actor.
    func testInPlaceArchiveMutationAsyncNonBlocking() async throws {
        let originalZip = tempDir.appendingPathComponent("async_source.zip")
        let file1 = tempDir.appendingPathComponent("doc1.txt")
        let file2 = tempDir.appendingPathComponent("doc2.txt")
        let file3 = tempDir.appendingPathComponent("doc3.txt")
        try "Doc 1 Initial".write(to: file1, atomically: true, encoding: .utf8)
        try "Doc 2 Initial".write(to: file2, atomically: true, encoding: .utf8)
        try "Doc 3 Replacement".write(to: file3, atomically: true, encoding: .utf8)

        let writer = ArchiveWriter()
        try await writer.createArchive(
            outputPath: originalZip.path,
            format: .zip,
            inputPaths: [file1.path]
        )

        let engine = InPlaceArchiveMutationEngine.shared

        // 1. Add file asynchronously
        try await engine.addFilesToArchive(
            archivePath: originalZip.path,
            sourceFilePaths: [file2.path]
        )

        // 2. Replace entry asynchronously
        try await engine.replaceEntry(
            archivePath: originalZip.path,
            entryPath: "doc1.txt",
            sourceFilePath: file3.path
        )

        // 3. Rename entry asynchronously
        try await engine.renameEntry(
            archivePath: originalZip.path,
            oldEntryPath: "doc2.txt",
            newEntryPath: "doc2_renamed.txt"
        )

        // 4. Remove file asynchronously
        try await engine.removeFilesFromArchive(
            archivePath: originalZip.path,
            entryPaths: ["doc1.txt"]
        )

        // Verify remaining entries via reader
        let reader = ArchiveReader()
        let entries = try await reader.listEntries(archivePath: originalZip.path)
        let entryPaths = Set(entries.map(\.path))
        XCTAssertTrue(entryPaths.contains("doc2_renamed.txt"), "Renamed entry should exist")
        XCTAssertFalse(entryPaths.contains("doc1.txt"), "Deleted entry should not exist")
        XCTAssertFalse(entryPaths.contains("doc2.txt"), "Old name should not exist")
    }
}
