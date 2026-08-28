// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import CryptoKit
import Darwin
@testable import TTZipCore

/// Comprehensive enterprise-grade test file and multi-modal payload generator for TTZip.
public enum TestFileGenerator {

    // MARK: - Fast Deterministic PRNG

    /// Ultra-fast deterministic XorShift128+ generator for reproducible zero-lock synthetic payloads.
    public struct FastPRNG: Sendable {
        private var s0: UInt64
        private var s1: UInt64

        public init(seed: UInt64 = 0x18F2_47A9_80C3_B1D5) {
            var z = seed &+ 0x9E37_79B9_7F4A_7C15
            z = (z ^ (z >> 30)) &* 0xBF58_476D_1CE4_E5B9
            z = (z ^ (z >> 27)) &* 0x94D0_49BB_1331_11EB
            self.s0 = z ^ (z >> 31)
            z = z &+ 0x9E37_79B9_7F4A_7C15
            z = (z ^ (z >> 30)) &* 0xBF58_476D_1CE4_E5B9
            z = (z ^ (z >> 27)) &* 0x94D0_49BB_1331_11EB
            self.s1 = z ^ (z >> 31)
        }

        public mutating func nextUInt64() -> UInt64 {
            var x = s0
            let y = s1
            s0 = y
            x ^= x << 23
            s1 = x ^ y ^ (x >> 17) ^ (y >> 26)
            return s1 &+ y
        }

        public mutating func nextUInt32() -> UInt32 {
            UInt32(truncatingIfNeeded: nextUInt64())
        }

        public mutating func nextDouble() -> Double {
            Double(nextUInt64() >> 11) * (1.0 / 9007199254740992.0)
        }

        public mutating func nextInt(in range: ClosedRange<Int>) -> Int {
            let span = UInt64(range.upperBound - range.lowerBound + 1)
            let offset = nextUInt64() % span
            return range.lowerBound + Int(offset)
        }
    }

    // MARK: - 1. Zipf Law Natural Text Generator

    private static let zipfDictionary: [String] = [
        "the", "of", "and", "a", "to", "in", "is", "you", "that", "it",
        "he", "was", "for", "on", "are", "as", "with", "his", "they", "i",
        "at", "be", "this", "have", "from", "or", "one", "had", "by", "word",
        "but", "not", "what", "all", "were", "we", "when", "your", "can", "said",
        "there", "use", "an", "each", "which", "she", "do", "how", "their", "if",
        "will", "up", "other", "about", "out", "many", "then", "them", "these", "so",
        "some", "her", "would", "make", "like", "him", "into", "time", "has", "look",
        "two", "more", "write", "go", "see", "number", "no", "way", "could", "people",
        "my", "than", "first", "water", "been", "call", "who", "oil", "its", "now",
        "find", "long", "down", "day", "did", "get", "come", "made", "may", "part",
        "compression", "archive", "kernel", "uniffi", "pipeline", "buffer", "stream", "checksum",
        "allocation", "entropy", "payload", "throughput", "concurrency", "dispatch", "filesystem", "apfs"
    ]

