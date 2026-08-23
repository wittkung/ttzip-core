// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

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
        let created = writer.createArchiveWithRust(
            outputPath: zipPath,
            format: .zip,
            inputPaths: [sampleFile],
            level: .normal,
            password: nil,
            totalBytes: Int64(content.utf8.count)
        )
        XCTAssertTrue(created)

        let initialTtzipTmpFiles = (try? FileManager.default.contentsOfDirectory(atPath: NSTemporaryDirectory()))?.filter { $0.contains("ttzip_") || $0.contains("probe_") }.count ?? 0

        // Perform 50 in-memory extractions
        for _ in 0..<50 {
            let data = try await ArchiveSelectiveExtractor.shared.extractSingleEntryData(
                archivePath: zipPath,
                entryPath: "entry1.txt"
            )
            XCTAssertNotNil(data)
            XCTAssertEqual(String(data: data!, encoding: .utf8), content)
        }

        let finalTtzipTmpFiles = (try? FileManager.default.contentsOfDirectory(atPath: NSTemporaryDirectory()))?.filter { $0.contains("ttzip_") || $0.contains("probe_") }.count ?? 0

        // Assert: No temporary files created on disk
        XCTAssertEqual(initialTtzipTmpFiles, finalTtzipTmpFiles)
    }
}
