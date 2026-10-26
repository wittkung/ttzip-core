// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Canonical typealias mapping UniFFI generated PascalCase to MetastudyLine naming standards.
public typealias UniFFIXxh3Hasher = UniFfiXxh3Hasher

/// High-speed XXH3 SIMD checksum and hash engine facade (64-bit and 128-bit variants).
public struct TTZipXXH3: Sendable {

    /// Computes 64-bit XXH3 hash for an in-memory data buffer with optional seed.
    public static func hash64(_ data: Data, seed: UInt64 = 0) -> UInt64 {
        uniffiXxh364(data: data, seed: seed == 0 ? nil : seed)
    }

    /// Computes 128-bit XXH3 hash as a tuple of `(high: UInt64, low: UInt64)`.
    public static func hash128(_ data: Data, seed: UInt64 = 0) -> (high: UInt64, low: UInt64) {
        let digest = uniffiXxh3128Digest(data: data, seed: seed == 0 ? nil : seed)
        return (high: digest.high, low: digest.low)
    }

    /// Computes 128-bit XXH3 hash returning 16 raw bytes.
    public static func hash128Data(_ data: Data, seed: UInt64 = 0) -> Data {
        uniffiXxh3128(data: data, seed: seed == 0 ? nil : seed)
    }

    /// Computes 128-bit XXH3 hash as a 32-character hexadecimal string.
    public static func hash128Hex(_ data: Data, seed: UInt64 = 0) -> String {
        let (high, low) = hash128(data, seed: seed)
        return String(format: "%016llx%016llx", high, low)
    }

    /// Decodes a 32-character little-endian hex string into (high: UInt64, low: UInt64) components.
    internal static func parse128Hex(_ hex: String) -> (high: UInt64, low: UInt64) {
        let utf8 = hex.utf8
        guard utf8.count == 32 else { return (0, 0) }

        @inline(__always)
        func nibble(_ c: UInt8) -> UInt64 {
            switch c {
            case 48...57: return UInt64(c - 48)
            case 97...102: return UInt64(c - 97 + 10)
            case 65...70: return UInt64(c - 65 + 10)
            default: return 0
            }
        }

        var low: UInt64 = 0
        var high: UInt64 = 0
        var idx = utf8.startIndex
        for i in 0..<8 {
            let b = (nibble(utf8[idx]) << 4) | nibble(utf8[utf8.index(after: idx)])
            low |= (b << (i * 8))
            idx = utf8.index(idx, offsetBy: 2)
        }
        for i in 0..<8 {
            let b = (nibble(utf8[idx]) << 4) | nibble(utf8[utf8.index(after: idx)])
            high |= (b << (i * 8))
            idx = utf8.index(idx, offsetBy: 2)
        }
        return (high: high, low: low)
    }

    /// Streaming accumulator for incremental multi-chunk XXH3 calculation backed by Rust microkernel.
    public final class Accumulator: @unchecked Sendable {
        private let lock = NSLock()
        private var hasher: UniFFIXxh3Hasher

        /// Seed value used for XXH3 calculation.
        public let seed: UInt64

        private var accumulatedData: Data?

        /// Creates a new streaming XXH3 accumulator instance with optional seed.
        public init(seed: UInt64 = 0) {
            self.seed = seed
            self.hasher = UniFFIXxh3Hasher()
            self.accumulatedData = seed == 0 ? nil : Data()
        }

        /// Feeds an incremental chunk of data into the running accumulator across FFI.
        public func update(_ chunk: Data) {
            guard !chunk.isEmpty else { return }
            lock.lock()
            defer { lock.unlock() }
            if seed != 0 {
                accumulatedData?.append(chunk)
            }
            hasher.update(data: chunk)
        }

        /// Finalizes and returns 64-bit XXH3 digest.
        public func digest64() -> UInt64 {
            lock.lock()
            defer { lock.unlock() }
            if seed != 0, let data = accumulatedData {
                return TTZipXXH3.hash64(data, seed: seed)
            }
            return hasher.digest64()
        }

        /// Finalizes and returns 128-bit XXH3 digest as a `(high: UInt64, low: UInt64)` tuple.
        public func digest128() -> (high: UInt64, low: UInt64) {
            lock.lock()
            defer { lock.unlock() }
            if seed != 0, let data = accumulatedData {
                return TTZipXXH3.hash128(data, seed: seed)
            }
            return internalDigest128()
        }

        /// Finalizes and returns 128-bit lowercase hex string.
        public func digest128Hex() -> String {
            lock.lock()
            defer { lock.unlock() }
            if seed != 0, let data = accumulatedData {
                return TTZipXXH3.hash128Hex(data, seed: seed)
            }
            let (high, low) = internalDigest128()
            return String(format: "%016llx%016llx", high, low)
        }

        /// Resets accumulator state for a new calculation cycle.
        public func reset() {
            lock.lock()
            defer { lock.unlock() }
            hasher = UniFFIXxh3Hasher()
            accumulatedData = seed == 0 ? nil : Data()
        }

        @inline(__always)
        private func internalDigest128() -> (high: UInt64, low: UInt64) {
            let hex = hasher.digest128Hex()
            return TTZipXXH3.parse128Hex(hex)
        }
    }
}

