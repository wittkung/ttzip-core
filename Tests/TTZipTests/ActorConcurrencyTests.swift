// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import XCTest
import Foundation
import os
@testable import TTZipCore

private struct DispatcherBeforeEnqueueFailure: Error {}

private final class DispatcherBlockingGate: @unchecked Sendable {
    private let semaphore = DispatchSemaphore(value: 0)
    private let state = OSAllocatedUnfairLock(initialState: false)

    func wait() {
        if !state.withLock({ $0 }) { semaphore.wait() }
    }

    func open() {
        let signal = state.withLock { open -> Bool in
            guard !open else { return false }
            open = true
            return true
        }
        if signal { for _ in 0..<16 { semaphore.signal() } }
    }
}

private final class BoundedDispatcherTask<Value: Sendable>: @unchecked Sendable {
    private let completion = XCTestExpectation(description: "Dispatcher caller completed")
    private let task: Task<Value, Error>

    init(_ operation: @escaping @Sendable () async throws -> Value) {
        let completed = completion
        task = Task {
            defer { completed.fulfill() }
            return try await operation()
        }
    }

    func cancel() { task.cancel() }

    func result(timeout: Double = 2.0) async -> Result<Value, Error>? {
        guard await XCTWaiter().fulfillment(of: [completion], timeout: timeout) == .completed else {
            return nil
        }
        do { return .success(try await task.value) }
        catch { return .failure(error) }
    }
}


private final class EventTimeline: @unchecked Sendable {
    private let lock = OSAllocatedUnfairLock(initialState: [String]())

    func record(_ event: String) {
        lock.withLock { $0.append(event) }
    }

    var events: [String] {
        lock.withLock { $0 }
    }
}

final class ActorConcurrencyTests: XCTestCase {
    private var sandbox: IsolatedTempSandbox!

    override func setUpWithError() throws {
        try super.setUpWithError()
        sandbox = try IsolatedTempSandbox(prefix: "actor_concurrency")
    }

    override func tearDownWithError() throws {
        sandbox?.cleanup()
        sandbox = nil
        try super.tearDownWithError()
    }

    // MARK: - 1. Swift 6 Actor Isolation & Race-Free Concurrent Archiving Operations

    func testActorIsolationRaceFreeConcurrentOperations() async throws {
        let engine = TTZipEngine.shared
        let concurrentTaskCount = 8

        // Prepare independent source files for each concurrent task
        var inputFiles: [URL] = []
        for i in 0..<concurrentTaskCount {
            let fileURL = sandbox.fileURL(named: "task_input_\(i).txt")
            let content = "Concurrent Actor Payload for task index \(i)\n" + String(repeating: "DATA_\(i)_", count: 200)
            try content.write(to: fileURL, atomically: true, encoding: .utf8)
            inputFiles.append(fileURL)
        }

        // Execute concurrent operations across Swift 6 actor-isolated boundaries
        try await withThrowingTaskGroup(of: (Int, String).self) { group in
            for i in 0..<concurrentTaskCount {
                let inputFile = inputFiles[i]
                let outZip = sandbox.fileURL(named: "concurrent_out_\(i).zip")
                let extractDir = try sandbox.createSubdirectory("concurrent_ext_\(i)")

                group.addTask {
                    // 1. Concurrent compressDirect on actor
                    let compressResult = try await engine.compressDirect(
                        inputs: [inputFile.path],
                        outputPath: outZip.path,
                        format: .zip,
                        level: .fast
                    )
                    XCTAssertTrue(FileManager.default.fileExists(atPath: compressResult.outputPath))
                    XCTAssertGreaterThan(compressResult.compressedBytes, 0)

                    // 2. Concurrent inspect on actor
                    let entries = try await engine.inspect(archivePath: outZip.path)
                    XCTAssertFalse(entries.isEmpty)

                    // 3. Concurrent extractDirect on actor
                    let extractResult = try await engine.extractDirect(
                        archivePath: outZip.path,
                        destinationDir: extractDir.path
                    )
                    XCTAssertEqual(extractResult.destinationDir, extractDir.path)
                    XCTAssertGreaterThanOrEqual(extractResult.durationSeconds, 0)

                    let extractedDoc = extractDir.appendingPathComponent("task_input_\(i).txt")
                    XCTAssertTrue(FileManager.default.fileExists(atPath: extractedDoc.path))
                    let readBack = try String(contentsOf: extractedDoc, encoding: .utf8)
                    XCTAssertTrue(readBack.contains("Concurrent Actor Payload for task index \(i)"))

                    return (i, outZip.path)
                }
            }

            var completedCount = 0
            for try await _ in group {
                completedCount += 1
            }
            XCTAssertEqual(completedCount, concurrentTaskCount, "All concurrent actor tasks must complete successfully")
        }
    }

