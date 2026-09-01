// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class TTZipTextEncodingServiceTests: XCTestCase {

    var service: TTZipTextEncodingService!

    override func setUp() {
        super.setUp()
        service = TTZipTextEncodingService.shared
        service.resetTelemetry()
        service.setEncodingOverride(nil)
    }

    override func tearDown() {
        service.resetTelemetry()
        service.setEncodingOverride(nil)
        super.tearDown()
    }

    // MARK: - Supported Encodings Catalog Tests

    func testSupportedEncodingsCatalog() {
        let encodings = service.supportedEncodings
        XCTAssertFalse(encodings.isEmpty)

        guard let utf8 = encodings.first(where: { $0.name == "UTF-8" }) else {
            return XCTFail("Missing UTF-8 in catalog")
        }
        XCTAssertTrue(utf8.isUnicode)
        XCTAssertFalse(utf8.isCJK)
        XCTAssertFalse(utf8.isSingleByte)

        guard let gb18030 = encodings.first(where: { $0.name == "GB18030" }) else {
            return XCTFail("Missing GB18030 in catalog")
        }
        XCTAssertFalse(gb18030.isUnicode)
        XCTAssertTrue(gb18030.isCJK)

        guard let shiftJIS = encodings.first(where: { $0.name == "Shift_JIS" }) else {
            return XCTFail("Missing Shift_JIS in catalog")
        }
        XCTAssertTrue(shiftJIS.isCJK)

        guard let win1252 = encodings.first(where: { $0.name == "windows-1252" }) else {
            return XCTFail("Missing windows-1252 in catalog")
        }
        XCTAssertTrue(win1252.isSingleByte)
        XCTAssertFalse(win1252.isCJK)
    }

    // MARK: - Encoding Sniffing & Detection Tests

    func testAsciiAndUtf8Detection() {
        let asciiData = "hello_world_test.zip".data(using: .utf8)!
        let detectedAscii = service.detectEncoding(data: asciiData)
        XCTAssertEqual(detectedAscii.encodingName, "ASCII")
        XCTAssertEqual(detectedAscii.confidence, 1.0)
        XCTAssertTrue(detectedAscii.isLossless)

        let utf8Text = "这是 TTZip 纯原生 UTF-8 探测测试 🚀"
        let utf8Data = utf8Text.data(using: .utf8)!
        let detectedUtf8 = service.detectEncoding(data: utf8Data)
        XCTAssertEqual(detectedUtf8.encodingName, "UTF-8")
        XCTAssertGreaterThan(detectedUtf8.confidence, 0.8)
        XCTAssertTrue(detectedUtf8.isLossless)
    }

    func testGB18030DetectionAndTranscoding() throws {
        let originalText = "中国开源归档压缩引擎发布说明文档.txt"
        let gbkCFEncoding = CFStringConvertEncodingToNSStringEncoding(CFStringEncoding(CFStringEncodings.GB_18030_2000.rawValue))
        let nsEncoding = String.Encoding(rawValue: gbkCFEncoding)

        guard let gbkData = originalText.data(using: nsEncoding) else {
            return XCTFail("Failed to encode test string to GB18030")
        }

        let detected = service.detectEncoding(data: gbkData)
        XCTAssertTrue(detected.encodingName == "GB18030" || detected.encodingName == "GBK")
        XCTAssertGreaterThan(detected.confidence, 0.5)

        let transcoded = try service.transcodeToUTF8(data: gbkData, encodingName: "GB18030")
        XCTAssertEqual(transcoded, originalText)

        let reEncoded = try service.transcodeFromUTF8(text: originalText, encodingName: "GB18030")
        XCTAssertEqual(reEncoded, gbkData)
    }

    func testShiftJISDetectionAndRemediation() {
        let originalText = "日本語ファイル名アーカイブ.zip"
        let sjisCFEncoding = CFStringConvertEncodingToNSStringEncoding(CFStringEncoding(CFStringEncodings.shiftJIS.rawValue))
        let nsEncoding = String.Encoding(rawValue: sjisCFEncoding)

        guard let sjisData = originalText.data(using: nsEncoding) else {
            return XCTFail("Failed to encode test string to Shift_JIS")
        }

        let detected = service.detectEncoding(data: sjisData)
        XCTAssertEqual(detected.encodingName, "Shift_JIS")
        XCTAssertGreaterThan(detected.confidence, 0.5)

        let result = service.remediateFilename(rawBytes: sjisData)
        XCTAssertEqual(result.remediatedName, originalText)
        XCTAssertEqual(result.encodingUsed, "Shift_JIS")
        XCTAssertTrue(result.wasRemediated)
        XCTAssertFalse(result.hasUnmappedChars)
    }

    // MARK: - Batch Filename Remediation Tests

    func testBatchFilenameRemediation() {
        let text1 = "财务报表_2026年Q1.xlsx"
        let text2 = "日次レポート_売上集計.csv"
        let text3 = "plain_english_readme.md"

        let gbkCFEncoding = CFStringConvertEncodingToNSStringEncoding(CFStringEncoding(CFStringEncodings.GB_18030_2000.rawValue))
        let sjisCFEncoding = CFStringConvertEncodingToNSStringEncoding(CFStringEncoding(CFStringEncodings.shiftJIS.rawValue))

        let data1 = text1.data(using: String.Encoding(rawValue: gbkCFEncoding))!
        let data2 = text2.data(using: String.Encoding(rawValue: sjisCFEncoding))!
        let data3 = text3.data(using: .utf8)!

        let results = service.remediateFilenamesBatch(items: [data1, data2, data3])
        XCTAssertEqual(results.count, 3)

        XCTAssertEqual(results[0].remediatedName, text1)
        XCTAssertTrue(results[0].wasRemediated)

        XCTAssertEqual(results[1].remediatedName, text2)
        XCTAssertTrue(results[1].wasRemediated)

        XCTAssertEqual(results[2].remediatedName, text3)
        XCTAssertFalse(results[2].wasRemediated)
    }

    // MARK: - Mojibake UTF-8 Repair Tests

    func testMojibakeRemediation() {
        let originalText = "这是一个被错误按照Windows-1252解码的GBK文件名"
        let gbkCFEncoding = CFStringConvertEncodingToNSStringEncoding(CFStringEncoding(CFStringEncodings.GB_18030_2000.rawValue))
        let gbkData = originalText.data(using: String.Encoding(rawValue: gbkCFEncoding))!

        // Simulate misdecoding as Windows-1252 / ISO Latin 1
        let garbledUtf8 = String(data: gbkData, encoding: .windowsCP1252) ?? String(data: gbkData, encoding: .isoLatin1)!

        let result = service.remediateMojibake(text: garbledUtf8, sourceEncoding: "GB18030")
        XCTAssertEqual(result.remediatedName, originalText)
        XCTAssertTrue(result.wasRemediated)
    }

    // MARK: - ArchiveEntry & Metadata Remediation Tests

    func testArchiveEntryRemediation() {
        let originalPath = "项目资料/技术规范.pdf"
        let gbkCFEncoding = CFStringConvertEncodingToNSStringEncoding(CFStringEncoding(CFStringEncodings.GB_18030_2000.rawValue))
        let gbkData = originalPath.data(using: String.Encoding(rawValue: gbkCFEncoding))!
        let misreadPath = String(data: gbkData, encoding: .isoLatin1)!

        let rawEntry = ArchiveEntry(
            path: misreadPath,
            uncompressedSize: 1024,
            isDirectory: false,
            detectedEncoding: "ISO-8859-1"
        )

        let remediatedEntry = service.remediateArchiveEntry(entry: rawEntry, fallbackEncoding: "GB18030")
        XCTAssertEqual(remediatedEntry.path, originalPath)
        XCTAssertEqual(remediatedEntry.name, "技术规范.pdf")
        XCTAssertEqual(remediatedEntry.detectedEncoding, "GB18030")
        XCTAssertEqual(remediatedEntry.uncompressedSize, 1024)
    }

    func testArchiveMetadataBatchRemediation() {
        let originalPath = "アーカイブ/仕様書.txt"
        let sjisCFEncoding = CFStringConvertEncodingToNSStringEncoding(CFStringEncoding(CFStringEncodings.shiftJIS.rawValue))
        let sjisData = originalPath.data(using: String.Encoding(rawValue: sjisCFEncoding))!
        let misreadPath = String(data: sjisData, encoding: .isoLatin1)!

        let meta = ArchiveEntryMetadata(
            path: misreadPath,
            uncompressedSize: 2048,
            detectedEncoding: "ISO-8859-1"
        )

        let results = service.remediateArchiveMetadataBatch(items: [meta], fallbackEncoding: "Shift_JIS")
        XCTAssertEqual(results.count, 1)
        XCTAssertEqual(results[0].path, originalPath)
        XCTAssertEqual(results[0].detectedEncoding, "Shift_JIS")
    }

    // MARK: - Observable Telemetry & Concurrency Tests

    func testObservableTelemetryTracking() async {
        XCTAssertEqual(service.totalDetectionsCount, 0)
        XCTAssertEqual(service.totalRemediationsCount, 0)

        let _ = await service.detectEncodingAsync(data: "async_test".data(using: .utf8)!)
        XCTAssertEqual(service.totalDetectionsCount, 1)

        let _ = service.remediateFilename(rawBytes: "remediation_test".data(using: .utf8)!)
        XCTAssertEqual(service.totalRemediationsCount, 1)

        service.setEncodingOverride("GB18030")
        XCTAssertEqual(service.activeEncodingOverride, "GB18030")

        service.resetTelemetry()
        XCTAssertEqual(service.totalDetectionsCount, 0)
        XCTAssertEqual(service.totalRemediationsCount, 0)
    }

    func testConcurrentDetections() async {
        guard let localService = self.service else { return }
        await withTaskGroup(of: Void.self) { group in
            for i in 0..<20 {
                group.addTask {
                    let sample = "Concurrent text payload #\(i)"
                    let detected = localService.detectEncoding(data: sample.data(using: .utf8)!)
                    XCTAssertEqual(detected.encodingName, "ASCII")
                }
            }
        }
    }
}
