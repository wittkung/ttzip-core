// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import XCTest
import os
@testable import TTZipCore

/// Exercises the public facade and real bridge with scoped fake symbol access.
/// Tokens are opaque: no native parser, mapping, archive file, vault or release is used.
final class VfsSnapshotBridgeTests: XCTestCase {
    func testMissingSymbolsReportBridgeUnavailableRatherThanMissingFile() async {
        let symbols = SnapshotSymbolScope(availability: .none)
        let facade = makeFacade(symbols)
        do {
            _ = try await facade.openVfsSnapshot(archivePath: "/synthetic/missing.zip")
            XCTFail("A missing bridge must not produce a snapshot handle")
        } catch {
            guard let archiveError = error as? ArchiveError,
                  case .engineFailure(let code, let message) = archiveError else {
                return XCTFail("Expected a bridge error, not fileNotFound: \(error)")
            }
            XCTAssertEqual(code, RustTTZipStatusCode.errUnsupportedFeature.rawValue)
            XCTAssertEqual(message, "VFS snapshot bridge is unavailable")
        }
        XCTAssertEqual(symbols.snapshot.lookups, [SnapshotSymbolScope.compact, SnapshotSymbolScope.legacy])
        XCTAssertTrue(symbols.snapshot.invocations.isEmpty)
        assertScopeClosed(symbols)
    }

    func testCompactSymbolWinsWithoutLookingUpLegacy() async throws {
        let symbols = SnapshotSymbolScope(availability: .both)
        let result = try await makeFacade(symbols).openVfsSnapshot(archivePath: "/synthetic/current.zip")
        XCTAssertEqual(result as? OpaquePointer, symbols.token)
        XCTAssertEqual(symbols.snapshot.lookups, [SnapshotSymbolScope.compact])
        XCTAssertEqual(symbols.snapshot.invocations.map(\.symbol), [SnapshotSymbolScope.compact])
        assertScopeClosed(symbols)
    }

    func testLegacySymbolIsUsedOnlyWhenCompactIsMissing() async throws {
        let symbols = SnapshotSymbolScope(availability: .legacyOnly)
        let result = try await makeFacade(symbols).openVfsSnapshot(archivePath: "/synthetic/legacy.zip")
        XCTAssertEqual(result as? OpaquePointer, symbols.token)
        XCTAssertEqual(symbols.snapshot.lookups, [SnapshotSymbolScope.compact, SnapshotSymbolScope.legacy])
        XCTAssertEqual(symbols.snapshot.invocations.map(\.symbol), [SnapshotSymbolScope.legacy])
        assertScopeClosed(symbols)
    }

    func testNullCompactResultIsOpenFailedWithoutLegacyRetry() async {
        let symbols = SnapshotSymbolScope(availability: .both, returnsNull: true)
        do {
            _ = try await makeFacade(symbols).openVfsSnapshot(
                archivePath: "/synthetic/unknown-cause.zip",
                password: "candidate"
            )
            XCTFail("A null native result must not produce a handle")
        } catch {
            XCTAssertEqual(error as? ArchiveError, .openFailed)
            XCTAssertFalse((error as? ArchiveError)?.isPasswordRelated ?? true)
        }
        XCTAssertEqual(symbols.snapshot.lookups, [SnapshotSymbolScope.compact])
        XCTAssertEqual(symbols.snapshot.invocations.map(\.symbol), [SnapshotSymbolScope.compact])
        assertScopeClosed(symbols)
    }

    func testSuccessfulAnyKeepsTheOpaquePointerDynamicValue() async throws {
        let symbols = SnapshotSymbolScope(availability: .both)
        let result: Any = try await makeFacade(symbols).openVfsSnapshot(archivePath: "/synthetic/value.zip")
        let pointer = try XCTUnwrap(result as? OpaquePointer, "Legacy callers must still be able to cast the Any")
        XCTAssertEqual(pointer, symbols.token)
        XCTAssertEqual(symbols.snapshot.invocations.count, 1)
        assertScopeClosed(symbols)
    }

    func testUnicodePathAndExplicitPasswordAreForwardedUnchanged() async throws {
        let symbols = SnapshotSymbolScope(availability: .both)
        let path = "/synthetic/目录/空 格-é.zip"
        let password = "候选-密碼-ß"
        _ = try await makeFacade(symbols).openVfsSnapshot(archivePath: path, password: password)
        let invocation = try XCTUnwrap(symbols.snapshot.invocations.first)
        XCTAssertEqual(invocation.path, path)
        XCTAssertEqual(invocation.password, password)
        assertScopeClosed(symbols)
    }

    func testDefaultNilPasswordIsNotReplacedByAnotherCredential() async throws {
        let symbols = SnapshotSymbolScope(availability: .both)
        _ = try await makeFacade(symbols).openVfsSnapshot(archivePath: "/synthetic/nil.zip")
        let invocation = try XCTUnwrap(symbols.snapshot.invocations.first)
        XCTAssertNil(invocation.password)
        assertScopeClosed(symbols)
    }

