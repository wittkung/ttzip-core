// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import XCTest
import Foundation
import os
@testable import TTZipCore

extension ActorConcurrencyTests {
    func test_r4_real_ffi_task_cancellation_waits_for_worker_and_serializes_same_target() async throws {
        let writer = ArchiveWriter()
        let srcDir = try sandbox.createSubdirectory("r4_real_ffi_src")
        for i in 0..<6 {
            let fileURL = srcDir.appendingPathComponent("chunk_\(i).txt")
            try String(repeating: "R4_Payload_\(i)_", count: 4096).write(to: fileURL, atomically: true, encoding: .utf8)
        }
        let outDir = try sandbox.createSubdirectory("r4_real_ffi_out")
        let targetZip = outDir.appendingPathComponent("r4_target.zip")
        try Data("ORIGINAL_COMMITTED_BYTES".utf8).write(to: targetZip)

        let timeline = EventTimeline()
        let workerBlockedInProgress = expectation(description: "real FFI worker reached progress callback")
        let task2WaiterEnqueued = expectation(description: "task2 enqueued on ArchiveTargetPathLockPool")
        let releaseWorkerGate = DispatcherBlockingGate()
        let didEnterOnce = OSAllocatedUnfairLock(initialState: false)
        let didEnterTask2Once = OSAllocatedUnfairLock(initialState: false)
        let observedStagingDirs = OSAllocatedUnfairLock(initialState: [String]())
        let task2EnteredProgress = OSAllocatedUnfairLock(initialState: false)
        let normalizedTarget = targetZip.standardizedFileURL.path

        defer {
            ArchiveTargetPathLockPool._onWaiterEnqueuedHook = nil
            ArchiveTargetPathLockPool._onLockReleasedHook = nil
            writer._afterWorkerCleanupBeforeLockReleaseHook = nil
            releaseWorkerGate.open()
        }
        installPathLockTimelineHooks(
            writer: writer, normalizedTarget: normalizedTarget, timeline: timeline, task2WaiterEnqueued: task2WaiterEnqueued
        )

        let task1 = Task {
            try await writer.createArchive(
                outputPath: targetZip.path, format: .zip, inputPaths: [srcDir.path],
                progressHandler: { _ in
                    guard didEnterOnce.withLock({ done -> Bool in guard !done else { return false }; done = true; return true }) else { return }
                    let entries = (try? FileManager.default.contentsOfDirectory(atPath: outDir.path)) ?? []
                    observedStagingDirs.withLock { $0 = entries.filter { $0.hasPrefix(".ttzip_create_txn_") } }
                    timeline.record("task1InFlightWithStaging")
                    workerBlockedInProgress.fulfill()
                    releaseWorkerGate.wait()
                    timeline.record("task1ProgressGateReleased")
                }
            )
        }

        await fulfillment(of: [workerBlockedInProgress], timeout: 5.0)
        XCTAssertEqual(observedStagingDirs.withLock { $0.count }, 1, "Staging dir must exist while worker is in-flight")
        task1.cancel()

        let task2 = Task {
            try await writer.createArchive(
                outputPath: targetZip.path, format: .zip, inputPaths: [srcDir.path],
                progressHandler: { _ in
                    if didEnterTask2Once.withLock({ done -> Bool in guard !done else { return false }; done = true; return true }) {
                        task2EnteredProgress.withLock { $0 = true }
                        timeline.record("task2ProgressEntered")
                    }
                }
            )
            timeline.record("task2Finished")
        }

        await fulfillment(of: [task2WaiterEnqueued], timeout: 5.0)
        XCTAssertFalse(task2EnteredProgress.withLock { $0 })
        XCTAssertGreaterThanOrEqual(NativeComputeDispatcher.shared.currentActiveLoads.reduce(0, +), 1)

        releaseWorkerGate.open()
        try await verifyR4SerializationOutcome(
            task1: task1,
            task2: task2,
            timeline: timeline,
            outDir: outDir,
            targetZip: targetZip
        )
    }

    private func verifyR4SerializationOutcome(
        task1: Task<Void, Error>,
        task2: Task<Void, Error>,
        timeline: EventTimeline,
        outDir: URL,
        targetZip: URL
    ) async throws {
        do {
            try await task1.value
            XCTFail("Expected task1 to throw ArchiveError.cancelled after real FFI cancellation")
        } catch let error as ArchiveError {
            XCTAssertEqual(error, .cancelled)
            timeline.record("task1CallerReturnedCancelled")
        }

        try await task2.value
        let events = timeline.events
        let idxInFlight = try XCTUnwrap(events.firstIndex(of: "task1InFlightWithStaging"))
        let idxEnqueued = try XCTUnwrap(events.firstIndex(of: "task2WaiterEnqueuedOnPathLock"))
        let idxCleanupDone = try XCTUnwrap(events.firstIndex(of: "task1WorkerAndCleanupCompleted"))
        let idxLockReleased = try XCTUnwrap(events.firstIndex(of: "task1PathLockReleased"))
        let idxTask2Entered = try XCTUnwrap(events.firstIndex(of: "task2ProgressEntered"))
        let idxTask1Returned = try XCTUnwrap(events.firstIndex(of: "task1CallerReturnedCancelled"))
        XCTAssertLessThan(idxInFlight, idxEnqueued)
        XCTAssertLessThan(idxEnqueued, idxCleanupDone)
        XCTAssertLessThan(idxCleanupDone, idxLockReleased)
        XCTAssertLessThan(idxLockReleased, idxTask2Entered)
        XCTAssertLessThan(idxCleanupDone, idxTask1Returned)

        let finalEntries = try FileManager.default.contentsOfDirectory(atPath: outDir.path)
        XCTAssertFalse(finalEntries.contains(where: { $0.hasPrefix(".ttzip_create_txn_") }))
        let inspected = try await TTZipEngine.shared.inspect(archivePath: targetZip.path)
        XCTAssertEqual(inspected.filter { !$0.isDirectory }.count, 6)
    }

