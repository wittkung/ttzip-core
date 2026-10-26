// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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
}
