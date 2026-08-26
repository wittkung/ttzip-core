// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
import Foundation
@testable import TTZipCore

final class CLICommandE2ETests: XCTestCase {
    private var tempDirectory: URL!

    private var ttzipBinaryURL: URL {
        let currentFile = URL(fileURLWithPath: #filePath)
        let coreDir = currentFile.deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let binPath = coreDir.appendingPathComponent("bin/ttzip")
        if FileManager.default.isExecutableFile(atPath: binPath.path) {
            return binPath
        }
        if let envBin = ProcessInfo.processInfo.environment["TTZIP_BIN_PATH"],
           FileManager.default.isExecutableFile(atPath: envBin) {
            return URL(fileURLWithPath: envBin)
        }
        return binPath
    }

    override func setUp() async throws {
        try await super.setUp()
        TTZipEngineFacade.initializeSubsystems()
        tempDirectory = FileManager.default.temporaryDirectory.appendingPathComponent("ttzip_cli_e2e_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
    }

    override func tearDown() async throws {
        if let temp = tempDirectory, FileManager.default.fileExists(atPath: temp.path) {
            try? FileManager.default.removeItem(at: temp)
        }
        try await super.tearDown()
    }

    @discardableResult
    private func runCLI(arguments: [String]) throws -> (exitCode: Int32, stdout: String, stderr: String) {
        let binaryURL = ttzipBinaryURL
        guard FileManager.default.fileExists(atPath: binaryURL.path) else {
            throw NSError(
                domain: "CLICommandE2ETests",
                code: 404,
                userInfo: [NSLocalizedDescriptionKey: "Binary not found at \(binaryURL.path)"]
            )
        }

        let process = Process()
        process.executableURL = binaryURL
        process.arguments = arguments

        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        try process.run()
        process.waitUntilExit()

        let stdoutData = stdoutPipe.fileHandleForReading.readDataToEndOfFile()
        let stderrData = stderrPipe.fileHandleForReading.readDataToEndOfFile()

        let stdoutStr = String(data: stdoutData, encoding: .utf8) ?? ""
        let stderrStr = String(data: stderrData, encoding: .utf8) ?? ""

        return (process.terminationStatus, stdoutStr, stderrStr)
    }

    func testCLI_CreateAndExtract_RoundTrip_Zip() throws {
        let sampleFile = tempDirectory.appendingPathComponent("sample.txt")
        let sampleContent = "TTZip CLI Roundtrip Verification Data - 2026"
        try sampleContent.write(to: sampleFile, atomically: true, encoding: .utf8)

        let archiveFile = tempDirectory.appendingPathComponent("test_archive.zip")
        let extractDir = tempDirectory.appendingPathComponent("extracted_zip")

        // 1. Create ZIP archive via real CLI Process
        let createResult = try runCLI(arguments: ["create", archiveFile.path, sampleFile.path, "--format", "zip", "--level", "1"])
        XCTAssertEqual(createResult.exitCode, 0, "CLI create zip should succeed: \(createResult.stderr)")
        XCTAssertTrue(FileManager.default.fileExists(atPath: archiveFile.path), "ZIP archive should be created on disk")

        // 2. List ZIP archive via CLI with --json
        let listResult = try runCLI(arguments: ["list", archiveFile.path, "--json"])
        XCTAssertEqual(listResult.exitCode, 0, "CLI list --json should succeed")
        XCTAssertTrue(listResult.stdout.contains("sample.txt"), "JSON list output should include sample.txt")

        // 3. Extract ZIP archive via real CLI Process
        let extractResult = try runCLI(arguments: ["extract", archiveFile.path, "-o", extractDir.path])
        XCTAssertEqual(extractResult.exitCode, 0, "CLI extract zip should succeed: \(extractResult.stderr)")

        let extractedFile = extractDir.appendingPathComponent("sample.txt")
        XCTAssertTrue(FileManager.default.fileExists(atPath: extractedFile.path), "Extracted file should exist")

        let extractedContent = try String(contentsOf: extractedFile, encoding: .utf8)
        XCTAssertEqual(extractedContent, sampleContent, "Extracted content must match original exactly")
    }

    func testCLI_CreateAndExtract_RoundTrip_SevenZip() throws {
        let sampleFile = tempDirectory.appendingPathComponent("sevenzip_sample.txt")
        let sampleContent = "TTZip CLI 7Z Verification Payload 2026"
        try sampleContent.write(to: sampleFile, atomically: true, encoding: .utf8)

        let archiveFile = tempDirectory.appendingPathComponent("test_archive.7z")
        let extractDir = tempDirectory.appendingPathComponent("extracted_7z")

        // 1. Create 7Z archive via real CLI Process
        let createResult = try runCLI(arguments: ["create", archiveFile.path, sampleFile.path, "--format", "7z", "--level", "1"])
        XCTAssertEqual(createResult.exitCode, 0, "CLI create 7z should succeed: \(createResult.stderr)")
        XCTAssertTrue(FileManager.default.fileExists(atPath: archiveFile.path), "7Z archive should be created on disk")

        // 2. Extract 7Z archive via real CLI Process
        let extractResult = try runCLI(arguments: ["extract", archiveFile.path, "-o", extractDir.path])
        XCTAssertEqual(extractResult.exitCode, 0, "CLI extract 7z should succeed: \(extractResult.stderr)")

        let extractedFile = extractDir.appendingPathComponent("sevenzip_sample.txt")
        XCTAssertTrue(FileManager.default.fileExists(atPath: extractedFile.path), "Extracted 7Z file should exist")

        let extractedContent = try String(contentsOf: extractedFile, encoding: .utf8)
        XCTAssertEqual(extractedContent, sampleContent)
    }

    func testCLI_Info_And_Check_Subcommands() throws {
        let sampleFile = tempDirectory.appendingPathComponent("inspect_me.txt")
        try "Content to inspect".write(to: sampleFile, atomically: true, encoding: .utf8)

        let archiveFile = tempDirectory.appendingPathComponent("inspect_test.zip")
        let createResult = try runCLI(arguments: ["create", archiveFile.path, sampleFile.path, "--format", "zip"])
        XCTAssertEqual(createResult.exitCode, 0)

        // Test info subcommand
        let infoResult = try runCLI(arguments: ["info", archiveFile.path])
        XCTAssertEqual(infoResult.exitCode, 0, "CLI info should succeed")
        XCTAssertTrue(infoResult.stdout.contains("Inspection Report") || infoResult.stdout.contains("ZIP"))

        // Test check subcommand
        let checkResult = try runCLI(arguments: ["check", archiveFile.path])
        XCTAssertEqual(checkResult.exitCode, 0, "CLI check should succeed")
        XCTAssertTrue(checkResult.stdout.contains("PASS") || checkResult.stdout.contains("compliant"))
    }

    func testCLI_InvalidArguments_ReturnsErrorCode() throws {
        let nonExistentFile = tempDirectory.appendingPathComponent("non_existent_archive.zip").path
        let result = try runCLI(arguments: ["extract", nonExistentFile, "-o", tempDirectory.path])
        XCTAssertNotEqual(result.exitCode, 0, "CLI should return non-zero exit code on non-existent archive")
    }
}
