// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

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
}
