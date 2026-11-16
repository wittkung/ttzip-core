// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import TTZipCoreBindings
import TTZipCoreCrypto
import TTZipCoreTypes
import Foundation

private final class ExtractionSessionProgressRelay: UniFfiProgressCallback, @unchecked Sendable {
    private let startTime: Date
    private let handler: ProgressSink?
    private let externalToken: CancellationToken?
    private let stageCancelToken: UniFfiCancellationToken?

    init(
        handler: ProgressSink?,
        externalToken: CancellationToken? = nil,
        stageCancelToken: UniFfiCancellationToken? = nil
    ) {
        self.startTime = Date()
        self.handler = handler
        self.externalToken = externalToken
        self.stageCancelToken = stageCancelToken
    }

    func onProgress(processedBytes: UInt64, totalBytes: UInt64, currentEntry: String?) -> Bool {
        if externalToken?.isCancelled() == true {
            stageCancelToken?.cancel()
            return false
        }
        guard let handler = handler else { return true }
        let duration = max(0.001, Date().timeIntervalSince(startTime))
        let throughput = (Double(processedBytes) / (1024 * 1024)) / duration
        handler(ArchiveProgress(
            state: .processing,
            bytesProcessed: processedBytes.clampedToInt64,
            totalBytes: totalBytes.clampedToInt64,
            currentFileName: currentEntry ?? "",
            throughputMBs: throughput
        ))
        if externalToken?.isCancelled() == true {
            stageCancelToken?.cancel()
            return false
        }
        return true
    }
}

