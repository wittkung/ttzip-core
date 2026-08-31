// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class TTZipMmapStreamServiceTests: XCTestCase {

    private var tempDir: URL!

    override func setUp() {
        super.setUp()
        tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("ttzip_mmap_test_\(UUID().uuidString)")
        try? FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    override func tearDown() {
        if let tempDir = tempDir {
            try? FileManager.default.removeItem(at: tempDir)
        }
        super.tearDown()
    }

    // MARK: - Error Handling & Edge Cases

    func testNonExistentFileThrows() {
        let nonExistentPath = tempDir.appendingPathComponent("does_not_exist.bin").path
        XCTAssertThrowsError(try TTZipMmapStreamService(path: nonExistentPath))
    }

    func testEmptyFileMapping() throws {
        let emptyURL = tempDir.appendingPathComponent("empty.bin")
        try Data().write(to: emptyURL)

        let service = try TTZipMmapStreamService(path: emptyURL.path)
        XCTAssertTrue(service.isEmpty)
        XCTAssertEqual(service.fileSize, 0)
        XCTAssertEqual(service.mappedSize, 0)
        XCTAssertTrue(service.isReadOnly)

        let allData = try service.readAll()
        XCTAssertTrue(allData.isEmpty)

        let slice = try service.readSlice(offset: 0, length: 100)
        XCTAssertEqual(slice.length, 0)
        XCTAssertTrue(slice.data.isEmpty)

        let chunks = try service.readChunks(chunkSize: 1024)
        XCTAssertTrue(chunks.isEmpty)

        let crc = try service.computeCRC32()
        XCTAssertEqual(crc, 0)
    }

    // MARK: - Payload Reading & Slicing

    func testPayloadReadingAndSlicing() throws {
        let fileURL = tempDir.appendingPathComponent("payload.bin")
        let testString = "TTZip High-Performance Native Zero-Copy Mmap Stream Facade"
        let testData = Data(testString.utf8)
        try testData.write(to: fileURL)

        let service = try TTZipMmapStreamService.open(path: fileURL.path)
        XCTAssertFalse(service.isEmpty)
        XCTAssertEqual(service.fileSize, UInt64(testData.count))
        XCTAssertEqual(service.path, fileURL.path)

        // Read all
        let readAllData = try service.readAll()
        XCTAssertEqual(readAllData, testData)

        // Read slice
        let slice = try service.readSlice(offset: 6, length: 16)
        XCTAssertEqual(slice.offset, 6)
        XCTAssertEqual(slice.length, 16)
        let sliceString = String(data: slice.data, encoding: .utf8)
        XCTAssertEqual(sliceString, "High-Performance")

        // Read bytes
        let bytesData = try service.readBytes(offset: 0, length: 5)
        XCTAssertEqual(String(data: bytesData, encoding: .utf8), "TTZip")

        // Metrics tracking
        XCTAssertGreaterThanOrEqual(service.totalSlicesRead, 2)
        XCTAssertGreaterThan(service.totalBytesStreamed, 0)
    }

    // MARK: - Zero-Copy Borrowing

    func testWithUnsafeSliceZeroCopyBorrowing() throws {
        let fileURL = tempDir.appendingPathComponent("borrow.bin")
        var buffer = [UInt8](repeating: 0, count: 8192)
        for i in 0..<buffer.count {
            buffer[i] = UInt8(i & 0xFF)
        }
        let originalData = Data(buffer)
        try originalData.write(to: fileURL)

        let service = try TTZipMmapStreamService(path: fileURL.path)

        let computedSum: UInt64 = try service.withUnsafeSlice(offset: 100, length: 200) { rawBuffer in
            XCTAssertEqual(rawBuffer.count, 200)
            var sum: UInt64 = 0
            for byte in rawBuffer {
                sum += UInt64(byte)
            }
            return sum
        }

        var expectedSum: UInt64 = 0
        for i in 100..<300 {
            expectedSum += UInt64(buffer[i])
        }
        XCTAssertEqual(computedSum, expectedSum)
    }

    // MARK: - Async Streaming APIs

    func testChunkSequenceAndDataStream() async throws {
        let fileURL = tempDir.appendingPathComponent("streaming.bin")
        let totalSize = 32 * 1024 // 32 KB
        var randomBytes = [UInt8](repeating: 0, count: totalSize)
        for i in 0..<totalSize {
            randomBytes[i] = UInt8((i * 31 + 7) & 0xFF)
        }
        let payload = Data(randomBytes)
        try payload.write(to: fileURL)

        let service = try await TTZipMmapStreamService.openAsync(path: fileURL.path)

        // Test chunkSequence
        var reassembledFromSlices = Data()
        for await slice in service.chunkSequence(chunkSize: 4096) {
            reassembledFromSlices.append(slice.data)
        }
        XCTAssertEqual(reassembledFromSlices, payload)

        // Test dataStream
        var reassembledFromData = Data()
        for await dataChunk in service.dataStream(chunkSize: 8192) {
            reassembledFromData.append(dataChunk)
        }
        XCTAssertEqual(reassembledFromData, payload)
    }

    // MARK: - Kernel Advice Controls

    func testKernelAdviceOperations() throws {
        let fileURL = tempDir.appendingPathComponent("advice.bin")
        let payload = Data(repeating: 0x42, count: 16384)
        try payload.write(to: fileURL)

        let service = try TTZipMmapStreamService(path: fileURL.path)

        try service.adviseSequential()
        XCTAssertEqual(service.lastAdviceIssued, .sequential)

        try service.adviseRandom()
        XCTAssertEqual(service.lastAdviceIssued, .random)

        try service.adviseWillNeed(offset: 0, length: 4096)
        XCTAssertEqual(service.lastAdviceIssued, .willNeed)

        try service.adviseDontNeed(offset: 0, length: 4096)
        XCTAssertEqual(service.lastAdviceIssued, .dontNeed)
    }

    // MARK: - Pattern Search & Checksum Verification

    func testSubsequenceSearchAndChecksums() async throws {
        let fileURL = tempDir.appendingPathComponent("search_checksum.bin")
        let header = "PK\u{03}\u{04}START"
        let middle = "MIDDLE_MARKER_FOR_TESTING"
        let trailer = "TRAILER_CRC_OK"
        let fullString = header + String(repeating: "A", count: 1000) + middle + String(repeating: "B", count: 1000) + trailer
        let fullData = Data(fullString.utf8)
        try fullData.write(to: fileURL)

        let service = try TTZipMmapStreamService(path: fileURL.path)

        // Subsequence string search
        let middleOffset = service.findSubsequence(pattern: middle)
        XCTAssertNotNil(middleOffset)
        XCTAssertEqual(middleOffset, UInt64(header.utf8.count + 1000))

        // Subsequence data search async
        let trailerData = Data(trailer.utf8)
        let trailerOffset = await service.findSubsequenceAsync(pattern: trailerData)
        XCTAssertNotNil(trailerOffset)

        // Non-existent pattern
        let notFound = service.findSubsequence(pattern: "NON_EXISTENT_MARKER")
        XCTAssertNil(notFound)

        // Checksums
        let crc = try service.computeCRC32()
        XCTAssertNotEqual(crc, 0)

        let crcAsync = try await service.computeCRC32Async()
        XCTAssertEqual(crc, crcAsync)

        let xxh = try service.computeXXH3()
        XCTAssertNotEqual(xxh, 0)

        let xxhAsync = try await service.computeXXH3Async()
        XCTAssertEqual(xxh, xxhAsync)

        // Diagnostics stats
        let stats = service.stats()
        XCTAssertEqual(stats.fileSize, UInt64(fullData.count))
        XCTAssertFalse(stats.isEmpty)
        XCTAssertTrue(stats.isReadonly)
    }

    // MARK: - Concurrency & Lifecycle

    func testConcurrentMultiTaskReading() async throws {
        let fileURL = tempDir.appendingPathComponent("concurrent.bin")
        let size = 64 * 1024
        let data = Data((0..<size).map { UInt8($0 % 256) })
        try data.write(to: fileURL)

        let service = try TTZipMmapStreamService(path: fileURL.path)

        try await withThrowingTaskGroup(of: Void.self) { group in
            for i in 0..<10 {
                group.addTask {
                    let offset = UInt64(i * 1000)
                    let slice = try service.readSlice(offset: offset, length: 500)
                    XCTAssertEqual(slice.length, 500)
                    XCTAssertEqual(slice.data, data.subdata(in: Int(offset)..<Int(offset) + 500))
                }
            }
            try await group.waitForAll()
        }

        XCTAssertGreaterThanOrEqual(service.totalSlicesRead, 10)
    }
}
