// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import XCTest
@testable import TTZipCore
#if canImport(TTZipCoreCrypto)
@testable import TTZipCoreCrypto
#endif

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
}
