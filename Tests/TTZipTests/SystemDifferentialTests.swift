// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
import Foundation
import CryptoKit
@testable import TTZipCore

/// Bidirectional differential testing oracle comparing TTZip microkernel execution
/// with macOS native `/usr/bin/tar` and `/usr/bin/unzip` system toolchains.
final class SystemDifferentialTests: XCTestCase {
    private var sandbox: IsolatedTempSandbox!

    private let tarBinaryURL = URL(fileURLWithPath: "/usr/bin/tar")
    private let unzipBinaryURL = URL(fileURLWithPath: "/usr/bin/unzip")
    private let zipBinaryURL = URL(fileURLWithPath: "/usr/bin/zip")

    override func setUpWithError() throws {
        try super.setUpWithError()
        sandbox = try IsolatedTempSandbox(prefix: "system_diff")
    }

    override func tearDownWithError() throws {
        sandbox?.cleanup()
        sandbox = nil
        try super.tearDownWithError()
    }

    // MARK: - 1. ZIP Bidirectional Differential Testing

    /// Direction 1: TTZip creates ZIP archive -> macOS native `/usr/bin/unzip` extracts and validates.
    func testZipDifferential_TTZipWriter_SystemUnzipExtractor() async throws {
        let fixtureDir = try sandbox.createSubdirectory("zip_fixture_a")
        let fileText = fixtureDir.appendingPathComponent("document.txt")
        let fileBin = fixtureDir.appendingPathComponent("binary.dat")
        let emptyFile = fixtureDir.appendingPathComponent("empty.txt")
        let subDir = fixtureDir.appendingPathComponent("nested_dir")
        try FileManager.default.createDirectory(at: subDir, withIntermediateDirectories: true)
        let fileNested = subDir.appendingPathComponent("nested.log")

        let textContent = "System Differential Oracle Test: TTZip -> /usr/bin/unzip"
        let binContent = Data((0..<4096).map { UInt8($0 & 0xFF) })
        let nestedContent = "Nested payload verifying directory hierarchy traversal."

        try textContent.write(to: fileText, atomically: true, encoding: .utf8)
        try binContent.write(to: fileBin)
        try Data().write(to: emptyFile)
        try nestedContent.write(to: fileNested, atomically: true, encoding: .utf8)

        // 1. TTZip microkernel compresses the files into ZIP
        let archiveURL = sandbox.fileURL(named: "ttzip_created.zip")
        let writer = ArchiveWriter()
        let report = try writer.createArchiveWithReport(
            outputPath: archiveURL.path,
            format: .zip,
            level: .normal,
            inputPaths: [fixtureDir.path]
        )
        TTZipAssertions.assertEngineExecution(report, expected: .rustStreamingParallelZip)
        TTZipAssertions.assertNoFallback(report)
        XCTAssertTrue(FileManager.default.fileExists(atPath: archiveURL.path))

        // 2. macOS native /usr/bin/unzip extracts the archive
        let extractDir = try sandbox.createSubdirectory("system_unzip_dest")
        let extractResult = try runProcess(
            executableURL: unzipBinaryURL,
            arguments: ["-q", archiveURL.path, "-d", extractDir.path]
        )
        XCTAssertEqual(extractResult.exitCode, 0, "/usr/bin/unzip failed: \(extractResult.stderr)")

        // 3. Differential assertions: verify exact directory and content equality
        let extractedRoot = extractDir.appendingPathComponent("zip_fixture_a")
        XCTAssertTrue(FileManager.default.fileExists(atPath: extractedRoot.path))
        assertDirectoryTreesEqual(expectedDir: fixtureDir, actualDir: extractedRoot)
    }

