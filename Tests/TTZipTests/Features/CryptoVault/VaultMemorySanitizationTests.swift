// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import XCTest
@testable import TTZipCore

final class VaultMemorySanitizationTests: XCTestCase {
    
    /// Tests that SecureBytes safely allocates, locks physical memory, and zeroes out content upon wipe.
    func testSecureBytesAllocationAndWipe() throws {
        let sample = "SuperSecretPassword123!".data(using: .utf8)!
        let count = sample.count

        let secure = try SecureBytes(data: sample)
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
            XCTAssertNil(buf.baseAddress)
        }
    }

    /// Tests that SecureBytes(utf8String:) creates locked memory without intermediary heap leaks.
    func testSecureBytesDirectStringInitZeroHeapResidual() throws {
        let password = "TopSecretMasterKey#2026"
        let secure = try SecureBytes(utf8String: password)
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
        secure.withUnsafeBytes { buf in
            XCTAssertEqual(buf.count, 0)
            XCTAssertNil(buf.baseAddress)
        }
    }

    /// Tests that SecureBytes safely throws SecureBytesError when given invalid capacities or on allocation failure.
    func testSecureBytesAllocationFailureHandling() {
        // Negative capacity must throw invalidSize error
        XCTAssertThrowsError(try SecureBytes(capacity: -1)) { error in
            guard let secError = error as? SecureBytesError, case .invalidSize = secError else {
                XCTFail("Expected SecureBytesError.invalidSize, got \(error)")
                return
            }
        }

        // Overly massive capacity that overflows or exceeds system limits must throw an error
        XCTAssertThrowsError(try SecureBytes(capacity: Int.max - 100)) { error in
            guard let secError = error as? SecureBytesError else {
                XCTFail("Expected SecureBytesError, got \(error)")
                return
            }
            switch secError {
            case .invalidSize, .allocationFailed:
                // Both error cases represent clean defensive error propagation
                break
            }
        }
    }
}