    // MARK: - 2. Swift 6 Task Cancellation during Streaming Compression

    func testTaskCancellationDuringCompressionStream() async throws {
        let engine = TTZipEngine.shared

        // Create substantial payload
        let largeFile = sandbox.fileURL(named: "cancellation_compress_src.bin")
        let data = Data((0..<(256 * 1024)).map { UInt8($0 % 251) })
        try data.write(to: largeFile)

        let outZip = sandbox.fileURL(named: "cancellation_compress_out.zip")

        let (stream, compressionTask) = await engine.compress(
            inputs: [largeFile.path],
            outputPath: outZip.path,
            format: .zip,
            level: .maximum
        )

        // Cancel the task asynchronously
        compressionTask.cancel()

        var didObserveTerminalOrCancellation = false
        for await progress in stream {
            if progress.state == .cancelled || progress.state == .failed(error: "cancelled") {
                didObserveTerminalOrCancellation = true
                break
            }
        }

        do {
            _ = try await compressionTask.value
            XCTFail("Should have thrown CancellationError")
        } catch {
            // Task cancellation is expected to throw CancellationError or ArchiveError.cancelled
            XCTAssertTrue(error is CancellationError || (error as? ArchiveError) == .cancelled)
            didObserveTerminalOrCancellation = true
        }

        XCTAssertTrue(didObserveTerminalOrCancellation, "Task cancellation must be cleanly handled and observed")
    }

    // MARK: - 3. Swift 6 Task Cancellation during Streaming Extraction

    func testTaskCancellationDuringExtractionStream() async throws {
        let engine = TTZipEngine.shared

        let largeFile = sandbox.fileURL(named: "cancellation_extract_src.txt")
        let payload = String(repeating: "TTZip Fast Stream Extraction Cancellation Verification\n", count: 2000)
        try payload.write(to: largeFile, atomically: true, encoding: .utf8)

        let archiveZip = sandbox.fileURL(named: "cancellation_extract.zip")
        let extractDir = try sandbox.createSubdirectory("cancellation_extract_dir")

        _ = try await engine.compressDirect(
            inputs: [largeFile.path],
            outputPath: archiveZip.path,
            format: .zip
        )
        XCTAssertTrue(FileManager.default.fileExists(atPath: archiveZip.path))

        let (stream, extractionTask) = await engine.extract(
            archivePath: archiveZip.path,
            destinationDir: extractDir.path
        )

        // Cancel extraction task
        extractionTask.cancel()

        for await _ in stream {
            // Drain stream
        }

        do {
            _ = try await extractionTask.value
            XCTFail("Should have thrown CancellationError")
        } catch {
            XCTAssertTrue(error is CancellationError || (error as? ArchiveError) == .cancelled)
        }
    }

    // MARK: - 4. NativeComputeDispatcher Task Cancellation & Token Bridging