package enum ArchiveExtractionSessionDriver {
    /// Drives the end-to-end extraction session state-machine loop:
    /// `new -> preflight -> [decide] -> stage -> [decide] -> commit -> while NeedsDecision { decide; commit }`
    ///
    /// Cancellation via Task cancellation or `token` maps to `session.abort()`.
    package static func extract(
        _ request: ExtractionRequest,
        handling: CollisionHandling,
        progress: ProgressSink? = nil,
        token: CancellationToken? = nil
    ) async throws -> ExtractionOutcome {
        if Task.isCancelled || token?.isCancelled() == true {
            throw ArchiveError.cancelled
        }
        var accumulatedResolutions = try makeInitialResolutions(for: handling)

        let session = try ExtractionSession(request: request)
        ExtractionStagingRegistry.shared.register(destinationRoot: request.destinationRoot)
        defer {
            ExtractionStagingRegistry.shared.unregister(destinationRoot: request.destinationRoot)
        }

        let cancelToken = UniFfiCancellationToken()
        let abortIfCancelled = {
            try checkAndAbortIfCancelled(session: session, externalToken: token, cancelToken: cancelToken)
        }

        if let preflightSummary = try session.preflight() {
            try abortIfCancelled()
            if let earlyOutcome = try await resolveConflictSummary(
                preflightSummary,
                session: session,
                handling: handling,
                accumulatedResolutions: &accumulatedResolutions
            ) {
                return earlyOutcome
            }
        }

        try abortIfCancelled()

        let stageReport = try await executeExtractionStage(
            session: session,
            progress: progress,
            externalToken: token,
            cancelToken: cancelToken
        )

        try abortIfCancelled()

        if let postStageConflicts = stageReport.postStageConflicts {
            if let earlyOutcome = try await resolveConflictSummary(
                postStageConflicts,
                session: session,
                handling: handling,
                accumulatedResolutions: &accumulatedResolutions
            ) {
                return earlyOutcome
            }
        }

        return try await commitExtractionLoop(
            session: session,
            handling: handling,
            initialResolutions: accumulatedResolutions,
            abortIfCancelled: abortIfCancelled
        )
    }

    private static func makeInitialResolutions(for handling: CollisionHandling) throws -> ResolutionSet {
        switch handling {
        case .policy(let policy):
            if policy == .ask {
                throw TtZipError.InvalidArgument(message: "CollisionPolicy.ask cannot be used as a non-interactive policy")
            }
            return ResolutionSet(defaultAction: policy.defaultAction, overrides: [])
        case .interactive:
            return ResolutionSet(defaultAction: nil, overrides: [])
        }
    }

    private static func checkAndAbortIfCancelled(
        session: ExtractionSession,
        externalToken: CancellationToken?,
        cancelToken: UniFfiCancellationToken
    ) throws {
        if Task.isCancelled || externalToken?.isCancelled() == true || cancelToken.isCancelled() {
            cancelToken.cancel()
            _ = try? session.abort()
            throw ArchiveError.cancelled
        }
    }

    private static func makeConflictPageFetcher(
        session: ExtractionSession
    ) -> @Sendable (UInt32, UInt32) async throws -> ConflictPage {
        { [weak session] offset, limit in
            guard let s = session else {
                throw ArchiveError.engineFailure(code: -1, message: "ExtractionSession deallocated during fetch")
            }
            return try s.conflictPage(offset: offset, limit: limit)
        }
    }

    private static func resolveConflictSummary(
        _ summary: ConflictSummary,
        session: ExtractionSession,
        handling: CollisionHandling,
        accumulatedResolutions: inout ResolutionSet
    ) async throws -> ExtractionOutcome? {
        switch handling {
        case .policy(let policy):
            if policy == .stop {
                return try session.abort()
            }
            return nil
        case .interactive(let provider):
            let fetch = makeConflictPageFetcher(session: session)
            guard let decisions = await provider.decide(summary: summary, fetch: fetch) else {
                return try session.abort()
            }
            accumulatedResolutions = mergeResolutions(accumulatedResolutions, with: decisions)
            return nil
        }
    }

    private static func executeExtractionStage(
        session: ExtractionSession,
        progress: ProgressSink?,
        externalToken: CancellationToken?,
        cancelToken: UniFfiCancellationToken
    ) async throws -> StageReport {
        do {
            return try await withTaskCancellationHandler {
                try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated) {
                    let relay = (progress != nil || externalToken != nil)
                        ? ExtractionSessionProgressRelay(
                            handler: progress,
                            externalToken: externalToken,
                            stageCancelToken: cancelToken
                        )
                        : nil
                    return try session.stage(progress: relay, cancel: cancelToken)
                }
            } onCancel: {
                cancelToken.cancel()
                _ = try? session.abort()
            }
        } catch {
            if Task.isCancelled || externalToken?.isCancelled() == true || cancelToken.isCancelled() {
                _ = try? session.abort()
                throw ArchiveError.cancelled
            }
            throw error
        }
    }

    private static func commitExtractionLoop(
        session: ExtractionSession,
        handling: CollisionHandling,
        initialResolutions: ResolutionSet,
        abortIfCancelled: () throws -> Void
    ) async throws -> ExtractionOutcome {
        var accumulatedResolutions = initialResolutions
        try abortIfCancelled()

        var commitResult = try session.commit(resolutions: accumulatedResolutions)
        while true {
            switch commitResult {
            case .finished(let outcome):
                return outcome
            case .needsDecision(let summary):
                try abortIfCancelled()
                if let abortedOutcome = try await updateResolutionsForCommitDecision(
                    summary: summary,
                    session: session,
                    handling: handling,
                    accumulatedResolutions: &accumulatedResolutions
                ) {
                    return abortedOutcome
                }
                try abortIfCancelled()
                commitResult = try session.commit(resolutions: accumulatedResolutions)
            }
        }
    }

    private static func updateResolutionsForCommitDecision(
        summary: ConflictSummary,
        session: ExtractionSession,
        handling: CollisionHandling,
        accumulatedResolutions: inout ResolutionSet
    ) async throws -> ExtractionOutcome? {
        switch handling {
        case .policy(let policy):
            if policy == .stop {
                return try session.abort()
            }
            accumulatedResolutions = ResolutionSet(
                defaultAction: policy.defaultAction,
                overrides: accumulatedResolutions.overrides
            )
            return nil
        case .interactive(let provider):
            let fetch = makeConflictPageFetcher(session: session)
            guard let decisions = await provider.decide(summary: summary, fetch: fetch) else {
                return try session.abort()
            }
            accumulatedResolutions = mergeResolutions(accumulatedResolutions, with: decisions)
            return nil
        }
    }

    private static func mergeResolutions(_ current: ResolutionSet, with newResolutions: ResolutionSet) -> ResolutionSet {
        var overrideMap: [UInt32: CollisionAction] = [:]
        for item in current.overrides {
            overrideMap[item.conflictId] = item.action
        }
        for item in newResolutions.overrides {
            overrideMap[item.conflictId] = item.action
        }
        let mergedOverrides = overrideMap.keys.sorted().compactMap { key in
            overrideMap[key].map { ResolutionOverride(conflictId: key, action: $0) }
        }
        let defaultAction = newResolutions.defaultAction ?? current.defaultAction
        return ResolutionSet(defaultAction: defaultAction, overrides: mergedOverrides)
    }

    /// Extracts one entry (a directory entry includes its descendants) directly into `destinationDir`.
    @discardableResult
    package static func extractSingleEntry(
        archivePath: String,
        entryPath: String,
        destinationDir: String,
        password: String? = nil,
        handling: CollisionHandling = .policy(.keepBoth)
    ) async throws -> ExtractionOutcome {
        let explicitPwd = password ?? ArchivePasswordStore.shared.getPassword(for: archivePath)
        return try await extractEntries(
            archivePath: archivePath,
            entryPaths: [entryPath],
            destinationDir: destinationDir,
            password: explicitPwd,
            handling: handling
        )
    }

    /// Extracts the selected entries, plus every descendant of a selected directory, through the
    /// collision-safe session loop. Entries are placed relative to their deepest common parent:
    /// selecting `docs/report.pdf` writes `<destinationDir>/report.pdf`.
    @discardableResult
    package static func extractEntries(
        archivePath: String,
        entryPaths: [String],
        destinationDir: String,
        password: String?,
        handling: CollisionHandling = .policy(.keepBoth),
        progress: ProgressSink? = nil
    ) async throws -> ExtractionOutcome {
        guard !archivePath.isEmpty, !destinationDir.isEmpty, FileManager.default.fileExists(atPath: archivePath) else {
            throw ArchiveError.fileNotFound
        }
        guard !entryPaths.isEmpty else { return .nothingSelected }
        try FileManager.default.createDirectory(atPath: destinationDir, withIntermediateDirectories: true)

        let spotlightCreated = ArchiveExtractor.preventSpotlightIndexing(at: destinationDir)
        defer {
            ArchiveExtractor.removeSpotlightPrevention(at: destinationDir, wasCreated: spotlightCreated)
        }

        let request = ExtractionRequest(
            archivePath: archivePath,
            destinationRoot: destinationDir,
            password: password,
            entryFilter: entryPaths,
            rootAtCommonParent: true,
            policy: handling.requestPolicy
        )
        let outcome: ExtractionOutcome
        do {
            outcome = try await extract(request, handling: handling, progress: progress)
        } catch {
            throw ArchiveError.from(error: error)
        }
        ArchiveExtractor.propagateQuarantine(from: archivePath, to: destinationDir)
        return outcome
    }

}

