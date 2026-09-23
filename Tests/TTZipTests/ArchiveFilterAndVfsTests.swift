// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import XCTest
import Foundation
@testable import TTZipCore

final class ArchiveFilterAndVfsTests: XCTestCase {
    
    // MARK: - 1. ArchiveFilter Rust C-ABI Direct Evaluator Tests
    
    func testArchiveFilterDSLDirectEvaluation() {
        let entry1 = ArchiveEntry(
            path: "images/photo.jpg",
            uncompressedSize: 5000,
            isDirectory: false,
            modificationDate: Date(timeIntervalSince1970: 10000)
        )
        let entry2 = ArchiveEntry(
            path: "docs/manual.pdf",
            uncompressedSize: 200,
            isDirectory: false,
            modificationDate: Date(timeIntervalSince1970: 10000)
        )
        
        let filterJpg = ArchiveFilter(expression: "ext:jpg")
        XCTAssertTrue(filterJpg.evaluate(entry: entry1))
        XCTAssertFalse(filterJpg.evaluate(entry: entry2))
        
        let filterSize = ArchiveFilter(expression: "size:>1000")
        XCTAssertTrue(filterSize.evaluate(entry: entry1))
        XCTAssertFalse(filterSize.evaluate(entry: entry2))
        
        let filterCompound = ArchiveFilter(expression: "ext:jpg AND size:>1000")
        XCTAssertTrue(filterCompound.evaluate(entry: entry1))
        XCTAssertFalse(filterCompound.evaluate(entry: entry2))
    }
    
    func testArchiveFilterStaticHelpersAndBatchFiltering() {
        let entry1 = ArchiveEntry(path: "src/main.rs", uncompressedSize: 1024, isDirectory: false)
        let entry2 = ArchiveEntry(path: "src/lib.swift", uncompressedSize: 2048, isDirectory: false)
        let entry3 = ArchiveEntry(path: "README.md", uncompressedSize: 512, isDirectory: false)
        let entries = [entry1, entry2, entry3]
        
        XCTAssertTrue(ArchiveFilter.evaluate(expression: "ext:rs", entry: entry1))
        XCTAssertFalse(ArchiveFilter.evaluate(expression: "ext:rs", entry: entry2))
        XCTAssertTrue(ArchiveFilter.evaluate(entry: entry2, query: "ext:swift"))
        
        let filtered = ArchiveFilter.filter(entries: entries, expression: "ext:rs OR ext:swift")
        XCTAssertEqual(filtered.count, 2)
        XCTAssertTrue(filtered.contains { $0.path == "src/main.rs" })
        XCTAssertTrue(filtered.contains { $0.path == "src/lib.swift" })
        
        // Empty expression matches all
        let all = ArchiveFilter.filter(entries: entries, expression: "")
        XCTAssertEqual(all.count, 3)
    }
    
    func testArchiveFilterDSLInterpreterFacadeAndOptionsIntegration() {
        let entry = ArchiveEntry(path: "test/unit.log", uncompressedSize: 300, isDirectory: false)
        
        XCTAssertTrue(ArchiveFilter.evaluate(entry: entry, query: "ext:log"))
        XCTAssertFalse(ArchiveFilter.evaluate(entry: entry, query: "ext:txt"))
        
        let options = ArchiveFilterOptions(skipMacJunk: true)
        XCTAssertTrue(options.matches(entry: entry, dslQuery: "ext:log"))
        XCTAssertFalse(options.matches(entry: entry, dslQuery: "ext:txt"))
        
        let macJunkEntry = ArchiveEntry(path: "__MACOSX/._test.log", uncompressedSize: 100, isDirectory: false)
        XCTAssertFalse(options.matches(entry: macJunkEntry, dslQuery: "ext:log"))
    }
    
    // MARK: - 2. RustVfsBridge & ArchiveReader VFS Integration Tests
    
