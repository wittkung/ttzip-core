// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class AdversarialIntegrityTests: XCTestCase {
    private var tempDir: URL!

    override func setUpWithError() throws {
        try super.setUpWithError()
        tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("ttzip_adv_test_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: tempDir)
        try super.tearDownWithError()
    }

    /// Tests that true stream-discarding integrity verification accurately detects corrupted blocks
    /// and flags CRC mismatch errors instead of fake counting.
    func testMutatedZipIntegrityDetection() async throws {
        let zipPath = tempDir.appendingPathComponent("test_archive.zip").path
        let sampleFile = tempDir.appendingPathComponent("payload.txt").path
        let originalContent = "Hello TTZip Secure Archiving System Verification Stream."
        try originalContent.write(toFile: sampleFile, atomically: true, encoding: .utf8)

        let writer = ArchiveWriter()
        try writer.createArchiveSync(
            outputPath: zipPath,
            format: .zip,
            level: .normal,
            inputPaths: [sampleFile]
        )

        // 1. Assert pristine integrity passes
        let checker = ArchiveIntegrityChecker()
        let pristineReport = try await checker.checkArchiveIntegrity(archivePath: zipPath)
        XCTAssertEqual(pristineReport.overallStatus, IntegrityStatus.passed)
        XCTAssertEqual(pristineReport.corruptedEntriesCount, 0)

        // 2. Corrupt bytes in the archive
        var data = try Data(contentsOf: URL(fileURLWithPath: zipPath))
        guard data.count > 50 else {
            XCTFail("Zip payload too small to mutate")
            return
        }
        for offset in (data.count - 40)..<(data.count - 20) {
            data[offset] ^= 0xFF
        }
        try data.write(to: URL(fileURLWithPath: zipPath))

        // 3. Assert mutated archive is flagged as corrupted
        let mutatedReport = try await checker.checkArchiveIntegrity(archivePath: zipPath)
        XCTAssertEqual(mutatedReport.overallStatus, IntegrityStatus.corrupted)
        XCTAssertGreaterThan(mutatedReport.corruptedEntriesCount, 0)
    }

    /// SEC-05: Tests that extraction failure does NOT delete existing files in target folder.
    func testExtractionFailurePreservesExistingDirectoryFiles() async throws {
        let targetDir = tempDir.appendingPathComponent("user_documents")
        try FileManager.default.createDirectory(at: targetDir, withIntermediateDirectories: true)

        let existingPreciousFile = targetDir.appendingPathComponent("important_tax_document.pdf")
        try "Precious Tax Data".write(to: existingPreciousFile, atomically: true, encoding: .utf8)

        let corruptZip = tempDir.appendingPathComponent("broken.zip")
        try Data([0x50, 0x4B, 0x03, 0x04, 0x00, 0x00, 0xFF, 0xFF]).write(to: corruptZip)

        let extractor = ArchiveExtractor()
        do {
            try await extractor.extract(
                archivePath: corruptZip.path,
                destinationDir: targetDir.path,
                options: .defaultClean
            )
            XCTFail("Extraction of broken archive should throw")
        } catch {
            // Expected error
        }

        // SEC-05 Assert: Precious existing file must STILL exist!
        XCTAssertTrue(FileManager.default.fileExists(atPath: existingPreciousFile.path))
        let content = try String(contentsOf: existingPreciousFile, encoding: .utf8)
        XCTAssertEqual(content, "Precious Tax Data")
    }
}
