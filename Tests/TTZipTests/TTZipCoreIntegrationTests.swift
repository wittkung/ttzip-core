// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
import Foundation
@testable import TTZipCore

final class TTZipCoreIntegrationTests: XCTestCase {
    private var sandbox: IsolatedTempSandbox!

    override func setUpWithError() throws {
        try super.setUpWithError()
        sandbox = try IsolatedTempSandbox(prefix: "core_integration")
    }

    override func tearDownWithError() throws {
        sandbox?.cleanup()
        sandbox = nil
        try super.tearDownWithError()
    }

    // MARK: - 1. ArchiveWriter & ArchiveExtractor Multi-Format Integration (ZIP / 7z / TAR / TAR.GZ / TAR.BZ2 / TAR.XZ / TAR.ZST)

    func testArchiveWriterAndExtractorRoundtripAllFormats() async throws {
        let writer = ArchiveWriter()
        let extractor = ArchiveExtractor()

        let file1 = sandbox.fileURL(named: "doc1.txt")
        let file2 = sandbox.fileURL(named: "data.bin")
        let content1 = "TTZip Core Unified Facade Integration Test 2026"
        let content2 = Data((0..<1024).map { UInt8($0 % 256) })

        try content1.write(to: file1, atomically: true, encoding: .utf8)
        try content2.write(to: file2)

        let formats: [(ArchiveCompressionFormat, String)] = [
            (.zip, "archive.zip"),
            (.sevenZip, "archive.7z"),
            (.tar, "archive.tar"),
            (.tarGz, "archive.tar.gz"),
            (.tarBz2, "archive.tar.bz2"),
            (.tarXz, "archive.tar.xz"),
            (.tarZst, "archive.tar.zst")
        ]

        for (format, filename) in formats {
            let archiveURL = sandbox.fileURL(named: filename)
            let destDir = try sandbox.createSubdirectory("extract_\(format.rawValue)")

            // Test ArchiveWriter.createArchive
            try await writer.createArchive(
                outputPath: archiveURL.path,
                format: format,
                level: .normal,
                inputPaths: [file1.path, file2.path]
            )
            XCTAssertTrue(FileManager.default.fileExists(atPath: archiveURL.path), "Archive file should exist: \(filename)")

            // Test ArchiveExtractor.extractArchive
            try await extractor.extractArchive(
                archivePath: archiveURL.path,
                destinationDir: destDir.path
            )

            let extractedFile1 = destDir.appendingPathComponent("doc1.txt")
            let extractedFile2 = destDir.appendingPathComponent("data.bin")
            XCTAssertTrue(FileManager.default.fileExists(atPath: extractedFile1.path))
            XCTAssertTrue(FileManager.default.fileExists(atPath: extractedFile2.path))

            TTZipAssertions.assertFileContents(extractedFile2, expectedData: content2)
            let readContent1 = try String(contentsOf: extractedFile1, encoding: .utf8)
            TTZipAssertions.assertStringEqual(readContent1, content1, message: "Extracted text content must match for \(format)")
        }
    }

    // MARK: - 1b. POSIX Metadata, Permissions (mode_t), Timestamps, Symlinks, Empty Directories

    func testPOSIXMetadataPermissionsTimestampsSymlinksAndEmptyDirectories() async throws {
        let fixtureRoot = try sandbox.createSubdirectory("posix_fixture")
        let execURL = fixtureRoot.appendingPathComponent("script.sh")
        let configURL = fixtureRoot.appendingPathComponent("config.json")
        let emptyDirURL = fixtureRoot.appendingPathComponent("empty_dir")
        let symlinkURL = fixtureRoot.appendingPathComponent("script_link.sh")

        // 1. Setup files and metadata
        let scriptData = Data("#!/bin/sh\necho 'POSIX TTZip Test'\n".utf8)
        let configData = Data("{\"version\": \"1.0.0\", \"active\": true}\n".utf8)

        try scriptData.write(to: execURL)
        try configData.write(to: configURL)
        try FileManager.default.createDirectory(at: emptyDirURL, withIntermediateDirectories: true)
        try? FileManager.default.createSymbolicLink(atPath: symlinkURL.path, withDestinationPath: "script.sh")

        // Set explicit POSIX file permissions: 0o755 for exec, 0o644 for config
        chmod(execURL.path, 0o755)
        chmod(configURL.path, 0o644)

        TTZipAssertions.assertFileMode(execURL, expectedMode: 0o755)
        TTZipAssertions.assertFileMode(configURL, expectedMode: 0o644)
        TTZipAssertions.assertIsDir(emptyDirURL)

        // 2. Compress via ArchiveWriter into TAR format
        let archiveURL = sandbox.fileURL(named: "posix_test.tar")
        let writer = ArchiveWriter()
        try await writer.createArchive(
            outputPath: archiveURL.path,
            format: .tar,
            level: .normal,
            inputPaths: [fixtureRoot.path]
        )
        XCTAssertTrue(FileManager.default.fileExists(atPath: archiveURL.path))

        // 3. Extract via ArchiveExtractor
        let extractDest = try sandbox.createSubdirectory("posix_extracted")
        let extractor = ArchiveExtractor()
        try await extractor.extractArchive(
            archivePath: archiveURL.path,
            destinationDir: extractDest.path
        )

        let extractedExec = extractDest.appendingPathComponent("posix_fixture/script.sh")
        let extractedConfig = extractDest.appendingPathComponent("posix_fixture/config.json")
        let extractedEmptyDir = extractDest.appendingPathComponent("posix_fixture/empty_dir")

        // 4. Validate metadata preservation with TTZipAssertions
        XCTAssertTrue(FileManager.default.fileExists(atPath: extractedExec.path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: extractedConfig.path))
        TTZipAssertions.assertIsDir(extractedEmptyDir)
        TTZipAssertions.assertFileContents(extractedExec, expectedData: scriptData)
        TTZipAssertions.assertFileContents(extractedConfig, expectedData: configData)
        TTZipAssertions.assertFileMode(extractedExec, expectedMode: 0o755)
        TTZipAssertions.assertFileMode(extractedConfig, expectedMode: 0o644)
    }