    func testRustVfsBridgeRenderTreeAndFuzzySearch() {
        let entry1 = ArchiveEntry(path: "src/engine/core.rs", uncompressedSize: 4096, isDirectory: false)
        let entry2 = ArchiveEntry(path: "src/engine/vfs.rs", uncompressedSize: 2048, isDirectory: false)
        let entry3 = ArchiveEntry(path: "docs/architecture.md", uncompressedSize: 1024, isDirectory: false)
        let entries = [entry1, entry2, entry3]
        
        let rendered = RustVfsBridge.renderTree(from: entries, rootName: "my_project")
        XCTAssertFalse(rendered.isEmpty)
        XCTAssertTrue(rendered.contains("core.rs") || rendered.contains("src"))
        
        let searchResults = RustVfsBridge.fuzzySearch(in: entries, query: "core")
        XCTAssertEqual(searchResults.count, 1)
        XCTAssertEqual(searchResults.first?.path, "src/engine/core.rs")
        
        let stats = RustVfsBridge.getStats(from: entries, rootName: "my_project")
        XCTAssertNotNil(stats)
        XCTAssertEqual(stats?.totalFiles, 3)
        XCTAssertEqual(stats?.totalSize, 4096 + 2048 + 1024)
    }

    func testDirectArchiveVfsTreeHandleAndPagination() throws {
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        // Create sample folder structure
        let subDir = tempDir.appendingPathComponent("assets")
        try FileManager.default.createDirectory(at: subDir, withIntermediateDirectories: true)
        let file1 = tempDir.appendingPathComponent("README.md")
        let file2 = subDir.appendingPathComponent("logo.png")
        try "Hello VFS".data(using: .utf8)?.write(to: file1)
        try "PNG_BYTES".data(using: .utf8)?.write(to: file2)

        let archiveUrl = tempDir.appendingPathComponent("test.zip")
        let writer = ArchiveWriter()
        try writer.createArchiveSync(
            outputPath: archiveUrl.path,
            format: .zip,
            level: .fast,
            inputPaths: [file1.path, file2.path]
        )

        // 1. Direct loadArchiveVfsTree from Rust
        let tree = try loadArchiveVfsTree(archivePath: archiveUrl.path, password: nil)
        XCTAssertGreaterThan(tree.totalEntries(), 0)

        // 2. Direct session caching via RustVfsBridge
        let session = RustVfsBridge.session(forArchivePath: archiveUrl.path)
        XCTAssertNotNil(session)
        XCTAssertEqual(session?.archivePath, archiveUrl.path)

        // 3. Direct paged queries from archive path
        let pagedRoot = RustVfsBridge.getChildrenPaged(archivePath: archiveUrl.path, subpath: nil, offset: 0, limit: 10)
        XCTAssertGreaterThan(pagedRoot.total, 0)
        XCTAssertFalse(pagedRoot.nodes.isEmpty)

        // 4. ArchiveTreeNode root nodes backed by UniFFIVfsTree
        let rootNodes = ArchiveTreeNode.buildRootNodes(from: tree, limit: 10)
        XCTAssertFalse(rootNodes.isEmpty)
        for node in rootNodes {
            XCTAssertNotNil(node.vfsTree)
            if node.isDirectory {
                let (children, count) = node.getChildrenPaged(offset: 0, limit: 10)
                XCTAssertGreaterThan(count, 0)
                XCTAssertFalse(children.isEmpty)
            }
        }

        // 5. Zero-allocation fuzzy search on UniFFIVfsTree
        let searchResults = ArchiveTreeNode.search(in: tree, query: "logo", maxResults: 10)
        XCTAssertEqual(searchResults.count, 1)
        XCTAssertTrue(searchResults.first?.name.contains("logo") == true)

        // 6. Direct tree rendering from archive path
        let rendered = RustVfsBridge.renderTree(archivePath: archiveUrl.path)
        XCTAssertFalse(rendered.isEmpty)

        // 7. Session removal and cleanup
        RustVfsBridge.removeSession(for: archiveUrl.path)
    }
    
    // MARK: - 3. System Metadata Cleaning & PaxHeader Invariant Tests
    
