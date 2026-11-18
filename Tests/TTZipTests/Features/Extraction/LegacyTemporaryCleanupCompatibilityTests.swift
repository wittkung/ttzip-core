// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import XCTest
import TTZipCore

/// Run only with the reviewed no-op shim, never with the retired sweeping implementation.
/// Every path created or removed by these tests belongs to its own IsolatedTempSandbox.
final class LegacyTemporaryCleanupCompatibilityTests: XCTestCase {
    func testLegacyShimPreservesTheTestsOwnedSentinel() throws {
        let sandbox = try IsolatedTempSandbox(prefix: "LegacyCleanupCompatibility")
        defer { sandbox.cleanup() }
        let sentinel = sandbox.fileURL(named: "sentinel.bin")
        let original = Data([0x54, 0x54, 0x5A, 0x49, 0x50, 0x00, 0xFF])
        try original.write(to: sentinel, options: .withoutOverwriting)

        let manager: TempDirectoryCleanUpManager = .shared
        manager.cleanupAllTemporaryDirectories()

        XCTAssertTrue(FileManager.default.fileExists(atPath: sandbox.path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: sentinel.path))
        XCTAssertEqual(try Data(contentsOf: sentinel), original)
    }

    func testRepeatedLegacyCallsPreserveNestedOwnedPathsAndBytes() throws {
        let sandbox = try IsolatedTempSandbox(prefix: "LegacyCleanupRepeat")
        defer { sandbox.cleanup() }
        let directory = try sandbox.createSubdirectory("owned-nested-directory")
        let sentinel = directory.appendingPathComponent("sentinel.txt")
        let original = Data("Owned by this test only.\n".utf8)
        try original.write(to: sentinel, options: .withoutOverwriting)

        let manager: TempDirectoryCleanUpManager = .shared
        XCTAssertTrue(manager === TempDirectoryCleanUpManager.shared)
        for _ in 0..<3 {
            manager.cleanupAllTemporaryDirectories()
            XCTAssertTrue(FileManager.default.fileExists(atPath: sandbox.path))
            XCTAssertTrue(FileManager.default.fileExists(atPath: directory.path))
            XCTAssertTrue(FileManager.default.fileExists(atPath: sentinel.path))
            XCTAssertEqual(try Data(contentsOf: sentinel), original)
        }
    }
}
