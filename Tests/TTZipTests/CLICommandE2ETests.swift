// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import XCTest
import Foundation
@testable import TTZipCore

final class CLICommandE2ETests: XCTestCase {
    private var tempDirectory: URL!

    private var candidateBinaryURLs: [URL] {
        var candidates: [URL] = []
        let env = ProcessInfo.processInfo.environment
        let fileManager = FileManager.default

        // 1. Explicit environment variable override
        if let envBin = env["TTZIP_BIN_PATH"] {
            candidates.append(URL(fileURLWithPath: envBin))
        }

        // 2. Bazel TEST_SRCDIR probe
        if let testSrcDir = env["TEST_SRCDIR"] {
            let base = URL(fileURLWithPath: testSrcDir)
            candidates.append(base.appendingPathComponent("_main/products/ttzip/core/rust/ttzip-tui/ttzip_cli"))
            candidates.append(base.appendingPathComponent("products/ttzip/core/rust/ttzip-tui/ttzip_cli"))
            candidates.append(base.appendingPathComponent("_main/products/ttzip/core/bin/ttzip"))
            candidates.append(base.appendingPathComponent("products/ttzip/core/bin/ttzip"))
        }

        // 3. Bazel RUNFILES_DIR probe
        if let runfilesDir = env["RUNFILES_DIR"] {
            let base = URL(fileURLWithPath: runfilesDir)
            candidates.append(base.appendingPathComponent("_main/products/ttzip/core/rust/ttzip-tui/ttzip_cli"))
            candidates.append(base.appendingPathComponent("products/ttzip/core/rust/ttzip-tui/ttzip_cli"))
            candidates.append(base.appendingPathComponent("_main/products/ttzip/core/bin/ttzip"))
            candidates.append(base.appendingPathComponent("products/ttzip/core/bin/ttzip"))
        }

        // 4. Current working directory probe
        let cwd = URL(fileURLWithPath: fileManager.currentDirectoryPath)
        candidates.append(cwd.appendingPathComponent("products/ttzip/core/rust/ttzip-tui/ttzip_cli"))
        candidates.append(cwd.appendingPathComponent("bazel-bin/products/ttzip/core/rust/ttzip-tui/ttzip_cli"))
        candidates.append(cwd.appendingPathComponent("products/ttzip/core/bin/ttzip"))
        candidates.append(cwd.appendingPathComponent("core/bin/ttzip"))
        candidates.append(cwd.appendingPathComponent("bin/ttzip"))

        // 5. Relative to current source file (#filePath)
        let currentFile = URL(fileURLWithPath: #filePath)
        let coreDir = currentFile.deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        candidates.append(coreDir.appendingPathComponent("rust/ttzip-tui/target/debug/ttzip_cli"))
        candidates.append(coreDir.appendingPathComponent("rust/ttzip-tui/target/release/ttzip_cli"))
        candidates.append(coreDir.appendingPathComponent("bin/ttzip"))

        return candidates
    }

    private var ttzipBinaryURL: URL? {
        let fileManager = FileManager.default
        for candidate in candidateBinaryURLs {
            if fileManager.isExecutableFile(atPath: candidate.path) {
                return candidate
            }
        }
        return nil
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
        guard let binaryURL = ttzipBinaryURL else {
            let searched = candidateBinaryURLs.map(\.path).joined(separator: "\n  - ")
            XCTFail("ttzip CLI binary not found in test environment. Inspected candidate paths:\n  - \(searched)\nEnsure //products/ttzip/core/rust/ttzip-tui:ttzip_cli is declared in BUILD.bazel data.")
            struct BinaryNotFoundError: Error {}
            throw BinaryNotFoundError()
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
