// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class PhysicalIOAccountingTests: XCTestCase {
    private var tempDir: URL!

    override func setUpWithError() throws {
        try super.setUpWithError()
        tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("ttzip_io_test_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: tempDir)
        try super.tearDownWithError()
    }

    /// Tests that single entry in-memory extraction performs ZERO disk writes to /tmp.
    func testSingleEntryExtractZeroDiskIOWrites() async throws {
        let zipPath = tempDir.appendingPathComponent("large_dataset.zip").path
        let sampleFile = tempDir.appendingPathComponent("entry1.txt").path
        let content = "Single Entry Stream Preview Test Content."
        try content.write(toFile: sampleFile, atomically: true, encoding: .utf8)

        let writer = ArchiveWriter()
        try writer.createArchiveSync(
            outputPath: zipPath,
            format: .zip,
            level: .normal,
            inputPaths: [sampleFile]
        )

        let initialTempDirFiles = try FileManager.default.contentsOfDirectory(atPath: tempDir.path)

        // Perform 50 in-memory extractions
        for _ in 0..<50 {
            let data = try await ArchiveSelectiveExtractor.shared.extractSingleEntryData(
                archivePath: zipPath,
                entryPath: "entry1.txt"
            )
            XCTAssertNotNil(data)
            XCTAssertEqual(String(data: data!, encoding: .utf8), content)
        }

        let finalTempDirFiles = try FileManager.default.contentsOfDirectory(atPath: tempDir.path)

        // Assert: No temporary files created on disk
        XCTAssertEqual(initialTempDirFiles.sorted(), finalTempDirFiles.sorted())
    }
}
