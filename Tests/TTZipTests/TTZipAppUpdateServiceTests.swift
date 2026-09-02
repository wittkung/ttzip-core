// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class TTZipAppUpdateServiceTests: XCTestCase {

    var service: TTZipAppUpdateService!

    override func setUp() {
        super.setUp()
        service = TTZipAppUpdateService()
        service.resetState()
    }

    override func tearDown() {
        service = nil
        super.tearDown()
    }

    // MARK: - In-Memory Delta Patch Tests

    func testDeltaPatchRoundtripAllFormats() throws {
        let baseText = "TTZip Desktop v1.0.0 baseline payload. Ultra-fast native compression with Swift 6 and Rust."
        let targetText = "TTZip Desktop v1.1.0 updated payload with new delta patching engine and Swift 6 Sendable architecture."

        let baseData = Data(baseText.utf8)
        let targetData = Data(targetText.utf8)

        let formats: [DeltaPatchFormat] = [.rawByteBlock, .zstdCompressed, .flateCompressed]

        for format in formats {
            let patchData = try service.createDeltaPatch(baseBytes: baseData, targetBytes: targetData, format: format)
            XCTAssertGreaterThan(patchData.count, 88, "Patch package must include valid header and payload for format \(format)")

            let result = try service.applyDeltaPatchInMemory(baseBytes: baseData, patchBytes: patchData)
            XCTAssertTrue(result.success)
            XCTAssertTrue(result.appliedInMemory)
            XCTAssertEqual(result.targetSize, UInt64(targetData.count))
            XCTAssertEqual(result.patchedBytes, targetData)
            XCTAssertFalse(result.targetHash.isEmpty)

            // Test hash validation
            let hashValidatedResult = try service.applyDeltaPatchInMemory(
                baseBytes: baseData,
                patchBytes: patchData,
                expectedHash: result.targetHash
            )
            XCTAssertEqual(hashValidatedResult.patchedBytes, targetData)
        }
    }

    func testDeltaPatchWrongBaseRejection() throws {
        let baseData = Data("Base data A".utf8)
        let targetData = Data("Target data B with new features".utf8)
        let patchData = try service.createDeltaPatch(baseBytes: baseData, targetBytes: targetData, format: .rawByteBlock)

        let wrongBaseData = Data("Completely different base data".utf8)
        XCTAssertThrowsError(try service.applyDeltaPatchInMemory(baseBytes: wrongBaseData, patchBytes: patchData))
    }

    func testDeltaPatchExpectedHashMismatchThrows() throws {
        let baseData = Data("Base data A".utf8)
        let targetData = Data("Target data B".utf8)
        let patchData = try service.createDeltaPatch(baseBytes: baseData, targetBytes: targetData, format: .rawByteBlock)

        let invalidHash = "0000000000000000000000000000000000000000000000000000000000000000"
        XCTAssertThrowsError(try service.applyDeltaPatchInMemory(baseBytes: baseData, patchBytes: patchData, expectedHash: invalidHash))
    }

    // MARK: - Tree Hash Tests

    func testCalculateTreeHashForFileAndDirectory() throws {
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("ttzip_tree_swift_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let fileA = tempDir.appendingPathComponent("fileA.txt")
        let subDir = tempDir.appendingPathComponent("nested")
        try FileManager.default.createDirectory(at: subDir, withIntermediateDirectories: true)
        let fileB = subDir.appendingPathComponent("fileB.txt")

        try "Content A".write(to: fileA, atomically: true, encoding: .utf8)
        try "Content B".write(to: fileB, atomically: true, encoding: .utf8)

        // Single file tree hash
        let fileHash = try service.calculateTreeHash(for: fileA.path)
        XCTAssertEqual(fileHash.count, 64)

        // Directory tree hash
        let dirHash1 = try service.calculateTreeHash(for: tempDir.path)
        XCTAssertEqual(dirHash1.count, 64)

        // Determinism check
        let dirHash2 = try service.calculateTreeHash(for: tempDir.path)
        XCTAssertEqual(dirHash1, dirHash2)

        // Mutation check
        try "Mutated A".write(to: fileA, atomically: true, encoding: .utf8)
        let dirHash3 = try service.calculateTreeHash(for: tempDir.path)
        XCTAssertNotEqual(dirHash1, dirHash3)
    }

    // MARK: - Version Monotonicity Tests

    func testVersionMonotonicityChecks() throws {
        XCTAssertTrue(try service.checkVersionMonotonicity(currentVersion: "1.0.0", incomingVersion: "1.0.1"))
        XCTAssertTrue(try service.checkVersionMonotonicity(currentVersion: "1.0.0", incomingVersion: "2.0.0"))
        XCTAssertTrue(try service.checkVersionMonotonicity(currentVersion: "1.2.3", incomingVersion: "1.2.3"))

        XCTAssertThrowsError(try service.checkVersionMonotonicity(currentVersion: "1.5.0", incomingVersion: "1.4.9"))
        XCTAssertThrowsError(try service.checkVersionMonotonicity(currentVersion: "2.0.0", incomingVersion: "1.9.9"))
    }

    // MARK: - Appcast Evaluation & State Machine Tests

    func testAppcastJsonParsingAndCandidateEvaluation() throws {
        let jsonFeed = """
        {
            "channel": "stable",
            "title": "TTZip for macOS",
            "feed_url": "https://updates.ttzip.io/appcast.json",
            "latest_version": "1.3.0",
            "latest_build": 10300,
            "items": [
                {
                    "version": "1.3.0",
                    "build_number": 10300,
                    "min_os_version": "14.0",
                    "release_notes_url": "https://ttzip.io/notes/1.3.0",
                    "download_url": "https://dl.ttzip.io/TTZip-1.3.0.dmg",
                    "download_size": 30000000,
                    "signature_ed25519": "sig1",
                    "sha256": "abc123",
                    "delta_patch_url": "https://dl.ttzip.io/patches/1.2.0-to-1.3.0.patch",
                    "delta_base_version": "1.2.0",
                    "delta_signature_ed25519": "sig_delta",
                    "delta_size": 2500000,
                    "is_critical": true,
                    "published_at_epoch_secs": 1740000000
                }
            ],
            "signature_valid": true,
            "checked_at_epoch_secs": 1740000000
        }
        """

        let meta = try service.parseAppcastJson(jsonFeed)
        XCTAssertEqual(meta.latestVersion, "1.3.0")
        XCTAssertEqual(meta.latestBuild, 10300)
        XCTAssertEqual(meta.items.count, 1)

        // Case 1: Base is 1.2.0 -> Delta Eligible
        let stateDelta = service.evaluateUpdateCandidates(
            metadata: meta,
            currentVersion: "1.2.0",
            currentBuild: 10200
        )
        if case .updateAvailable(let item, let isDelta) = stateDelta {
            XCTAssertEqual(item.version, "1.3.0")
            XCTAssertTrue(isDelta)
            XCTAssertTrue(item.isCritical)
        } else {
            XCTFail("Expected .updateAvailable state with delta eligibility")
        }

        // Case 2: Base is 1.1.0 -> Not Delta Eligible (Full download needed)
        let stateFull = service.evaluateUpdateCandidates(
            metadata: meta,
            currentVersion: "1.1.0",
            currentBuild: 10100
        )
        if case .updateAvailable(let item, let isDelta) = stateFull {
            XCTAssertEqual(item.version, "1.3.0")
            XCTAssertFalse(isDelta)
        } else {
            XCTFail("Expected .updateAvailable state without delta eligibility")
        }

        // Case 3: Already at 1.3.0 -> Up to date
        let stateUpToDate = service.evaluateUpdateCandidates(
            metadata: meta,
            currentVersion: "1.3.0",
            currentBuild: 10300
        )
        XCTAssertEqual(stateUpToDate, .upToDate)
    }

    // MARK: - AppGroup State Synchronization Tests

    func testAppGroupStateSynchronization() throws {
        let testSuite = "com.ttzip.tests.appgroup.\(UUID().uuidString)"
        service.syncStateToAppGroup(suiteName: testSuite)

        let initialTimestamp = service.loadLastCheckTimestamp(suiteName: testSuite)
        XCTAssertNil(initialTimestamp)

        // Run mock patch to accumulate metrics with realistic size payload
        let base = Data(String(repeating: "TTZip Base Payload 1.0.0 Architecture Invariant Data Block. ", count: 50).utf8)
        let target = Data(String(repeating: "TTZip Base Payload 1.0.0 Architecture Invariant Data Block. ", count: 50).appending("With new appended delta blocks.").utf8)
        let patch = try service.createDeltaPatch(baseBytes: base, targetBytes: target, format: .rawByteBlock)
        _ = try service.applyDeltaPatchInMemory(baseBytes: base, patchBytes: patch)

        XCTAssertEqual(service.cumulativePatchesApplied, 1)
        XCTAssertGreaterThan(service.totalBandwidthSavedBytes, 0)

        // Sync and reload
        service.syncStateToAppGroup(suiteName: testSuite)
        let defaults = UserDefaults(suiteName: testSuite)
        XCTAssertEqual(defaults?.integer(forKey: "TTZipCumulativePatchesApplied"), 1)
    }
}
