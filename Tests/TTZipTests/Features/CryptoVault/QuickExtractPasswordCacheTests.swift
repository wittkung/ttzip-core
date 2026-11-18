// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import XCTest
@testable import TTZipCore

final class QuickExtractPasswordCacheTests: XCTestCase {
    func testCallerCanOwnPasswordCachePublication() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let source = root.appendingPathComponent("source.txt")
        let archive = root.appendingPathComponent("archive.zip")
        let destination = root.appendingPathComponent("extracted")
        try Data("fixture".utf8).write(to: source)
        try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: true)
        _ = try createArchiveStream(
            sourcePaths: [source.path], outputPath: archive.path, format: .zip,
            level: 6, password: nil, progress: nil, token: nil
        )
        let password = UUID().uuidString
        defer { ArchivePasswordStore.shared.removePassword(for: archive.path) }

        let result = try await TTZipEngineFacade.shared.quickExtract(
            archivePath: archive.path, destinationDir: destination.path,
            password: password, autoVaultUnlock: false, cachePasswordOnSuccess: false
        )

        XCTAssertEqual(result.unlockedPassword, password)
        XCTAssertNil(ArchivePasswordStore.shared.getPassword(for: archive.path))
    }
}
