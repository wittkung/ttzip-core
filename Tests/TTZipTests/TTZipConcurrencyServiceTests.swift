// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
import Foundation
@testable import TTZipCore

final class TTZipConcurrencyServiceTests: XCTestCase {
    private var sandbox: IsolatedTempSandbox!
    private var service: TTZipConcurrencyService!

    override func setUpWithError() throws {
        try super.setUpWithError()
        sandbox = try IsolatedTempSandbox(prefix: "concurrency_service_tests")
        service = TTZipConcurrencyService()
    }

    override func tearDownWithError() throws {
        sandbox?.cleanup()
        sandbox = nil
        service = nil
        try super.tearDownWithError()
    }

    // MARK: - 1. Observable Initial State & Counters

    func testObservableInitialStateAndCounters() {
        XCTAssertTrue(service.isIdle)
        XCTAssertEqual(service.activeTaskCount, 0)
        XCTAssertEqual(service.activeTasks.count, 0)
        XCTAssertEqual(service.totalCompletedTasks, 0)
        XCTAssertEqual(service.totalFailedTasks, 0)
        XCTAssertEqual(service.totalCancelledTasks, 0)
        XCTAssertEqual(service.totalBytesProcessed, 0)
        XCTAssertNil(service.latestError)

        service.clearCounters()
        XCTAssertEqual(service.totalCompletedTasks, 0)
    }

    // MARK: - 2. Streaming Compression with 60fps Progress & Observability

    func testCompressStreamProgressAndCompletion() async throws {
        let inputFile = sandbox.fileURL(named: "stream_source.txt")
        let content = "Concurrency Stream Test Payload\n" + String(repeating: "STREAM_DATA_BLOCK_", count: 1000)
        try content.write(to: inputFile, atomically: true, encoding: .utf8)

        let outZip = sandbox.fileURL(named: "stream_output.zip")

        let (stream, handle, task) = service.compressStream(
            inputs: [inputFile.path],
            outputPath: outZip.path,
            format: .zip,
            level: .fast,
            qos: .userInitiated
        )

        XCTAssertFalse(handle.isCancelled)

        // Consume stream asynchronously in structured Task returning value
        let streamConsumer = Task { () -> [ArchiveProgress] in
            var items: [ArchiveProgress] = []
            for await progress in stream {
                items.append(progress)
            }
            return items
        }

        let result = try await task.value
        let emittedProgresses = await streamConsumer.value

        XCTAssertTrue(FileManager.default.fileExists(atPath: outZip.path))
        XCTAssertGreaterThan(result.compressedBytes, 0)
        XCTAssertEqual(service.totalCompletedTasks, 1)
        XCTAssertEqual(service.totalFailedTasks, 0)
        XCTAssertEqual(service.totalCancelledTasks, 0)
        XCTAssertTrue(service.isIdle)
        XCTAssertFalse(emittedProgresses.isEmpty)
    }

    // MARK: - 3. Streaming Extraction with 60fps Progress & Observability

    func testExtractStreamProgressAndCompletion() async throws {
        let inputFile = sandbox.fileURL(named: "extract_source.txt")
        let content = "Extract Stream Test Payload\n" + String(repeating: "EXTRACT_SAMPLE_BLOCK_", count: 500)
        try content.write(to: inputFile, atomically: true, encoding: .utf8)

        let outZip = sandbox.fileURL(named: "extract_test.zip")
        let (_, _, compressTask) = service.compressStream(
            inputs: [inputFile.path],
            outputPath: outZip.path,
            format: .zip
        )
        _ = try await compressTask.value

        let destDir = try sandbox.createSubdirectory("extracted_dest")

        let (extStream, handle, extTask) = service.extractStream(
            archivePath: outZip.path,
            destinationDir: destDir.path,
            qos: .userInitiated
        )

        XCTAssertFalse(handle.isCancelled)

        let streamConsumer = Task { () -> [ArchiveProgress] in
            var items: [ArchiveProgress] = []
            for await progress in extStream {
                items.append(progress)
            }
            return items
        }

        let extractResult = try await extTask.value
        let emittedProgresses = await streamConsumer.value

        XCTAssertEqual(extractResult.destinationDir, destDir.path)
        let extractedFile = destDir.appendingPathComponent("extract_source.txt")
        XCTAssertTrue(FileManager.default.fileExists(atPath: extractedFile.path))
        XCTAssertEqual(service.totalCompletedTasks, 2)
        XCTAssertTrue(service.isIdle)
        XCTAssertFalse(emittedProgresses.isEmpty)
    }

