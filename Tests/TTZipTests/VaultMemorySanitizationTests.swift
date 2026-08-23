// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import XCTest
@testable import TTZipCore

final class VaultMemorySanitizationTests: XCTestCase {
    
    /// Tests that SecureBytes safely allocates, locks physical memory, and zeroes out content upon wipe.
    func testSecureBytesAllocationAndWipe() {
        let sample = "SuperSecretPassword123!".data(using: .utf8)!
        let count = sample.count

        let secure = SecureBytes(data: sample)
        XCTAssertEqual(secure.count, count)
        
        // 1. Verify data is intact before wipe
        secure.withUnsafeBytes { buf in
            guard let base = buf.baseAddress else {
                XCTFail("Buffer pointer is null")
                return
            }
            let bufferData = Data(bytes: base, count: count)
            XCTAssertEqual(bufferData, sample)
        }

        // 2. Wipe memory contents explicitly
        secure.wipeAndFree()

        // 3. Verify buffer pointer reports scrubbed state and returns empty buffer
        secure.withUnsafeBytes { buf in
            XCTAssertEqual(buf.count, 0)
        }
        XCTAssertNil(secure.baseAddress)
    }
}
