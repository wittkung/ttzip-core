// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

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

    /// Streaming accumulator for incremental multi-chunk XXH3 calculation.
    public final class Accumulator: @unchecked Sendable {
        private let lock = NSLock()
        private var buffer = Data()
        private let seed: UInt64

        public init(seed: UInt64 = 0) {
            self.seed = seed
        }

        /// Feeds an incremental chunk of data into the running accumulator.
        public func update(_ chunk: Data) {
            lock.lock()
            defer { lock.unlock() }
            buffer.append(chunk)
        }

        /// Finalizes and returns 64-bit XXH3 digest.
        public func digest64() -> UInt64 {
            lock.lock()
            defer { lock.unlock() }
            return TTZipXXH3.hash64(buffer, seed: seed)
        }

        /// Finalizes and returns 128-bit XXH3 digest.
        public func digest128() -> (high: UInt64, low: UInt64) {
            lock.lock()
            defer { lock.unlock() }
            return TTZipXXH3.hash128(buffer, seed: seed)
        }

        /// Finalizes and returns 128-bit hex string.
        public func digest128Hex() -> String {
            lock.lock()
            defer { lock.unlock() }
            return TTZipXXH3.hash128Hex(buffer, seed: seed)
        }

        /// Resets accumulator state for new calculation cycle.
        public func reset() {
            lock.lock()
            defer { lock.unlock() }
            buffer.removeAll(keepingCapacity: true)
        }
    }
}
