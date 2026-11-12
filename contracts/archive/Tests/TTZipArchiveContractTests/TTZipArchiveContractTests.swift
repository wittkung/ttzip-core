// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import XCTest
import TTZipArchiveContract

final class TTZipArchiveContractTests: XCTestCase {
    func testArchiveEntryPathNameExtensionDirectoryPrefixAndMime() {
        let fileEntry = ArchiveEntry(
            path: "Sources/TTZipCore/ArchiveEntry.SWIFT",
            uncompressedSize: 2048,
            isDirectory: false,
            posixPermissions: 0o644,
            compressedSize: 1024,
            crc32: 0xDEADBEEF
        )
        XCTAssertEqual(fileEntry.id, "Sources/TTZipCore/ArchiveEntry.SWIFT")
        XCTAssertEqual(fileEntry.path, "Sources/TTZipCore/ArchiveEntry.SWIFT")
        XCTAssertEqual(fileEntry.name, "ArchiveEntry.SWIFT")
        XCTAssertEqual(fileEntry.extensionName, "swift")
        XCTAssertEqual(fileEntry.mimeType, "text/x-swift")
        XCTAssertEqual(fileEntry.directoryPrefix, "Sources/TTZipCore/")
        XCTAssertEqual(fileEntry.posixPermissions, 0o644)
        XCTAssertEqual(fileEntry.compressedSize, 1024)
        XCTAssertEqual(fileEntry.crc32, 0xDEADBEEF)

        let dirEntry = ArchiveEntry(
            path: "Sources/TTZipCore/",
            uncompressedSize: 0,
            isDirectory: true
        )
        XCTAssertEqual(dirEntry.name, "TTZipCore")
        XCTAssertEqual(dirEntry.extensionName, "")
        XCTAssertEqual(dirEntry.mimeType, "application/octet-stream")
        XCTAssertEqual(dirEntry.directoryPrefix, "Sources/TTZipCore/")
    }

    func testEightEncryptionFlagCombinations() {
        let cases: [(Bool, Bool, Bool, Bool, Bool, Bool)] = [
            // (E, D, M) -> expected (isEncrypted, isDataEncrypted, isMetadataEncrypted)
            (false, false, false, false, false, false),
            (false, false, true,  true,  false, true),
            (false, true,  false, true,  true,  false),
            (false, true,  true,  true,  true,  true),
            (true,  false, false, true,  true,  false),
            (true,  false, true,  true,  false, true),
            (true,  true,  false, true,  true,  false),
            (true,  true,  true,  true,  true,  true),
        ]
        for (e, d, m, expE, expD, expM) in cases {
            let entry = ArchiveEntry(
                path: "secret.txt",
                uncompressedSize: 16,
                isDirectory: false,
                isEncrypted: e,
                isDataEncrypted: d,
                isMetadataEncrypted: m
            )
            XCTAssertEqual(entry.isEncrypted, expE, "Failed isEncrypted for (\(e), \(d), \(m))")
            XCTAssertEqual(entry.isDataEncrypted, expD, "Failed isDataEncrypted for (\(e), \(d), \(m))")
            XCTAssertEqual(entry.isMetadataEncrypted, expM, "Failed isMetadataEncrypted for (\(e), \(d), \(m))")
        }
    }
}