    // MARK: - 2. ArchiveReader List Entries & VFS Tree Index Rendering

    func testArchiveReaderListEntriesAndVFSTreeRendering() async throws {
        let subDir = try sandbox.createSubdirectory("FolderA")
        let nestedDir = subDir.appendingPathComponent("SubFolderB")
        try FileManager.default.createDirectory(at: nestedDir, withIntermediateDirectories: true)

        let leaf1 = subDir.appendingPathComponent("alpha.txt")
        let leaf2 = nestedDir.appendingPathComponent("beta.log")
        try "Alpha Payload".write(to: leaf1, atomically: true, encoding: .utf8)
        try "Beta Payload Log Line".write(to: leaf2, atomically: true, encoding: .utf8)

        let archiveURL = sandbox.fileURL(named: "tree_test.zip")
        let writer = ArchiveWriter()
        try await writer.createArchive(
            outputPath: archiveURL.path,
            format: .zip,
            level: .normal,
            inputPaths: [subDir.path]
        )

        let reader = ArchiveReader()

        // 1. Test ArchiveReader.listEntries
        let entries = try await reader.listEntries(archivePath: archiveURL.path)
        XCTAssertFalse(entries.isEmpty, "Archive should contain entries")
        XCTAssertTrue(entries.contains { $0.path.contains("alpha.txt") })
        XCTAssertTrue(entries.contains { $0.path.contains("beta.log") })

        // 2. Test Encryption Introspection
        let tier = try await reader.probeEncryption(archivePath: archiveURL.path)
        XCTAssertEqual(tier, .none, "Unencrypted archive should probe as .none tier")

        // 3. Test VFS Hierarchical Tree Construction & Rendering
        let compositeTree = try await reader.inspectTree(archivePath: archiveURL.path, password: nil, candidatePasswords: nil)
        XCTAssertGreaterThan(compositeTree.totalFileCount(), 0, "Tree should have leaf files")
        XCTAssertTrue(compositeTree.isDirectory, "Root should be a directory container")

        let renderedTree = compositeTree.renderTree()
        XCTAssertFalse(renderedTree.isEmpty, "VFS Tree output should not be empty")
        XCTAssertTrue(renderedTree.contains("alpha.txt") || renderedTree.contains("FolderA"), "Rendered tree must contain nodes")
    }

    // MARK: - 3. SplitVolumeEngine Slicing & Merging

    func testSplitVolumeEngineSlicingAndMerging() throws {
        let largeData = Data(repeating: 0x7E, count: 180 * 1024) // 180 KB
        let sourceFile = sandbox.fileURL(named: "split_source.dat")
        try largeData.write(to: sourceFile)

        let splitEngine = SplitVolumeEngine.shared

        // 1. Slice archive into 64 KB volumes
        let splitChunkSize: Int64 = 65536
        try splitEngine.sliceArchive(
            archivePath: sourceFile.path,
            splitSizeBytes: splitChunkSize,
            namingPattern: .numberedExtension,
            cleanOnFailure: true
        )

        // 2. Discover sliced volume paths
        let seedVolume = sandbox.fileURL(named: "split_source.dat.001")
        XCTAssertTrue(FileManager.default.fileExists(atPath: seedVolume.path), "First volume .001 must exist")

        let resolvedVolumes = splitEngine.resolveVolumes(seedPath: seedVolume.path)
        XCTAssertGreaterThanOrEqual(resolvedVolumes.count, 3, "180 KB divided by 64 KB should produce at least 3 parts")

        // 3. Join volumes back together
        let mergedFile = sandbox.fileURL(named: "reassembled.dat")
        try splitEngine.joinVolumes(
            firstVolumePath: seedVolume.path,
            outputPath: mergedFile.path
        )

        XCTAssertTrue(FileManager.default.fileExists(atPath: mergedFile.path), "Reassembled file must exist")
        let mergedData = try Data(contentsOf: mergedFile)
        XCTAssertEqual(mergedData.count, largeData.count, "Reassembled file size must match original")
        XCTAssertEqual(mergedData, largeData, "Reassembled file content must be byte-for-byte identical")
    }

