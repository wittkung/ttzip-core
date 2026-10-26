// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import XCTest
@testable import TTZipCore

/// Comprehensive test suite for `ExtractionStagingRegistry` verifying persistence,
/// thread safety, and stale staging directory reclamation on startup.
final class ExtractionStagingRegistryTests: XCTestCase {
    private var suiteName: String!
    private var testDefaults: UserDefaults!
    private var registry: ExtractionStagingRegistry!
    private var tempDirectories: [URL] = []

    override func setUpWithError() throws {
        try super.setUpWithError()
        suiteName = "ExtractionStagingRegistryTests_\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            XCTFail("Failed to instantiate isolated UserDefaults suite: \(suiteName ?? "")")
            return
        }
        testDefaults = defaults
        registry = ExtractionStagingRegistry(userDefaults: testDefaults)
        tempDirectories = []
    }

    override func tearDownWithError() throws {
        for dir in tempDirectories {
            try? FileManager.default.removeItem(at: dir)
        }
        tempDirectories.removeAll()

        if let suiteName, let testDefaults {
            testDefaults.removePersistentDomain(forName: suiteName)
        }
        testDefaults = nil
        registry = nil
        try super.tearDownWithError()
    }

    private func makeTempDirectory(prefix: String = "staging_test") throws -> URL {
        let fm = FileManager.default
        let url = fm.temporaryDirectory.appendingPathComponent("ttzip_\(prefix)_\(UUID().uuidString)")
        try fm.createDirectory(at: url, withIntermediateDirectories: true)
        tempDirectories.append(url)
        return url
    }

    func testRegisterAndUnregisterUpdatesUserDefaults() throws {
        let rootA = "/tmp/test_destination_a"
        let rootB = "/tmp/test_destination_b"

        XCTAssertTrue(registry.registeredRoots().isEmpty)

        // Register first root
        registry.register(destinationRoot: rootA)
        XCTAssertEqual(registry.registeredRoots(), Set([rootA]))
        let storedRootsA = testDefaults.stringArray(forKey: ExtractionStagingRegistry.userDefaultsKey) ?? []
        XCTAssertEqual(Set(storedRootsA), Set([rootA]))

        // Register second root and duplicate root
        registry.register(destinationRoot: rootB)
        registry.register(destinationRoot: rootA)
        XCTAssertEqual(registry.registeredRoots(), Set([rootA, rootB]))

        // Register empty string should be a no-op
        registry.register(destinationRoot: "")
        XCTAssertEqual(registry.registeredRoots(), Set([rootA, rootB]))

        // Unregister rootA
        registry.unregister(destinationRoot: rootA)
        XCTAssertEqual(registry.registeredRoots(), Set([rootB]))
        let storedRootsAfterRemoval = testDefaults.stringArray(forKey: ExtractionStagingRegistry.userDefaultsKey) ?? []
        XCTAssertEqual(Set(storedRootsAfterRemoval), Set([rootB]))

        // Unregister non-existent and empty string
        registry.unregister(destinationRoot: "/tmp/non_existent")
        registry.unregister(destinationRoot: "")
        XCTAssertEqual(registry.registeredRoots(), Set([rootB]))

        // Unregister rootB
        registry.unregister(destinationRoot: rootB)
        XCTAssertTrue(registry.registeredRoots().isEmpty)
    }

    func testRegisteredRootsSnapshotImmutability() throws {
        let root1 = "/tmp/snapshot_root_1"
        let root2 = "/tmp/snapshot_root_2"

        registry.register(destinationRoot: root1)
        let snapshot1 = registry.registeredRoots()
        XCTAssertEqual(snapshot1, Set([root1]))

        registry.register(destinationRoot: root2)
        let snapshot2 = registry.registeredRoots()
        XCTAssertEqual(snapshot2, Set([root1, root2]))

        // Prior snapshot must remain immutable
        XCTAssertEqual(snapshot1, Set([root1]))
    }

    func testConcurrentRegisterAndUnregisterSafety() throws {
        let queue = DispatchQueue(label: "com.ttzip.staging.test.concurrent", attributes: .concurrent)
        let group = DispatchGroup()
        let iterations = 100

        for i in 0..<iterations {
            let path = "/tmp/concurrent_test_path_\(i)"
            group.enter()
            queue.async {
                self.registry.register(destinationRoot: path)
                _ = self.registry.registeredRoots()
                group.leave()
            }
        }

        let waitResult = group.wait(timeout: .now() + 5.0)
        XCTAssertEqual(waitResult, .success, "Concurrent registration operations timed out")
        XCTAssertEqual(registry.registeredRoots().count, iterations)

        // Concurrently unregister half of the items
        for i in 0..<(iterations / 2) {
            let path = "/tmp/concurrent_test_path_\(i)"
            group.enter()
            queue.async {
                self.registry.unregister(destinationRoot: path)
                group.leave()
            }
        }

        let unregisterWait = group.wait(timeout: .now() + 5.0)
        XCTAssertEqual(unregisterWait, .success, "Concurrent unregistration operations timed out")
        XCTAssertEqual(registry.registeredRoots().count, iterations / 2)
    }

    func testSweepOnLaunchCleansAbandonedStagingAndRemovesRoot() throws {
        let destRoot = try makeTempDirectory(prefix: "sweep_abandoned")
        let deadPid: Int32 = 99999
        let stagingName = ".ttzip_staging_\(deadPid)_12345_67890"
        let stagingDir = destRoot.appendingPathComponent(stagingName)

        let fm = FileManager.default
        try fm.createDirectory(at: stagingDir, withIntermediateDirectories: true)
        let dummyFile = stagingDir.appendingPathComponent("orphaned_payload.bin")
        try Data("stale extraction data".utf8).write(to: dummyFile)

        XCTAssertTrue(fm.fileExists(atPath: stagingDir.path))

        registry.register(destinationRoot: destRoot.path)
        XCTAssertTrue(registry.registeredRoots().contains(destRoot.path))

        let sweptCount = registry.sweepOnLaunch()
        XCTAssertEqual(sweptCount, 1, "Abandoned staging directory must be reclaimed")

        // Abandoned staging folder should be deleted
        XCTAssertFalse(fm.fileExists(atPath: stagingDir.path))
        // Destination root itself must remain
        XCTAssertTrue(fm.fileExists(atPath: destRoot.path))
        // Root must be purged from registry upon successful sweep
        XCTAssertFalse(registry.registeredRoots().contains(destRoot.path))
        let remainingDefaults = testDefaults.stringArray(forKey: ExtractionStagingRegistry.userDefaultsKey) ?? []
        XCTAssertFalse(remainingDefaults.contains(destRoot.path))
    }

    func testSweepOnLaunchPreservesActiveProcessStaging() throws {
        let destRoot = try makeTempDirectory(prefix: "sweep_active_pid")
        let livePid = ProcessInfo.processInfo.processIdentifier
        let activeStagingName = ".ttzip_staging_\(livePid)_88888_11111"
        let activeStagingDir = destRoot.appendingPathComponent(activeStagingName)

        let deadPid: Int32 = 99999
        let staleStagingName = ".ttzip_staging_\(deadPid)_99999_22222"
        let staleStagingDir = destRoot.appendingPathComponent(staleStagingName)

        let fm = FileManager.default
        try fm.createDirectory(at: activeStagingDir, withIntermediateDirectories: true)
        try fm.createDirectory(at: staleStagingDir, withIntermediateDirectories: true)

        let activeFile = activeStagingDir.appendingPathComponent("in_flight.bin")
        try Data("active stream".utf8).write(to: activeFile)

        registry.register(destinationRoot: destRoot.path)

        let sweptCount = registry.sweepOnLaunch()
        XCTAssertEqual(sweptCount, 1, "Only dead PID staging directory should be swept")

        XCTAssertFalse(fm.fileExists(atPath: staleStagingDir.path))
        XCTAssertTrue(fm.fileExists(atPath: activeStagingDir.path))
        XCTAssertTrue(fm.fileExists(atPath: activeFile.path))
    }

    func testSweepOnLaunchPreservesRegularAndHiddenEntries() throws {
        let destRoot = try makeTempDirectory(prefix: "sweep_preserve_regular")
        let fm = FileManager.default

        let regularFile = destRoot.appendingPathComponent("user_document.txt")
        try Data("important user document".utf8).write(to: regularFile)

        let hiddenConfigDir = destRoot.appendingPathComponent(".git")
        try fm.createDirectory(at: hiddenConfigDir, withIntermediateDirectories: true)

        let deadPid: Int32 = 99999
        let staleStagingDir = destRoot.appendingPathComponent(".ttzip_staging_\(deadPid)_55555_66666")
        try fm.createDirectory(at: staleStagingDir, withIntermediateDirectories: true)

        registry.register(destinationRoot: destRoot.path)

        let sweptCount = registry.sweepOnLaunch()
        XCTAssertEqual(sweptCount, 1)

        XCTAssertFalse(fm.fileExists(atPath: staleStagingDir.path))
        XCTAssertTrue(fm.fileExists(atPath: regularFile.path))
        XCTAssertTrue(fm.fileExists(atPath: hiddenConfigDir.path))
    }

    func testSweepOnLaunchHandlesNonexistentAndMultipleRoots() throws {
        let root1 = try makeTempDirectory(prefix: "root1")
        let root2 = try makeTempDirectory(prefix: "root2")
        let rootNonExistent = FileManager.default.temporaryDirectory
            .appendingPathComponent("ttzip_ghost_root_\(UUID().uuidString)")

        let deadPid: Int32 = 99999
        let staging1 = root1.appendingPathComponent(".ttzip_staging_\(deadPid)_100_200")
        let staging2 = root2.appendingPathComponent(".ttzip_staging_\(deadPid)_300_400")

        let fm = FileManager.default
        try fm.createDirectory(at: staging1, withIntermediateDirectories: true)
        try fm.createDirectory(at: staging2, withIntermediateDirectories: true)

        registry.register(destinationRoot: root1.path)
        registry.register(destinationRoot: root2.path)
        registry.register(destinationRoot: rootNonExistent.path)

        let sweptCount = registry.sweepOnLaunch()
        XCTAssertEqual(sweptCount, 2)

        XCTAssertFalse(fm.fileExists(atPath: staging1.path))
        XCTAssertFalse(fm.fileExists(atPath: staging2.path))

        // All entries should be removed from registry (nonexistent was pruned, root1 and root2 swept)
        XCTAssertTrue(registry.registeredRoots().isEmpty)
    }

    func testSharedSingletonAccess() {
        let sharedRegistry = ExtractionStagingRegistry.shared
        _ = sharedRegistry.registeredRoots()
        XCTAssertNotNil(sharedRegistry)
    }
}
