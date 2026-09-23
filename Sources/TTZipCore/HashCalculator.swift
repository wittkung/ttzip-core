// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation
import Compression

/// Multi-core parallel chunked hash and checksum calculator.
public final class HashCalculator: HashCalculating, @unchecked Sendable {
    public init(hardwareTuner: HardwareTunerProtocol? = nil) {}
    
    public func computeHashSync(filePath: String, type: HashType) throws -> String {
        switch type {
        case .crc32:
            let crc = try computeFileCrc32(filePath: filePath)
            return String(format: "%08X", crc)
            
        case .sha256:
            return try Self.computeFileSha256(filePath: filePath)
            
        case .md5:
            return try computeFileHash(path: filePath, algorithm: "md5")
            
        case .sha1:
            return try computeFileHash(path: filePath, algorithm: "sha1")
        }
    }
    
    public func computeHash(filePath: String, type: HashType) async throws -> String {
        return try await Task.detached(priority: .userInitiated) {
            try self.computeHashSync(filePath: filePath, type: type)
        }.value
    }
    
    /// Static convenience method for calculating SHA-256 fingerprint of a file via microkernel.
    @inlinable
    public static func computeFileSha256(filePath: String) throws -> String {
        #if SWIFT_PACKAGE
        try TTZipCore.computeFileSha256(filePath: filePath)
        #else
        try TTZipCoreBindings.computeFileSha256(filePath: filePath)
        #endif
    }
    
    /// Static convenience method for calculating SHA-256 fingerprint of a file.
    public static func calculateSHA256(filePath: String) -> String? {
        return try? computeFileSha256(filePath: filePath)
    }
    
    /// Static convenience method for calculating SHA-256 fingerprint of raw in-memory data.
    public static func calculateSHA256(data: Data) -> String? {
        return computeBytesSha256(data: data)
    }
}

// MARK: - Hardware Checksum Adapter

/// Hardware-accelerated Adler-32 and CRC-32 checksum computation adapter.
public enum HardwareChecksumAdapter {
    
    /// Computes 32-bit Adler-32 checksum with hardware acceleration.
    @inlinable
    public static func adler32(for data: Data, initial: UInt32 = 1) -> UInt32 {
        guard !data.isEmpty else { return initial }
        if initial == 1 {
            return uniffiAdler32(data: data)
        } else {
            return uniffiAdler32Rolling(initial: initial, data: data)
        }
    }
    
    /// Computes 32-bit Adler-32 checksum via direct pointer access.
    @inlinable
    public static func adler32(ptr: UnsafePointer<UInt8>, count: Int, initial: UInt32 = 1) -> UInt32 {
        guard count > 0 else { return initial }
        let data = Data(bytesNoCopy: UnsafeMutableRawPointer(mutating: ptr), count: count, deallocator: .none)
        if initial == 1 {
            return uniffiAdler32(data: data)
        } else {
            return uniffiAdler32Rolling(initial: initial, data: data)
        }
    }

    /// Computes 32-bit CRC-32 checksum.
    @inlinable
    public static func crc32(for data: Data, initial: UInt32 = 0) -> UInt32 {
        guard !data.isEmpty else { return initial }
        if initial == 0 {
            return uniffiCrc32(data: data)
        } else {
            return uniffiCrc32Rolling(initial: initial, data: data)
        }
    }

    /// Computes 32-bit CRC-32 checksum via direct pointer access.
    @inlinable
    public static func crc32(ptr: UnsafePointer<UInt8>, count: Int, initial: UInt32 = 0) -> UInt32 {
        guard count > 0 else { return initial }
        let data = Data(bytesNoCopy: UnsafeMutableRawPointer(mutating: ptr), count: count, deallocator: .none)
        if initial == 0 {
            return uniffiCrc32(data: data)
        } else {
            return uniffiCrc32Rolling(initial: initial, data: data)
        }
    }

    @inlinable
    public static func computeCRC32(data: Data) -> UInt32 {
        return crc32(for: data)
    }

    @inlinable
    public static func combineCRC32(crc1: UInt32, crc2: UInt32, len2: Int) -> UInt32 {
        guard len2 > 0 else { return crc1 }
        return uniffiCrc32Combine(crc1: crc1, crc2: crc2, len2: UInt64(len2))
    }
}

// MARK: - Apple Libcompression Accelerator

/// High-performance DEFLATE compression and decompression acceleration infrastructure using Apple's libcompression.
public final class AppleLibcompressionAccelerator: @unchecked Sendable {
    public static let shared = AppleLibcompressionAccelerator()
    
    private init() {}
    
    /// Compresses buffer via Apple Silicon Hardware Compression framework.
    public func compress(
        src: UnsafeRawPointer,
        srcSize: Int,
        dst: UnsafeMutableRawPointer,
        dstCapacity: Int
    ) -> Int {
        let dstPtr = dst.bindMemory(to: UInt8.self, capacity: dstCapacity)
        let srcPtr = src.bindMemory(to: UInt8.self, capacity: srcSize)
        let written = compression_encode_buffer(
            dstPtr,
            dstCapacity,
            srcPtr,
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
        let dstPtr = dst.bindMemory(to: UInt8.self, capacity: dstCapacity)
        let srcPtr = src.bindMemory(to: UInt8.self, capacity: srcSize)
        let written = compression_decode_buffer(
            dstPtr,
            dstCapacity,
            srcPtr,
            srcSize,
            nil,
            COMPRESSION_ZLIB
        )
        return written
    }
    
    /// Convenience helper compressing Swift `Data` buffers without zero-filling or double buffering.
    public func compressData(_ data: Data, level: Int = 6) -> Data? {
        guard !data.isEmpty else { return Data() }
        let maxBound = max(data.count + 4096, Int(Double(data.count) * 1.05) + 1024)
        let rawPtr = UnsafeMutableRawPointer.allocate(byteCount: maxBound, alignment: 64)
        defer { rawPtr.deallocate() }
        let written = CUnsafeBufferAdapter.withBufferPointer(data) { srcPtr, count in
            self.compress(src: srcPtr, srcSize: count, dst: rawPtr, dstCapacity: maxBound)
        }
        guard written > 0 else {
            return nil
        }
        return Data(bytes: rawPtr, count: written)
    }
    
    /// Convenience helper decompressing Swift `Data` buffers without zero-filling or double buffering.
    public func decompressData(_ data: Data, originalSize: Int) -> Data? {
        guard !data.isEmpty else { return Data() }
        let rawPtr = UnsafeMutableRawPointer.allocate(byteCount: originalSize, alignment: 64)
        let actual = CUnsafeBufferAdapter.withBufferPointer(data) { srcPtr, count in
            self.decompress(src: srcPtr, srcSize: count, dst: rawPtr, dstCapacity: originalSize)
        }
        guard actual == originalSize else {
            rawPtr.deallocate()
            return nil
        }
        return Data(bytesNoCopy: rawPtr, count: actual, deallocator: .custom { ptr, _ in ptr.deallocate() })
    }
}
