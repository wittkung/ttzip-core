// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

extension TTZipEngineFacade {
    /// Drives an extraction session through its conflict decisions, staging and commit.
    public func extract(
        _ request: ExtractionRequest,
        handling: CollisionHandling,
        progress: ProgressSink? = nil,
        token: CancellationToken? = nil
    ) async throws -> ExtractionOutcome {
        try await ArchiveExtractionSessionDriver.extract(request, handling: handling, progress: progress, token: token)
    }

    /// Extracts one entry (a directory entry includes its descendants) directly into `destinationDir`.
    @discardableResult
    public func extractSingleEntry(
        archivePath: String,
        entryPath: String,
        destinationDir: String,
        password: String? = nil,
        handling: CollisionHandling = .policy(.keepBoth)
    ) async throws -> ExtractionOutcome {
        try await ArchiveExtractionSessionDriver.extractSingleEntry(
            archivePath: archivePath,
            entryPath: entryPath,
            destinationDir: destinationDir,
            password: password,
            handling: handling
        )
    }

    /// Extracts the selected entries, plus every descendant of a selected directory, through the
    /// collision-safe session loop. Entries are placed relative to their deepest common parent:
    /// selecting `docs/report.pdf` writes `<destinationDir>/report.pdf`.
    @discardableResult
    public func extractEntries(
        archivePath: String,
        entryPaths: [String],
        destinationDir: String,
        password: String?,
        handling: CollisionHandling = .policy(.keepBoth),
        progress: ProgressSink? = nil
    ) async throws -> ExtractionOutcome {
        try await ArchiveExtractionSessionDriver.extractEntries(
            archivePath: archivePath,
            entryPaths: entryPaths,
            destinationDir: destinationDir,
            password: password,
            handling: handling,
            progress: progress
        )
    }

    internal func executePipelineExtract(
        archivePath: String,
        destinationDir: String,
        password: String?,
        progress: (@Sendable (ArchiveProgress) -> Void)?,
        token: CancellationToken? = nil
    ) async throws -> Double {
        let startTime = CFAbsoluteTimeGetCurrent()
        let request = ExtractionRequest(
            archivePath: archivePath,
            destinationRoot: destinationDir,
            password: password,
            entryFilter: nil,
            rootAtCommonParent: false,
            policy: .keepBoth
        )
        let outcome = try await extract(request, handling: .policy(.keepBoth), progress: progress, token: token)
        switch outcome.status {
        case .stopped:
            throw ArchiveError.extractionStopped
        case .completedWithFailures:
            throw ArchiveError.partialExtractionFailure(outcome: outcome)
        case .completed:
            return max(0.001, CFAbsoluteTimeGetCurrent() - startTime)
        }
    }
}