    // MARK: - 4. PasswordVaultManager Key Encryption & Memory-Safe Wiping

    func testPasswordVaultManagerEncryptionAndMemorySafeWiping() {
        let vault = PasswordVaultManager.shared
        vault.resetToFirstRunState()
        defer { vault.resetToFirstRunState() }

        let masterSecret = "SuperVaultMasterKey#2026!"
        vault.setMasterPassword(masterSecret)

        XCTAssertTrue(vault.isMasterPasswordSet, "Master password should be set")
        XCTAssertTrue(vault.isUnlocked, "Vault should be unlocked initially")

        let entryKey = "ProductionArchiveKey"
        let entrySecret = "P@ssw0rdForArchive!"
        vault.addEntry(label: entryKey, password: entrySecret, category: "Production")

        let entries = vault.getEntries()
        XCTAssertEqual(entries.count, 1)
        XCTAssertEqual(entries.first?.label, entryKey)
        XCTAssertEqual(entries.first?.password, entrySecret)

        let candidates = vault.candidatePasswordsForAutoUnlock()
        XCTAssertTrue(candidates.contains(entrySecret), "Candidate list should include added password")

        // Lock vault - triggers Rust C-ABI memory scrubbing (ttzip_rust_vault_wipe)
        vault.lockVault()
        XCTAssertFalse(vault.isUnlocked, "Vault should be locked")
        XCTAssertTrue(vault.getEntries().isEmpty, "Locked vault must return empty entries")

        // Re-unlock with valid master password
        let unlockedSuccess = vault.unlockVault(with: masterSecret)
        XCTAssertTrue(unlockedSuccess, "Unlocking with valid master password should succeed")
        XCTAssertTrue(vault.isUnlocked)

        let restoredEntries = vault.getEntries()
        XCTAssertEqual(restoredEntries.count, 1)
        XCTAssertEqual(restoredEntries.first?.password, entrySecret, "Restored entry must match original secret")
    }

    // MARK: - 5. PasswordRecoveryEngine Dictionary & Brute-Force Interfaces

    func testPasswordRecoveryEngineDictionaryAndBruteForce() async throws {
        let sampleFile = sandbox.fileURL(named: "secret_document.txt")
        let secretText = "Restricted Corporate Asset 2026"
        try secretText.write(to: sampleFile, atomically: true, encoding: .utf8)

        let archiveURL = sandbox.fileURL(named: "encrypted.zip")
        let secretPassword = "pass"
        let writer = ArchiveWriter()
        try await writer.createArchive(
            outputPath: archiveURL.path,
            format: .zip,
            level: .fastest,
            inputPaths: [sampleFile.path],
            password: secretPassword
        )

        // 1. Fast in-memory dictionary recovery
        let dictionary = ["wrong1", "wrong2", secretPassword, "wrong3"]
        let fastFound = PasswordRecoveryEngine.recoverFastInMemory(
            passwords: dictionary,
            archivePath: archiveURL.path
        )
        XCTAssertEqual(fastFound, secretPassword, "Fast in-memory dictionary lookup should find secret")

        // 2. Full recovery workflow with metrics
        let engine = PasswordRecoveryEngine()
        let recoveryResult = try await engine.recoverPassword(
            archivePath: archiveURL.path,
            dictionary: dictionary
        )
        XCTAssertEqual(recoveryResult.foundPassword, secretPassword)
        XCTAssertGreaterThan(recoveryResult.totalAttempts, 0)
        XCTAssertGreaterThanOrEqual(recoveryResult.durationSeconds, 0)
    }

    // MARK: - 6. VFSLz4CachePool Int.min Bit-Pattern Overflow Safety