    func testNativeComputeDispatcherTaskCancellation() async throws {
        let dispatcher = NativeComputeDispatcher.shared
        let cancelHandle = TaskExecutionHandle()

        let task = Task {
            try await dispatcher.dispatchCompute(qos: .userInitiated, cancellationHandle: cancelHandle) { isCancelled in
                for _ in 0..<100 {
                    if isCancelled.withLock({ $0 }) || cancelHandle.isCancelled {
                        throw CancellationError()
                    }
                    Thread.sleep(forTimeInterval: 0.005)
                }
                return 42
            }
        }

        // Trigger cancellation
        try await Task.sleep(nanoseconds: 10_000_000) // 10ms
        cancelHandle.cancel()
        task.cancel()

        do {
            _ = try await task.value
            XCTFail("Compute task should have been cancelled")
        } catch {
            XCTAssertTrue(error is CancellationError)
        }
        XCTAssertTrue(cancelHandle.isCancelled)
    }

    func testNativeComputeDispatcherWorkerPoolSize() {
        let poolSize = NativeComputeDispatcher.defaultWorkerPoolSize()
        XCTAssertGreaterThanOrEqual(poolSize, 2, "Worker pool size must be at least 2")
        XCTAssertLessThanOrEqual(poolSize, 16, "Worker pool size must not exceed 16")
        
        let customDispatcher = NativeComputeDispatcher(maxConcurrency: 8)
        XCTAssertEqual(customDispatcher.maxConcurrency, 8)
    }

    func testNativeComputeDispatcherDispatchIO() async throws {
        let dispatcher = NativeComputeDispatcher.shared
        let result = try await dispatcher.dispatchIO {
            return "io_payload_verified"
        }
        XCTAssertEqual(result, "io_payload_verified")
    }

    func testNativeComputeDispatcherDispatchHighPriority() async throws {
        let dispatcher = NativeComputeDispatcher.shared
        let result = try await dispatcher.dispatchHighPriority {
            return 9999
        }
        XCTAssertEqual(result, 9999)
    }

    func testNativeComputeDispatcherLoadBalancedPool() async throws {
        let dispatcher = NativeComputeDispatcher.shared
        let taskCount = 12
        
        try await withThrowingTaskGroup(of: Int.self) { group in
            for i in 0..<taskCount {
                group.addTask {
                    try await dispatcher.dispatchCompute(qos: .userInitiated) {
                        return i * i
                    }
                }
            }
            
            var results: [Int] = []
            for try await val in group {
                results.append(val)
            }
            XCTAssertEqual(results.count, taskCount)
        }
    }

    // MARK: - 5. ProgressStreamBridge High-Concurrency Lock-Free Streaming & Cancellation

    func testProgressStreamBridgeConcurrentEmissionAndCancellation() async throws {
        let (bridge, stream) = ConcurrencyBridge.ProgressStreamBridge.create()
        let emitIterations = 50

        let emissionTask = Task.detached {
            ConcurrencyBridge.parallelFor(count: emitIterations) { index in
                bridge.emit(
                    bytesProcessed: Int64(index * 100),
                    totalBytes: Int64(emitIterations * 100),
                    currentFileName: "file_\(index).bin",
                    force: true
                )
            }
        }

        await emissionTask.value
        XCTAssertFalse(bridge.isCancelled)

        // Cancel bridge
        bridge.cancel()
        XCTAssertTrue(bridge.isCancelled)

        var count = 0
        for await _ in stream {
            count += 1
        }
        XCTAssertGreaterThan(count, 0, "Stream should have received emitted progress items before finishing")
    }

    // MARK: - 6. Concurrency Resource Budgets Topology

    func testConcurrencyResourceBudgets() {
        let optimalThreads = ConcurrencyBridge.ThreadBudget.optimalThreadCount()
        XCTAssertGreaterThan(optimalThreads, 0)
        XCTAssertEqual(ConcurrencyBridge.ThreadBudget.optimalThreadCount(for: 4), 4)

        let safeMemory = ConcurrencyBridge.MemoryBudget.safeBudget
        XCTAssertGreaterThan(safeMemory, 0)

        let clamped = ConcurrencyBridge.MemoryBudget.clamp(
            desiredBytes: 1024 * 1024,
            minBytes: 64 * 1024,
            maxBytes: 1024 * 1024 * 1024
        )
        XCTAssertEqual(clamped, 1024 * 1024)
    }

