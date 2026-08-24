// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class LargeVolumeStressTests: XCTestCase {

    var tempWorkingDir: URL!

    override func setUpWithError() throws {
        try super.setUpWithError()
        tempWorkingDir = FileManager.default.temporaryDirectory.appendingPathComponent("ttzip_stress_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempWorkingDir, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        if let dir = tempWorkingDir, FileManager.default.fileExists(atPath: dir.path) {
            try? FileManager.default.removeItem(at: dir)
        }
        try super.tearDownWithError()
    }

    func testDifferentialRollbackPreservesExistingFiles() throws {
        let destDir = tempWorkingDir.appendingPathComponent("destination")
        try FileManager.default.createDirectory(at: destDir, withIntermediateDirectories: true)

        // 1. Create pre-existing user files
        let existingFile = destDir.appendingPathComponent("existing_important_user_doc.txt")
        let existingContent = "User sensitive data that must not be altered."
        try existingContent.write(to: existingFile, atomically: true, encoding: .utf8)

        // 2. Initialize DifferentialExtractTransaction
        var transaction = DifferentialExtractTransaction(destinationPath: destDir.path)

        // Simulate extracting new files
        let newFile1 = destDir.appendingPathComponent("extracted_file_1.txt")
        let newSubdir = destDir.appendingPathComponent("nested_dir")
        let newFile2 = newSubdir.appendingPathComponent("extracted_file_2.txt")

        try FileManager.default.createDirectory(at: newSubdir, withIntermediateDirectories: true)
        transaction.recordCreated(path: newSubdir.path, isDirectory: true)

        try "New File 1 Content".write(to: newFile1, atomically: true, encoding: .utf8)
        transaction.recordCreated(path: newFile1.path, isDirectory: false)

        try "New File 2 Content".write(to: newFile2, atomically: true, encoding: .utf8)
        transaction.recordCreated(path: newFile2.path, isDirectory: false)

        XCTAssertTrue(FileManager.default.fileExists(atPath: newFile1.path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: newFile2.path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: existingFile.path))

        // 3. Trigger Rollback
        transaction.executeRollback()

        // 4. Assert: newly extracted files are gone, pre-existing user files are intact
        XCTAssertFalse(FileManager.default.fileExists(atPath: newFile1.path))
        XCTAssertFalse(FileManager.default.fileExists(atPath: newFile2.path))
        XCTAssertFalse(FileManager.default.fileExists(atPath: newSubdir.path))

        XCTAssertTrue(FileManager.default.fileExists(atPath: existingFile.path))
        let remainingContent = try String(contentsOf: existingFile, encoding: .utf8)
        XCTAssertEqual(remainingContent, existingContent)
    }

    func testMultiVolumeSplitArchiveInspectionZeroDiskStaging() throws {
        // 1. Generate split volumes using SplitVolumeEngine
        let sourceFile = tempWorkingDir.appendingPathComponent("large_sample_data.bin")
        let chunk = Data(repeating: 0xAB, count: 64 * 1024)
        var fullData = Data()
        for _ in 0..<16 {
            fullData.append(chunk) // 1MB total
        }
        try fullData.write(to: sourceFile)

        let splitEngine = SplitVolumeEngine()
        try splitEngine.sliceArchive(
            archivePath: sourceFile.path,
            splitSizeBytes: 300 * 1024, // 300KB volumes
            namingPattern: .numberedExtension,
            cleanOnFailure: true
        )

        let discoveredVolumes = splitEngine.resolveVolumes(seedPath: sourceFile.path + ".001")
        XCTAssertGreaterThanOrEqual(discoveredVolumes.count, 3)

        // 2. Reassemble and verify data integrity
        let reassembledFile = tempWorkingDir.appendingPathComponent("reassembled.bin")
        try splitEngine.joinVolumes(
            firstVolumePath: sourceFile.path + ".001",
            outputPath: reassembledFile.path
        )

        let reassembledData = try Data(contentsOf: reassembledFile)
        XCTAssertEqual(reassembledData, fullData)

        // 3. Verify zero temp file concatenation created in /tmp
        let tmpFiles = (try? FileManager.default.contentsOfDirectory(atPath: "/tmp")) ?? []
        let concatenatedLeaks = tmpFiles.filter { $0.contains("ttzip_split_concat") }
        XCTAssertTrue(concatenatedLeaks.isEmpty)
    }
}
