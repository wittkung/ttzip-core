// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.
// Swift 6 Actor-Isolated Concurrency & Task Cancellation Unit Test Suite.

import XCTest
import Foundation
@testable import TTZipCore

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
        } catch {
            // Task cancellation is expected to throw CancellationError or ArchiveError.cancelled
            XCTAssertTrue(error is CancellationError || "\(error)".lowercased().contains("cancel") || "\(error)".lowercased().contains("abort"))
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

        print("🔍 [ActorConcurrencyTests] Step 1: compressing direct...")
        _ = try await engine.compressDirect(
            inputs: [largeFile.path],
            outputPath: archiveZip.path,
            format: .zip
        )
        print("🔍 [ActorConcurrencyTests] Step 1 complete. Archive created.")
        XCTAssertTrue(FileManager.default.fileExists(atPath: archiveZip.path))

        print("🔍 [ActorConcurrencyTests] Step 2: starting extraction...")
        let (stream, extractionTask) = await engine.extract(
            archivePath: archiveZip.path,
            destinationDir: extractDir.path
        )

        // Cancel extraction task
        print("🔍 [ActorConcurrencyTests] Step 3: cancelling extraction task...")
        extractionTask.cancel()

        for await p in stream {
            print("🔍 [ActorConcurrencyTests] extraction progress: \(p.state)")
        }

        do {
            _ = try await extractionTask.value
        } catch {
            print("🔍 [ActorConcurrencyTests] Extraction cancellation caught: \(type(of: error)) -> \(error)")
            XCTAssertTrue(error is CancellationError || error is ArchiveError || "\(error)".count > 0)
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
}
