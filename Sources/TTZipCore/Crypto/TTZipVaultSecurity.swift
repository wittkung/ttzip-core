// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import CommonCrypto
import CTTZipBridge

/// Supported ciphers for vault and archive payload encryption.
public enum TTZipVaultCipher: UInt8, Sendable, CaseIterable {
    case aes256Gcm = 1
    case chacha20Poly1305 = 2
    case aes256Ctr = 3
    case aes256Cbc = 4

    /// Display name of the cipher.
    public var displayName: String {
        switch self {
        case .aes256Gcm: return "AES-256-GCM (Authenticated)"
        case .chacha20Poly1305: return "ChaCha20-Poly1305 AEAD"
        case .aes256Ctr: return "AES-256-CTR"
        case .aes256Cbc: return "AES-256-CBC"
        }
    }

    /// Required Nonce/IV byte size.
    public var nonceLength: Int {
        switch self {
        case .aes256Gcm: return 12
        case .chacha20Poly1305: return 12
        case .aes256Ctr: return 8
        case .aes256Cbc: return 16
        }
    }

    /// Authentication Tag byte size (if AEAD).
    public var tagLength: Int {
        switch self {
        case .aes256Gcm, .chacha20Poly1305: return 16
        case .aes256Ctr, .aes256Cbc: return 0
        }
    }
}

/// Magic identifier for TTZip Encrypted Vault Record (`TTZV`).
private let VAULT_MAGIC: UInt32 = 0x54545A56

/// Errors thrown by the Vault Security facade.
public enum TTZipVaultError: Error, Sendable, LocalizedError {
    case invalidParameter
    case keyDerivationFailed
    case encryptionFailed(status: Int32)
    case decryptionFailed(status: Int32)
    case authenticationFailed
    case corruptVaultFormat

    public var errorDescription: String? {
        switch self {
        case .invalidParameter:
            return "Invalid cryptographic parameters provided."
        case .keyDerivationFailed:
            return "PBKDF2 key derivation failed."
        case .encryptionFailed(let st):
            return "Encryption failed with status code \(st)."
        case .decryptionFailed(let st):
            return "Decryption failed with status code \(st)."
        case .authenticationFailed:
            return "Authentication tag verification failed. Invalid password or corrupted payload."
        case .corruptVaultFormat:
            return "Corrupted vault container header."
        }
    }
}

/// Swift 6 Authenticated Encryption and Hardware-Accelerated Vault Security Facade.
public struct TTZipVaultSecurity: Sendable {

    /// Derives a 32-byte (256-bit) encryption key from password and salt using PBKDF2-HMAC-SHA256.
    public static func deriveKey(password: String, salt: Data, rounds: UInt32 = 100_000) throws -> SecureBytes {
        guard !password.isEmpty, salt.count >= 16 else {
            throw TTZipVaultError.invalidParameter
        }

        let secureKey = SecureBytes(capacity: 32)
        guard let keyPtr = secureKey.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
            throw TTZipVaultError.keyDerivationFailed
        }

        let status = salt.withUnsafeBytes { saltBuf -> Int32 in
            guard let saltPtr = saltBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
            return CCKeyDerivationPBKDF(
                CCPBKDFAlgorithm(kCCPBKDF2),
                password,
                password.utf8.count,
                saltPtr,
                salt.count,
                CCPseudoRandomAlgorithm(kCCPRFHmacAlgSHA256),
                rounds,
                keyPtr,
                32
            )
        }

        guard status == kCCSuccess else {
            throw TTZipVaultError.keyDerivationFailed
        }

