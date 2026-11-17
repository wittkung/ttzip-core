// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import TTZipCoreBindings
import TTZipCoreTypes

public enum ArchiveRuntime {
    public static func entrySizes(
        archivePath: String,
        password: String?
    ) throws -> [(path: String, size: UInt64)] {
        try inspectArchiveEntries(archivePath: archivePath, password: password).map {
            (path: $0.path, size: $0.uncompressedSize)
        }
    }

    public static func readEntry(
        archivePath: String,
        entryPath: String,
        password: String?
    ) async throws -> Data? {
        try await ArchiveSelectiveExtractor.shared.extractPreviewEntryData(
            archivePath: archivePath, entryPath: entryPath, password: password
        )
    }

    @discardableResult
    public static func extractSingleEntry(
        archivePath: String,
        entryPath: String,
        destinationDir: String,
        password: String? = nil,
        handling: CollisionHandling = .policy(.keepBoth)
    ) async throws -> ExtractionOutcome {
        try await ArchiveExtractionSessionDriver.extractSingleEntry(
            archivePath: archivePath, entryPath: entryPath, destinationDir: destinationDir,
            password: password, handling: handling
        )
    }
}
