// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation
import CryptoKit

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
        switch algorithm {
        case .crc32:
            var c = uniffiCrc32(data: data).littleEndian
            return Data(bytes: &c, count: 4)

        case .adler32:
            var a = uniffiAdler32(data: data).littleEndian
            return Data(bytes: &a, count: 4)

        case .crc64:
            var c = uniffiCrc64(data: data, seed: nil).littleEndian
            return Data(bytes: &c, count: 8)

        case .xxh3_64:
            var h = uniffiXxh364(data: data, seed: nil).littleEndian
            return Data(bytes: &h, count: 8)

        case .xxh3_128:
            return uniffiXxh3128(data: data, seed: nil)

        case .blake3:
            return uniffiBlake3(data: data)

        case .md5:
            return uniffiMd5(data: data)

        case .sha1:
            return uniffiSha1(data: data)

        case .sha256:
            return uniffiSha256(data: data)
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
        case .xxh3_128:
            let (low, high) = digest.withUnsafeBytes { raw in
                (
                    raw.loadUnaligned(fromByteOffset: 0, as: UInt64.self).littleEndian,
                    raw.loadUnaligned(fromByteOffset: 8, as: UInt64.self).littleEndian
                )
            }
            return String(format: "%016llx%016llx", high, low)
        default:
            return digest.fastHexEncodedString()
        }
    }

    /// Computes hash digest for a local file at given URL with zero-copy chunked streaming.
    /// Delegates to the Rust microkernel's 64KB fixed-buffer streaming implementation (O(1) memory complexity).
    public static func hashFile(at url: URL, algorithm: TTZipHashAlgorithm) throws -> String {
        try computeFileStreamHash(filePath: url.path, algorithm: algorithm.rawValue)
    }

    /// Asynchronously consumes an `AsyncThrowingStream` and returns computed hash string.
    public static func hashStream(
        source: AsyncThrowingStream<Data, Error>,
        algorithm: TTZipHashAlgorithm
    ) async throws -> String {
        try Task.checkCancellation()
        switch algorithm {
        case .crc32:
            var crc: UInt32 = 0
            for try await chunk in source {
                try Task.checkCancellation()
                crc = uniffiCrc32Rolling(initial: crc, data: chunk)
            }
            try Task.checkCancellation()
            return String(format: "%08X", crc)

        case .adler32:
            var adler: UInt32 = 1
            for try await chunk in source {
                try Task.checkCancellation()
                adler = uniffiAdler32Rolling(initial: adler, data: chunk)
            }
            try Task.checkCancellation()
            return String(format: "%08X", adler)

        case .crc64:
            var crc: UInt64 = 0
            for try await chunk in source {
                try Task.checkCancellation()
                crc = uniffiCrc64(data: chunk, seed: crc)
            }
            try Task.checkCancellation()
            return String(format: "%016llx", crc)

        case .xxh3_64:
            let accumulator = TTZipXXH3.Accumulator()
            for try await chunk in source {
                try Task.checkCancellation()
                accumulator.update(chunk)
            }
            try Task.checkCancellation()
            return String(format: "%016llx", accumulator.digest64())

        case .xxh3_128:
            let accumulator = TTZipXXH3.Accumulator()
            for try await chunk in source {
                try Task.checkCancellation()
                accumulator.update(chunk)
            }
            try Task.checkCancellation()
            return accumulator.digest128Hex()

        case .blake3:
            let hasher = TTZipBLAKE3.Hasher()
            for try await chunk in source {
                try Task.checkCancellation()
                hasher.update(chunk)
            }
            try Task.checkCancellation()
            return try hasher.finalizeHex()

        case .sha256:
            var hasher = SHA256()
            for try await chunk in source {
                try Task.checkCancellation()
                hasher.update(data: chunk)
            }
            try Task.checkCancellation()
            let digest = hasher.finalize()
            return Data(digest).fastHexEncodedString()

        case .sha1:
            var hasher = Insecure.SHA1()
            for try await chunk in source {
                try Task.checkCancellation()
                hasher.update(data: chunk)
            }
            try Task.checkCancellation()
            let digest = hasher.finalize()
            return Data(digest).fastHexEncodedString()

        case .md5:
            var hasher = Insecure.MD5()
            for try await chunk in source {
                try Task.checkCancellation()
                hasher.update(data: chunk)
            }
            try Task.checkCancellation()
            let digest = hasher.finalize()
            return Data(digest).fastHexEncodedString()
        }
    }
}

// MARK: - Fast 256-Element Hex Lookup Table Extension

/// Static 256-entry lookup table mapping every byte 0...255 to its two-character lowercase hexadecimal ASCII representation.
@usableFromInline
package let hexLUT: [(UInt8, UInt8)] = {
    let digits = Array("0123456789abcdef".utf8)
    var lut = [(UInt8, UInt8)]()
    lut.reserveCapacity(256)
    for i in 0..<256 {
        lut.append((digits[i >> 4], digits[i & 0x0F]))
    }
    return lut
}()

extension Data {
    /// Fast lowercase hex-encoded string conversion using 256-entry precomputed lookup table (zero intermediate String allocations).
    @inlinable
    package func fastHexEncodedString() -> String {
        let byteCount = self.count
        guard byteCount > 0 else { return "" }
        return String(unsafeUninitializedCapacity: byteCount * 2) { buffer in
            var outIdx = 0
            for byte in self {
                let pair = hexLUT[Int(byte)]
                buffer[outIdx] = pair.0
                buffer[outIdx + 1] = pair.1
                outIdx += 2
            }
            return byteCount * 2
        }
    }
}
