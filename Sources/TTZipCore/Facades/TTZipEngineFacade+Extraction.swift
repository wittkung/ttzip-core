// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

private final class FacadeExtractionProgressRelay: UniFfiProgressCallback, @unchecked Sendable {
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

extension TTZipEngineFacade {
    /// Drives the end-to-end extraction session state-machine loop:
    /// `new -> preflight -> [decide] -> stage -> [decide] -> commit -> while NeedsDecision { decide; commit }`
    ///
    /// Cancellation via Task cancellation or `token` maps to `session.abort()`.
    public func extract(
        _ request: ExtractionRequest,
        handling: CollisionHandling,
        progress: ProgressSink? = nil,
        token: CancellationToken? = nil
    ) async throws -> ExtractionOutcome {
        if Task.isCancelled || token?.isCancelled() == true {
            throw ArchiveError.cancelled
        }
        let initialResolutions: ResolutionSet
        switch handling {
        case .policy(let policy):
            if policy == .ask {
                throw TtZipError.InvalidArgument(message: "CollisionPolicy.ask cannot be used as a non-interactive policy")
            }
            initialResolutions = ResolutionSet(defaultAction: policy.defaultAction, overrides: [])
        case .interactive:
            initialResolutions = ResolutionSet(defaultAction: nil, overrides: [])
        }

        let session = try ExtractionSession(request: request)
        ExtractionStagingRegistry.shared.register(destinationRoot: request.destinationRoot)
        defer {
            ExtractionStagingRegistry.shared.unregister(destinationRoot: request.destinationRoot)
        }
        var accumulatedResolutions = initialResolutions


        let cancelToken = UniFfiCancellationToken()
        let abortIfCancelled = {
            if Task.isCancelled || token?.isCancelled() == true || cancelToken.isCancelled() {
                cancelToken.cancel()
                _ = try? session.abort()
                throw ArchiveError.cancelled
            }
        }

        if let preflightSummary = try session.preflight() {
            try abortIfCancelled()
            switch handling {
            case .policy(let policy):
                if policy == .stop {
                    return try session.abort()
                }
            case .interactive(let provider):
                let fetch: @Sendable (UInt32, UInt32) async throws -> ConflictPage = { [weak session] offset, limit in
                    guard let s = session else {
                        throw ArchiveError.engineFailure(code: -1, message: "ExtractionSession deallocated during fetch")
                    }
                    return try s.conflictPage(offset: offset, limit: limit)
                }
                guard let decisions = await provider.decide(summary: preflightSummary, fetch: fetch) else {
                    return try session.abort()
                }
                accumulatedResolutions = mergeResolutions(accumulatedResolutions, with: decisions)
            }
        }

        try abortIfCancelled()

        let stageReport: StageReport
        do {
            stageReport = try await withTaskCancellationHandler {
                try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated) {
                    let relay = (progress != nil || token != nil)
                        ? FacadeExtractionProgressRelay(
                            handler: progress,
                            externalToken: token,
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
            if Task.isCancelled || token?.isCancelled() == true || cancelToken.isCancelled() {
                _ = try? session.abort()
                throw ArchiveError.cancelled
            }
            throw error
        }

        try abortIfCancelled()

        if let postStageConflicts = stageReport.postStageConflicts {
            switch handling {
            case .policy(let policy):
                if policy == .stop {
                    return try session.abort()
                }
            case .interactive(let provider):
                let fetch: @Sendable (UInt32, UInt32) async throws -> ConflictPage = { [weak session] offset, limit in
                    guard let s = session else {
                        throw ArchiveError.engineFailure(code: -1, message: "ExtractionSession deallocated during fetch")
                    }
                    return try s.conflictPage(offset: offset, limit: limit)
                }
                guard let decisions = await provider.decide(summary: postStageConflicts, fetch: fetch) else {
                    return try session.abort()
                }
                accumulatedResolutions = mergeResolutions(accumulatedResolutions, with: decisions)
            }
        }

        // Commit Point of No Return #1: check both Task.isCancelled and external token before session.commit.
        try abortIfCancelled()

        var commitResult = try session.commit(resolutions: accumulatedResolutions)
        while true {
            switch commitResult {
            case .finished(let outcome):
                return outcome
            case .needsDecision(let summary):
                try abortIfCancelled()
                switch handling {
                case .policy(let policy):
                    if policy == .stop {
                        return try session.abort()
                    }
                    accumulatedResolutions = ResolutionSet(
                        defaultAction: policy.defaultAction,
                        overrides: accumulatedResolutions.overrides
                    )
                    try abortIfCancelled()
                    commitResult = try session.commit(resolutions: accumulatedResolutions)
                case .interactive(let provider):
                    let fetch: @Sendable (UInt32, UInt32) async throws -> ConflictPage = { [weak session] offset, limit in
                        guard let s = session else {
                            throw ArchiveError.engineFailure(code: -1, message: "ExtractionSession deallocated during fetch")
                        }
                        return try s.conflictPage(offset: offset, limit: limit)
                    }
                    guard let decisions = await provider.decide(summary: summary, fetch: fetch) else {
                        return try session.abort()
                    }
                    accumulatedResolutions = mergeResolutions(accumulatedResolutions, with: decisions)
                    // Commit Point of No Return #2: check both Task.isCancelled and external token after decide returns.
                    try abortIfCancelled()
                    commitResult = try session.commit(resolutions: accumulatedResolutions)
                }
            }
        }
    }

    private func mergeResolutions(_ current: ResolutionSet, with newResolutions: ResolutionSet) -> ResolutionSet {
        var overrideMap: [UInt32: CollisionAction] = [:]
        for item in current.overrides {
            overrideMap[item.conflictId] = item.action
        }
        for item in newResolutions.overrides {
            overrideMap[item.conflictId] = item.action
        }
        let mergedOverrides = overrideMap.keys.sorted().map {
            ResolutionOverride(conflictId: $0, action: overrideMap[$0]!)
        }
        let defaultAction = newResolutions.defaultAction ?? current.defaultAction
        return ResolutionSet(defaultAction: defaultAction, overrides: mergedOverrides)
    }
}
