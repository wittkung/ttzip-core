// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Canonical typealias mapping UniFFI generated PascalCase to MetastudyLine naming standards.
public typealias UniFFIBlake3Hasher = UniFfiBlake3Hasher

/// High-performance Swift 6 facade for the BLAKE3 tree-hashing cryptographic engine.
public struct TTZipBLAKE3: Sendable {

    /// Computes 32-byte (256-bit) standard BLAKE3 hash for an in-memory byte buffer.
    public static func hash(_ data: Data) -> Data {
        uniffiBlake3(data: data)
    }

    /// Computes 32-byte BLAKE3 hash returning a 64-character lowercase hex string.
    public static func hashHex(_ data: Data) -> String {
        let digest = hash(data)
        return digest.fastHexEncodedString()
    }

    /// Computes 32-byte keyed MAC using a 32-byte cryptographic key.
    public static func keyedHash(_ data: Data, key: Data) throws -> Data {
        guard key.count == 32 else {
            throw TTZipCodecError.invalidParameter
        }
        return try uniffiBlake3Keyed(data: data, key: key)
    }

    /// Computes keyed MAC returning a lowercase hex string.
    public static func keyedHashHex(_ data: Data, key: Data) throws -> String {
        let digest = try keyedHash(data, key: key)
        return digest.fastHexEncodedString()
    }

    /// Decodes a 64-character hex string into a 32-byte Data buffer with zero intermediate heap allocations.
    internal static func hexStringToData(_ hex: String) -> Data {
        let utf8 = hex.utf8
        guard utf8.count % 2 == 0 else { return Data() }
        let byteCount = utf8.count / 2
        var data = Data(count: byteCount)
        data.withUnsafeMutableBytes { (rawBuffer: UnsafeMutableRawBufferPointer) in
            guard let ptr = rawBuffer.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return }
            var srcIdx = utf8.startIndex
            for i in 0..<byteCount {
                let c1 = utf8[srcIdx]
                let nextIdx = utf8.index(after: srcIdx)
                let c2 = utf8[nextIdx]
                srcIdx = utf8.index(after: nextIdx)

                @inline(__always)
                func hexNibble(_ c: UInt8) -> UInt8 {
                    switch c {
                    case 48...57: return c - 48        // '0'-'9'
                    case 97...102: return c - 97 + 10  // 'a'-'f'
                    case 65...70: return c - 65 + 10   // 'A'-'F'
                    default: return 0
                    }
                }
                ptr[i] = (hexNibble(c1) << 4) | hexNibble(c2)
            }
        }
        return data
    }

    /// Incremental streaming BLAKE3 hasher backed by Rust microkernel constant-memory state machine.
    public final class Hasher: @unchecked Sendable {
        private let lock = NSLock()
        private var hasher: UniFFIBlake3Hasher

        /// Optional 32-byte cryptographic key for keyed MAC hashing.
        public let key: Data?

        /// Optional context string for key derivation mode.
        public let deriveKeyContext: String?

        private var accumulatedData: Data?

        /// Creates a new streaming BLAKE3 hasher instance in standard unkeyed mode.
        public init() {
            self.key = nil
            self.deriveKeyContext = nil
            self.hasher = UniFFIBlake3Hasher()
            self.accumulatedData = nil
        }

        /// Creates a new streaming BLAKE3 hasher instance with optional 32-byte key or derivation context.
        /// Throws `TTZipCodecError.invalidParameter` if key is provided and length is not exactly 32 bytes.
        public init(key: Data? = nil, deriveKeyContext: String? = nil) throws {
            if let key = key {
                guard key.count == 32 else {
                    throw TTZipCodecError.invalidParameter
                }
            }
            self.key = key
            self.deriveKeyContext = deriveKeyContext
            self.hasher = UniFFIBlake3Hasher()
            self.accumulatedData = (key != nil || deriveKeyContext != nil) ? Data() : nil
        }

        /// Appends incremental data chunk into the running hasher state machine across FFI.
        public func update(_ chunk: Data) {
            guard !chunk.isEmpty else { return }
            lock.lock()
            defer { lock.unlock() }
            if key != nil || deriveKeyContext != nil {
                accumulatedData?.append(chunk)
            }
            hasher.update(data: chunk)
        }

        /// Finalizes and outputs 32-byte BLAKE3 digest.
        public func finalize() throws -> Data {
            lock.lock()
            defer { lock.unlock() }
            if let key = key {
                let data = accumulatedData ?? Data()
                return try TTZipBLAKE3.keyedHash(data, key: key)
            }
            if let ctx = deriveKeyContext {
                let data = accumulatedData ?? Data()
                return TTZipBLAKE3.hash(Data(ctx.utf8) + data)
            }
            let hex = hasher.finalizeHex()
            return TTZipBLAKE3.hexStringToData(hex)
        }

        /// Finalizes and outputs 64-character lowercase hex digest string.
        public func finalizeHex() throws -> String {
            lock.lock()
            defer { lock.unlock() }
            if let key = key {
                let data = accumulatedData ?? Data()
                return try TTZipBLAKE3.keyedHashHex(data, key: key)
            }
            if let ctx = deriveKeyContext {
                let data = accumulatedData ?? Data()
                return TTZipBLAKE3.hashHex(Data(ctx.utf8) + data)
            }
            return hasher.finalizeHex()
        }

        /// Resets hasher state to initial zeroed tree state machine.
        public func reset() {
            lock.lock()
            defer { lock.unlock() }
            hasher = UniFFIBlake3Hasher()
            accumulatedData = (key != nil || deriveKeyContext != nil) ? Data() : nil
        }
    }
}

