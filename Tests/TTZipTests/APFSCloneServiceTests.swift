// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class APFSCloneServiceTests: XCTestCase {

    private var tempDir: URL!

    override func setUp() {
        super.setUp()
        tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("ttzip_apfs_test_\(UUID().uuidString)")
        try? FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    override func tearDown() {
        if let tempDir = tempDir {
            try? FileManager.default.removeItem(at: tempDir)
        }
        super.tearDown()
    }

    func testAPFSFileSystemDetection() {
        let isAPFS = APFSCloneService.isAPFSFileSystem(at: tempDir.path)
        XCTAssertTrue(isAPFS, "macOS system temporary directory must be on an APFS volume")
    }

    func testAPFSCloneFilePreservesContentAndDemonstratesCoW() throws {
        let srcURL = tempDir.appendingPathComponent("source_payload.bin")
        let dstURL = tempDir.appendingPathComponent("cloned_payload.bin")

        // Create 1 MB random test data
        var randomBytes = [UInt8](repeating: 0, count: 1024 * 1024)
        for i in 0..<randomBytes.count {
            randomBytes[i] = UInt8(i & 0xFF)
        }
        let originalData = Data(randomBytes)
        try originalData.write(to: srcURL)

        // Clone file via APFS
        let success = APFSCloneService.cloneFile(from: srcURL.path, to: dstURL.path, overwrite: true)
        XCTAssertTrue(success, "APFS Clonefile should succeed on APFS filesystem")

        // Verify cloned file exists and data is identical
        XCTAssertTrue(FileManager.default.fileExists(atPath: dstURL.path))
        let clonedData = try Data(contentsOf: dstURL)
        XCTAssertEqual(clonedData, originalData, "Cloned file content must exactly match source")

        // Verify Copy-on-Write isolation: mutate source file
        let modifiedData = Data("Mutated Source Content".utf8)
        try modifiedData.write(to: srcURL)

        // Cloned data must remain unchanged
        let clonedDataAfterMutation = try Data(contentsOf: dstURL)
        XCTAssertEqual(clonedDataAfterMutation, originalData, "Cloned file must remain unmodified after source mutation (CoW)")
    }

    func testAPFSCloneFailsGracefullyOnNonExistentSource() {
        let nonExistent = tempDir.appendingPathComponent("does_not_exist.bin").path
        let dst = tempDir.appendingPathComponent("out.bin").path

        let result = APFSCloneService.cloneFile(from: nonExistent, to: dst)
        XCTAssertFalse(result, "Cloning non-existent source must return false without crashing")
    }
}
