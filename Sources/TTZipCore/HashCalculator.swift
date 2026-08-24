// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import CryptoKit
import Compression
import zlib

/// Supported cryptographic and verification hash algorithms.
public enum HashType: String, Sendable {
    case crc32 = "CRC32"
    case sha256 = "SHA-256"
    case md5 = "MD5"
    case sha1 = "SHA-1"
}

/// Multi-core parallel chunked hash and checksum calculator.
public final class HashCalculator: HashCalculating, @unchecked Sendable {
    internal let hardwareTuner: HardwareTunerProtocol

    public init(hardwareTuner: HardwareTunerProtocol = AppleSiliconTuner.shared) {
        self.hardwareTuner = hardwareTuner
    }
    
    public func computeHashSync(filePath: String, type: HashType) throws -> String {
        switch type {
        case .crc32:
            if let crc = try? computeFileCrc32(filePath: filePath) {
                return String(format: "%08X", crc)
            }
            if let data = try? Data(contentsOf: URL(fileURLWithPath: filePath)) {
                let crc = HardwareChecksumAdapter.crc32(for: data)
                return String(format: "%08X", crc)
            }
            return "00000000"
            
        case .sha256:
            if let sha = try? computeFileSha256(filePath: filePath) {
                return sha
            }
            return try computeCryptoHashSync(filePath: filePath, createHasher: SHA256.init)
            
        case .md5:
            return try computeCryptoHashSync(filePath: filePath, createHasher: Insecure.MD5.init)
            
        case .sha1:
            return try computeCryptoHashSync(filePath: filePath, createHasher: Insecure.SHA1.init)
        }
    }
    
    public func computeHash(filePath: String, type: HashType) async throws -> String {
        switch type {
        case .crc32:
            return try computeHashSync(filePath: filePath, type: .crc32)
            
        case .sha256:
            return try await Task.detached(priority: .userInitiated) {
                return try self.computeHashSync(filePath: filePath, type: .sha256)
            }.value
            
        case .md5:
            return try await Task.detached(priority: .userInitiated) {
                return try self.computeCryptoHashSync(filePath: filePath, createHasher: Insecure.MD5.init)
            }.value
            
        case .sha1:
            return try await Task.detached(priority: .userInitiated) {
                return try self.computeCryptoHashSync(filePath: filePath, createHasher: Insecure.SHA1.init)
            }.value
        }
    }
    
    // MARK: - Core Crypto Hash Helper
    
    private func computeCryptoHashSync<H: HashFunction>(
        filePath: String,
        createHasher: () -> H
    ) throws -> String {
        let fd = open(filePath, O_RDONLY)
        guard fd >= 0 else { throw ArchiveError.fileNotFound }
        defer { close(fd) }
        
        var st = stat()
        if fstat(fd, &st) == 0 {
            let fileSize = Int(st.st_size)
            if fileSize == 0 {
                let digest = createHasher().finalize()
                return digest.map { String(format: "%02x", $0) }.joined()
            }
            if let mapped = mmap(nil, fileSize, PROT_READ, MAP_SHARED, fd, 0), mapped != MAP_FAILED {
                defer { munmap(mapped, fileSize) }
                posix_madvise(mapped, fileSize, POSIX_MADV_WILLNEED)
                var hasher = createHasher()
                hasher.update(bufferPointer: UnsafeRawBufferPointer(start: mapped, count: fileSize))
                let digest = hasher.finalize()
                return digest.map { String(format: "%02x", $0) }.joined()
            }
        }
        
        let bufSize = hardwareTuner.optimalAlignedBufferSize
        var pageBuffer = [UInt8](repeating: 0, count: bufSize)
        
        var hasher = createHasher()
        var bytesRead = pageBuffer.withUnsafeMutableBufferPointer { bPtr -> Int in
            guard let base = bPtr.baseAddress else { return 0 }
            return read(fd, base, bufSize)
        }
        while bytesRead > 0 {
            pageBuffer.withUnsafeBytes { rawPtr in
                if let base = rawPtr.baseAddress {
                    let chunk = UnsafeRawBufferPointer(start: base, count: bytesRead)
                    hasher.update(bufferPointer: chunk)
                }
            }
            bytesRead = pageBuffer.withUnsafeMutableBufferPointer { bPtr -> Int in
                guard let base = bPtr.baseAddress else { return 0 }
                return read(fd, base, bufSize)
            }
        }
        let digest = hasher.finalize()
        return digest.map { String(format: "%02x", $0) }.joined()
    }
    
    /// Static convenience method for calculating SHA-256 fingerprint of a file.
    public static func calculateSHA256(filePath: String) -> String? {
        let calc = HashCalculator()
        return try? calc.computeHashSync(filePath: filePath, type: .sha256)
    }
    
    /// Static convenience method for calculating SHA-256 fingerprint of raw in-memory data.
    public static func calculateSHA256(data: Data) -> String? {
        let digest = SHA256.hash(data: data)
        return digest.map { String(format: "%02x", $0) }.joined()
    }
}