    func test_r4_real_ffi_post_commit_task_cancellation_and_cleanup_error_propagation() async throws {
        let writer = ArchiveWriter()
        let srcDir = try sandbox.createSubdirectory("r4_post_commit_src")
        let srcFile = srcDir.appendingPathComponent("doc.txt")
        try String(repeating: "PostCommitPayload_", count: 2048).write(to: srcFile, atomically: true, encoding: .utf8)

        try await verifyPostCommitCancellationPreservesCommittedArchive(writer: writer, srcFile: srcFile)
        try await verifyCancelledStagingCleanupErrorPropagation(writer: writer, srcFile: srcFile)
    }

    private func verifyPostCommitCancellationPreservesCommittedArchive(
        writer: ArchiveWriter,
        srcFile: URL
    ) async throws {
        let committedZip = try sandbox.createSubdirectory("r4_post_commit_out").appendingPathComponent("post_commit_race.zip")
        let committedReachedHook = expectation(description: "real FFI committed archive on disk before delivery")
        let releasePostCommitHook = DispatcherBlockingGate()
        writer._postFfiCommitBeforeDeliveryHook = {
            committedReachedHook.fulfill()
            releasePostCommitHook.wait()
        }

        let postCommitTask = Task {
            try await writer.createArchiveWithOutcome(outputPath: committedZip.path, format: .zip, inputPaths: [srcFile.path])
        }
        await fulfillment(of: [committedReachedHook], timeout: 5.0)
        let diskBytesAtHook = try Data(contentsOf: committedZip)

        postCommitTask.cancel()
        releasePostCommitHook.open()
        writer._postFfiCommitBeforeDeliveryHook = nil

        let report = try await postCommitTask.value
        XCTAssertEqual(try Data(contentsOf: committedZip), diskBytesAtHook)
        XCTAssertNil(report.outcome)
    }

    private func verifyCancelledStagingCleanupErrorPropagation(
        writer: ArchiveWriter,
        srcFile: URL
    ) async throws {
        let cleanupFailDir = try sandbox.createSubdirectory("r4_cleanup_fail_out")
        let cleanupFailZip = cleanupFailDir.appendingPathComponent("cleanup_fail.zip")
        let lockedTxnDirs = OSAllocatedUnfairLock(initialState: [URL]())
        let lockedInProgress = expectation(description: "locked staging dir during real FFI progress")
        let cancelPropagated = expectation(description: "cancellation propagated to TaskExecutionHandle")
        let releaseLockedProgress = DispatcherBlockingGate()
        let didLockOnce = OSAllocatedUnfairLock(initialState: false)
        defer {
            writer._onCancellationHandleCreatedHook = nil
            for dirURL in lockedTxnDirs.withLock({ $0 }) {
                chmod(dirURL.path, 0o700)
                try? FileManager.default.removeItem(at: dirURL)
            }
        }
        writer._onCancellationHandleCreatedHook = { handle in
            handle.onCancel { cancelPropagated.fulfill() }
        }

        let failTask = Task {
            try await writer.createArchive(
                outputPath: cleanupFailZip.path, format: .zip, inputPaths: [srcFile.path],
                progressHandler: { _ in
                    let shouldLock = didLockOnce.withLock { done -> Bool in
                        guard !done else { return false }
                        done = true
                        return true
                    }
                    guard shouldLock else { return }
                    let items = (try? FileManager.default.contentsOfDirectory(
                        at: cleanupFailDir, includingPropertiesForKeys: nil
                    )) ?? []
                    for item in items where item.lastPathComponent.hasPrefix(".ttzip_create_txn_") {
                        chmod(item.path, 0o500)
                        lockedTxnDirs.withLock { $0.append(item) }
                    }
                    lockedInProgress.fulfill()
                    releaseLockedProgress.wait()
                }
            )
        }

        await fulfillment(of: [lockedInProgress], timeout: 5.0)
        failTask.cancel()
        await fulfillment(of: [cancelPropagated], timeout: 5.0)
        releaseLockedProgress.open()

        do {
            try await failTask.value
            XCTFail("Expected CreateTransactionIncomplete when cancelled staging cleanup fails")
        } catch let error as ArchiveError {
            guard case let .createTransactionIncomplete(report) = error else {
                XCTFail("Expected .createTransactionIncomplete instead of .cancelled, got \(error)")
                return
            }
            XCTAssertEqual(report.primaryStatusCode, 2)
            XCTAssertTrue(report.wasPrimaryCancelled)
            XCTAssertTrue(report.isCancellationRequested)
            XCTAssertTrue(report.stageDirPreserved)
            XCTAssertFalse(report.cleanupFailures.isEmpty)
            XCTAssertTrue(report.stageDir.contains(".ttzip_create_txn_"))
        }
    }
}