    /// Direction 2: macOS native `/usr/bin/zip` creates ZIP -> TTZip `ArchiveExtractor` extracts and validates.
    func testZipDifferential_SystemZipWriter_TTZipExtractor() async throws {
        let fixtureDir = try sandbox.createSubdirectory("zip_fixture_b")
        let fileText = fixtureDir.appendingPathComponent("system_doc.txt")
        let fileBin = fixtureDir.appendingPathComponent("system_data.bin")
        let subDir = fixtureDir.appendingPathComponent("inner")
        try FileManager.default.createDirectory(at: subDir, withIntermediateDirectories: true)
        let innerText = subDir.appendingPathComponent("inner_file.txt")

        let textData = Data("macOS Native /usr/bin/zip Generated Payload".utf8)
        let binData = Data((0..<8192).map { UInt8(($0 * 7) & 0xFF) })
        let innerData = Data("Inner directory content for cross-extractor parity".utf8)

        try textData.write(to: fileText)
        try binData.write(to: fileBin)
        try innerData.write(to: innerText)

        // 1. macOS native /usr/bin/zip creates the ZIP archive
        let archiveURL = sandbox.fileURL(named: "system_created.zip")
        let zipResult = try runProcess(
            executableURL: zipBinaryURL,
            arguments: ["-q", "-r", archiveURL.path, "zip_fixture_b"],
            currentDirectoryURL: sandbox.url
        )
        XCTAssertEqual(zipResult.exitCode, 0, "/usr/bin/zip creation failed: \(zipResult.stderr)")
        XCTAssertTrue(FileManager.default.fileExists(atPath: archiveURL.path))

        // 2. TTZip microkernel extracts the system-created archive
        let extractDir = try sandbox.createSubdirectory("ttzip_extract_dest")
        let extractor = ArchiveExtractor()
        let (_, provenance) = try await EngineProvenanceCollector.captureAsync(expectedEngine: .rustStreamingParallelZip) {
            try await extractor.extractArchive(
                archivePath: archiveURL.path,
                destinationDir: extractDir.path
            )
        }
        TTZipAssertions.assertEngineExecution(provenance, expected: .rustStreamingParallelZip)
        TTZipAssertions.assertNoFallback(provenance)

        // 3. Differential assertions: verify exact directory and content equality
        let extractedRoot = extractDir.appendingPathComponent("zip_fixture_b")
        XCTAssertTrue(FileManager.default.fileExists(atPath: extractedRoot.path))
        assertDirectoryTreesEqual(expectedDir: fixtureDir, actualDir: extractedRoot)
    }

    /// Differential Parity: Both TTZip and `/usr/bin/unzip` extract the SAME archive; verify identical output.
    func testZipDifferential_CrossEngineExtractionParity() async throws {
        let fixtureDir = try sandbox.createSubdirectory("zip_parity_fixture")
        let leafA = fixtureDir.appendingPathComponent("alpha.bin")
        let leafB = fixtureDir.appendingPathComponent("beta.txt")
        let payloadA = Data((0..<2048).map { UInt8($0 % 251) })
        let payloadB = "Cross Engine Extraction Parity Check 2026"
        try payloadA.write(to: leafA)
        try payloadB.write(to: leafB, atomically: true, encoding: .utf8)

        let archiveURL = sandbox.fileURL(named: "shared_parity.zip")
        let writer = ArchiveWriter()
        let report = try writer.createArchiveWithReport(
            outputPath: archiveURL.path,
            format: .zip,
            level: .normal,
            inputPaths: [fixtureDir.path]
        )
        TTZipAssertions.assertNoFallback(report)

        // Extract with TTZip
        let ttzipDest = try sandbox.createSubdirectory("extract_ttzip")
        let extractor = ArchiveExtractor()
        try await extractor.extractArchive(archivePath: archiveURL.path, destinationDir: ttzipDest.path)

        // Extract with /usr/bin/unzip
        let systemDest = try sandbox.createSubdirectory("extract_system")
        let unzipResult = try runProcess(
            executableURL: unzipBinaryURL,
            arguments: ["-q", archiveURL.path, "-d", systemDest.path]
        )
        XCTAssertEqual(unzipResult.exitCode, 0, "/usr/bin/unzip failed: \(unzipResult.stderr)")

        // Assert 100% bit-exact equality between TTZip extraction and /usr/bin/unzip extraction
        let rootTTZip = ttzipDest.appendingPathComponent("zip_parity_fixture")
        let rootSystem = systemDest.appendingPathComponent("zip_parity_fixture")
        assertDirectoryTreesEqual(expectedDir: rootSystem, actualDir: rootTTZip)
    }