    /// Generates natural English prose obeying Zipf's law with realistic sentence and paragraph structures.
    public static func generateZipfText(byteCount: Int, seed: UInt64 = 0x51A1_290F) -> Data {
        var prng = FastPRNG(seed: seed)
        var buffer = Data()
        buffer.reserveCapacity(byteCount)

        let dictCount = zipfDictionary.count
        let harmonicH: Double = (1...dictCount).reduce(0.0) { $0 + (1.0 / Double($1)) }
        var cdf = [Double](repeating: 0.0, count: dictCount)
        var cumulative = 0.0
        for i in 0..<dictCount {
            cumulative += (1.0 / Double(i + 1)) / harmonicH
            cdf[i] = cumulative
        }

        func pickWord(rng: inout FastPRNG) -> String {
            let r = rng.nextDouble()
            var low = 0
            var high = dictCount - 1
            while low < high {
                let mid = (low + high) / 2
                if cdf[mid] < r {
                    low = mid + 1
                } else {
                    high = mid
                }
            }
            return zipfDictionary[low]
        }

        var isStartOfSentence = true
        var wordsInSentence = 0
        let targetWordsInSentence = prng.nextInt(in: 6...18)
        var sentencesInParagraph = 0

        while buffer.count < byteCount {
            var word = pickWord(rng: &prng)
            if isStartOfSentence {
                word = word.prefix(1).uppercased() + word.dropFirst()
                isStartOfSentence = false
            }

            let wordBytes = Array(word.utf8)
            buffer.append(contentsOf: wordBytes)
            wordsInSentence += 1

            if buffer.count >= byteCount { break }

            if wordsInSentence >= targetWordsInSentence {
                buffer.append(0x2E) // '.'
                buffer.append(0x20) // ' '
                isStartOfSentence = true
                wordsInSentence = 0
                sentencesInParagraph += 1

                if sentencesInParagraph >= 4 {
                    buffer.append(0x0A) // '\n'
                    buffer.append(0x0A) // '\n'
                    sentencesInParagraph = 0
                }
            } else {
                if wordsInSentence % 5 == 0 && prng.nextDouble() < 0.25 {
                    buffer.append(0x2C) // ','
                    buffer.append(0x20) // ' '
                } else {
                    buffer.append(0x20) // ' '
                }
            }
        }

        if buffer.count > byteCount {
            buffer = buffer.subdata(in: 0..<byteCount)
        }
        return buffer
    }

    // MARK: - 2. Machine Code & DWARF Metadata Generator

    public enum MachineCodeArch: Sendable {
        case arm64
        case x86_64
        case mixed
    }