extension ArchiveExtractor {
    /// Extracts a single entry (a directory entry includes its descendants) directly into `destinationDir`.
    @discardableResult
    public func extractSingleFile(
        archivePath: String,
        entryPath: String,
        destinationDir: String,
        password: String?,
        handling: CollisionHandling = .policy(.keepBoth)
    ) async throws -> ExtractionOutcome {
        try await ArchiveExtractionSessionDriver.extractEntries(
            archivePath: archivePath,
            entryPaths: [entryPath],
            destinationDir: destinationDir,
            password: password,
            handling: handling
        )
    }
}

extension ArchiveSelectiveExtractor {
    /// Extracts the selected entries (a directory entry includes its descendants) into `destinationDir`,
    /// relative to their deepest common parent.
    @discardableResult
    public func extractSelected(
        archivePath: String,
        targetEntryPaths: Set<String>,
        destinationDir: String,
        password: String? = nil,
        handling: CollisionHandling = .policy(.keepBoth)
    ) async throws -> ExtractionOutcome {
        try await ArchiveExtractionSessionDriver.extractEntries(
            archivePath: archivePath,
            entryPaths: targetEntryPaths.sorted(),
            destinationDir: destinationDir,
            password: password,
            handling: handling
        )
    }
}

// MARK: - Extraction Request Helpers

package extension CollisionHandling {
    /// Policy recorded in the `ExtractionRequest`; interactive sessions resolve conflicts
    /// through the decision provider, so the request carries the non-destructive default.
    var requestPolicy: CollisionPolicy {
        switch self {
        case .policy(let policy):
            return policy
        case .interactive:
            return .keepBoth
        }
    }
}

private extension ExtractionOutcome {
    /// Outcome of a selection that names no entries: nothing is read or written.
    static var nothingSelected: ExtractionOutcome {
        ExtractionOutcome(status: .completed, extracted: 0, replaced: 0, keptBoth: 0, skipped: 0, failed: [])
    }
}