    func testIsSystemMetadataComprehensive() {
        // macOS & AppleDouble
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "__MACOSX"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "__MACOSX/._file.txt"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "folder/__MACOSX/._image.png"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "._metadata"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "sub/._hidden"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: ".DS_Store"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "docs/.DS_Store"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: ".localized"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: ".VolumeIcon.icns"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: ".Spotlight-V100/Store"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: ".Trashes/501/item"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: ".fseventsd/log"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: ".TemporaryItems/temp"))
        
        // POSIX PaxHeader
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "PaxHeader/file.txt"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "PaxHeaders.0/entry"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "subfolder/PaxHeader/data"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "PaxHeader"))
        
        // Windows system metadata
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "Thumbs.db"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "sub/thumbs.db"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "desktop.ini"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "ehthumbs.db"))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "$RECYCLE.BIN/file"))
        
        // Legitimate files MUST NOT be treated as system metadata
        XCTAssertFalse(ArchiveFilterOptions.isSystemMetadata(path: "main.swift"))
        XCTAssertFalse(ArchiveFilterOptions.isSystemMetadata(path: "Sources/PaxHeaderHelper.swift"))
        XCTAssertFalse(ArchiveFilterOptions.isSystemMetadata(path: "Docs/DS_Store.txt"))
        XCTAssertFalse(ArchiveFilterOptions.isSystemMetadata(path: "images/photo.png"))
    }

    // MARK: - 4. P1-A Model & Bridge Verification Tests

    func testArchiveLeafFileValueSemanticsAndSendable() {
        let leaf1 = ArchiveLeafFile(name: "test.txt", path: "docs/test.txt", sizeBytes: 1024)
        var leaf2 = leaf1
        XCTAssertEqual(leaf1, leaf2)
        XCTAssertEqual(leaf1.sizeBytes, 1024)
        XCTAssertFalse(leaf1.isDirectory)
        XCTAssertEqual(leaf1.getChildren().count, 0)

        // Struct value copy mutation does not affect original
        leaf2 = ArchiveLeafFile(name: "test2.txt", path: "docs/test2.txt", sizeBytes: 2048)
        XCTAssertNotEqual(leaf1, leaf2)
        XCTAssertEqual(leaf1.sizeBytes, 1024)
        XCTAssertEqual(leaf2.sizeBytes, 2048)
    }

    func testVfsSessionManagerContentFingerprintingAndIsolation() {
        VfsSessionManager.shared.clearCache()

        let catalogA = [
            ArchiveEntry(path: "a.txt", uncompressedSize: 100, isDirectory: false),
            ArchiveEntry(path: "b.txt", uncompressedSize: 200, isDirectory: false)
        ]
        let catalogB = [
            ArchiveEntry(path: "x.txt", uncompressedSize: 500, isDirectory: false),
            ArchiveEntry(path: "y.txt", uncompressedSize: 600, isDirectory: false)
        ]

        let fpA = VfsSessionManager.computeFingerprint(for: catalogA)
        let fpB = VfsSessionManager.computeFingerprint(for: catalogB)
        XCTAssertNotEqual(fpA, fpB, "Fingerprints for different catalogs must never collide")

        let sessionA = VfsSessionManager.shared.session(for: catalogA)
        let sessionA2 = VfsSessionManager.shared.session(for: catalogA)
        XCTAssertTrue(sessionA === sessionA2, "Repeated calls with identical catalogs must return cached session")

        let sessionB = VfsSessionManager.shared.session(for: catalogB)
        XCTAssertFalse(sessionA === sessionB, "Distinct catalogs must yield distinct sessions")

        // Explicit identifier isolation
        let sessionPathA = VfsSessionManager.shared.session(for: catalogA, identifier: "/path/archive1.zip")
        let sessionPathB = VfsSessionManager.shared.session(for: catalogA, identifier: "/path/archive2.zip")
        XCTAssertFalse(sessionPathA === sessionPathB, "Distinct physical paths must yield isolated sessions")

        VfsSessionManager.shared.clearCache()
        let sessionAfterClear = VfsSessionManager.shared.session(for: catalogA)
        XCTAssertFalse(sessionA === sessionAfterClear, "Cache clear must purge existing sessions")
    }

    func testArchiveReaderErrorHandling() async {
        let reader = ArchiveReader()

        // Non-existent path must throw ArchiveError.fileNotFound
        do {
            _ = try await reader.inspect(archivePath: "/nonexistent/path/never_exists.zip", password: nil)
            XCTFail("Must throw fileNotFound for nonexistent file")
        } catch let error as ArchiveError {
            XCTAssertEqual(error, .fileNotFound)
        } catch {
            XCTFail("Unexpected error type: \(error)")
        }
    }
}
