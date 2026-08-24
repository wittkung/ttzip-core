// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

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
            XCTFail("detectArchiveFormat threw error: \(error)")
        }
    }
}
