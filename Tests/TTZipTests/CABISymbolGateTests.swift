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
import CTTZipBridge
@testable import TTZipCore

final class CABISymbolGateTests: XCTestCase {

    func testCoreRuntimeAndVersionSymbols() {
        let versionPtr = ttzip_rust_version()
        XCTAssertNotNil(versionPtr)
        let versionStr = String(cString: versionPtr!)
        XCTAssertTrue(versionStr.contains("rust-engine") || versionStr.contains("1.0.0"))

        let initStatus = ttzip_rust_init()
        XCTAssertEqual(initStatus, TTZIP_STATUS_OK)

        let isHw = ttzip_rust_is_hardware_accelerated()
        #if arch(arm64)
        XCTAssertTrue(isHw)
        #endif

        let okStatusStr = String(cString: ttzip_rust_status_string(TTZIP_STATUS_OK))
        XCTAssertEqual(okStatusStr, "OK")
    }

    func testArchiveExtractionSelectedSymbolCallable() {
        // Dynamic assertion that the C-ABI function symbol is linked and callable
        let dummyPath = "/tmp/non_existent_archive_test.zip"
        var count: Int = 0
        let status = ttzip_rust_archive_extract_selected(dummyPath, nil, 0, "/tmp", nil, &count)
        // Should return a valid failure error code rather than symbol lookup crash
        XCTAssertNotEqual(status, TTZIP_STATUS_OK)
    }

    func testVfsTreeAndSearchSymbolsCallable() {
        let token = ttzip_rust_cancellation_token_new()
        XCTAssertNotNil(token)
        XCTAssertFalse(ttzip_rust_cancellation_token_is_cancelled(token))
        ttzip_rust_cancellation_token_cancel(token, 1)
        XCTAssertTrue(ttzip_rust_cancellation_token_is_cancelled(token))
        ttzip_rust_cancellation_token_free(token)
    }
}
