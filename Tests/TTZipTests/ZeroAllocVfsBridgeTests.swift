// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class ZeroAllocVfsBridgeTests: XCTestCase {
    
    func testSwiftVfsSearchZeroAllocIntegration() throws {
        var entries: [ArchiveEntry] = []
        entries.reserveCapacity(50_000)
        for i in 0..<50_000 {
            entries.append(ArchiveEntry(
                path: "usr/lib/module_\(i / 1000)/item_\(i).dylib",
                uncompressedSize: 4096,
                isDirectory: false,
                detectedEncoding: "UTF-8",
                modificationDate: Date(),
                isEncrypted: false
            ))
        }
        
        guard let session = RustVfsSession(entries: entries, rootName: "SystemTree") else {
            XCTFail("Failed to initialize RustVfsSession")
            return
        }
        
        let matches = session.searchZeroAlloc(query: "item_49999", maxResults: 16)
        XCTAssertGreaterThan(matches.count, 0)
        XCTAssertEqual(matches.first?.path, "usr/lib/module_49/item_49999.dylib")
        print("✓ Swift VFS Search Zero-Alloc Verified on 50,000 nodes. Matches: \(matches.count)")
    }
}
