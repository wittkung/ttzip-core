// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import XCTest
@testable import TTZipCore

final class RustVfsSessionSearchBenchmarkTests: XCTestCase {
    
    func testVfsSessionSearchPerformance10k() {
        var entries: [ArchiveEntry] = []
        entries.reserveCapacity(10_000)
        
        for i in 0..<10_000 {
            entries.append(ArchiveEntry(
                path: "src/deep/module_\(i)/component_\(i).swift",
                uncompressedSize: 1024,
                isDirectory: false,
                detectedEncoding: "UTF-8",
                modificationDate: Date(),
                isEncrypted: false
            ))
        }
        
        guard let session = RustVfsSession(entries: entries, rootName: "project") else {
            XCTFail("Failed to build RustVfsSession")
            return
        }
        
        let start = Date()
        let results = session.fuzzySearch(query: "comp_99")
        let elapsed = Date().timeIntervalSince(start)
        
        XCTAssertGreaterThan(results.count, 0)
        XCTAssertLessThan(elapsed, 0.05, "10k entry search must complete in < 50ms")
    }
}