    /// Generates realistic ARM64 / x86-64 machine code instructions combined with aligned DWARF debug records.
    public static func generateMachineCode(byteCount: Int, arch: MachineCodeArch = .arm64, seed: UInt64 = 0xAA64_4889) -> Data {
        var prng = FastPRNG(seed: seed)
        var data = Data()
        data.reserveCapacity(byteCount)

        // Standard Mach-O 64-bit Header emulation (32 bytes)
        let magic: UInt32 = 0xFEEDFACF // MH_MAGIC_64
        let cputype: UInt32 = (arch == .x86_64) ? 0x01000007 : 0x0100000C // CPU_TYPE_X86_64 / ARM64
        let cpusubtype: UInt32 = 0x80000002
        let filetype: UInt32 = 0x00000002 // MH_EXECUTE
        let ncmds: UInt32 = 0x00000004
        let sizeofcmds: UInt32 = 0x00000100
        let flags: UInt32 = 0x00200085
        let reserved: UInt32 = 0x00000000

        withUnsafeBytes(of: magic) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: cputype) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: cpusubtype) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: filetype) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: ncmds) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: sizeofcmds) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: flags) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: reserved) { data.append(contentsOf: $0) }

        // ARM64 canonical instruction templates
        let arm64Templates: [UInt32] = [
            0xA9BF7BFD, // stp x29, x30, [sp, #-16]!
            0x910003FD, // mov x29, sp
            0xA8C17BFD, // ldp x29, x30, [sp], #16
            0xD65F03C0, // ret
            0xD503201F, // nop
            0xAA0003E0, // mov x0, x0
            0x8B010000, // add x0, x0, x1
            0xCB010000, // sub x0, x0, x1
            0xF9400000, // ldr x0, [x0]
            0xF9000000, // str x0, [x0]
            0x94000001, // bl +4
            0x14000001  // b +4
        ]

        // x86-64 canonical opcode byte sequences
        let x86Sequences: [[UInt8]] = [
            [0x55, 0x48, 0x89, 0xE5],             // push %rbp; mov %rsp, %rbp
            [0x48, 0x83, 0xEC, 0x20],             // sub $0x20, %rsp
            [0x48, 0x8B, 0x05, 0x00, 0x00, 0x00, 0x00], // mov 0(%rip), %rax
            [0x48, 0x01, 0xD8],                   // add %rbx, %rax
            [0x48, 0x89, 0x45, 0xF8],             // mov %rax, -8(%rbp)
            [0x5D, 0xC3],                         // pop %rbp; retq
            [0x90, 0x90, 0x90, 0x90],             // 4x nop
            [0x0F, 0x1F, 0x44, 0x00, 0x00]        // 5-byte multi-byte nop
        ]

        let selectedArch = (arch == .mixed) ? (prng.nextDouble() > 0.5 ? MachineCodeArch.arm64 : .x86_64) : arch

        while data.count < byteCount {
            if selectedArch == .arm64 {
                var insn = arm64Templates[prng.nextInt(in: 0...(arm64Templates.count - 1))]
                // Add minor register variations
                let regVar = UInt32(prng.nextInt(in: 0...7))
                insn ^= regVar
                withUnsafeBytes(of: insn) { data.append(contentsOf: $0) }
            } else {
                let seq = x86Sequences[prng.nextInt(in: 0...(x86Sequences.count - 1))]
                data.append(contentsOf: seq)
            }

            // Periodically inject DWARF-like LEB128 & string records every 256 bytes
            if data.count % 256 < 16 && data.count + 32 <= byteCount {
                // Emulate .debug_str / .debug_line header
                let dwarfTag: [UInt8] = [0x00, 0x74, 0x74, 0x7A, 0x69, 0x70, 0x5F, 0x65, 0x6E, 0x67, 0x69, 0x6E, 0x65, 0x00] // "ttzip_engine\0"
                data.append(contentsOf: dwarfTag)
            }
        }

        if data.count > byteCount {
            data = data.subdata(in: 0..<byteCount)
        }
        return data
    }

    // MARK: - 3. Spatial Gradient RGB Image Matrix Generator

    /// Generates a PPM P6 binary image matrix featuring continuous 2D spatial gradients with high compression locality.
    public static func generateSpatialGradientRGB(width: Int, height: Int) -> Data {
        let header = "P6\n\(width) \(height)\n255\n"
        var data = Data(header.utf8)
        data.reserveCapacity(data.count + width * height * 3)

        let wF = Double(max(1, width))
        let hF = Double(max(1, height))

        for y in 0..<height {
            let yNorm = Double(y) / hF
            for x in 0..<width {
                let xNorm = Double(x) / wF
                // Continuous 2D trigonometric color field
                let rVal = UInt8(clamping: Int((sin(xNorm * .pi * 2.0) * 0.5 + 0.5) * 255.0))
                let gVal = UInt8(clamping: Int((cos(yNorm * .pi * 2.0) * 0.5 + 0.5) * 255.0))
                let bVal = UInt8(clamping: Int((sin((xNorm + yNorm) * .pi) * 0.5 + 0.5) * 255.0))

                data.append(rVal)
                data.append(gVal)
                data.append(bVal)
            }
        }
        return data
    }

    // MARK: - 4. Pseudo-Random High-Entropy White Noise Generator

    /// Generates high-entropy white noise (Shannon Entropy ~ 8.0 bits/byte) that is virtually incompressible.
    public static func generateHighEntropyNoise(byteCount: Int, seed: UInt64 = 0xFEEDBEEF_CAFEF00D) -> Data {
        var prng = FastPRNG(seed: seed)
        var data = Data(count: byteCount)
        data.withUnsafeMutableBytes { rawBuf in
            guard let ptr = rawBuf.baseAddress?.assumingMemoryBound(to: UInt64.self) else { return }
            let u64Count = byteCount / 8
            for i in 0..<u64Count {
                ptr[i] = prng.nextUInt64()
            }
            let rem = byteCount % 8
            if rem > 0 {
                var lastVal = prng.nextUInt64()
                withUnsafeBytes(of: &lastVal) { bytePtr in
                    for b in 0..<rem {
                        rawBuf[u64Count * 8 + b] = bytePtr[b]
                    }
                }
            }
        }
        return data
    }

    // MARK: - 5. APFS Sparse Hole Large File Generator

    public struct SparseFileInfo: Sendable {
        public let logicalSize: Int64
        public let allocatedPhysicalSize: Int64
        public let headerSignature: String
        public let footerSignature: String
    }

    /// Allocates an authentic APFS sparse file with non-allocated sparse holes via POSIX `lseek`.
    public static func createSparseHoleFile(
        at targetURL: URL,
        logicalSizeBytes: Int64,
        holeIntervalBytes: Int64 = 64 * 1024 * 1024,
        chunkSizeBytes: Int = 64 * 1024
    ) throws -> SparseFileInfo {
        let parentDir = targetURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: parentDir, withIntermediateDirectories: true)

        let path = targetURL.path
        let fd = open(path, O_WRONLY | O_CREAT | O_TRUNC, 0o644)
        guard fd >= 0 else {
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }
        defer { close(fd) }

        let headerData = generateMachineCode(byteCount: chunkSizeBytes, arch: .arm64, seed: 0x11112222)
        let footerData = generateZipfText(byteCount: chunkSizeBytes, seed: 0x33334444)

        // 1. Write Header chunk
        try headerData.withUnsafeBytes { rawPtr in
            guard let base = rawPtr.baseAddress else { return }
            let written = write(fd, base, chunkSizeBytes)
            guard written == chunkSizeBytes else {
                throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
            }
        }

        // 2. Seek across sparse holes if file is large enough
        var currentOffset: Int64 = Int64(chunkSizeBytes)
        while currentOffset + holeIntervalBytes + Int64(chunkSizeBytes) < logicalSizeBytes {
            currentOffset += holeIntervalBytes
            let seekRes = lseek(fd, off_t(currentOffset), SEEK_SET)
            guard seekRes >= 0 else {
                throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
            }

            // Write a small intermediate 4KB checkpoint
            let checkpointData = generateZipfText(byteCount: 4096, seed: UInt64(currentOffset))
            try checkpointData.withUnsafeBytes { rawPtr in
                guard let base = rawPtr.baseAddress else { return }
                let written = write(fd, base, 4096)
                guard written == 4096 else {
                    throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
                }
            }
            currentOffset += 4096
        }

        // 3. Seek to footer boundary
        let footerOffset = logicalSizeBytes - Int64(chunkSizeBytes)
        if footerOffset > currentOffset {
            let seekRes = lseek(fd, off_t(footerOffset), SEEK_SET)
            guard seekRes >= 0 else {
                throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
            }
        }

        // 4. Write Footer chunk
        try footerData.withUnsafeBytes { rawPtr in
            guard let base = rawPtr.baseAddress else { return }
            let written = write(fd, base, chunkSizeBytes)
            guard written == chunkSizeBytes else {
                throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
            }
        }

        // Query APFS physical allocated block size
        var statInfo = stat()
        guard stat(path, &statInfo) == 0 else {
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }

        let physicalAllocated = Int64(statInfo.st_blocks) * 512
        let headerHash = SHA256.hash(data: headerData).compactMap { String(format: "%02x", $0) }.joined()
        let footerHash = SHA256.hash(data: footerData).compactMap { String(format: "%02x", $0) }.joined()

        return SparseFileInfo(
            logicalSize: Int64(statInfo.st_size),
            allocatedPhysicalSize: physicalAllocated,
            headerSignature: headerHash,
            footerSignature: footerHash
        )
    }

    // MARK: - Multi-Modal Directory Tree Generation (10,000+ Files Support)

    /// Generates a deeply nested directory tree populated with thousands of realistic multi-modal test files.
    @discardableResult
    public static func createMultiModalFileTree(
        in rootDir: URL,
        totalFiles: Int,
        maxDepth: Int = 6,
        minFileSize: Int = 512,
        maxFileSize: Int = 4096
    ) throws -> [URL] {
        let fm = FileManager.default
        try fm.createDirectory(at: rootDir, withIntermediateDirectories: true)

        // Pre-create multi-level directory hierarchy
        var directoryPool: [URL] = [rootDir]
        let dirBreadth = max(2, Int(cbrt(Double(totalFiles))) / 2)

        func buildDirs(parent: URL, currentDepth: Int) throws {
            guard currentDepth < maxDepth else { return }
            for d in 0..<dirBreadth {
                let sub = parent.appendingPathComponent("layer_\(currentDepth)_dir_\(d)")
                try fm.createDirectory(at: sub, withIntermediateDirectories: true)
                directoryPool.append(sub)
                if currentDepth + 1 < maxDepth && d < 2 {
                    try buildDirs(parent: sub, currentDepth: currentDepth + 1)
                }
            }
        }
        try buildDirs(parent: rootDir, currentDepth: 1)

        var generatedURLs = [URL]()
        generatedURLs.reserveCapacity(totalFiles)

        var prng = FastPRNG(seed: 0x9876_5432_10FE_DCBA)
        let dirPoolCount = directoryPool.count

        // Dispatch chunked writes
        let batchSize = 500
        let batches = (totalFiles + batchSize - 1) / batchSize

        for b in 0..<batches {
            let startIdx = b * batchSize
            let endIdx = min(totalFiles, startIdx + batchSize)

            for i in startIdx..<endIdx {
                let dir = directoryPool[i % dirPoolCount]
                let fileSize = prng.nextInt(in: minFileSize...maxFileSize)
                let modalityIdx = i % 4

                let filename: String
                let data: Data

                switch modalityIdx {
                case 0:
                    filename = "doc_\(i).txt"
                    data = generateZipfText(byteCount: fileSize, seed: UInt64(i))
                case 1:
                    filename = "lib_\(i).o"
                    data = generateMachineCode(byteCount: fileSize, arch: .arm64, seed: UInt64(i))
                case 2:
                    let dim = max(8, Int(sqrt(Double(fileSize / 3))))
                    filename = "image_\(i).ppm"
                    data = generateSpatialGradientRGB(width: dim, height: dim)
                default:
                    filename = "noise_\(i).bin"
                    data = generateHighEntropyNoise(byteCount: fileSize, seed: UInt64(i))
                }

                let fileURL = dir.appendingPathComponent(filename)
                try data.write(to: fileURL)
                generatedURLs.append(fileURL)
            }
        }

        return generatedURLs
    }

    // MARK: - Legacy Compatibility Methods (Now Powered by Multi-Modal Engine)

    @discardableResult
    public static func createBatchSmallFiles(in directory: URL, count: Int, sizePerFileInKB: Int) throws -> [URL] {
        let sizeInBytes = sizePerFileInKB * 1024
        return try createMultiModalFileTree(
            in: directory,
            totalFiles: count,
            maxDepth: 3,
            minFileSize: sizeInBytes,
            maxFileSize: sizeInBytes
        )
    }

    public static func createHugeFile(at targetURL: URL, sizeInMB: Int) throws {
        let parentDir = targetURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: parentDir, withIntermediateDirectories: true)

        guard let stream = OutputStream(url: targetURL, append: false) else { return }
        stream.open()
        defer { stream.close() }

        let chunkSize = 1024 * 1024
        for mb in 0..<sizeInMB {
            let chunkData = generateMachineCode(byteCount: chunkSize, arch: .arm64, seed: UInt64(mb))
            _ = chunkData.withUnsafeBytes { rawPtr in
                guard let base = rawPtr.baseAddress else { return 0 }
                return stream.write(base.assumingMemoryBound(to: UInt8.self), maxLength: chunkSize)
            }
        }
    }

    public static func createRealisticLogFile(at targetURL: URL, linesCount: Int) throws {
        let parentDir = targetURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: parentDir, withIntermediateDirectories: true)

        var logData = Data()
        let ips = ["192.168.1.10", "10.0.4.22", "172.16.8.99", "127.0.0.1", "10.200.15.1"]
        let endpoints = ["/api/v1/archive/inspect", "/vfs/tree/query", "/compression/deflate", "/auth/keychain/vault", "/bench/pipeline"]
        var prng = FastPRNG(seed: 0x4242)

        for lineIdx in 0..<linesCount {
            let ip = ips[lineIdx % ips.count]
            let endpoint = endpoints[lineIdx % endpoints.count]
            let status = (lineIdx % 20 == 0) ? 500 : 200
            let duration = prng.nextInt(in: 1...120)
            let line = "2026-08-28T15:\(String(format: "%02d", lineIdx % 60)):00.123Z [INFO] host=\(ip) method=POST path=\(endpoint) status=\(status) duration=\(duration)ms\n"
            logData.append(contentsOf: line.utf8)
        }
        try logData.write(to: targetURL)
    }

    public static func createHugeEncryptedFile(at targetURL: URL, sizeInMB: Int) throws {
        let parentDir = targetURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: parentDir, withIntermediateDirectories: true)

        let rawData = generateHighEntropyNoise(byteCount: sizeInMB * 1024 * 1024, seed: 0x9999)
        let key = SymmetricKey(size: .bits256)
        let sealedBox = try AES.GCM.seal(rawData, using: key)
        guard let combinedData = sealedBox.combined else { return }
        try combinedData.write(to: targetURL)
    }

    public static func createInstantHugeFile(atPath path: String, sizeInMB: Int) {
        let url = URL(fileURLWithPath: path)
        _ = try? createSparseHoleFile(at: url, logicalSizeBytes: Int64(sizeInMB) * 1024 * 1024)
    }
}

