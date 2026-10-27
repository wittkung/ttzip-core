// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import XCTest
@testable import TTZipCore

/// Ownership of the shared password cache during archive inspection (MM48).
///
/// Contract (products/ttzip/docs/twin/multiwindow-and-password-recovery.md §7):
///   * an inspection is a query: it may read the shared cache to pick a candidate and it keeps
///     the vault retry policy, but it must never publish, restore or remove a shared credential;
///   * the credential that worked is reported in `unlockedPassword`; the owner that accepts the
///     result commits it in its own acceptance block;
///   * a query that is in flight must not disturb a credential another owner committed while it
///     was running, neither by publishing its own candidate nor by restoring an older one.
///
/// The tests drive the real production facade with an injected reader and vault. The reader is
/// paused at a controlled gate so the interleaving is deterministic: no sleeps, no timing races.
final class ArchiveInspectionSharedCacheOwnershipTests: XCTestCase {
    private actor InspectionReceipt<Value: Sendable> {
        private var hasStored: Bool = false
        private var stored: Value?
        private var waiters: [CheckedContinuation<Value?, Never>] = []
        private var waiterIDs: [UUID] = []

        /// Records the outcome of the observed operation. The first outcome wins.
        func store(_ value: Value) {
            guard !hasStored else { return }
            hasStored = true
            stored = value
            let pending = waiters
            waiters = []
            waiterIDs = []
            for waiter in pending { waiter.resume(returning: value) }
        }

        /// The outcome recorded so far, without waiting.
        func recorded() -> Value? {
            hasStored ? stored : nil
        }

        /// Waits for the outcome, bounded by `timeout`.
        ///
        /// Returns `nil` when the budget expired before an outcome arrived. The caller keeps the
        /// responsibility to release whatever the operation is blocked on.
        func waitForValue(timeout: Duration) async -> Value? {
            if hasStored { return stored }
            let id = UUID()
            return await withCheckedContinuation { (continuation: CheckedContinuation<Value?, Never>) in
                waiters.append(continuation)
                waiterIDs.append(id)
                // The task inherits the actor isolation, so the expiry runs on the actor itself.
                Task {
                    try? await Task.sleep(for: timeout)
                    self.expire(id: id)
                }
            }
        }

        /// Resumes a waiter with `nil` when its budget expired.
        private func expire(id: UUID) {
            guard let index = waiterIDs.firstIndex(of: id) else { return }
            waiterIDs.remove(at: index)
            let waiter = waiters.remove(at: index)
            waiter.resume(returning: nil)
        }
    }

    // MARK: - Controlled environment

    /// A one-shot suspension point. Every wait is bounded and the gate is always released.
    private final class InspectionGate: @unchecked Sendable {
        private let lock = NSLock()
        private var waiter: CheckedContinuation<Void, Never>?
        private var released = false
        private let enteredSemaphore = DispatchSemaphore(value: 0)

        func suspend() async {
            enteredSemaphore.signal()
            await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
                install(continuation)
            }
        }

        private func install(_ continuation: CheckedContinuation<Void, Never>) {
            lock.lock()
            if released {
                lock.unlock()
                continuation.resume()
                return
            }
            waiter = continuation
            lock.unlock()
        }

        /// Bounded wait for "the reader reached the suspension point".
        func waitUntilEntered(timeout: TimeInterval) -> Bool {
            enteredSemaphore.wait(timeout: .now() + timeout) == .success
        }