    // MARK: - 4. Task Cancellation Propagation

    func testTaskCancellationPropagation() async throws {
        let largeFile = sandbox.fileURL(named: "cancellation_source.bin")
        let data = Data((0..<(512 * 1024)).map { UInt8($0 % 251) })
        try data.write(to: largeFile)

        let outZip = sandbox.fileURL(named: "cancellation_out.zip")

        let (_, handle, task) = service.compressStream(
            inputs: [largeFile.path],
            outputPath: outZip.path,
            format: .sevenZip,
            level: .ultra
        )

        // Cancel immediately
        handle.cancel()
        task.cancel()

        do {
            _ = try await task.value
        } catch is CancellationError {
            XCTAssertTrue(handle.isCancelled)
        } catch {
            XCTAssertTrue(handle.isCancelled)
        }

        XCTAssertTrue(service.isIdle)
    }

    // MARK: - 5. Cancel All Active Tasks

    func testCancelAllActiveTasks() async throws {
        let file1 = sandbox.fileURL(named: "cancel_all_1.bin")
        let file2 = sandbox.fileURL(named: "cancel_all_2.bin")
        let data = Data(repeating: 0x5A, count: 256 * 1024)
        try data.write(to: file1)
        try data.write(to: file2)

        let outZip1 = sandbox.fileURL(named: "cancel_out_1.zip")
        let outZip2 = sandbox.fileURL(named: "cancel_out_2.zip")

        let (_, _, task1) = service.compressStream(inputs: [file1.path], outputPath: outZip1.path, format: .sevenZip, level: .ultra)
        let (_, _, task2) = service.compressStream(inputs: [file2.path], outputPath: outZip2.path, format: .sevenZip, level: .ultra)

        service.cancelAll()

        _ = try? await task1.value
        _ = try? await task2.value

        XCTAssertTrue(service.isIdle)
    }

    // MARK: - 6. Asynchronous Inspection & Hash Calculation

    func testInspectAsyncAndHashCalculation() async throws {
        let doc = sandbox.fileURL(named: "inspect_doc.txt")
        try "Inspectable Content 1234567890".write(to: doc, atomically: true, encoding: .utf8)

        let outZip = sandbox.fileURL(named: "inspect_test.zip")
        let (_, _, compTask) = service.compressStream(inputs: [doc.path], outputPath: outZip.path, format: .zip)
        _ = try await compTask.value

        let inspectResult = try await service.inspectAsync(archivePath: outZip.path)
        XCTAssertEqual(inspectResult.archivePath, outZip.path)
        XCTAssertFalse(inspectResult.entries.isEmpty)

        let sha256 = try await service.calculateHashAsync(filePath: outZip.path, type: .sha256)
        XCTAssertEqual(sha256.count, 64)

        let crc32 = try await service.calculateHashAsync(filePath: outZip.path, type: .crc32)
        XCTAssertEqual(crc32.count, 8)
    }

    // MARK: - 7. Batch Compression & Extraction with Bounded Concurrency

