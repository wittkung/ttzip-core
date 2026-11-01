// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import XCTest
@testable import TTZipCore
#if canImport(TTZipCoreCrypto)
@testable import TTZipCoreCrypto
#endif

final class ArchiveScopedVaultTests: XCTestCase {
    private var directory: URL!

    override func setUp() {
        super.setUp()
        directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    override func tearDown() {
        if let directory { try? FileManager.default.removeItem(at: directory) }
        super.tearDown()
    }

    func testScopedCredentialSurvivesRestartWithoutGlobalCandidateLeak() throws {
        let vaultURL = directory.appendingPathComponent("scoped.enc")
        let configURL = directory.appendingPathComponent("scoped.json")
        let master = UUID().uuidString
        let archivePassword = UUID().uuidString
        let genericPassword = UUID().uuidString
        let logicalKey = UUID().uuidString
        let contentKey = UUID().uuidString
        let manager = PasswordVaultManager(vaultURL: vaultURL, configURL: configURL)

        XCTAssertThrowsError(try manager.archivePassword(
            logicalKey: logicalKey, contentKey: contentKey
        )) { XCTAssertEqual($0 as? ArchiveVaultCredentialError, .locked) }
        manager.setMasterPassword(master)
        _ = manager.addEntryVerified(label: "generic fixture", password: genericPassword)
        try manager.upsertArchivePassword(
            archivePassword, logicalKey: logicalKey, contentKey: contentKey, label: "archive.zip"
        )
        XCTAssertEqual(manager.getEntries().count, 1)
        XCTAssertEqual(manager.candidatePasswordsForAutoUnlock(), [genericPassword])
        XCTAssertNil(try manager.archivePassword(
            logicalKey: logicalKey, contentKey: UUID().uuidString
        ))

        manager.lockVault()
        let reopened = PasswordVaultManager(vaultURL: vaultURL, configURL: configURL)
        XCTAssertEqual(reopened.unlockVaultResult(with: master), .unlocked)
        XCTAssertEqual(try reopened.archivePassword(
            logicalKey: logicalKey, contentKey: contentKey
        ), archivePassword)
        XCTAssertEqual(reopened.candidatePasswordsForAutoUnlock(), [genericPassword])

        try reopened.removeArchivePassword(
            logicalKey: logicalKey, contentKey: contentKey, ifPasswordMatches: genericPassword
        )
        XCTAssertEqual(try reopened.archivePassword(
            logicalKey: logicalKey, contentKey: contentKey
        ), archivePassword)
        try reopened.removeArchivePassword(logicalKey: logicalKey)
        XCTAssertNil(try reopened.archivePassword(
            logicalKey: logicalKey, contentKey: contentKey
        ))
        XCTAssertEqual(reopened.getEntries().count, 1)
    }

    func testFailedScopedWriteKeepsPriorCredential() throws {
        let vaultURL = directory.appendingPathComponent("failed-write.enc")
        let manager = PasswordVaultManager(
            vaultURL: vaultURL, configURL: directory.appendingPathComponent("failed-write.json")
        )
        let logicalKey = UUID().uuidString
        let contentKey = UUID().uuidString
        let oldPassword = UUID().uuidString
        manager.setMasterPassword(UUID().uuidString)
        try manager.upsertArchivePassword(
            oldPassword, logicalKey: logicalKey, contentKey: contentKey, label: "archive.zip"
        )
        try Data("external test mutation".utf8).write(to: vaultURL, options: .atomic)
        XCTAssertThrowsError(try manager.upsertArchivePassword(
            UUID().uuidString, logicalKey: logicalKey,
            contentKey: contentKey, label: "archive.zip"
        )) { XCTAssertEqual($0 as? ArchiveVaultCredentialError, .writeFailed) }
        XCTAssertEqual(try manager.archivePassword(
            logicalKey: logicalKey, contentKey: contentKey
        ), oldPassword)
    }
}