        /// Releases the gate. Called unconditionally, including on the failure path.
        func release() {
            lock.lock()
            released = true
            let continuation = waiter
            waiter = nil
            lock.unlock()
            continuation?.resume()
        }
    }

    /// Injected reader: succeeds only for the accepted passwords, and may pause at a gate.
    private actor ControlledReader: ArchiveReading {
        private let acceptedPasswords: Set<String>
        private let failure: ArchiveError
        private let gate: InspectionGate?

        init(
            acceptedPasswords: Set<String>,
            failure: ArchiveError = .passwordRequired,
            gate: InspectionGate? = nil
        ) {
            self.acceptedPasswords = acceptedPasswords
            self.failure = failure
            self.gate = gate
        }

        func inspect(archivePath: String) async throws -> [ArchiveEntry] {
            try await inspect(archivePath: archivePath, password: nil, candidatePasswords: nil)
        }

        func inspect(archivePath: String, password: String?, candidatePasswords: [String]?) async throws -> [ArchiveEntry] {
            if let gate {
                await gate.suspend()
            }
            guard let password, acceptedPasswords.contains(password) else {
                throw failure
            }
            return [ArchiveEntry(path: "entry.txt", uncompressedSize: 1, isDirectory: false)]
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

    /// Injected vault: a fixed candidate list plus a usage counter.
    private final class ControlledVault: PasswordVaultManaging, @unchecked Sendable {
        private let lock = NSLock()
        private var entries: [PasswordVaultEntry]
        private var usages: [UUID] = []

        var autoUnlockArchives: Bool { true }

        init(entries: [PasswordVaultEntry] = []) {
            self.entries = entries
        }

        func getEntries() -> [PasswordVaultEntry] {
            lock.lock(); defer { lock.unlock() }
            return entries
        }

        func recordUsage(id: UUID) {
            lock.lock(); defer { lock.unlock() }
            usages.append(id)
        }

        var recordedUsages: [UUID] {
            lock.lock(); defer { lock.unlock() }
            return usages
        }
    }

    // MARK: - Fixtures

    /// A unique archive path per case. The shared cache is never cleared globally: each case
    /// removes only its own entry, so a parallel case is never disturbed.
    private func makeFixture() throws -> (url: URL, path: String) {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("mm48-ownership-\(UUID().uuidString)")
        try Data([0x54, 0x54]).write(to: url, options: .withoutOverwriting)
        return (url, url.path)
    }

    private func cleanUp(_ url: URL, path: String) {
        ArchivePasswordStore.shared.removePassword(for: path)
        try? FileManager.default.removeItem(at: url)
    }

    /// Preserve the real facade outcome while making missing entry/completion observable.
    private func observedInspection(
        facade: TTZipEngineFacade, path: String, password: String?, gate: InspectionGate,
        whileSuspended: @Sendable () -> Void
    ) async throws -> ArchiveInspectionResult {
        let receipt = InspectionReceipt<Result<ArchiveInspectionResult, Error>>()
        let task = Task.detached {
            do {
                let result = try await facade.inspectArchive(
                    archivePath: path, password: password, autoVaultUnlock: false
                )
                await receipt.store(.success(result))
            } catch { await receipt.store(.failure(error)) }
        }
        defer { gate.release(); task.cancel() }
        let entered = gate.waitUntilEntered(timeout: 5.0)
        if entered { whileSuspended() }
        gate.release()
        let outcome = await receipt.waitForValue(timeout: .seconds(5))
        XCTAssertTrue(entered, "The injected reader must reach the controlled suspension point in budget")
        return try XCTUnwrap(outcome, "Inspection did not complete after releasing its gate").get()
    }

    // MARK: - Query never publishes

    func testSuccessfulInspectionReportsCredentialWithoutPublishingIt() async throws {
        let (url, path) = try makeFixture()
        defer { cleanUp(url, path: path) }
        let reader = ControlledReader(acceptedPasswords: ["query-pw"])
        let vault = ControlledVault()
        let facade = TTZipEngineFacade(reader: reader, passwordVault: vault)

        let result = try await facade.inspectArchive(
            archivePath: path,
            password: "query-pw",
            autoVaultUnlock: false
        )

        XCTAssertEqual(result.unlockedPassword, "query-pw")
        XCTAssertNil(
            ArchivePasswordStore.shared.getPassword(for: path),
            "An inspection must not publish the credential it used"
        )
    }

    func testVaultDiscoveryReportsCredentialWithoutPublishingItAndStillRecordsUsage() async throws {
        let (url, path) = try makeFixture()
        defer { cleanUp(url, path: path) }
        let reader = ControlledReader(acceptedPasswords: ["vault-pw"])
        let vault = ControlledVault(entries: [PasswordVaultEntry(label: "Synthetic", password: "vault-pw")])
        let facade = TTZipEngineFacade(reader: reader, passwordVault: vault)

        let result = try await facade.inspectArchive(
            archivePath: path,
            password: nil,
            autoVaultUnlock: true
        )

        XCTAssertEqual(result.unlockedPassword, "vault-pw")
        XCTAssertNil(
            ArchivePasswordStore.shared.getPassword(for: path),
            "A vault discovery must not publish the credential before its owner accepts the result"
        )
        XCTAssertEqual(
            vault.recordedUsages.count,
            1,
            "The vault usage of a discovered credential keeps its existing semantics"
        )
    }

    func testExplicitWrongPasswordKeepsAnotherOwnersCredentialAndNeverConsultsTheVault() async throws {
        let (url, path) = try makeFixture()
        defer { cleanUp(url, path: path) }
        // Another owner already committed a credential for this path.
        ArchivePasswordStore.shared.setPassword("owner-pw", for: path)

        let reader = ControlledReader(acceptedPasswords: ["vault-pw"], failure: .wrongPassword(archivePath: path))
        let vault = ControlledVault(entries: [PasswordVaultEntry(label: "Synthetic", password: "vault-pw")])
        let facade = TTZipEngineFacade(reader: reader, passwordVault: vault)

        do {
            _ = try await facade.inspectArchive(
                archivePath: path,
                password: "wrong-pw",
                autoVaultUnlock: true
            )
            XCTFail("An explicit wrong credential must fail")
        } catch {
            guard case .wrongPassword = ArchiveError.from(error: error) else {
                return XCTFail("Wrong-password classification was lost: \(error)")
            }
        }

        XCTAssertEqual(vault.recordedUsages.count, 0, "An explicit credential owns its failure contract")
        XCTAssertEqual(
            ArchivePasswordStore.shared.getPassword(for: path),
            "owner-pw",
            "A failed query must not remove another owner's credential"
        )
    }

    // MARK: - A query in flight must not disturb a concurrent commit

    func testPausedQueryDoesNotOverwriteAConcurrentCommitWhenItSucceeds() async throws {
        let (url, path) = try makeFixture()
        defer { cleanUp(url, path: path) }
        let gate = InspectionGate()
        let reader = ControlledReader(acceptedPasswords: ["query-pw"], gate: gate)
        let vault = ControlledVault()
        let facade = TTZipEngineFacade(reader: reader, passwordVault: vault)

        let result = try await observedInspection(
            facade: facade, path: path, password: "query-pw", gate: gate
        ) {
            ArchivePasswordStore.shared.setPassword("owner-pw", for: path)
        }
        XCTAssertEqual(result.unlockedPassword, "query-pw")
        XCTAssertEqual(
            ArchivePasswordStore.shared.getPassword(for: path),
            "owner-pw",
            "A query must not overwrite a credential another owner committed while it was running"
        )
    }

    func testPausedQueryDoesNotOverwriteAConcurrentCommitWhenItFails() async throws {
        let (url, path) = try makeFixture()
        defer { cleanUp(url, path: path) }
        let gate = InspectionGate()
        let reader = ControlledReader(acceptedPasswords: ["other-pw"], failure: .wrongPassword(archivePath: path), gate: gate)
        let vault = ControlledVault()
        let facade = TTZipEngineFacade(reader: reader, passwordVault: vault)

        do {
            _ = try await observedInspection(
                facade: facade, path: path, password: "query-pw", gate: gate
            ) {
                ArchivePasswordStore.shared.setPassword("owner-pw", for: path)
            }
            XCTFail("The query must fail with the wrong credential")
        } catch {
            guard case .wrongPassword = ArchiveError.from(error: error) else {
                return XCTFail("Wrong-password classification was lost: \(error)")
            }
        }
        XCTAssertEqual(
            ArchivePasswordStore.shared.getPassword(for: path),
            "owner-pw",
            "A failed query must not remove a credential another owner committed while it was running"
        )
    }

    func testPausedQueryDoesNotRestoreAnOlderCachedCandidateOverANewerCommit() async throws {
        let (url, path) = try makeFixture()
        defer { cleanUp(url, path: path) }
        // The cache holds an older credential that this query will read as its candidate.
        ArchivePasswordStore.shared.setPassword("old-pw", for: path)

        let gate = InspectionGate()
        let reader = ControlledReader(acceptedPasswords: ["old-pw"], gate: gate)
        let vault = ControlledVault()
        let facade = TTZipEngineFacade(reader: reader, passwordVault: vault)

        let result = try await observedInspection(
            facade: facade, path: path, password: nil, gate: gate
        ) {
            ArchivePasswordStore.shared.setPassword("new-pw", for: path)
        }
        XCTAssertEqual(result.unlockedPassword, "old-pw")
        XCTAssertEqual(
            ArchivePasswordStore.shared.getPassword(for: path),
            "new-pw",
            "A query must not restore the candidate it read before its await over a newer commit"
        )
    }
}