    func testBatchCompressAndExtractBoundedConcurrency() async throws {
        let count = 4
        var compressRequests: [TTZipBatchCompressRequest] = []
        var zipPaths: [URL] = []
        var extractRequests: [TTZipBatchExtractRequest] = []

        for i in 0..<count {
            let src = sandbox.fileURL(named: "batch_src_\(i).txt")
            try "Batch payload number \(i)".write(to: src, atomically: true, encoding: .utf8)

            let outZip = sandbox.fileURL(named: "batch_out_\(i).zip")
            zipPaths.append(outZip)
            compressRequests.append(
                TTZipBatchCompressRequest(
                    inputs: [src.path],
                    outputPath: outZip.path,
                    format: .zip,
                    level: .fast
                )
            )

            let extDir = try sandbox.createSubdirectory("batch_ext_\(i)")
            extractRequests.append(
                TTZipBatchExtractRequest(
                    archivePath: outZip.path,
                    destinationDir: extDir.path
                )
            )
        }

        // Execute batch compress with concurrency limit 2
        let compResults = try await service.executeBatchCompress(
            requests: compressRequests,
            maxConcurrentTasks: 2,
            qos: .userInitiated
        )
        XCTAssertEqual(compResults.count, count)
        for (i, res) in compResults.enumerated() {
            XCTAssertEqual(res.outputPath, zipPaths[i].path)
            XCTAssertTrue(FileManager.default.fileExists(atPath: res.outputPath))
        }

        // Execute batch extract with concurrency limit 2
        let extResults = try await service.executeBatchExtract(
            requests: extractRequests,
            maxConcurrentTasks: 2,
            qos: .userInitiated
        )
        XCTAssertEqual(extResults.count, count)
        for (i, res) in extResults.enumerated() {
            let extractedFile = URL(fileURLWithPath: res.destinationDir).appendingPathComponent("batch_src_\(i).txt")
            XCTAssertTrue(FileManager.default.fileExists(atPath: extractedFile.path))
        }
    }

    // MARK: - 8. End-to-End Pipeline Execution

    func testExecutePipelineEndToEnd() async throws {
        let srcFile = sandbox.fileURL(named: "pipeline_doc.txt")
        try "End to End Pipeline Structured Concurrency Payload".write(to: srcFile, atomically: true, encoding: .utf8)

        let outZip = sandbox.fileURL(named: "pipeline_archive.zip")
        let destDir = try sandbox.createSubdirectory("pipeline_extracted")

        let pipelineResult = try await service.executePipeline(
            inputs: [srcFile.path],
            archivePath: outZip.path,
            destinationDir: destDir.path,
            format: .zip,
            level: .normal,
            verifyIntegrity: true,
            qos: .userInitiated
        )

        XCTAssertTrue(pipelineResult.isVerified)
        XCTAssertNotNil(pipelineResult.sha256Checksum)
        XCTAssertNotNil(pipelineResult.crc32Checksum)
        XCTAssertNotNil(pipelineResult.extractResult)
        XCTAssertGreaterThan(pipelineResult.archiveResult.compressedBytes, 0)
        XCTAssertGreaterThanOrEqual(pipelineResult.totalDurationSeconds, 0)

        let finalDoc = destDir.appendingPathComponent("pipeline_doc.txt")
        XCTAssertTrue(FileManager.default.fileExists(atPath: finalDoc.path))
    }

    // MARK: - 9. QoS Profile Execution Safety

    func testQoSProfilesDispatch() async throws {
        let profiles: [ExecutionQoSProfile] = [.interactive, .userInitiated, .utility, .background]

        for (idx, qos) in profiles.enumerated() {
            let file = sandbox.fileURL(named: "qos_src_\(idx).txt")
            try "QoS profile payload \(idx)".write(to: file, atomically: true, encoding: .utf8)
            let outZip = sandbox.fileURL(named: "qos_out_\(idx).zip")

            let (_, _, task) = service.compressStream(
                inputs: [file.path],
                outputPath: outZip.path,
                format: .zip,
                qos: qos
            )
            let res = try await task.value
            XCTAssertTrue(FileManager.default.fileExists(atPath: res.outputPath))
        }
    }
}