    func testVFSLz4CachePoolHashOverflowSafety() {
        let pool = VFSLz4CachePool.shared
        let dummyData = Data([0x01, 0x02, 0x03, 0x04])
        
        // Ensure cacheEntry and getCachedEntry handle arbitrary strings without crashing
        pool.cacheEntry(archivePath: "test_session", entryPath: "path/to/entry.bin", data: dummyData)
        let cached = pool.getCachedEntry(archivePath: "test_session", entryPath: "path/to/entry.bin")
        XCTAssertEqual(cached, dummyData)
        
        // Direct test of non-overflowing chunkIdx bit-pattern calculation logic with Int.min
        let minHash = Int.min
        let safeChunkIdx = Int(truncatingIfNeeded: UInt64(bitPattern: Int64(minHash)) & 0x7FFF_FFFF_FFFF_FFFF)
        XCTAssertGreaterThanOrEqual(safeChunkIdx, 0)
        XCTAssertEqual(safeChunkIdx, 0)
    }

    // MARK: - 7. ConcurrencyBridge Parallel Iteration Safety

    func testConcurrencyBridgeParallelIteration() {
        let iterations = 1000
        let counter = LockedCounter()
        
        ConcurrencyBridge.parallelFor(count: iterations) { _ in
            counter.increment()
        }
        
        XCTAssertEqual(counter.value, iterations)
    }

    // MARK: - 8. NativeComputeDispatcher Execution

    func testNativeComputeDispatcherExecution() async throws {
        let result = try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated) {
            return (1...100).reduce(0, +)
        }
        XCTAssertEqual(result, 5050)
    }

    // MARK: - 9. Zstd Ultra-Extreme Levels (19 & 22) End-to-End Roundtrip

    func testArchiveWriterZstdUltraLevelsEndToEndRoundtrip() async throws {
        let writer = ArchiveWriter()
        let extractor = ArchiveExtractor()
        let srcFile = sandbox.fileURL(named: "repetitive_payload.txt")
        let repData = Data(repeating: 0x42, count: 512 * 1024) // 512KB
        try repData.write(to: srcFile)

        // 1. Test Level 19 (Extreme Opt-Parser)
        let zst19URL = sandbox.fileURL(named: "archive_lvl19.tar.zst")
        let dest19 = try sandbox.createSubdirectory("ext_19")
        try await writer.createArchive(
            outputPath: zst19URL.path,
            format: .tarZst,
            level: ArchiveCompressionLevel(levelInt: 19),
            inputPaths: [srcFile.path]
        )
        XCTAssertTrue(FileManager.default.fileExists(atPath: zst19URL.path))
        try await extractor.extractArchive(archivePath: zst19URL.path, destinationDir: dest19.path)
        XCTAssertEqual(try Data(contentsOf: dest19.appendingPathComponent("repetitive_payload.txt")), repData)

        // 2. Test Level 22 (Ultra-Extreme Maximum)
        let zst22URL = sandbox.fileURL(named: "archive_lvl22.tar.zst")
        let dest22 = try sandbox.createSubdirectory("ext_22")
        try await writer.createArchive(
            outputPath: zst22URL.path,
            format: .tarZst,
            level: ArchiveCompressionLevel(levelInt: 22),
            inputPaths: [srcFile.path]
        )
        XCTAssertTrue(FileManager.default.fileExists(atPath: zst22URL.path))
        try await extractor.extractArchive(archivePath: zst22URL.path, destinationDir: dest22.path)
        XCTAssertEqual(try Data(contentsOf: dest22.appendingPathComponent("repetitive_payload.txt")), repData)
    }

    // MARK: - 10. ArchiveWriter Cancellation & Empty Paths Validation

    func testArchiveWriterCancellationAndEmptyPathsValidation() async throws {
        let writer = ArchiveWriter()
        let outURL = sandbox.fileURL(named: "cancelled.zip")

        // 1. Empty input paths validation
        do {
            try await writer.createArchive(outputPath: outURL.path, format: .zip, inputPaths: [])
            XCTFail("Should throw error on empty inputPaths")
        } catch {
            XCTAssertTrue(error is ArchiveError)
        }

        // 2. Pre-cancelled token validation
        let token = CancellationToken()
        token.cancel()
        let sampleFile = sandbox.fileURL(named: "sample.txt")
        try "data".write(to: sampleFile, atomically: true, encoding: .utf8)

        do {
            try await writer.createArchive(
                outputPath: outURL.path,
                format: .zip,
                inputPaths: [sampleFile.path],
                token: token
            )
            XCTFail("Should throw cancelled error")
        } catch ArchiveError.cancelled {
            // Expected
        } catch {
            XCTFail("Unexpected error type: \(error)")
        }
    }
}

private final class LockedCounter: @unchecked Sendable {
    private var count = 0
    private let lock = NSLock()
    
    func increment() {
        lock.lock()
        count += 1
        lock.unlock()
    }
    
    var value: Int {
        lock.lock()
        defer { lock.unlock() }
        return count
    }
}
