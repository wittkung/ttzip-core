// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class CABISymbolGateTests: XCTestCase {

    func testCoreRuntimeAndVersionSymbols() {
        let dummyData = Data([0x01, 0x02, 0x03, 0x04])
        let entropy = estimateShannonEntropy(data: dummyData)
        XCTAssertGreaterThanOrEqual(entropy, 0.0)
    }

    func testVfsTreeAndSearchSymbolsCallable() {
        let token = CancellationToken()
        XCTAssertFalse(token.isCancelled())
        token.cancel()
        XCTAssertTrue(token.isCancelled())
    }
    
    func testUniFFIVfsTreeBuilding() {
        let entries = [
            UniFfiEntryMetadata(
                path: "test/file.txt",
                uncompressedSize: 100,
                compressedSize: 50,
                crc32: 12345,
                mtimeEpochSecs: 1000,
                mode: 0o644,
                isDirectory: false,
                isEncrypted: false,
                compressionMethod: "store",
                detectedEncoding: "UTF-8"
            )
        ]
        let tree = UniFfiVfsTree.build(entries: entries, rootName: "root")
        let stats = tree.getStats()
        XCTAssertEqual(stats.totalFiles, 1)
        XCTAssertEqual(stats.totalDirs, 1)
        
        let searchResults = tree.search(query: "file", maxResults: 10)
        XCTAssertEqual(searchResults.count, 1)
        XCTAssertEqual(searchResults.first?.path, "test/file.txt")
    }
}