    func testSemaphoreCancellationBeforeRegistrationDoesNotLoseCancellation() async throws {
        let entered = expectation(description: "Before waiter registration")
        let cancelled = expectation(description: "Cancellation handler completed")
        let gate = DispatcherBlockingGate()
        defer { gate.open() }
        let semaphore = AsyncSemaphore(value: 1, waiterHooks: .init(
            beforeRegistration: { entered.fulfill(); gate.wait() },
            didCancel: { cancelled.fulfill() }
        ))
        try await semaphore.wait()
        defer { semaphore.signal() }
        let waiter = BoundedDispatcherTask<Int> { try await semaphore.wait(); return 1 }
        guard await XCTWaiter().fulfillment(of: [entered], timeout: 2.0) == .completed else {
            waiter.cancel()
            XCTFail("Waiter did not reach the controlled pre-registration boundary")
            return
        }
        waiter.cancel()
        guard await XCTWaiter().fulfillment(of: [cancelled], timeout: 2.0) == .completed else {
            XCTFail("Cancellation was not observed before registration")
            return
        }
        gate.open()
        let result = await waiter.result()
        guard case .failure(let error)? = result else {
            XCTFail("Cancelled unregistered waiter must terminate before the owner releases its permit")
            return
        }
        XCTAssertTrue(error is CancellationError)
        XCTAssertEqual(semaphore.pendingWaiterCount, 0)
        XCTAssertEqual(semaphore.currentAvailablePermits, 0)
    }

    func testSemaphoreRegisteredWaiterCancellationRemovesItWithoutConsumingPermit() async throws {
        let registered = expectation(description: "Waiter actually registered")
        let semaphore = AsyncSemaphore(value: 1, waiterHooks: .init(didRegister: { registered.fulfill() }))
        try await semaphore.wait()
        defer { semaphore.signal() }
        let waiter = BoundedDispatcherTask<Int> { try await semaphore.wait(); return 1 }
        guard await XCTWaiter().fulfillment(of: [registered], timeout: 2.0) == .completed else {
            waiter.cancel()
            XCTFail("Waiter registration was not observed")
            return
        }
        XCTAssertEqual(semaphore.pendingWaiterCount, 1)
        waiter.cancel()
        let result = await waiter.result()
        guard case .failure(let error)? = result else {
            XCTFail("Registered cancelled waiter must terminate without an owner signal")
            return
        }
        XCTAssertTrue(error is CancellationError)
        XCTAssertEqual(semaphore.pendingWaiterCount, 0)
        XCTAssertEqual(semaphore.currentAvailablePermits, 0)
    }

    func testDispatcherPreEnqueueFailureReleasesResourcesExactlyOnce() async throws {
        let released = expectation(description: "Pre-enqueue resources released")
        let dispatcher = NativeComputeDispatcher(
            maxConcurrency: 2, waiterHooks: .init(), onResourcesReleased: { released.fulfill() },
            beforeEnqueue: { throw DispatcherBeforeEnqueueFailure() }
        )
        let task = BoundedDispatcherTask<Int> {
            try await dispatcher.dispatchCompute {
                (_: OSAllocatedUnfairLock<Bool>) in
                XCTFail("Failed admission must not invoke the worker")
                return 1
            }
        }
        let result = await task.result()
        guard case .failure(let error)? = result else { return XCTFail("Expected bounded pre-enqueue failure") }
        XCTAssertTrue(error is DispatcherBeforeEnqueueFailure)
        let resourceReceipt = await XCTWaiter().fulfillment(of: [released], timeout: 2.0)
        XCTAssertEqual(resourceReceipt, .completed)
        XCTAssertEqual(dispatcher.currentAvailablePermits, 2)
        XCTAssertEqual(dispatcher.currentActiveLoads, [0, 0])
    }

