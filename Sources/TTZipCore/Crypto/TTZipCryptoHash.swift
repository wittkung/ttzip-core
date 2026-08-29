// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import CTTZipBridge

/// Supported cryptographic and non-cryptographic checksum/hash algorithms.
public enum TTZipHashAlgorithm: String, Sendable, CaseIterable {
    case crc32
    case crc64
    case adler32
    case md5
    case sha1
    case sha256
    case xxh3_64
    case xxh3_128
    case blake3

    /// Display name of the hash algorithm.
    public var displayName: String {
        switch self {
        case .crc32: return "CRC-32 (Castagnoli/IEEE)"
        case .crc64: return "CRC-64 (ECMA-182)"
        case .adler32: return "Adler-32"
        case .md5: return "MD5"
        case .sha1: return "SHA-1"
        case .sha256: return "SHA-256"
        case .xxh3_64: return "XXH3 (64-bit)"
        case .xxh3_128: return "XXH3 (128-bit)"
        case .blake3: return "BLAKE3 (256-bit)"
        }
    }

    /// Output byte length of the digest.
    public var digestLength: Int {
        switch self {
        case .crc32, .adler32: return 4
        case .crc64, .xxh3_64: return 8
        case .md5, .xxh3_128: return 16
        case .sha1: return 20
        case .sha256, .blake3: return 32
        }
    }
}

/// Swift 6 unified cryptographic and high-speed SIMD hash facade.
public struct TTZipCryptoHash: Sendable {

    /// Computes raw binary digest for data buffer.
    public static func rawHash(_ data: Data, algorithm: TTZipHashAlgorithm) -> Data {
        data.withUnsafeBytes { buf in
            let ptr = buf.baseAddress?.assumingMemoryBound(to: UInt8.self)
            let len = data.count

            switch algorithm {
            case .crc32:
                var c = ttzip_rust_crc32(0, ptr, len).littleEndian
                return Data(bytes: &c, count: 4)

            case .adler32:
                var a = ttzip_rust_adler32(1, ptr, len).littleEndian
                return Data(bytes: &a, count: 4)

            case .crc64:
                var c = ttzip_rust_crc64(0, ptr, len).littleEndian
                return Data(bytes: &c, count: 8)

            case .xxh3_64:
                var h = ttzip_rust_xxh3_64(ptr, len, 0).littleEndian
                return Data(bytes: &h, count: 8)

            case .xxh3_128:
                var out = Data(count: 16)
                out.withUnsafeMutableBytes { outBuf in
                    _ = ttzip_rust_xxh3_128(ptr, len, 0, outBuf.baseAddress?.assumingMemoryBound(to: UInt8.self))
                }
                return out

            case .blake3:
                var out = Data(count: 32)
                out.withUnsafeMutableBytes { outBuf in
                    _ = ttzip_rust_blake3(ptr, len, outBuf.baseAddress?.assumingMemoryBound(to: UInt8.self))
                }
                return out

            case .md5:
                var out = Data(count: 16)
                out.withUnsafeMutableBytes { outBuf in
                    _ = ttzip_rust_md5(ptr, len, outBuf.baseAddress?.assumingMemoryBound(to: UInt8.self))
                }
                return out

            case .sha1:
                var out = Data(count: 20)
                out.withUnsafeMutableBytes { outBuf in
                    _ = ttzip_rust_sha1(ptr, len, outBuf.baseAddress?.assumingMemoryBound(to: UInt8.self))
                }
                return out

            case .sha256:
                var out = Data(count: 32)
                out.withUnsafeMutableBytes { outBuf in
                    _ = ttzip_rust_sha256(ptr, len, outBuf.baseAddress?.assumingMemoryBound(to: UInt8.self))
                }
                return out
            }
        }
    }

    /// Computes lowercase hex-formatted digest string for data buffer.
    public static func hash(_ data: Data, algorithm: TTZipHashAlgorithm) -> String {
        let digest = rawHash(data, algorithm: algorithm)
        switch algorithm {
        case .crc32, .adler32:
            let val = digest.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 0, as: UInt32.self).littleEndian }
            return String(format: "%08X", val)
        case .crc64, .xxh3_64:
            let val = digest.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 0, as: UInt64.self).littleEndian }
            return String(format: "%016llx", val)
        default:
            return digest.map { String(format: "%02x", $0) }.joined()
        }
    }

    /// Computes hash digest for a local file at given URL with buffered streaming.
    public static func hashFile(at url: URL, algorithm: TTZipHashAlgorithm) throws -> String {
        let fileHandle = try FileHandle(forReadingFrom: url)
        defer { try? fileHandle.close() }

        let bufferSize = 1024 * 1024 // 1MB buffer
        var accumulatorData = Data()

        while let chunk = try fileHandle.read(upToCount: bufferSize), !chunk.isEmpty {
            accumulatorData.append(chunk)
        }

        return hash(accumulatorData, algorithm: algorithm)
    }

    /// Asynchronously consumes an `AsyncThrowingStream` and returns computed hash string.
    public static func hashStream(
        source: AsyncThrowingStream<Data, Error>,
        algorithm: TTZipHashAlgorithm
    ) async throws -> String {
        var completeData = Data()
        for try await chunk in source {
            if Task.isCancelled { break }
            completeData.append(chunk)
        }
        return hash(completeData, algorithm: algorithm)
    }
}