    // MARK: - 2. TAR Bidirectional Differential Testing

    /// Direction 1: TTZip creates TAR archive -> macOS native `/usr/bin/tar` extracts and validates.
    func testTarDifferential_TTZipWriter_SystemTarExtractor() async throws {
        let fixtureDir = try sandbox.createSubdirectory("tar_fixture_a")
        let execScript = fixtureDir.appendingPathComponent("run.sh")
        let configDoc = fixtureDir.appendingPathComponent("config.cfg")
        let subFolder = fixtureDir.appendingPathComponent("sub")
        let emptyFolder = fixtureDir.appendingPathComponent("empty_folder")
        try FileManager.default.createDirectory(at: subFolder, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: emptyFolder, withIntermediateDirectories: true)
        let nestedBin = subFolder.appendingPathComponent("payload.bin")

        let scriptData = Data("#!/bin/sh\necho 'Tar System Differential'\n".utf8)
        let configData = Data("key=value\nmode=strict\n".utf8)
        let nestedData = Data((0..<3072).map { UInt8($0 % 199) })

        try scriptData.write(to: execScript)
        try configData.write(to: configDoc)
        try nestedData.write(to: nestedBin)

        // Set POSIX permissions (0o755 for script, 0o644 for config)
        chmod(execScript.path, 0o755)
        chmod(configDoc.path, 0o644)
        TTZipAssertions.assertFileMode(execScript, expectedMode: 0o755)
        TTZipAssertions.assertFileMode(configDoc, expectedMode: 0o644)

        // 1. TTZip microkernel compresses files into TAR
        let archiveURL = sandbox.fileURL(named: "ttzip_created.tar")
        let writer = ArchiveWriter()
        let report = try writer.createArchiveWithReport(
            outputPath: archiveURL.path,
            format: .tar,
            level: .normal,
            inputPaths: [fixtureDir.path]
        )
        TTZipAssertions.assertNoFallback(report)
        XCTAssertTrue(FileManager.default.fileExists(atPath: archiveURL.path))

        // 2. macOS native /usr/bin/tar extracts the archive
        let extractDir = try sandbox.createSubdirectory("system_tar_dest")
        let tarResult = try runProcess(
            executableURL: tarBinaryURL,
            arguments: ["-xf", archiveURL.path, "-C", extractDir.path]
        )
        XCTAssertEqual(tarResult.exitCode, 0, "/usr/bin/tar extraction failed: \(tarResult.stderr)")

        // 3. Differential assertions: verify exact directory and content equality
        let extractedRoot = extractDir.appendingPathComponent("tar_fixture_a")
        XCTAssertTrue(FileManager.default.fileExists(atPath: extractedRoot.path))
        assertDirectoryTreesEqual(expectedDir: fixtureDir, actualDir: extractedRoot)

        // Verify POSIX permission fidelity
        let extractedExec = extractedRoot.appendingPathComponent("run.sh")
        let extractedConfig = extractedRoot.appendingPathComponent("config.cfg")
        TTZipAssertions.assertFileMode(extractedExec, expectedMode: 0o755)
        TTZipAssertions.assertFileMode(extractedConfig, expectedMode: 0o644)
    }