    func testDispatcherSaturatedRegisteredWaiterCancellationKeepsRunningResources() async throws {
        let registered = expectation(description: "Third caller actually registered")
        let started = expectation(description: "Both workers entered")
        started.expectedFulfillmentCount = 2
        let released = expectation(description: "Both worker resources released")
        released.expectedFulfillmentCount = 2
        let gate = DispatcherBlockingGate()
        defer { gate.open() }
        let dispatcher = NativeComputeDispatcher(
            maxConcurrency: 2, waiterHooks: .init(didRegister: { registered.fulfill() }),
            onResourcesReleased: { released.fulfill() }
        )
        let first = BoundedDispatcherTask<Int> {
            try await dispatcher.dispatchCompute { (_: OSAllocatedUnfairLock<Bool>) in
                started.fulfill(); gate.wait(); return 1
            }
        }
        let second = BoundedDispatcherTask<Int> {
            try await dispatcher.dispatchCompute { (_: OSAllocatedUnfairLock<Bool>) in
                started.fulfill(); gate.wait(); return 2
            }
        }
        guard await XCTWaiter().fulfillment(of: [started], timeout: 2.0) == .completed else {
            first.cancel(); second.cancel(); return XCTFail("Workers did not enter")
        }
        let third = BoundedDispatcherTask<Int> {
            try await dispatcher.dispatchCompute { (_: OSAllocatedUnfairLock<Bool>) in
                XCTFail("Cancelled registered caller must not invoke worker"); return 3
            }
        }
        guard await XCTWaiter().fulfillment(of: [registered], timeout: 2.0) == .completed else {
            third.cancel(); return XCTFail("Third caller never registered")
        }
        third.cancel()
        let thirdResult = await third.result()
        guard case .failure(let error)? = thirdResult else { return XCTFail("Third caller did not terminate") }
        XCTAssertTrue(error is CancellationError)
        XCTAssertEqual(dispatcher.currentAvailablePermits, 0)
        XCTAssertEqual(dispatcher.currentActiveLoads.reduce(0, +), 2)
        gate.open()
        let firstResult = await first.result()
        let secondResult = await second.result()
        XCTAssertNotNil(firstResult)
        XCTAssertNotNil(secondResult)
        let resourceReceipt = await XCTWaiter().fulfillment(of: [released], timeout: 2.0)
        XCTAssertEqual(resourceReceipt, .completed)
        XCTAssertEqual(dispatcher.currentAvailablePermits, 2)
        XCTAssertEqual(dispatcher.currentActiveLoads, [0, 0])
    }

    func testExplicitWaitCancellationDoesNotDeliverCallerBeforeWorkerCleanup() async throws {
        let started = expectation(description: "Wait worker entered")
        let cancelled = expectation(description: "Wait cancellation arbitration completed")
        let released = expectation(description: "Wait worker resources released")
        let delivered = OSAllocatedUnfairLock<Bool?>(initialState: nil)
        let gate = DispatcherBlockingGate()
        defer { gate.open() }
        let dispatcher = NativeComputeDispatcher(
            maxConcurrency: 2, waiterHooks: .init(),
            onResourcesReleased: { released.fulfill() },
            onCancellationHandled: { value in delivered.withLock { $0 = value }; cancelled.fulfill() }
        )
        let task = BoundedDispatcherTask<Int> {
            try await dispatcher.dispatchCompute(cancellationPolicy: .waitForWorkerCompletion) {
                (_: OSAllocatedUnfairLock<Bool>) in
                started.fulfill(); gate.wait(); throw CancellationError()
            }
        }
        guard await XCTWaiter().fulfillment(of: [started], timeout: 2.0) == .completed else {
            task.cancel(); return XCTFail("Worker did not enter")
        }
        task.cancel()
        let cancellationReceipt = await XCTWaiter().fulfillment(of: [cancelled], timeout: 2.0)
        XCTAssertEqual(cancellationReceipt, .completed)
        XCTAssertEqual(delivered.withLock { $0 }, false)
        XCTAssertEqual(dispatcher.currentAvailablePermits, 1)
        XCTAssertEqual(dispatcher.currentActiveLoads.reduce(0, +), 1)
        gate.open()
        let result = await task.result()
        guard case .failure(let error)? = result else { return XCTFail("Expected bounded cancellation") }
        XCTAssertTrue(error is CancellationError)
        let resourceReceipt = await XCTWaiter().fulfillment(of: [released], timeout: 2.0)
        XCTAssertEqual(resourceReceipt, .completed)
        XCTAssertEqual(dispatcher.currentAvailablePermits, 2)
        XCTAssertEqual(dispatcher.currentActiveLoads, [0, 0])
    }