        return secureKey
    }

    /// Encrypts an in-memory plaintext buffer with password into a self-contained authenticated vault container.
    public static func encrypt(
        data: Data,
        password: String,
        cipher: TTZipVaultCipher = .aes256Gcm,
        aad: Data? = nil
    ) throws -> Data {
        if data.isEmpty {
            return Data()
        }

        // Generate 16-byte random Salt and Nonce
        var salt = Data(count: 16)
        var nonce = Data(count: cipher.nonceLength)
        _ = salt.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, 16, $0.baseAddress!) }
        _ = nonce.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, cipher.nonceLength, $0.baseAddress!) }

        let key = try deriveKey(password: password, salt: salt)

        var payloadToEncrypt = data
        if cipher == .aes256Cbc {
            let padLen = 16 - (data.count % 16)
            payloadToEncrypt.append(contentsOf: repeatElement(UInt8(padLen), count: padLen))
        }

        let (ciphertext, tag) = try encryptRaw(
            plaintext: payloadToEncrypt,
            key: key,
            nonce: nonce,
            cipher: cipher,
            aad: aad
        )

        // Assemble Vault Container Format:
        // Magic (4B), Cipher (1B), Reserved (3B), Salt (16B), Nonce (Len), Tag (Len), Ciphertext
        var vault = Data()
        var magic = VAULT_MAGIC.littleEndian
        var cipherByte = cipher.rawValue
        var reserved = UInt32(0)

        vault.append(Data(bytes: &magic, count: 4))
        vault.append(Data(bytes: &cipherByte, count: 1))
        vault.append(Data(bytes: &reserved, count: 3))
        vault.append(salt)
        vault.append(nonce)
        if let tagData = tag {
            vault.append(tagData)
        }
        vault.append(ciphertext)

        return vault
    }

    /// Decrypts a self-contained authenticated vault container with password.
    public static func decrypt(
        vaultData: Data,
        password: String,
        aad: Data? = nil
    ) throws -> Data {
        if vaultData.isEmpty {
            return Data()
        }

        guard vaultData.count >= 24 else {
            throw TTZipVaultError.corruptVaultFormat
        }

        let magic = vaultData.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 0, as: UInt32.self).littleEndian }
        guard magic == VAULT_MAGIC else {
            throw TTZipVaultError.corruptVaultFormat
        }

        let cipherRaw = vaultData[4]
        guard let cipher = TTZipVaultCipher(rawValue: cipherRaw) else {
            throw TTZipVaultError.corruptVaultFormat
        }

        var offset = 8
        let salt = vaultData.subdata(in: offset..<offset + 16)
        offset += 16

        let nonce = vaultData.subdata(in: offset..<offset + cipher.nonceLength)
        offset += cipher.nonceLength

        var tag: Data? = nil
        if cipher.tagLength > 0 {
            tag = vaultData.subdata(in: offset..<offset + cipher.tagLength)
            offset += cipher.tagLength
        }

        let ciphertext = vaultData.subdata(in: offset..<vaultData.count)

        let key = try deriveKey(password: password, salt: salt)

        let decrypted = try decryptRaw(
            ciphertext: ciphertext,
            key: key,
            nonce: nonce,
            cipher: cipher,
            tag: tag,
            aad: aad
        )

        if cipher == .aes256Cbc {
            guard let padLen = decrypted.last, padLen >= 1, padLen <= 16, decrypted.count >= Int(padLen) else {
                throw TTZipVaultError.corruptVaultFormat
            }
            return decrypted.subdata(in: 0..<decrypted.count - Int(padLen))
        }

        return decrypted
    }

    /// Low-level direct zeroized memory encryption.
    public static func encryptRaw(
        plaintext: Data,
        key: SecureBytes,
        nonce: Data,
        cipher: TTZipVaultCipher,
        aad: Data? = nil
    ) throws -> (ciphertext: Data, tag: Data?) {
        guard key.count == 32, nonce.count == cipher.nonceLength else {
            throw TTZipVaultError.invalidParameter
        }

        guard let keyPtr = key.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
            throw TTZipVaultError.invalidParameter
        }

        var ciphertext = Data(count: plaintext.count)
        var tag: Data? = cipher.tagLength > 0 ? Data(count: cipher.tagLength) : nil

        let status: Int32 = try plaintext.withUnsafeBytes { srcBuf in
            guard let srcPtr = srcBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                throw TTZipVaultError.invalidParameter
            }
            return try nonce.withUnsafeBytes { nonceBuf in
                guard let noncePtr = nonceBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                    throw TTZipVaultError.invalidParameter
                }
                return try ciphertext.withUnsafeMutableBytes { dstBuf in
                    guard let dstPtr = dstBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                        throw TTZipVaultError.invalidParameter
                    }

                    switch cipher {
                    case .aes256Gcm:
                        return try tag!.withUnsafeMutableBytes { tagBuf in
                            guard let tagPtr = tagBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                                throw TTZipVaultError.invalidParameter
                            }
                            if let aadData = aad, !aadData.isEmpty {
                                return aadData.withUnsafeBytes { aadBuf in
                                    ttzip_rust_vault_encrypt_key(
                                        keyPtr, noncePtr, srcPtr, plaintext.count,
                                        aadBuf.baseAddress?.assumingMemoryBound(to: UInt8.self), aadData.count,
                                        dstPtr, tagPtr
                                    )
                                }
                            } else {
                                return ttzip_rust_vault_encrypt_key(keyPtr, noncePtr, srcPtr, plaintext.count, nil, 0, dstPtr, tagPtr)
                            }
                        }

                    case .chacha20Poly1305:
                        return try tag!.withUnsafeMutableBytes { tagBuf in
                            guard let tagPtr = tagBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                                throw TTZipVaultError.invalidParameter
                            }
                            if let aadData = aad, !aadData.isEmpty {
                                return aadData.withUnsafeBytes { aadBuf in
                                    ttzip_rust_chacha20_poly1305_encrypt(
                                        keyPtr, noncePtr, srcPtr, plaintext.count,
                                        aadBuf.baseAddress?.assumingMemoryBound(to: UInt8.self), aadData.count,
                                        dstPtr, tagPtr
                                    )
                                }
                            } else {
                                return ttzip_rust_chacha20_poly1305_encrypt(keyPtr, noncePtr, srcPtr, plaintext.count, nil, 0, dstPtr, tagPtr)
                            }
                        }

                    case .aes256Ctr:
                        let counter = nonce.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 0, as: UInt64.self).littleEndian }
                        return ttzip_rust_aes256_ctr(keyPtr, counter, srcPtr, plaintext.count, dstPtr)

                    case .aes256Cbc:
                        return ttzip_rust_aes256_cbc_encrypt(keyPtr, noncePtr, srcPtr, plaintext.count, dstPtr)
                    }
                }
            }
        }

        guard status == 0 else {
            throw TTZipVaultError.encryptionFailed(status: status)
        }

        return (ciphertext, tag)
    }

    /// Low-level direct zeroized memory decryption.
    public static func decryptRaw(
        ciphertext: Data,
        key: SecureBytes,
        nonce: Data,
        cipher: TTZipVaultCipher,
        tag: Data?,
        aad: Data? = nil
    ) throws -> Data {
        guard key.count == 32, nonce.count == cipher.nonceLength else {
            throw TTZipVaultError.invalidParameter
        }

        guard let keyPtr = key.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
            throw TTZipVaultError.invalidParameter
        }

        var plaintext = Data(count: ciphertext.count)

        let status: Int32 = try ciphertext.withUnsafeBytes { srcBuf in
            guard let srcPtr = srcBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                throw TTZipVaultError.invalidParameter
            }
            return try nonce.withUnsafeBytes { nonceBuf in
                guard let noncePtr = nonceBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                    throw TTZipVaultError.invalidParameter
                }
                return try plaintext.withUnsafeMutableBytes { dstBuf in
                    guard let dstPtr = dstBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                        throw TTZipVaultError.invalidParameter
                    }

                    switch cipher {
                    case .aes256Gcm:
                        guard let tagData = tag, tagData.count == 16 else {
                            throw TTZipVaultError.invalidParameter
                        }
                        return try tagData.withUnsafeBytes { tagBuf in
                            guard let tagPtr = tagBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                                throw TTZipVaultError.invalidParameter
                            }
                            if let aadData = aad, !aadData.isEmpty {
                                return aadData.withUnsafeBytes { aadBuf in
                                    ttzip_rust_vault_decrypt_key(
                                        keyPtr, noncePtr, srcPtr, ciphertext.count,
                                        aadBuf.baseAddress?.assumingMemoryBound(to: UInt8.self), aadData.count,
                                        tagPtr, dstPtr
                                    )
                                }
                            } else {
                                return ttzip_rust_vault_decrypt_key(keyPtr, noncePtr, srcPtr, ciphertext.count, nil, 0, tagPtr, dstPtr)
                            }
                        }

                    case .chacha20Poly1305:
                        guard let tagData = tag, tagData.count == 16 else {
                            throw TTZipVaultError.invalidParameter
                        }
                        return try tagData.withUnsafeBytes { tagBuf in
                            guard let tagPtr = tagBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                                throw TTZipVaultError.invalidParameter
                            }
                            if let aadData = aad, !aadData.isEmpty {
                                return aadData.withUnsafeBytes { aadBuf in
                                    ttzip_rust_chacha20_poly1305_decrypt(
                                        keyPtr, noncePtr, srcPtr, ciphertext.count,
                                        aadBuf.baseAddress?.assumingMemoryBound(to: UInt8.self), aadData.count,
                                        tagPtr, dstPtr
                                    )
                                }
                            } else {
                                return ttzip_rust_chacha20_poly1305_decrypt(keyPtr, noncePtr, srcPtr, ciphertext.count, nil, 0, tagPtr, dstPtr)
                            }
                        }

                    case .aes256Ctr:
                        let counter = nonce.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 0, as: UInt64.self).littleEndian }
                        return ttzip_rust_aes256_ctr(keyPtr, counter, srcPtr, ciphertext.count, dstPtr)

                    case .aes256Cbc:
                        return ttzip_rust_aes256_cbc_decrypt(keyPtr, noncePtr, srcPtr, ciphertext.count, dstPtr)
                    }
                }
            }
        }

        guard status == 0 else {
            throw TTZipVaultError.authenticationFailed
        }

        return plaintext
    }
}