    /// Direction 2: macOS native `/usr/bin/tar` creates TAR -> TTZip `ArchiveExtractor` extracts and validates.
    func testTarDifferential_SystemTarWriter_TTZipExtractor() async throws {
        let fixtureDir = try sandbox.createSubdirectory("tar_fixture_b")
        let scriptFile = fixtureDir.appendingPathComponent("entrypoint.sh")
        let dataFile = fixtureDir.appendingPathComponent("corpus.bin")
        let innerDir = fixtureDir.appendingPathComponent("nested_tar")
        try FileManager.default.createDirectory(at: innerDir, withIntermediateDirectories: true)
        let innerDoc = innerDir.appendingPathComponent("manifest.json")

        let scriptBytes = Data("#!/usr/bin/env bash\nexit 0\n".utf8)
        let dataBytes = Data((0..<5120).map { UInt8(($0 * 13) & 0xFF) })
        let docBytes = Data("{\"differential\": true, \"oracle\": \"native_tar\"}\n".utf8)

        try scriptBytes.write(to: scriptFile)
        try dataBytes.write(to: dataFile)
        try docBytes.write(to: innerDoc)

        chmod(scriptFile.path, 0o755)
        chmod(dataFile.path, 0o644)

        // 1. macOS native /usr/bin/tar creates the TAR archive
        let archiveURL = sandbox.fileURL(named: "system_created.tar")
        let tarResult = try runProcess(
            executableURL: tarBinaryURL,
            arguments: ["-cf", archiveURL.path, "tar_fixture_b"],
            currentDirectoryURL: sandbox.url
        )
        XCTAssertEqual(tarResult.exitCode, 0, "/usr/bin/tar creation failed: \(tarResult.stderr)")
        XCTAssertTrue(FileManager.default.fileExists(atPath: archiveURL.path))

        // 2. TTZip microkernel extracts the system-created archive
        let extractDir = try sandbox.createSubdirectory("ttzip_tar_extract_dest")
        let extractor = ArchiveExtractor()
        let (_, provenance) = try await EngineProvenanceCollector.captureAsync(expectedEngine: .rustStreamingParallelZip) {
            try await extractor.extractArchive(
                archivePath: archiveURL.path,
                destinationDir: extractDir.path
            )
        }
        TTZipAssertions.assertEngineExecution(provenance, expected: .rustStreamingParallelZip)
        TTZipAssertions.assertNoFallback(provenance)

        // 3. Differential assertions: verify exact directory and content equality
        let extractedRoot = extractDir.appendingPathComponent("tar_fixture_b")
        XCTAssertTrue(FileManager.default.fileExists(atPath: extractedRoot.path))
        assertDirectoryTreesEqual(expectedDir: fixtureDir, actualDir: extractedRoot)

        // Verify POSIX mode preservation
        let extractedScript = extractedRoot.appendingPathComponent("entrypoint.sh")
        let extractedData = extractedRoot.appendingPathComponent("corpus.bin")
        TTZipAssertions.assertFileMode(extractedScript, expectedMode: 0o755)
        TTZipAssertions.assertFileMode(extractedData, expectedMode: 0o644)
    }

    /// Differential Parity: Both TTZip and `/usr/bin/tar` extract the SAME TAR archive; verify identical output.
    func testTarDifferential_CrossEngineExtractionParity() async throws {
        let fixtureDir = try sandbox.createSubdirectory("tar_parity_fixture")
        let docA = fixtureDir.appendingPathComponent("report.txt")
        let docB = fixtureDir.appendingPathComponent("checksums.sha256")
        let payloadA = "System Differential Dual Engine Extraction Parity Check"
        let payloadB = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        try payloadA.write(to: docA, atomically: true, encoding: .utf8)
        try payloadB.write(to: docB, atomically: true, encoding: .utf8)

        let archiveURL = sandbox.fileURL(named: "shared_parity.tar")
        let writer = ArchiveWriter()
        let report = try writer.createArchiveWithReport(
            outputPath: archiveURL.path,
            format: .tar,
            level: .normal,
            inputPaths: [fixtureDir.path]
        )
        TTZipAssertions.assertNoFallback(report)

        // Extract with TTZip
        let ttzipDest = try sandbox.createSubdirectory("tar_extract_ttzip")
        let extractor = ArchiveExtractor()
        try await extractor.extractArchive(archivePath: archiveURL.path, destinationDir: ttzipDest.path)

        // Extract with /usr/bin/tar
        let systemDest = try sandbox.createSubdirectory("tar_extract_system")
        let tarResult = try runProcess(
            executableURL: tarBinaryURL,
            arguments: ["-xf", archiveURL.path, "-C", systemDest.path]
        )
        XCTAssertEqual(tarResult.exitCode, 0, "/usr/bin/tar failed: \(tarResult.stderr)")

        // Assert 100% bit-exact equality between TTZip extraction and /usr/bin/tar extraction
        let rootTTZip = ttzipDest.appendingPathComponent("tar_parity_fixture")
        let rootSystem = systemDest.appendingPathComponent("tar_parity_fixture")
        assertDirectoryTreesEqual(expectedDir: rootSystem, actualDir: rootTTZip)
    }

