// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import XCTest
@testable import TTZipCore

/// Selected-entry extraction (single entry, selection, directory entry) through the collision-safe session loop.
final class SelectiveExtractionCollisionTests: XCTestCase {
    private var sandbox: URL!
    private var archivePath: String!

    /// Archive layout under test: every payload is unique so byte comparisons identify the source entry.
    private static let payloads: [String: String] = [
        "docs/report.txt": "archived report payload",
        "docs/a.txt": "archived a",
        "docs/b.txt": "archived b",
        "docs/c.txt": "archived c",
        "docs/d.txt": "archived d",
        "docs/e.txt": "archived e",
        "docs/img/one.png": "archived image one",
        "docs/img/two.png": "archived image two",
        "notes/todo.txt": "archived todo",
    ]

    override func setUpWithError() throws {
        try super.setUpWithError()
        let fm = FileManager.default
        sandbox = fm.temporaryDirectory.appendingPathComponent("ttzip_selective_collision_\(UUID().uuidString)")
        let source = sandbox.appendingPathComponent("source", isDirectory: true)
        for (relativePath, payload) in Self.payloads {
            let fileURL = source.appendingPathComponent(relativePath)
            try fm.createDirectory(at: fileURL.deletingLastPathComponent(), withIntermediateDirectories: true)
            try Data(payload.utf8).write(to: fileURL)
        }

        archivePath = sandbox.appendingPathComponent("fixture.zip").path
        _ = try createArchiveStream(
            sourcePaths: [source.appendingPathComponent("docs").path, source.appendingPathComponent("notes").path],
            outputPath: archivePath,
            format: ArchiveFormat.zip,
            level: 6,
            password: nil,
            progress: nil,
            token: nil
        )
        let entryPaths = Set(try inspectArchiveEntries(archivePath: archivePath, password: nil).map(\.path))
        XCTAssertTrue(
            Set(Self.payloads.keys).isSubset(of: entryPaths),
            "Fixture archive must contain the nested layout; got \(entryPaths.sorted())"
        )
    }

    override func tearDownWithError() throws {
        if let sandbox {
            try? FileManager.default.removeItem(at: sandbox)
        }
        try super.tearDownWithError()
    }

    // MARK: - (1) Single entry, Keep Both

    func testSingleEntryKeepBothPreservesEditedFileAndWritesSibling() async throws {
        let destination = try makeDestination("keep_both")
        let edited = destination.appendingPathComponent("report.txt")
        let editedBytes = Data("locally edited report".utf8)
        try editedBytes.write(to: edited)

        let outcome = try await TTZipEngineFacade.shared.extractSingleEntry(
            archivePath: archivePath,
            entryPath: "docs/report.txt",
            destinationDir: destination.path,
            password: nil,
            handling: .policy(.keepBoth)
        )

        XCTAssertEqual(outcome.status, .completed)
        XCTAssertEqual(outcome.keptBoth, 1)
        XCTAssertEqual(try Data(contentsOf: edited), editedBytes, "Existing file must stay byte-identical")
        let sibling = destination.appendingPathComponent("report 2.txt")
        XCTAssertEqual(try String(contentsOf: sibling, encoding: .utf8), Self.payloads["docs/report.txt"])
        XCTAssertEqual(try tree(of: destination), ["report 2.txt", "report.txt"])
    }

    // MARK: - (2) Stop is all-or-nothing

    func testStopPolicyWritesNoneOfTheSelectedEntriesWhenAnyConflicts() async throws {
        let destination = try makeDestination("stop")
        let existingA = Data("existing a".utf8)
        let existingB = Data("existing b".utf8)
        try existingA.write(to: destination.appendingPathComponent("a.txt"))
        try existingB.write(to: destination.appendingPathComponent("b.txt"))

        let outcome = try await ArchiveSelectiveExtractor.shared.extractSelected(
            archivePath: archivePath,
            targetEntryPaths: ["docs/a.txt", "docs/b.txt", "docs/c.txt", "docs/d.txt", "docs/e.txt"],
            destinationDir: destination.path,
            handling: .policy(.stop)
        )

        XCTAssertEqual(outcome.status, .stopped)
        XCTAssertEqual(try tree(of: destination), ["a.txt", "b.txt"], "No selected entry may be written")
        XCTAssertEqual(try Data(contentsOf: destination.appendingPathComponent("a.txt")), existingA)
        XCTAssertEqual(try Data(contentsOf: destination.appendingPathComponent("b.txt")), existingB)
    }

    // MARK: - (3) Root at common parent

    func testNestedEntryIsPlacedAtDestinationRoot() async throws {
        let destination = try makeDestination("common_parent")

        let outcome = try await ArchiveSelectiveExtractor.shared.extractSelected(
            archivePath: archivePath,
            targetEntryPaths: ["docs/img/one.png"],
            destinationDir: destination.path
        )

        XCTAssertEqual(outcome.status, .completed)
        XCTAssertEqual(outcome.extracted, 1)
        XCTAssertEqual(try tree(of: destination), ["one.png"])
        XCTAssertEqual(
            try String(contentsOf: destination.appendingPathComponent("one.png"), encoding: .utf8),
            Self.payloads["docs/img/one.png"]
        )
    }

    // MARK: - (4) Directory entry selects descendants

    func testDirectoryEntryExtractsItsDescendants() async throws {
        let destination = try makeDestination("directory")

        let outcome = try await ArchiveExtractor().extractSingleFile(
            archivePath: archivePath,
            entryPath: "docs/img/",
            destinationDir: destination.path,
            password: nil,
            handling: .policy(.keepBoth)
        )

        XCTAssertEqual(outcome.status, .completed)
        let files = try tree(of: destination).filter { !$0.hasSuffix("/") }
        XCTAssertEqual(files, ["img/one.png", "img/two.png"])
        XCTAssertEqual(
            try String(contentsOf: destination.appendingPathComponent("img/two.png"), encoding: .utf8),
            Self.payloads["docs/img/two.png"]
        )
    }

    // MARK: - Helpers

    private func makeDestination(_ name: String) throws -> URL {
        let url = sandbox.appendingPathComponent("dest_\(name)", isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    /// Sorted relative paths under `root`; directories carry a trailing "/".
    private func tree(of root: URL) throws -> [String] {
        let fm = FileManager.default
        let relativePaths = try fm.subpathsOfDirectory(atPath: root.path)
        return relativePaths.map { relativePath in
            var isDirectory: ObjCBool = false
            _ = fm.fileExists(atPath: root.appendingPathComponent(relativePath).path, isDirectory: &isDirectory)
            return isDirectory.boolValue ? relativePath + "/" : relativePath
        }.sorted()
    }
}