    func testDefaultCancellationDetachesCallerAndRetainsWorkerResources() async throws {
        let started = expectation(description: "Default worker entered")
        let released = expectation(description: "Default worker resources released")
        let gate = DispatcherBlockingGate()
        defer { gate.open() }
        let dispatcher = NativeComputeDispatcher(
            maxConcurrency: 2, waiterHooks: .init(), onResourcesReleased: { released.fulfill() }
        )
        let task = BoundedDispatcherTask<Int> {
            try await dispatcher.dispatchCompute { (_: OSAllocatedUnfairLock<Bool>) in
                started.fulfill(); gate.wait(); return 42
            }
        }
        guard await XCTWaiter().fulfillment(of: [started], timeout: 2.0) == .completed else {
            task.cancel(); return XCTFail("Worker did not enter")
        }
        task.cancel()
        let result = await task.result()
        guard case .failure(let error)? = result else { return XCTFail("Default caller did not detach") }
        XCTAssertTrue(error is CancellationError)
        XCTAssertEqual(dispatcher.currentAvailablePermits, 1)
        XCTAssertEqual(dispatcher.currentActiveLoads.reduce(0, +), 1)
        gate.open()
        let resourceReceipt = await XCTWaiter().fulfillment(of: [released], timeout: 2.0)
        XCTAssertEqual(resourceReceipt, .completed)
        XCTAssertEqual(dispatcher.currentAvailablePermits, 2)
        XCTAssertEqual(dispatcher.currentActiveLoads, [0, 0])
    }

    // MARK: - R4 / C4: Real Swift Task Cancellation, Post-Commit Delivery Race & Deterministic Same-Target Exclusion Over Real FFI

    private func installPathLockTimelineHooks(
        writer: ArchiveWriter,
        normalizedTarget: String,
        timeline: EventTimeline,
        task2WaiterEnqueued: XCTestExpectation
    ) {
        let didEnqueueOnce = OSAllocatedUnfairLock(initialState: false)
        let didReleaseOnce = OSAllocatedUnfairLock(initialState: false)
        ArchiveTargetPathLockPool._onWaiterEnqueuedHook = { path, count in
            guard path == normalizedTarget, count >= 1 else { return }
            if didEnqueueOnce.withLock({ done -> Bool in guard !done else { return false }; done = true; return true }) {
                timeline.record("task2WaiterEnqueuedOnPathLock")
                task2WaiterEnqueued.fulfill()
            }
        }
        ArchiveTargetPathLockPool._onLockReleasedHook = { path in
            guard path == normalizedTarget else { return }
            if didReleaseOnce.withLock({ done -> Bool in guard !done else { return false }; done = true; return true }) {
                timeline.record("task1PathLockReleased")
            }
        }
        writer._afterWorkerCleanupBeforeLockReleaseHook = { path in
            if URL(fileURLWithPath: path).standardizedFileURL.path == normalizedTarget {
                timeline.record("task1WorkerAndCleanupCompleted")
            }
        }
    }

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