    // MARK: - 3. Process & Assertion Utilities

    @discardableResult
    private func runProcess(
        executableURL: URL,
        arguments: [String],
        currentDirectoryURL: URL? = nil
    ) throws -> (exitCode: Int32, stdout: String, stderr: String) {
        guard FileManager.default.isExecutableFile(atPath: executableURL.path) else {
            throw NSError(
                domain: "SystemDifferentialTests",
                code: 404,
                userInfo: [NSLocalizedDescriptionKey: "Binary not executable at \(executableURL.path)"]
            )
        }

        let process = Process()
        process.executableURL = executableURL
        process.arguments = arguments
        if let cwd = currentDirectoryURL {
            process.currentDirectoryURL = cwd
        }

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

    /// Recursively asserts that two directory trees have exact matching relative paths and SHA-256 digests.
    private func assertDirectoryTreesEqual(
        expectedDir: URL,
        actualDir: URL,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        let expectedFiles = collectRelativeFiles(at: expectedDir)
        let actualFiles = collectRelativeFiles(at: actualDir)

        let expectedSet = Set(expectedFiles.keys)
        let actualSet = Set(actualFiles.keys)

        XCTAssertEqual(
            actualSet,
            expectedSet,
            "Directory hierarchy mismatch. Missing: \(expectedSet.subtracting(actualSet)), Extra: \(actualSet.subtracting(expectedSet))",
            file: file,
            line: line
        )

        for relativePath in expectedSet {
            guard let expectedHash = expectedFiles[relativePath],
                  let actualHash = actualFiles[relativePath] else {
                XCTFail("Missing file hash for \(relativePath)", file: file, line: line)
                continue
            }
            XCTAssertEqual(
                actualHash,
                expectedHash,
                "SHA-256 digest mismatch for file: \(relativePath)",
                file: file,
                line: line
            )
        }
    }

    /// Collects relative file paths and their SHA-256 digests from a root directory.
    private func collectRelativeFiles(at rootURL: URL) -> [String: String] {
        var results: [String: String] = [:]
        let fm = FileManager.default
        let prefixLen = rootURL.path.hasSuffix("/") ? rootURL.path.count : rootURL.path.count + 1

        guard let enumerator = fm.enumerator(
            at: rootURL,
            includingPropertiesForKeys: [.isRegularFileKey, .isDirectoryKey],
            options: [.skipsHiddenFiles]
        ) else {
            return results
        }

        for case let fileURL as URL in enumerator {
            guard let resourceValues = try? fileURL.resourceValues(forKeys: [.isRegularFileKey]),
                  resourceValues.isRegularFile == true else {
                continue
            }

            let fullPath = fileURL.path
            guard fullPath.count >= prefixLen else { continue }
            let relativePath = String(fullPath.suffix(from: fullPath.index(fullPath.startIndex, offsetBy: prefixLen)))

            if let data = try? Data(contentsOf: fileURL) {
                let digest = SHA256.hash(data: data)
                let hashString = digest.compactMap { String(format: "%02x", $0) }.joined()
                results[relativePath] = hashString
            }
        }

        return results
    }
}
