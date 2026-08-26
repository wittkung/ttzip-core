// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

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
        let tempUrl = FileManager.default.temporaryDirectory.appendingPathComponent("test_dummy.zip")
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
}
