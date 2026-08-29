// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import CTTZipBridge

/// High-performance Swift 6 facade for the BLAKE3 tree-hashing cryptographic engine.
public struct TTZipBLAKE3: Sendable {

    /// Computes 32-byte (256-bit) standard BLAKE3 hash for an in-memory byte buffer.
    public static func hash(_ data: Data) -> Data {
        var out = Data(count: 32)
        let status = out.withUnsafeMutableBytes { outBuf -> Int32 in
            guard let outPtr = outBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
            return data.withUnsafeBytes { inBuf in
                guard let inPtr = inBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
                return ttzip_rust_blake3(inPtr, data.count, outPtr)
            }
        }
        guard status == 0 else { return Data(repeating: 0, count: 32) }
        return out
    }

    /// Computes 32-byte BLAKE3 hash returning a 64-character lowercase hex string.
    public static func hashHex(_ data: Data) -> String {
        let digest = hash(data)
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    /// Computes 32-byte keyed MAC using a 32-byte cryptographic key.
    public static func keyedHash(_ data: Data, key: Data) throws -> Data {
        guard key.count == 32 else {
            throw TTZipCodecError.invalidParameter
        }

        var out = Data(count: 32)
        let status = out.withUnsafeMutableBytes { outBuf -> Int32 in
            guard let outPtr = outBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
            return key.withUnsafeBytes { keyBuf in
                guard let keyPtr = keyBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
                return data.withUnsafeBytes { dataBuf in
                    guard let dataPtr = dataBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
                    return ttzip_rust_blake3_keyed(keyPtr, dataPtr, data.count, outPtr)
                }
            }
        }

        guard status == 0 else {
            throw TTZipCodecError.compressionFailed(status: status)
        }

        return out
    }

    /// Computes keyed MAC returning a lowercase hex string.
    public static func keyedHashHex(_ data: Data, key: Data) throws -> String {
        let digest = try keyedHash(data, key: key)
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    /// Incremental streaming BLAKE3 hasher.
    public final class Hasher: @unchecked Sendable {
        private let lock = NSLock()
        private var buffer = Data()
        private let key: Data?

        public init(key: Data? = nil) {
            self.key = key
        }

        /// Appends incremental data chunk into the running hasher.
        public func update(_ chunk: Data) {
            lock.lock()
            defer { lock.unlock() }
            buffer.append(chunk)
        }

        /// Finalizes and outputs 32-byte BLAKE3 digest.
        public func finalize() throws -> Data {
            lock.lock()
            defer { lock.unlock() }
            if let key = key {
                return try TTZipBLAKE3.keyedHash(buffer, key: key)
            } else {
                return TTZipBLAKE3.hash(buffer)
            }
        }

        /// Finalizes and outputs lowercase hex string.
        public func finalizeHex() throws -> String {
            let digest = try finalize()
            return digest.map { String(format: "%02x", $0) }.joined()
        }

        /// Resets hasher state.
        public func reset() {
            lock.lock()
            defer { lock.unlock() }
            buffer.removeAll(keepingCapacity: true)
        }
    }
}
