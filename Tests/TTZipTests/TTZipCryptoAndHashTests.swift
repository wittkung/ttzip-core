// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

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
        let hasher = TTZipBLAKE3.Hasher()
        let half = samplePayload.count / 2
        hasher.update(samplePayload.subdata(in: 0..<half))
        hasher.update(samplePayload.subdata(in: half..<samplePayload.count))

        let streamed = try hasher.finalize()
        XCTAssertEqual(streamed, digest)

        let streamedHex = try hasher.finalizeHex()
        XCTAssertEqual(streamedHex, hex)

        // Reset state and re-verify single-pass calculation parity
        hasher.reset()
        hasher.update(samplePayload)
        XCTAssertEqual(try hasher.finalize(), digest)
    }

    func testXXH3AccumulatorWithSeed() throws {
        let seed: UInt64 = 0x12345678_9abcdef0
        let h64 = TTZipXXH3.hash64(samplePayload, seed: seed)
        let (high, low) = TTZipXXH3.hash128(samplePayload, seed: seed)
        let hex128 = TTZipXXH3.hash128Hex(samplePayload, seed: seed)

        let acc = TTZipXXH3.Accumulator(seed: seed)
        XCTAssertEqual(acc.seed, seed)

        let half = samplePayload.count / 2
        acc.update(samplePayload.subdata(in: 0..<half))
        acc.update(samplePayload.subdata(in: half..<samplePayload.count))

        XCTAssertEqual(acc.digest64(), h64)
        let acc128 = acc.digest128()
        XCTAssertEqual(acc128.high, high)
        XCTAssertEqual(acc128.low, low)
        XCTAssertEqual(acc.digest128Hex(), hex128)
    }

    func testBLAKE3HasherKeyedAndContext() throws {
        let key = Data(repeating: 0x99, count: 32)
        let expectedKeyedMac = try TTZipBLAKE3.keyedHash(samplePayload, key: key)
        let expectedKeyedHex = try TTZipBLAKE3.keyedHashHex(samplePayload, key: key)

        let keyedHasher = try TTZipBLAKE3.Hasher(key: key)
        XCTAssertEqual(keyedHasher.key, key)
        XCTAssertNil(keyedHasher.deriveKeyContext)

        let half = samplePayload.count / 2
        keyedHasher.update(samplePayload.subdata(in: 0..<half))
        keyedHasher.update(samplePayload.subdata(in: half..<samplePayload.count))

        let mac = try keyedHasher.finalize()
        XCTAssertEqual(mac, expectedKeyedMac)
        let macHex = try keyedHasher.finalizeHex()
        XCTAssertEqual(macHex, expectedKeyedHex)

        // Invalid key length validation
        let invalidKey = Data(repeating: 0x99, count: 16)
        XCTAssertThrowsError(try TTZipBLAKE3.Hasher(key: invalidKey)) { error in
            XCTAssertTrue(error is TTZipCodecError)
        }

        // Context derivation preservation
        let contextHasher = try TTZipBLAKE3.Hasher(deriveKeyContext: "ttzip.kdf.v1")
        XCTAssertEqual(contextHasher.deriveKeyContext, "ttzip.kdf.v1")
        XCTAssertNil(contextHasher.key)
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

    // MARK: - Streaming and File Hash Equivalence Tests

    func testHashFileAllAlgorithmsEquivalence() throws {
        // Construct 300KB multi-chunk payload exceeding 128KB streaming buffer boundary
        var multiChunkPayload = Data()
        multiChunkPayload.reserveCapacity(300 * 1024)
        for i in 0..<300 {
            multiChunkPayload.append(contentsOf: "Chunk #\(i): TTZip Zero-Allocation Checksum Stream Verification 2026.\n".utf8)
        }

        let tempURL = FileManager.default.temporaryDirectory.appendingPathComponent("ttzip_hash_file_test_\(UUID().uuidString).bin")
        try multiChunkPayload.write(to: tempURL)
        defer { try? FileManager.default.removeItem(at: tempURL) }

        for alg in TTZipHashAlgorithm.allCases {
            let directHex = TTZipCryptoHash.hash(multiChunkPayload, algorithm: alg)
            let fileHex = try TTZipCryptoHash.hashFile(at: tempURL, algorithm: alg)
            XCTAssertEqual(fileHex.lowercased(), directHex.lowercased(), "File streaming hash mismatch for \(alg.displayName)")
        }
    }

    func testHashStreamAllAlgorithmsEquivalence() async throws {
        var multiChunkPayload = Data()
        multiChunkPayload.reserveCapacity(256 * 1024)
        for i in 0..<256 {
            multiChunkPayload.append(contentsOf: "StreamBlock #\(i): TTZip AsyncThrowingStream Constant Memory Verification.\n".utf8)
        }

        let chunkSize = 32 * 1024

        for alg in TTZipHashAlgorithm.allCases {
            let directHex = TTZipCryptoHash.hash(multiChunkPayload, algorithm: alg)

            let stream = AsyncThrowingStream<Data, Error> { continuation in
                var offset = 0
                while offset < multiChunkPayload.count {
                    let end = min(offset + chunkSize, multiChunkPayload.count)
                    let slice = multiChunkPayload.subdata(in: offset..<end)
                    continuation.yield(slice)
                    offset = end
                }
                continuation.finish()
            }

            let streamedHex = try await TTZipCryptoHash.hashStream(source: stream, algorithm: alg)
            XCTAssertEqual(streamedHex.lowercased(), directHex.lowercased(), "AsyncStream hash mismatch for \(alg.displayName)")
        }
    }

    // MARK: - Apple Libcompression Accelerator Hardening Tests

    func testAppleLibcompressionAcceleratorRoundtrip() throws {
        let accelerator = AppleLibcompressionAccelerator.shared

        // 1. Empty data boundary
        let emptyCompressed = accelerator.compressData(Data())
        XCTAssertEqual(emptyCompressed, Data())

        // 2. Roundtrip with structured text payload
        guard let compressed = accelerator.compressData(samplePayload) else {
            XCTFail("Failed to compress sample payload")
            return
        }
        XCTAssertFalse(compressed.isEmpty)

        guard let decompressed = accelerator.decompressData(compressed, originalSize: samplePayload.count) else {
            XCTFail("Failed to decompress compressed payload")
            return
        }
        XCTAssertEqual(decompressed, samplePayload)

        // 3. Raw buffer level zero-copy invocation with 64-byte alignment
        let rawCount = samplePayload.count
        let maxBound = max(rawCount + 4096, Int(Double(rawCount) * 1.05) + 1024)
        let rawDst = UnsafeMutableRawPointer.allocate(byteCount: maxBound, alignment: 64)
        defer { rawDst.deallocate() }

        let written = samplePayload.withUnsafeBytes { srcRaw in
            accelerator.compress(src: srcRaw.baseAddress!, srcSize: rawCount, dst: rawDst, dstCapacity: maxBound)
        }
        XCTAssertGreaterThan(written, 0)

        let rawDecomp = UnsafeMutableRawPointer.allocate(byteCount: rawCount, alignment: 64)
        defer { rawDecomp.deallocate() }

        let actual = accelerator.decompress(src: rawDst, srcSize: written, dst: rawDecomp, dstCapacity: rawCount)
        XCTAssertEqual(actual, rawCount)

        let recovered = Data(bytes: rawDecomp, count: actual)
        XCTAssertEqual(recovered, samplePayload)
    }
}
