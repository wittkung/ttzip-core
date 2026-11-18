// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import XCTest
@testable import TTZipCore

/// Exercises the production facade's retry boundary without reading the user's vault.
final class ArchiveInspectionRetryBoundaryTests: XCTestCase {
    private actor Reader: ArchiveReading {
        private var failures: [ArchiveError]
        private(set) var calls = 0
        private(set) var implicitVaultAttempts = 0

        init(_ failures: [ArchiveError]) { self.failures = failures }

        func inspect(archivePath: String) async throws -> [ArchiveEntry] {
            try await inspect(archivePath: archivePath, password: nil, candidatePasswords: nil)
        }

        func inspect(archivePath: String, password: String?, candidatePasswords: [String]?) async throws -> [ArchiveEntry] {
            calls += 1
            if candidatePasswords == nil || candidatePasswords?.isEmpty == false { implicitVaultAttempts += 1 }
            guard !failures.isEmpty else { throw ArchiveError.openFailed }
            throw failures.removeFirst()
        }

        func inspectTree(archivePath: String, password: String?, candidatePasswords: [String]?) async throws -> ArchiveCompositeDirectory {
            XCTFail("The inspection facade must use the reader's inspection entry point")
            throw ArchiveError.openFailed
        }

        func probeEncryption(archivePath: String) async throws -> ArchiveEncryptionTier {
            XCTFail("A failed inspection must not guess encryption through another entry point")
            return .unsupported
        }
    }

    /// Synchronous vault protocol; every mutable field is protected by this lock.
    private final class Vault: PasswordVaultManaging, @unchecked Sendable {
        private let lock = NSLock()
        private var reads = 0
        var autoUnlockArchives: Bool { true }
        var readCount: Int { lock.lock(); defer { lock.unlock() }; return reads }

        func getEntries() -> [PasswordVaultEntry] {
            lock.lock()
            reads += 1
            lock.unlock()
            return ["candidate-one", "candidate-two"].map { .init(label: "Synthetic", password: $0) }
        }

        func recordUsage(id: UUID) { XCTFail("No failing candidate may be recorded as successful") }
    }

    private func fixture() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("inspection-boundary-\(UUID().uuidString)")
        try Data([0x54, 0x54]).write(to: url, options: .withoutOverwriting)
        return url
    }

    func testFormatFailureDoesNotConsultVaultOrBecomePasswordRequired() async throws {
        let url = try fixture()
        defer { try? FileManager.default.removeItem(at: url) }
        let reader = Reader([.invalidFormat])
        let vault = Vault()
        let facade = TTZipEngineFacade(reader: reader, passwordVault: vault)
        do {
            _ = try await facade.inspectArchive(archivePath: url.path)
            XCTFail("A format error must fail inspection")
        } catch {
            guard case .invalidFormat = ArchiveError.from(error: error) else {
                return XCTFail("Original format classification was lost: \(error)")
            }
        }
        let calls = await reader.calls
        let implicit = await reader.implicitVaultAttempts
        XCTAssertEqual(calls, 1)
        XCTAssertEqual(implicit, 0, "The facade owns candidate discovery")
        XCTAssertEqual(vault.readCount, 0)
    }

    func testExplicitWrongPasswordIsNotReplacedByVaultDiscovery() async throws {
        let url = try fixture()
        defer { try? FileManager.default.removeItem(at: url) }
        let reader = Reader([.wrongPassword(archivePath: url.path)])
        let vault = Vault()
        let facade = TTZipEngineFacade(reader: reader, passwordVault: vault)
        do {
            _ = try await facade.inspectArchive(archivePath: url.path, password: "explicit-synthetic")
            XCTFail("An explicit wrong credential must fail")
        } catch {
            guard case .wrongPassword = ArchiveError.from(error: error) else {
                return XCTFail("Wrong-password classification was lost: \(error)")
            }
        }
        let calls = await reader.calls
        XCTAssertEqual(calls, 1)
        XCTAssertEqual(vault.readCount, 0)
    }

    func testNonPasswordFailureDuringCandidateStopsRemainingRetries() async throws {
        let url = try fixture()
        defer { try? FileManager.default.removeItem(at: url) }
        let reader = Reader([.passwordRequired, .corruptHeader])
        let vault = Vault()
        let facade = TTZipEngineFacade(reader: reader, passwordVault: vault)
        do {
            _ = try await facade.inspectArchive(archivePath: url.path)
            XCTFail("A damaged candidate read must fail")
        } catch {
            guard case .corruptHeader = ArchiveError.from(error: error) else {
                return XCTFail("Candidate corruption was rewritten: \(error)")
            }
        }
        let calls = await reader.calls
        let implicit = await reader.implicitVaultAttempts
        XCTAssertEqual(calls, 2, "The second vault candidate must never be attempted")
        XCTAssertEqual(implicit, 0)
        XCTAssertEqual(vault.readCount, 1)
    }
}
