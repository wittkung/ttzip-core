// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import XCTest
@testable import TTZipCore

final class UniFFIBindingsTests: XCTestCase {
    func testEntropyEstimationViaUniFFI() {
        let textData = Data("Hello World Mozilla UniFFI Direct Rust Binding!".utf8)
        let entropy = estimateShannonEntropy(data: textData)
        XCTAssertGreaterThan(entropy, 2.0)
        XCTAssertLessThanOrEqual(entropy, 8.0)
    }

    func testCodecRecommendationViaUniFFI() {
        let compressible = Data(repeating: UInt8(42), count: 1024)
        let rec = recommendCodec(data: compressible, scenario: 0)
        XCTAssertFalse(rec.isEmpty)
    }

    func testCancellationTokenLifecycle() {
        let token = CancellationToken()
        XCTAssertFalse(token.isCancelled())
        token.cancel()
        XCTAssertTrue(token.isCancelled())
    }

    func testArchiveFormatDetection() {
        let tempUrl = FileManager.default.temporaryDirectory.appendingPathComponent("test_dummy_\(UUID().uuidString).zip")
        let zipMagic: [UInt8] = [0x50, 0x4B, 0x03, 0x04, 0x00, 0x00]
        try? Data(zipMagic).write(to: tempUrl)
        defer { try? FileManager.default.removeItem(at: tempUrl) }

        do {
            let format = try detectArchiveFormat(path: tempUrl.path)
            XCTAssertEqual(format, .zip)
        } catch {
            XCTFail("Failed to detect format: \(error)")
        }
    }

    func testVfsTreeBuildingAndFuzzySearch() {
        let entries = [
            UniFfiEntryMetadata(
                path: "Photos/2026/Vacation/beach.png",
                uncompressedSize: 2048576,
                compressedSize: 1024288,
                crc32: 0x12345678,
                mtimeEpochSecs: 1770000000,
                mode: 0o644,
                isDirectory: false,
                isEncrypted: false,
                compressionMethod: "deflate",
                detectedEncoding: "UTF-8"
            ),
            UniFfiEntryMetadata(
                path: "Documents/Financial/report_q1.pdf",
                uncompressedSize: 512000,
                compressedSize: 256000,
                crc32: 0x87654321,
                mtimeEpochSecs: 1770000000,
                mode: 0o644,
                isDirectory: false,
                isEncrypted: false,
                compressionMethod: "deflate",
                detectedEncoding: "UTF-8"
            )
        ]

        let vfs = UniFfiVfsTree.build(entries: entries, rootName: "ArchiveRoot")
        XCTAssertEqual(vfs.totalEntries(), 2)

        let matches = vfs.search(query: "beach", maxResults: 10)
        XCTAssertFalse(matches.isEmpty)
        XCTAssertEqual(matches.first?.name, "beach.png")
        XCTAssertEqual(matches.first?.size, 2048576)
    }

    func testFileSha256AndCrc32() {
        let tempUrl = FileManager.default.temporaryDirectory.appendingPathComponent("test_crypto_\(UUID().uuidString).bin")
        let payload = Data("The quick brown fox jumps over the lazy dog".utf8)
        try? payload.write(to: tempUrl)
        defer { try? FileManager.default.removeItem(at: tempUrl) }

        do {
            let sha = try computeFileSha256(filePath: tempUrl.path)
            XCTAssertEqual(sha, "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592")

            let crc = try computeFileCrc32(filePath: tempUrl.path)
            XCTAssertEqual(crc, 0x414FA339)
        } catch {
            XCTFail("Crypto computation failed: \(error)")
        }
    }

