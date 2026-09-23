// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

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

        // Read slice (UniFFI record overload)
        let slice = try service.readSlice(offset: 6, length: 16)
        XCTAssertEqual(slice.offset, 6)
        XCTAssertEqual(slice.length, 16)
        let sliceString = String(data: slice.data, encoding: .utf8)
        XCTAssertEqual(sliceString, "High-Performance")

        // Read slice (signed offset Data overload)
        let sliceData = try service.readSlice(at: 6, length: 16)
        XCTAssertEqual(String(data: sliceData, encoding: .utf8), "High-Performance")

        // Read bytes
        let bytesData = try service.readBytes(offset: 0, length: 5)
        XCTAssertEqual(String(data: bytesData, encoding: .utf8), "TTZip")

        // Metrics tracking
        XCTAssertGreaterThanOrEqual(service.totalSlicesRead, 2)
        XCTAssertGreaterThan(service.totalBytesStreamed, 0)
    }

    // MARK: - Scoped Zero-Copy Borrowing

    func testWithSliceScopedClosure() throws {
        let fileURL = tempDir.appendingPathComponent("scoped_borrow.bin")
        let testString = "ScopedMemoryBorrowingValidation12345"
        let testData = Data(testString.utf8)
        try testData.write(to: fileURL)

        let service = try TTZipMmapStreamService(path: fileURL.path)

        let extracted: String = try service.withSlice(at: 6, length: 15) { rawBuffer in
            XCTAssertEqual(rawBuffer.count, 15)
            let data = Data(bytes: rawBuffer.baseAddress!, count: rawBuffer.count)
            return String(data: data, encoding: .utf8) ?? ""
        }
        XCTAssertEqual(extracted, "MemoryBorrowing")

        // Out of bounds / invalid parameters throw
        XCTAssertThrowsError(try service.withSlice(at: -1, length: 5) { _ in })
        XCTAssertThrowsError(try service.withSlice(at: 0, length: -1) { _ in })
        XCTAssertThrowsError(try service.withSlice(at: 1000, length: 5) { _ in })
    }

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

    func testReadZeroCopyData() throws {
        let fileURL = tempDir.appendingPathComponent("zerocopy.bin")
        let content = "ZeroCopyMemoryMappingTestPayload1234567890"
        let data = content.data(using: .utf8)!
        try data.write(to: fileURL)

        let service = try TTZipMmapStreamService(path: fileURL.path)
        let sliceData = try service.readZeroCopy(offset: 8, length: 13)
        XCTAssertEqual(sliceData.count, 13)
        let str = String(data: sliceData, encoding: .utf8)
        XCTAssertEqual(str, "MemoryMapping")

        // Out of bounds offset
        XCTAssertThrowsError(try service.readZeroCopy(offset: 1000, length: 10))
    }

    // MARK: - Post-Deinit Data Lifetime Safety (Dangling Pointer / SIGBUS Prevention)

    func testEscapingDataSafeAfterServiceDeallocation() throws {
        let fileURL = tempDir.appendingPathComponent("dealloc_safety.bin")
        let payloadString = "DanglingPointerPreventionAndKernelMmapSafetyVerification"
        let testData = Data(payloadString.utf8)
        try testData.write(to: fileURL)

        var escapedSlice: TTZipMmapSlice? = nil
        var escapedData: Data? = nil

        try {
            let localService = try TTZipMmapStreamService(path: fileURL.path)
            escapedSlice = try localService.readSlice(offset: 0, length: UInt64(testData.count))
            escapedData = try localService.readSlice(at: 15, length: 10)
            // localService goes out of scope and is deallocated here
        }()

        // Accessing escaped data after deinit must NOT crash with SIGBUS or EXC_BAD_ACCESS
        XCTAssertNotNil(escapedSlice)
        XCTAssertEqual(escapedSlice?.data, testData)
        XCTAssertNotNil(escapedData)
        XCTAssertEqual(String(data: escapedData!, encoding: .utf8), "Prevention")
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

        // Test streamChunks (AsyncThrowingStream)
        var reassembledFromStreamChunks = Data()
        for try await chunk in service.streamChunks(chunkSize: 4096) {
            reassembledFromStreamChunks.append(chunk.data)
        }
        XCTAssertEqual(reassembledFromStreamChunks, payload)
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

    func testMmapResultCacheReuseAcrossSlices() throws {
        let fileURL = tempDir.appendingPathComponent("cache_reuse.bin")
        let pattern = "ReusableMmapBufferValidationAcrossMultipleSlices1234567890"
        let testString = String(repeating: pattern, count: 10) // 580 bytes
        let testData = Data(testString.utf8)
        try testData.write(to: fileURL)

        let reader = try UniFfiMmapReader.open(path: fileURL.path)
        let slice1 = try reader.dataNoCopy(offset: 0, length: 64)
        let slice2 = try reader.dataNoCopy(offset: 64, length: 64)

        XCTAssertEqual(slice1.count, 64)
        XCTAssertEqual(slice2.count, 64)

        // Verify physical VMA reuse: both slices are derived from the same base address
        slice1.withUnsafeBytes { b1 in
            slice2.withUnsafeBytes { b2 in
                XCTAssertEqual(b1.baseAddress?.advanced(by: 64), b2.baseAddress)
            }
        }

        // Verify withZeroCopyBytes borrows from the exact same memory page
        try reader.withZeroCopyBytes(offset: 128, length: 64) { b3 in
            slice1.withUnsafeBytes { b1 in
                XCTAssertEqual(b1.baseAddress?.advanced(by: 128), b3.baseAddress)
            }
        }
    }
}
