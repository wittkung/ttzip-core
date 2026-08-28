// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
import CryptoKit
import Darwin
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

    // MARK: - 1. Differential Rollback Integrity

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

    // MARK: - 2. Multi-Volume Split Archive Zero Disk Staging

    func testMultiVolumeSplitArchiveInspectionZeroDiskStaging() throws {
        // 1. Generate split volumes using SplitVolumeEngine
        let sourceFile = tempWorkingDir.appendingPathComponent("large_sample_data.bin")
        let sampleData = TestFileGenerator.generateMachineCode(byteCount: 1024 * 1024, arch: .mixed, seed: 0x55AA)
        try sampleData.write(to: sourceFile)

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
        XCTAssertEqual(reassembledData, sampleData)

        // 3. Verify zero temp file concatenation created in /tmp
        let tmpFiles = (try? FileManager.default.contentsOfDirectory(atPath: "/tmp")) ?? []
        let concatenatedLeaks = tmpFiles.filter { $0.contains("ttzip_split_concat") }
        XCTAssertTrue(concatenatedLeaks.isEmpty)
    }

    // MARK: - 3. Silesia Corpus Compression & Verification Stress

    func testSilesiaCorpusCompressionAndVerificationStress() async throws {
        let dickensURL = try SilesiaFixtureLoader.fileURL(named: "dickens")
        let mozillaURL = try SilesiaFixtureLoader.fileURL(named: "mozilla")

        let archiveURL = tempWorkingDir.appendingPathComponent("silesia_corpus.zip")
        let extractDir = tempWorkingDir.appendingPathComponent("silesia_extracted")

        // 1. Compress Silesia fixtures
        let writer = ArchiveWriter()
        try await writer.createArchive(
            outputPath: archiveURL.path,
            format: .zip,
            level: .fast,
            inputPaths: [dickensURL.path, mozillaURL.path]
        )
        XCTAssertTrue(FileManager.default.fileExists(atPath: archiveURL.path))

        // 2. Extract and verify integrity
        let extractor = ArchiveExtractor()
        try await extractor.extractArchive(
            archivePath: archiveURL.path,
            destinationDir: extractDir.path
        )

        let extractedDickens = extractDir.appendingPathComponent("dickens")
        let extractedMozilla = extractDir.appendingPathComponent("mozilla")

        XCTAssertTrue(FileManager.default.fileExists(atPath: extractedDickens.path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: extractedMozilla.path))

        let originalDickensData = try SilesiaFixtureLoader.mappedData(named: "dickens")
        let originalMozillaData = try SilesiaFixtureLoader.mappedData(named: "mozilla")

        let readDickens = try Data(contentsOf: extractedDickens)
        let readMozilla = try Data(contentsOf: extractedMozilla)

        TTZipAssertions.assertDataEqual(readDickens, originalDickensData, message: "Dickens corpus verification mismatch")
        TTZipAssertions.assertDataEqual(readMozilla, originalMozillaData, message: "Mozilla corpus verification mismatch")
    }

    // MARK: - 4. 10,000+ Dense Micro-Files 6-Layer Deep Tree Stress

    func testTenThousandDenseMicroFilesTreeStress() async throws {
        let inputTreeDir = tempWorkingDir.appendingPathComponent("tree_10k_input")
        let extractDir = tempWorkingDir.appendingPathComponent("tree_10k_extracted")
        let archiveURL = tempWorkingDir.appendingPathComponent("tree_10k_archive.zip")

        let targetFileCount = 10_000
        TTLogger.debug("Generating \(targetFileCount) multi-modal micro-files across 6 directory layers...")
        let startTime = CFAbsoluteTimeGetCurrent()

        let generatedURLs = try TestFileGenerator.createMultiModalFileTree(
            in: inputTreeDir,
            totalFiles: targetFileCount,
            maxDepth: 6,
            minFileSize: 512,
            maxFileSize: 4096
        )
        let genDuration = CFAbsoluteTimeGetCurrent() - startTime
        XCTAssertEqual(generatedURLs.count, targetFileCount)
        TTLogger.debug("Generation complete in \(String(format: "%.3f", genDuration)) s")

        // 1. Compress entire 6-layer 10,000-file directory tree via ArchiveWriter
        let writer = ArchiveWriter()
        let compressStartTime = CFAbsoluteTimeGetCurrent()
        try await writer.createArchive(
            outputPath: archiveURL.path,
            format: .zip,
            level: .fast,
            inputPaths: [inputTreeDir.path]
        )
        let compressDuration = CFAbsoluteTimeGetCurrent() - compressStartTime
        XCTAssertTrue(FileManager.default.fileExists(atPath: archiveURL.path))
        let archiveAttrs = try FileManager.default.attributesOfItem(atPath: archiveURL.path)
        let archiveSize = archiveAttrs[.size] as? Int64 ?? 0
        TTLogger.debug("Compressed \(targetFileCount) files into \(archiveSize / 1024) KB in \(String(format: "%.3f", compressDuration)) s")

        // 2. Inspect Central Directory structure via ArchiveReader & verify entry count
        let reader = ArchiveReader()
        let entries = try await reader.inspect(archivePath: archiveURL.path)
        XCTAssertGreaterThanOrEqual(entries.count, targetFileCount, "Archive Central Directory must index all generated files and subdirectories")

        // 3. Extract via ArchiveExtractor
        let extractor = ArchiveExtractor()
        let extractStartTime = CFAbsoluteTimeGetCurrent()
        let extractedBytes = try await extractor.extractArchive(
            archivePath: archiveURL.path,
            destinationDir: extractDir.path
        )
        let extractDuration = CFAbsoluteTimeGetCurrent() - extractStartTime
        XCTAssertGreaterThan(extractedBytes, 0)
        TTLogger.debug("Extracted \(extractedBytes / 1024) KB in \(String(format: "%.3f", extractDuration)) s")

        // 4. Verify sampled file content integrity across multiple directory depths
        var prng = TestFileGenerator.FastPRNG(seed: 0xABCD_1234)
        for _ in 0..<100 {
            let sampleIdx = prng.nextInt(in: 0...(generatedURLs.count - 1))
            let origURL = generatedURLs[sampleIdx]
            let relativePath = origURL.path.replacingOccurrences(of: inputTreeDir.path + "/", with: "")
            
            // Check extracted location (either nested under root folder name or directly extracted)
            let directURL = extractDir.appendingPathComponent(relativePath)
            let nestedURL = extractDir.appendingPathComponent(inputTreeDir.lastPathComponent).appendingPathComponent(relativePath)
            let targetURL = FileManager.default.fileExists(atPath: directURL.path) ? directURL : nestedURL

            XCTAssertTrue(FileManager.default.fileExists(atPath: targetURL.path), "File missing at extracted path: \(targetURL.path)")
            let origData = try Data(contentsOf: origURL)
            let extractedData = try Data(contentsOf: targetURL)
            TTZipAssertions.assertDataEqual(extractedData, origData, message: "Sampled file mismatch for \(relativePath)")
        }
    }

    // MARK: - 5. 1GB APFS Sparse Large File Streaming Pipeline Stress

    func testOneGigabyteSparseFileStreamingPipelineStress() async throws {
        let sparseFileURL = tempWorkingDir.appendingPathComponent("sparse_1gb_payload.img")
        let archiveURL = tempWorkingDir.appendingPathComponent("sparse_1gb.zip")
        let extractDir = tempWorkingDir.appendingPathComponent("sparse_1gb_extracted")

        let oneGigabyte: Int64 = 1024 * 1024 * 1024 // 1GB
        TTLogger.debug("Allocating 1GB APFS sparse file with non-allocated sparse holes...")
        let startTime = CFAbsoluteTimeGetCurrent()

        let sparseInfo = try TestFileGenerator.createSparseHoleFile(
            at: sparseFileURL,
            logicalSizeBytes: oneGigabyte,
            holeIntervalBytes: 64 * 1024 * 1024,
            chunkSizeBytes: 64 * 1024
        )
        let allocDuration = CFAbsoluteTimeGetCurrent() - startTime
        XCTAssertEqual(sparseInfo.logicalSize, oneGigabyte, "Sparse file logical size must be exactly 1GB")
        XCTAssertLessThan(sparseInfo.allocatedPhysicalSize, 50 * 1024 * 1024, "APFS physical allocated blocks must be minimal (< 50MB)")
        TTLogger.debug("1GB sparse file allocated in \(String(format: "%.3f", allocDuration)) s (Physical: \(sparseInfo.allocatedPhysicalSize / 1024) KB)")

        // 1. Stream-compress the 1GB sparse file
        let writer = ArchiveWriter()
        let compressStartTime = CFAbsoluteTimeGetCurrent()
        try await writer.createArchive(
            outputPath: archiveURL.path,
            format: .zip,
            level: .fast,
            inputPaths: [sparseFileURL.path]
        )
        let compressDuration = CFAbsoluteTimeGetCurrent() - compressStartTime
        XCTAssertTrue(FileManager.default.fileExists(atPath: archiveURL.path))

        let archiveAttrs = try FileManager.default.attributesOfItem(atPath: archiveURL.path)
        let archiveSize = archiveAttrs[.size] as? Int64 ?? 0
        XCTAssertLessThan(archiveSize, 50 * 1024 * 1024, "Compressed sparse archive must be compact (< 50MB)")
        TTLogger.debug("1GB sparse archive created (\(archiveSize / 1024) KB) in \(String(format: "%.3f", compressDuration)) s")

        // 2. Stream-extract the archive
        let extractor = ArchiveExtractor()
        let extractStartTime = CFAbsoluteTimeGetCurrent()
        let extractedBytes = try await extractor.extractArchive(
            archivePath: archiveURL.path,
            destinationDir: extractDir.path
        )
        let extractDuration = CFAbsoluteTimeGetCurrent() - extractStartTime
        XCTAssertGreaterThanOrEqual(extractedBytes, oneGigabyte)
        TTLogger.debug("1GB sparse archive extracted in \(String(format: "%.3f", extractDuration)) s")

        // 3. Verify extracted file logical size and boundary chunk SHA-256 signatures
        let extractedFile = extractDir.appendingPathComponent(sparseFileURL.lastPathComponent)
        XCTAssertTrue(FileManager.default.fileExists(atPath: extractedFile.path))

        let extractedAttrs = try FileManager.default.attributesOfItem(atPath: extractedFile.path)
        let extractedSize = extractedAttrs[.size] as? Int64 ?? 0
        XCTAssertEqual(extractedSize, oneGigabyte, "Extracted file logical size must be exactly 1GB")

        // Verify header 64KB chunk
        let fileHandle = try FileHandle(forReadingFrom: extractedFile)
        defer { try? fileHandle.close() }

        let readHeaderData = fileHandle.readData(ofLength: 64 * 1024)
        let readHeaderHash = SHA256.hash(data: readHeaderData).compactMap { String(format: "%02x", $0) }.joined()
        XCTAssertEqual(readHeaderHash, sparseInfo.headerSignature, "Extracted header signature mismatch")

        // Verify footer 64KB chunk
        try fileHandle.seek(toOffset: UInt64(oneGigabyte - 64 * 1024))
        let readFooterData = fileHandle.readData(ofLength: 64 * 1024)
        let readFooterHash = SHA256.hash(data: readFooterData).compactMap { String(format: "%02x", $0) }.joined()
        XCTAssertEqual(readFooterHash, sparseInfo.footerSignature, "Extracted footer signature mismatch")
    }

    // MARK: - 6. Multi-Core Concurrent Read/Write I/O Contention Stress

    func testMultiCoreConcurrentReadWriteIOContentionStress() async throws {
        let concurrency = 16
        let workerDir = tempWorkingDir.appendingPathComponent("concurrent_io_workers")
        try FileManager.default.createDirectory(at: workerDir, withIntermediateDirectories: true)

        // Pre-create a shared seed archive for reader workers
        let seedDir = workerDir.appendingPathComponent("seed_payload")
        try FileManager.default.createDirectory(at: seedDir, withIntermediateDirectories: true)
        let seedFiles = try TestFileGenerator.createMultiModalFileTree(
            in: seedDir,
            totalFiles: 200,
            maxDepth: 3,
            minFileSize: 1024,
            maxFileSize: 8192
        )
        let seedArchiveURL = workerDir.appendingPathComponent("shared_seed.zip")
        let writer = ArchiveWriter()
        try await writer.createArchive(
            outputPath: seedArchiveURL.path,
            format: .zip,
            level: .fast,
            inputPaths: seedFiles.map(\.path)
        )

        TTLogger.debug("Launching \(concurrency) concurrent multi-modal I/O tasks across all CPU cores...")
        let startTime = CFAbsoluteTimeGetCurrent()

        try await withThrowingTaskGroup(of: Int.self) { group in
            for workerId in 0..<concurrency {
                group.addTask {
                    let taskSubdir = workerDir.appendingPathComponent("worker_\(workerId)")
                    try FileManager.default.createDirectory(at: taskSubdir, withIntermediateDirectories: true)

                    switch workerId % 4 {
                    case 0:
                        // Worker type 0: Multi-modal compression stress
                        let inputDir = taskSubdir.appendingPathComponent("input")
                        let files = try TestFileGenerator.createMultiModalFileTree(
                            in: inputDir,
                            totalFiles: 100,
                            maxDepth: 3,
                            minFileSize: 512,
                            maxFileSize: 2048
                        )
                        let outZip = taskSubdir.appendingPathComponent("compressed.zip")
                        let taskWriter = ArchiveWriter()
                        try await taskWriter.createArchive(
                            outputPath: outZip.path,
                            format: .zip,
                            level: .fast,
                            inputPaths: files.map(\.path)
                        )
                        XCTAssertTrue(FileManager.default.fileExists(atPath: outZip.path))
                        return 1

                    case 1:
                        // Worker type 1: Concurrent archive extraction
                        let extractDest = taskSubdir.appendingPathComponent("extracted")
                        let taskExtractor = ArchiveExtractor()
                        let extractedBytes = try await taskExtractor.extractArchive(
                            archivePath: seedArchiveURL.path,
                            destinationDir: extractDest.path
                        )
                        XCTAssertGreaterThan(extractedBytes, 0)
                        return 2

                    case 2:
                        // Worker type 2: Concurrent ArchiveReader inspection and metadata queries
                        let taskReader = ArchiveReader()
                        let entries = try await taskReader.inspect(archivePath: seedArchiveURL.path)
                        XCTAssertEqual(entries.count, 200)
                        return 3

                    default:
                        // Worker type 3: Parallel stream hashing & vault password generator contention
                        let hashCalc = HashCalculator()
                        let hashVal = try await hashCalc.computeHash(filePath: seedArchiveURL.path, type: .sha256)
                        XCTAssertFalse(hashVal.isEmpty)

                        for _ in 0..<10 {
                            let pwd = PasswordVaultManager.shared.generateRandomPassword(length: 32, includeSymbols: true)
                            let evaluation = PasswordVaultManager.shared.evaluatePasswordStrength(pwd)
                            XCTAssertGreaterThan(evaluation.score, 0)
                        }
                        return 4
                    }
                }
            }

            var completedCount = 0
            for try await _ in group {
                completedCount += 1
            }
            XCTAssertEqual(completedCount, concurrency, "All \(concurrency) concurrent I/O tasks must complete successfully")
        }

        let elapsed = CFAbsoluteTimeGetCurrent() - startTime
        TTLogger.debug("All \(concurrency) concurrent tasks successfully executed in \(String(format: "%.3f", elapsed)) s without race conditions or deadlocks")
    }
}