// MARK: - Diagnostic Test Logger

public enum TTZipTestLogger {
    public static func logHeader(_ title: String) {
        TTLogger.debug("\n================================================================================")
        TTLogger.debug("  📊 [TTZip Test Suite] \(title)")
        TTLogger.debug("================================================================================")
    }

    public static func logMetricsRow(
        format: String,
        payloadMB: Double,
        compressedMB: Double,
        compressSpeedMBs: Double,
        decompressSpeedMBs: Double,
        elapsedSeconds: Double
    ) {
        let ratio = (compressedMB / max(0.001, payloadMB)) * 100.0
        let status = (compressSpeedMBs >= 150.0 && decompressSpeedMBs >= 500.0) ? "PASS [PERF_OPTIMAL]" : "PASS [PERF_ACCEPTABLE]"
        let pMB = String(format: "%.2f", payloadMB)
        let cMB = String(format: "%.2f", compressedMB)
        let rP = String(format: "%.1f", ratio)
        let cSpd = String(format: "%.1f", compressSpeedMBs)
        let dSpd = String(format: "%.1f", decompressSpeedMBs)
        let el = String(format: "%.3f", elapsedSeconds)
        TTLogger.debug("  [▶ \(format)] Payload: \(pMB) MB | Archive: \(cMB) MB (\(rP)%) | Codec: \(cSpd) / \(dSpd) MB/s | Elapsed: \(el) s -> \(status)")
    }

    public static func logSuiteSummary(suiteName: String, totalTests: Int, passed: Int, failed: Int, duration: Double) {
        TTLogger.debug("--------------------------------------------------------------------------------")
        TTLogger.debug("  ✅ Test Suite [\(suiteName)] Completed: \(totalTests) tests | \(passed) passed | \(failed) failed | Total time: \(String(format: "%.3f", duration)) s")
        TTLogger.debug("--------------------------------------------------------------------------------\n")
    }
}
