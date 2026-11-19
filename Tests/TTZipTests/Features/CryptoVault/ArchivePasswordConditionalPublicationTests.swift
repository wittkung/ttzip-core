// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import XCTest
@testable import TTZipCore
#if canImport(TTZipCoreCrypto)
@testable import TTZipCoreCrypto
#endif

private final class LockedTestCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var value = 0

    func increment() {
        lock.lock()
        value += 1
        lock.unlock()
    }

    func reset() -> Int {
        lock.lock()
        defer { lock.unlock() }
        let current = value
        value = 0
        return current
    }
}

final class ArchivePasswordConditionalPublicationTests: XCTestCase {
    func testMatchingAbsentCandidatePublishesAndMismatchingNewOwnerIsPreserved() {
        let store = ArchivePasswordStore(maxCapacity: 8)
        XCTAssertTrue(store.setPassword("vault", for: "/tmp/mm48-publication.zip", ifCurrentPasswordIs: nil))
        store.setPassword("owner", for: "/tmp/mm48-publication.zip")
        XCTAssertFalse(store.setPassword("late-vault", for: "/tmp/mm48-publication.zip", ifCurrentPasswordIs: "vault"))
        XCTAssertEqual(store.getPassword(for: "/tmp/mm48-publication.zip"), "owner")
    }

    func testEmptyAndAbsentCandidatesAreDistinct() {
        let store = ArchivePasswordStore(maxCapacity: 8)
        store.setPassword("", for: "/tmp/mm48-empty.zip")
        XCTAssertFalse(store.setPassword("vault", for: "/tmp/mm48-empty.zip", ifCurrentPasswordIs: nil))
        XCTAssertEqual(store.getPassword(for: "/tmp/mm48-empty.zip"), "")
        XCTAssertTrue(store.setPassword("vault", for: "/tmp/mm48-empty.zip", ifCurrentPasswordIs: ""))
    }

    func testConditionalPublicationUsesTheSameNormalizedKeyAndLRUCapacity() {
        let store = ArchivePasswordStore(maxCapacity: 1)
        XCTAssertTrue(store.setPassword("vault", for: "file:///tmp/mm48-normalized.zip", ifCurrentPasswordIs: nil))
        XCTAssertEqual(store.getPassword(for: "/tmp/mm48-normalized.zip"), "vault")
        store.setPassword("other", for: "/tmp/mm48-other.zip")
        XCTAssertNil(store.getPassword(for: "/tmp/mm48-normalized.zip"))
        XCTAssertEqual(store.getPassword(for: "/tmp/mm48-other.zip"), "other")
    }

    func testConditionalRemovalEvictsMatchingEntryAndReleasesLRUSlot() {
        let store = ArchivePasswordStore(maxCapacity: 2)
        store.setPassword("stale", for: "/tmp/mm48-evict-a.zip")
        store.setPassword("keep", for: "/tmp/mm48-evict-b.zip")

        XCTAssertTrue(
            store.removePassword(for: "/tmp/mm48-evict-a.zip", ifCurrentPasswordIs: "stale"),
            "Matching cached password must be removed"
        )
        XCTAssertNil(store.getPassword(for: "/tmp/mm48-evict-a.zip"))

        store.setPassword("third", for: "/tmp/mm48-evict-c.zip")
        XCTAssertEqual(
            store.getPassword(for: "/tmp/mm48-evict-b.zip"),
            "keep",
            "Evicting entry A must free its LRU slot so entry B is retained"
        )
        XCTAssertEqual(store.getPassword(for: "/tmp/mm48-evict-c.zip"), "third")
    }

    func testConditionalRemovalPreservesConcurrentlyUpdatedPassword() {
        let store = ArchivePasswordStore(maxCapacity: 8)
        let path = "/tmp/mm48-evict-mismatch.zip"
        store.setPassword("stale", for: path)
        store.setPassword("new-valid", for: path)

        XCTAssertFalse(
            store.removePassword(for: path, ifCurrentPasswordIs: "stale"),
            "Stale conditional removal must not evict a newer valid password"
        )
        XCTAssertEqual(store.getPassword(for: path), "new-valid")
    }

    func testConditionalRemovalReturnsFalseForAbsentPath() {
        let store = ArchivePasswordStore(maxCapacity: 8)
        XCTAssertFalse(store.removePassword(for: "/tmp/mm48-evict-absent.zip", ifCurrentPasswordIs: "stale"))
        XCTAssertFalse(store.removePassword(for: "/tmp/mm48-evict-absent.zip", ifCurrentPasswordIs: ""))
    }

    func testConditionalRemovalUsesNormalizedPathEquivalence() {
        let store = ArchivePasswordStore(maxCapacity: 8)
        store.setPassword("stale", for: "file:///tmp/mm48-evict-normalized.zip")
        XCTAssertFalse(
            store.removePassword(for: "/tmp/mm48-evict-normalized.zip", ifCurrentPasswordIs: "other"),
            "Mismatching expected password under normalized alias must preserve entry"
        )
        XCTAssertTrue(
            store.removePassword(for: "/tmp/mm48-evict-normalized.zip", ifCurrentPasswordIs: "stale"),
            "Matching expected password under normalized alias must remove entry"
        )
        XCTAssertNil(store.getPassword(for: "file:///tmp/mm48-evict-normalized.zip"))

        store.setPassword("stale-space", for: "/tmp/mm48 evict space.zip")
        XCTAssertTrue(
            store.removePassword(for: "file:///tmp/mm48%20evict%20space.zip", ifCurrentPasswordIs: "stale-space"),
            "Percent-encoded file URL must resolve to the same normalized key"
        )
        XCTAssertNil(store.getPassword(for: "/tmp/mm48 evict space.zip"))
    }

    func testConcurrentUpgradeAndStaleConditionalRemovalNeverEvictsValidPassword() {
        let store = ArchivePasswordStore(maxCapacity: 8)
        let path = "/tmp/mm48-concurrent-evict.zip"
        let evictSuccesses = LockedTestCounter()

        // Each round starts with "stale" and races one upgrade to "valid" against concurrent stale removals.
        // Under single-lock linearizability, every round must finish with "valid" and at most one removal hit.
        for round in 0..<400 {
            store.setPassword("stale", for: path)
            DispatchQueue.concurrentPerform(iterations: 6) { worker in
                if worker == 0 {
                    store.setPassword("valid", for: path)
                } else if store.removePassword(for: path, ifCurrentPasswordIs: "stale") {
                    evictSuccesses.increment()
                }
            }
            XCTAssertEqual(
                store.getPassword(for: path),
                "valid",
                "Round \(round): valid password must never be evicted by a racing stale removal"
            )
            XCTAssertLessThanOrEqual(
                evictSuccesses.reset(),
                1,
                "Round \(round): at most one stale removal may succeed before the upgrade"
            )
        }

        let unexpectedEvictions = LockedTestCounter()
        store.setPassword("valid", for: path)
        DispatchQueue.concurrentPerform(iterations: 8) { worker in
            for _ in 0..<250 {
                if worker.isMultiple(of: 2) {
                    store.setPassword("valid", for: path)
                } else if store.removePassword(for: path, ifCurrentPasswordIs: "stale") {
                    unexpectedEvictions.increment()
                }
            }
        }
        XCTAssertEqual(unexpectedEvictions.reset(), 0, "Stale removal must never succeed once valid is stored")
        XCTAssertEqual(store.getPassword(for: path), "valid")
    }
}