    func testArchiveCreationAndInspectionViaUniFFI() {
        let fm = FileManager.default
        let tempDir = fm.temporaryDirectory.appendingPathComponent("ttzip_uniffi_test_\(UUID().uuidString)")
        try? fm.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: tempDir) }

        let sampleFile = tempDir.appendingPathComponent("hello.txt")
        try? Data("UniFFI Direct Engine Test Content".utf8).write(to: sampleFile)

        let archivePath = tempDir.appendingPathComponent("output.zip").path
        do {
            let report = try createArchiveStream(
                sourcePaths: [sampleFile.path],
                outputPath: archivePath,
                format: ArchiveFormat.zip,
                level: 6,
                password: nil,
                progress: nil,
                token: nil
            )
            XCTAssertGreaterThan(report.compressedBytes, 0)

            let entries = try inspectArchiveEntries(archivePath: archivePath, password: nil)
            XCTAssertFalse(entries.isEmpty)
            XCTAssertTrue(entries.contains { $0.path.contains("hello.txt") })

            let extractDir = tempDir.appendingPathComponent("extracted")
            let extractReport = try extractArchiveStream(
                archivePath: archivePath,
                destinationDir: extractDir.path,
                password: nil,
                progress: nil,
                token: nil
            )
            XCTAssertGreaterThan(extractReport.uncompressedBytes, 0)
            XCTAssertTrue(fm.fileExists(atPath: extractDir.appendingPathComponent("hello.txt").path))
        } catch {
            XCTFail("UniFFI archive pipeline failed: \(error)")
        }
    }

    func testSliceArchiveFileAndJoinVolumes() throws {
        let fm = FileManager.default
        let tempDir = fm.temporaryDirectory.appendingPathComponent("ttzip_split_test_\(UUID().uuidString)")
        try? fm.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: tempDir) }

        let originalFile = tempDir.appendingPathComponent("payload.dat")
        var originalData = Data()
        for i in 0..<5000 {
            originalData.append(UInt8(i % 256))
        }
        try originalData.write(to: originalFile)

        // 1. Slice using SplitVolumeEngine backed by Rust UniFFI
        try SplitVolumeEngine.shared.sliceArchive(
            archivePath: originalFile.path,
            splitSizeBytes: 1000,
            namingPattern: .numberedExtension
        )

        let volumes = SplitVolumeEngine.shared.resolveVolumes(seedPath: "\(originalFile.path).001")
        XCTAssertEqual(volumes.count, 5)

        // 2. Join volumes back
        let restoredFile = tempDir.appendingPathComponent("restored.dat")
        try SplitVolumeEngine.shared.joinVolumes(
            firstVolumePath: "\(originalFile.path).001",
            outputPath: restoredFile.path
        )

        let restoredData = try Data(contentsOf: restoredFile)
        XCTAssertEqual(restoredData, originalData)
    }

    func testVfsPagedChildrenAndTotalCount() {
        var entries: [ArchiveEntry] = []
        for i in 0..<30 {
            entries.append(ArchiveEntry(
                path: String(format: "Folder/item_%02d.txt", i),
                uncompressedSize: 100,
                isDirectory: false,
                detectedEncoding: "UTF-8",
                modificationDate: Date(),
                isEncrypted: false
            ))
        }

        guard let session = RustVfsSession(entries: entries, rootName: "Root") else {
            XCTFail("Failed to initialize session")
            return
        }

        let paged = session.getChildrenPaged(subpath: "Folder", offset: 10, limit: 5)
        XCTAssertEqual(paged.nodes.count, 5)
        XCTAssertEqual(paged.total, 30)
        XCTAssertEqual(paged.nodes.first?.name, "item_10.txt")
    }

    func testNaturalCompareAndSort() {
        let unsorted = ["file10.txt", "file2.txt", "file1.txt", "file20.txt"]
        let sorted = NativeMicrokernelBridge.naturalSort(unsorted)
        XCTAssertEqual(sorted, ["file1.txt", "file2.txt", "file10.txt", "file20.txt"])

        XCTAssertEqual(NativeMicrokernelBridge.naturalCompare("v1.2.0", "v1.10.0"), .orderedAscending)
        XCTAssertEqual(NativeMicrokernelBridge.naturalCompare("v1.10.0", "v1.2.0"), .orderedDescending)
        XCTAssertEqual(NativeMicrokernelBridge.naturalCompare("abc", "abc"), .orderedSame)
    }

    func testComputeFileStreamHashAllAlgorithms() throws {
        let tempUrl = FileManager.default.temporaryDirectory.appendingPathComponent("test_stream_hash_\(UUID().uuidString).txt")
        let testData = Data("123456789".utf8)
        try testData.write(to: tempUrl)
        defer { try? FileManager.default.removeItem(at: tempUrl) }

        // CRC32
        let crc32 = try computeFileStreamHash(filePath: tempUrl.path, algorithm: "crc32")
        XCTAssertEqual(crc32, "CBF43926")

        // Adler32
        let adler32 = try computeFileStreamHash(filePath: tempUrl.path, algorithm: "adler32")
        XCTAssertEqual(adler32, "091E01DE")

        // CRC64
        let crc64 = try computeFileStreamHash(filePath: tempUrl.path, algorithm: "crc64")
        XCTAssertEqual(crc64, "b86883e6fa710a9f")

        // SHA-256
        let sha256 = try computeFileStreamHash(filePath: tempUrl.path, algorithm: "sha256")
        XCTAssertEqual(sha256, "15e2b0d3c33891ebb0f1ef609ec419420c20e320ce94c65fbc8c3312448eb225")

        // MD5
        let md5 = try computeFileStreamHash(filePath: tempUrl.path, algorithm: "md5")
        XCTAssertEqual(md5, "25f9e794323b453885f5181f1b624d0b")
    }

    func testArchiveStandardsComplianceViaUniFFI() throws {
        let tempUrl = FileManager.default.temporaryDirectory.appendingPathComponent("test_compliance_\(UUID().uuidString).zip")
        let eocd: [UInt8] = [
            0x50, 0x4B, 0x05, 0x06,
            0x00, 0x00,
            0x00, 0x00,
            0x00, 0x00,
            0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00
        ]
        try Data(eocd).write(to: tempUrl)
        defer { try? FileManager.default.removeItem(at: tempUrl) }

        let report = try checkArchiveStandardsCompliance(archivePath: tempUrl.path)
        XCTAssertEqual(report.formatName, "Zip")
        XCTAssertTrue(report.isCompliant)
        XCTAssertTrue(report.violations.isEmpty)
    }

    func testStandardsComplianceCheckerIntegration() throws {
        let tempUrl = FileManager.default.temporaryDirectory.appendingPathComponent("test_checker_\(UUID().uuidString).zip")
        let eocd: [UInt8] = [
            0x50, 0x4B, 0x05, 0x06,
            0x00, 0x00,
            0x00, 0x00,
            0x00, 0x00,
            0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00
        ]
        try Data(eocd).write(to: tempUrl)
        defer { try? FileManager.default.removeItem(at: tempUrl) }

        // Test checkCompliance with fileURL
        let report = try StandardsComplianceChecker.checkCompliance(fileURL: tempUrl)
        XCTAssertEqual(report.format, .zip)
        XCTAssertTrue(report.isCompliant)
        XCTAssertTrue(report.violations.isEmpty)

        // Test checkComplianceNative
        let nativeRes = StandardsComplianceChecker.checkComplianceNative(fileURL: tempUrl)
        XCTAssertTrue(nativeRes.isCompliant)
        XCTAssertNotNil(nativeRes.reportJson)
        XCTAssertTrue(nativeRes.reportJson?.contains("\"isCompliant\" : true") == true)

        // Test non-existent file
        let nonExistentURL = URL(fileURLWithPath: "/tmp/non_existent_archive_\(UUID().uuidString).zip")
        let missingReport = try StandardsComplianceChecker.checkCompliance(fileURL: nonExistentURL)
        XCTAssertFalse(missingReport.isCompliant)
        XCTAssertFalse(missingReport.violations.isEmpty)
    }

    func testPlatformPathSanitizerWithMicrokernel() {
        // Normal relative path
        let clean = PlatformPathSanitizer.sanitize(path: "documents/photos/summer.jpg")
        XCTAssertEqual(clean.normalizedPath, "documents/photos/summer.jpg")
        XCTAssertFalse(clean.hasTraversalAttack)
        XCTAssertFalse(clean.isAbsolute)
        XCTAssertNil(clean.strippedAlternateDataStream)

        // ZipSlip path traversal
        let traversal = PlatformPathSanitizer.sanitize(path: "../../etc/shadow")
        XCTAssertTrue(traversal.hasTraversalAttack)

        // Windows Alternate Data Stream (ADS)
        let ads = PlatformPathSanitizer.sanitize(path: "payload.txt:hidden_stream")
        XCTAssertNotNil(ads.strippedAlternateDataStream)
        XCTAssertEqual(ads.strippedAlternateDataStream, ":hidden_stream")
        XCTAssertEqual(ads.normalizedPath, "payload.txt")

        // Windows DOS reserved device name
        let reserved = PlatformPathSanitizer.sanitize(path: "NUL")
        XCTAssertTrue(reserved.containsWindowsReservedDeviceName)
    }

    func testMicrokernelDecompressionLimits() {
        // Safe entry: 2 MB decompressed from 1 MB compressed (2:1 ratio)
        let safe = MicrokernelDecompressionLimits.isDecompressionBomb(
            uncompressedSize: 2 * 1024 * 1024,
            compressedSize: 1024 * 1024
        )
        XCTAssertFalse(safe)

        // Malicious entry exceeding defaultSuspiciousRatio (100:1) above threshold (1 MB)
        let bomb = MicrokernelDecompressionLimits.isDecompressionBomb(
            uncompressedSize: 200 * 1024 * 1024,
            compressedSize: 100 * 1024 // 2000:1 ratio
        )
        XCTAssertTrue(bomb)

        // Small entry below activation threshold (500 KB < 1 MB)
        let small = MicrokernelDecompressionLimits.isDecompressionBomb(
            uncompressedSize: 500 * 1024,
            compressedSize: 100
        )
        XCTAssertFalse(small)

        // Massive entry exceeding defaultMaxEntrySizeBytes (10 GiB)
        let oversized = MicrokernelDecompressionLimits.isDecompressionBomb(
            uncompressedSize: 11 * 1024 * 1024 * 1024,
            compressedSize: 1024 * 1024 * 1024
        )
        XCTAssertTrue(oversized)
    }
}