// MARK: - Hardware Checksum Adapter

/// Hardware-accelerated Adler-32 and CRC-32 checksum computation adapter.
public enum HardwareChecksumAdapter {
    
    /// Computes 32-bit Adler-32 checksum with hardware acceleration.
    @inlinable
    public static func adler32(for data: Data, initial: UInt32 = 1) -> UInt32 {
        guard !data.isEmpty else { return initial }
        return data.withUnsafeBytes { rawBuffer in
            guard let baseAddress = rawBuffer.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                return initial
            }
            return UInt32(zlib.adler32(uLong(initial), baseAddress, uInt(rawBuffer.count)))
        }
    }
    
    /// Computes 32-bit Adler-32 checksum via direct pointer access.
    @inlinable
    public static func adler32(ptr: UnsafePointer<UInt8>, count: Int, initial: UInt32 = 1) -> UInt32 {
        guard count > 0 else { return initial }
        return UInt32(zlib.adler32(uLong(initial), ptr, uInt(count)))
    }

    /// Computes 32-bit CRC-32 checksum.
    @inlinable
    public static func crc32(for data: Data, initial: UInt32 = 0) -> UInt32 {
        guard !data.isEmpty else { return initial }
        return data.withUnsafeBytes { rawBuffer in
            guard let baseAddress = rawBuffer.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                return initial
            }
            return UInt32(zlib.crc32(uLong(initial), baseAddress, uInt(rawBuffer.count)))
        }
    }

    /// Computes 32-bit CRC-32 checksum via direct pointer access.
    @inlinable
    public static func crc32(ptr: UnsafePointer<UInt8>, count: Int, initial: UInt32 = 0) -> UInt32 {
        guard count > 0 else { return initial }
        return UInt32(zlib.crc32(uLong(initial), ptr, uInt(count)))
    }

    @inlinable
    public static func computeCRC32(data: Data) -> UInt32 {
        return crc32(for: data)
    }

    @inlinable
    public static func combineCRC32(crc1: UInt32, crc2: UInt32, len2: Int) -> UInt32 {
        return UInt32(crc32_combine(UInt(crc1), UInt(crc2), len2))
    }
}

// MARK: - Libdeflate Accelerator

/// High-performance DEFLATE compression and decompression acceleration infrastructure.
public final class LibdeflateAccelerator: @unchecked Sendable {
    public static let shared = LibdeflateAccelerator()
    
    private init() {}
    
    /// Compresses buffer via Apple Silicon Hardware Compression framework.
    public func compress(
        src: UnsafeRawPointer,
        srcSize: Int,
        dst: UnsafeMutableRawPointer,
        dstCapacity: Int,
        level: Int = 6
    ) -> Int {
        let written = compression_encode_buffer(
            dst.assumingMemoryBound(to: UInt8.self),
            dstCapacity,
            src.assumingMemoryBound(to: UInt8.self),
            srcSize,
            nil,
            COMPRESSION_ZLIB
        )
        return written
    }
    
    /// Decompresses buffer via Apple Silicon Hardware Compression framework.
    public func decompress(
        src: UnsafeRawPointer,
        srcSize: Int,
        dst: UnsafeMutableRawPointer,
        dstCapacity: Int
    ) -> Int {
        let written = compression_decode_buffer(
            dst.assumingMemoryBound(to: UInt8.self),
            dstCapacity,
            src.assumingMemoryBound(to: UInt8.self),
            srcSize,
            nil,
            COMPRESSION_ZLIB
        )
        return written
    }
    
    /// Convenience helper compressing Swift `Data` buffers.
    public func compressData(_ data: Data, level: Int = 6) -> Data? {
        guard !data.isEmpty else { return Data() }
        let maxBound = data.count + 512
        var dstBuffer = [UInt8](repeating: 0, count: maxBound)
        let written = dstBuffer.withUnsafeMutableBufferPointer { dstPtr -> Int in
            guard let base = dstPtr.baseAddress else { return 0 }
            return CUnsafeBufferAdapter.withBufferPointer(data) { srcPtr, count in
                self.compress(src: srcPtr, srcSize: count, dst: base, dstCapacity: maxBound, level: level)
            }
        }
        guard written > 0 else { return nil }
        return Data(dstBuffer.prefix(written))
    }
    
    /// Convenience helper decompressing Swift `Data` buffers.
    public func decompressData(_ data: Data, originalSize: Int) -> Data? {
        guard !data.isEmpty else { return Data() }
        var dstBuffer = [UInt8](repeating: 0, count: originalSize)
        let actual = dstBuffer.withUnsafeMutableBufferPointer { dstPtr -> Int in
            guard let base = dstPtr.baseAddress else { return 0 }
            return CUnsafeBufferAdapter.withBufferPointer(data) { srcPtr, count in
                self.decompress(src: srcPtr, srcSize: count, dst: base, dstCapacity: originalSize)
            }
        }
        guard actual == originalSize else { return nil }
        return Data(dstBuffer)
    }
}