    func testEmptyPasswordRemainsDistinctFromNil() async throws {
        let symbols = SnapshotSymbolScope(availability: .both)
        _ = try await makeFacade(symbols).openVfsSnapshot(archivePath: "/synthetic/empty.zip", password: "")
        let invocation = try XCTUnwrap(symbols.snapshot.invocations.first)
        XCTAssertNotNil(invocation.password)
        XCTAssertEqual(invocation.password, "")
        assertScopeClosed(symbols)
    }

    func testActualInvocationRemainsInsideTheSymbolAccessCallback() async throws {
        let symbols = SnapshotSymbolScope(availability: .both)
        _ = try await makeFacade(symbols).openVfsSnapshot(archivePath: "/synthetic/scope.zip")
        XCTAssertEqual(symbols.snapshot.events, ["enter", "lookup:compact", "invoke:compact", "exit"])
        XCTAssertEqual(symbols.snapshot.invocations.map(\.insideScope), [true])
        assertScopeClosed(symbols)
    }

    private func makeFacade(_ symbols: SnapshotSymbolScope) -> TTZipEngineFacade {
        TTZipEngineFacade(passwordVault: UnusedSnapshotVault(), snapshotBridge: symbols.bridge)
    }

    private func assertScopeClosed(
        _ symbols: SnapshotSymbolScope,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        let snapshot = symbols.snapshot
        XCTAssertEqual(snapshot.entries, 1, file: file, line: line)
        XCTAssertEqual(snapshot.exits, 1, file: file, line: line)
        XCTAssertFalse(snapshot.isInsideScope, file: file, line: line)
        XCTAssertTrue(snapshot.invocations.allSatisfy(\.insideScope), file: file, line: line)
        XCTAssertTrue(snapshot.invocations.allSatisfy(\.outputIsAligned), file: file, line: line)
    }
}

private struct UnusedSnapshotVault: PasswordVaultManaging {
    var autoUnlockArchives: Bool { false }

    func getEntries() -> [PasswordVaultEntry] {
        XCTFail("The snapshot bridge must not query the vault")
        return []
    }

    func recordUsage(id: UUID) {
        XCTFail("The snapshot bridge must not record vault use")
    }
}

/// Uses C strings supplied by the real bridge; the returned snapshot token is never read.
private final class SnapshotSymbolScope: @unchecked Sendable {
    enum Availability: Sendable { case both, legacyOnly, none }

    struct Invocation: Sendable {
        let symbol: String
        let path: String
        let password: String?
        let insideScope: Bool
        let outputIsAligned: Bool
    }

    struct State: Sendable {
        var entries = 0
        var exits = 0
        var isInsideScope = false
        var lookups: [String] = []
        var invocations: [Invocation] = []
        var events: [String] = []
    }

    static let compact = "ttzip_vfs_open_compact_catalog"
    static let legacy = "ttzip_vfs_open_snapshot"
    let token = OpaquePointer(bitPattern: 0x5A23)!
    private let availability: Availability
    private let returnsNull: Bool
    private let state = OSAllocatedUnfairLock(initialState: State())

    init(availability: Availability, returnsNull: Bool = false) {
        self.availability = availability
        self.returnsNull = returnsNull
    }

    var snapshot: State { state.withLock { $0 } }

    var bridge: VfsSnapshotBridge {
        VfsSnapshotBridge(withSymbols: { [self] body in
            enter()
            defer { exit() }
            return try body { [self] name in self.lookup(name) }
        })
    }

    private func enter() {
        state.withLock {
            $0.entries += 1
            $0.isInsideScope = true
            $0.events.append("enter")
        }
    }

    private func exit() {
        state.withLock {
            $0.exits += 1
            $0.isInsideScope = false
            $0.events.append("exit")
        }
    }

    private func lookup(_ name: String) -> VfsSnapshotBridge.OpenFunction? {
        state.withLock {
            $0.lookups.append(name)
            $0.events.append("lookup:" + shortName(name))
        }
        let available: Bool
        switch availability {
        case .both: available = name == Self.compact || name == Self.legacy
        case .legacyOnly: available = name == Self.legacy
        case .none: available = false
        }
        guard available else { return nil }
        return { [self] path, password, view in
            let pathValue = String(cString: path)
            let passwordValue = password.map { String(cString: $0) }
            let aligned = UInt(bitPattern: view).isMultiple(of: 8)
            state.withLock {
                $0.events.append("invoke:" + shortName(name))
                $0.invocations.append(Invocation(
                    symbol: name,
                    path: pathValue,
                    password: passwordValue,
                    insideScope: $0.isInsideScope,
                    outputIsAligned: aligned
                ))
            }
            return returnsNull ? nil : token
        }
    }

    private func shortName(_ name: String) -> String {
        name == Self.compact ? "compact" : "legacy"
    }
}
