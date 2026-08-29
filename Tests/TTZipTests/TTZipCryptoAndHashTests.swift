// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class TTZipCryptoAndHashTests: XCTestCase {

    private let samplePayload = "TTZip High-Speed Cryptographic and Checksum Verification 2026.".data(using: .utf8)!

    // MARK: - Multi-Algorithm CryptoHash Tests

    func testAllHashAlgorithms() throws {
        for alg in TTZipHashAlgorithm.allCases {
            let raw = TTZipCryptoHash.rawHash(samplePayload, algorithm: alg)
            XCTAssertEqual(raw.count, alg.digestLength, "Digest length mismatch for \(alg.displayName)")

            let hex = TTZipCryptoHash.hash(samplePayload, algorithm: alg)
            XCTAssertFalse(hex.isEmpty, "Hex hash string empty for \(alg.displayName)")
        }
    }

    func testDeterministicChecksumParity() throws {
        let crc1 = TTZipCryptoHash.hash(samplePayload, algorithm: .crc32)
        let crc2 = TTZipCryptoHash.hash(samplePayload, algorithm: .crc32)
        XCTAssertEqual(crc1, crc2)

        let sha256_1 = TTZipCryptoHash.hash(samplePayload, algorithm: .sha256)
        let sha256_2 = TTZipCryptoHash.hash(samplePayload, algorithm: .sha256)
        XCTAssertEqual(sha256_1, sha256_2)
    }

    // MARK: - XXH3 Engine & Accumulator Tests

    func testXXH3DirectAndAccumulator() throws {
        let h64 = TTZipXXH3.hash64(samplePayload)
        XCTAssertNotEqual(h64, 0)

        let (high, low) = TTZipXXH3.hash128(samplePayload)
        XCTAssertTrue(high != 0 || low != 0)

        let hex128 = TTZipXXH3.hash128Hex(samplePayload)
        XCTAssertEqual(hex128.count, 32)

        // Accumulator chunking test
        let half = samplePayload.count / 2
        let p1 = samplePayload.subdata(in: 0..<half)
        let p2 = samplePayload.subdata(in: half..<samplePayload.count)

        let acc = TTZipXXH3.Accumulator()
        acc.update(p1)
        acc.update(p2)

        XCTAssertEqual(acc.digest64(), h64)
        XCTAssertEqual(acc.digest128Hex(), hex128)
    }

    // MARK: - BLAKE3 Plain & Keyed Tests

    func testBLAKE3PlainKeyedAndStreaming() throws {
        let digest = TTZipBLAKE3.hash(samplePayload)
        XCTAssertEqual(digest.count, 32)

        let hex = TTZipBLAKE3.hashHex(samplePayload)
        XCTAssertEqual(hex.count, 64)

        // Keyed MAC
        let key = Data(repeating: 0x42, count: 32)
        let mac = try TTZipBLAKE3.keyedHash(samplePayload, key: key)
        XCTAssertEqual(mac.count, 32)
        XCTAssertNotEqual(mac, digest)

        // Streaming Hasher
        let hasher = TTZipBLAKE3.Hasher(key: key)
        let half = samplePayload.count / 2
        hasher.update(samplePayload.subdata(in: 0..<half))
        hasher.update(samplePayload.subdata(in: half..<samplePayload.count))

        let streamedMac = try hasher.finalize()
        XCTAssertEqual(streamedMac, mac)
    }

    // MARK: - Vault Security Authenticated Encryption Tests

    func testVaultSecurityAllCiphersRoundtrip() throws {
        let password = "TTZipMasterEnterpriseVaultKey2026!"
        let aad = "HeaderMetadataAudit".data(using: .utf8)!

        for cipher in TTZipVaultCipher.allCases {
            let vaultData = try TTZipVaultSecurity.encrypt(
                data: samplePayload,
                password: password,
                cipher: cipher,
                aad: aad
            )
            XCTAssertFalse(vaultData.isEmpty, "Vault container empty for \(cipher.displayName)")
            XCTAssertNotEqual(vaultData, samplePayload)

            let decrypted = try TTZipVaultSecurity.decrypt(
                vaultData: vaultData,
                password: password,
                aad: aad
            )
            XCTAssertEqual(decrypted, samplePayload, "Decryption mismatch for \(cipher.displayName)")
        }
    }

    func testVaultSecurityWrongPasswordRejection() throws {
        let password = "ValidPassword2026!"
        let wrongPassword = "WrongPassword2026!"

        let vaultData = try TTZipVaultSecurity.encrypt(
            data: samplePayload,
            password: password,
            cipher: .aes256Gcm
        )

        XCTAssertThrowsError(
            try TTZipVaultSecurity.decrypt(vaultData: vaultData, password: wrongPassword)
        ) { error in
            XCTAssertTrue(error is TTZipVaultError)
        }
    }

    func testVaultSecurityDirectZeroizedRawKeys() throws {
        let keyBytes = [UInt8](repeating: 0x77, count: 32)
        let key = SecureBytes(bytes: keyBytes)
        let nonce = Data(repeating: 0x12, count: 12)
        let aad = "CustomAAD".data(using: .utf8)!

        // AES-GCM Raw
        let (cipherGcm, tagGcm) = try TTZipVaultSecurity.encryptRaw(
            plaintext: samplePayload,
            key: key,
            nonce: nonce,
            cipher: .aes256Gcm,
            aad: aad
        )
        let plainGcm = try TTZipVaultSecurity.decryptRaw(
            ciphertext: cipherGcm,
            key: key,
            nonce: nonce,
            cipher: .aes256Gcm,
            tag: tagGcm,
            aad: aad
        )
        XCTAssertEqual(plainGcm, samplePayload)

        // ChaCha20-Poly1305 Raw
        let (cipherChaCha, tagChaCha) = try TTZipVaultSecurity.encryptRaw(
            plaintext: samplePayload,
            key: key,
            nonce: nonce,
            cipher: .chacha20Poly1305,
            aad: aad
        )
        let plainChaCha = try TTZipVaultSecurity.decryptRaw(
            ciphertext: cipherChaCha,
            key: key,
            nonce: nonce,
            cipher: .chacha20Poly1305,
            tag: tagChaCha,
            aad: aad
        )
        XCTAssertEqual(plainChaCha, samplePayload)
    }
}
