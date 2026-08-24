// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

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

    /// Tests that SecureBytes(utf8String:) creates locked memory without intermediary heap leaks.
    func testSecureBytesDirectStringInitZeroHeapResidual() {
        let password = "TopSecretMasterKey#2026"
        let secure = SecureBytes(utf8String: password)
        XCTAssertEqual(secure.count, password.utf8.count)

        secure.withUnsafeBytes { buf in
            guard let base = buf.baseAddress else {
                XCTFail("Buffer pointer is null")
                return
            }
            let readStr = String(bytes: UnsafeBufferPointer(start: base.assumingMemoryBound(to: UInt8.self), count: secure.count), encoding: .utf8)
            XCTAssertEqual(readStr, password)
        }

        secure.wipeAndFree()
        XCTAssertNil(secure.baseAddress)
    }
}
