// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import CryptoKit

/// Verification result enumeration.
public enum LicenseVerificationResult: Sendable, Equatable {
    case valid(LicensePayload)
    case invalidSignature
    case malformedKey(String)
}

/// Zero-network, sub-millisecond Ed25519 offline license verifier utilizing Apple CryptoKit.
public final class Ed25519LicenseVerifier: Sendable {
    
    /// Official TTZip embedded Ed25519 public key (32 bytes in Base64).
    public static let defaultPublicKeyBase64 = "pOkv5VfIP3WVbXalJnc+OkkLGo1MazH4m0TMPw8dZrs="
    
    /// Verifies a license key string against the official embedded public key or custom key.
    public static func verify(
        licenseKey: String,
        publicKeyBase64: String = defaultPublicKeyBase64
    ) -> LicenseVerificationResult {
        let trimmed = licenseKey.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix("TTZIP1-") else {
            return .malformedKey("Missing TTZIP1- protocol prefix")
        }
        
        let token = String(trimmed.dropFirst("TTZIP1-".count))
        let parts = token.components(separatedBy: ".")
        guard parts.count == 2 else {
            return .malformedKey("Invalid token format, expected <payload>.<signature>")
        }
        
        let payloadB64 = parts[0]
        let signatureB64 = parts[1]
        
        guard let payloadData = Data(base64Encoded: payloadB64) else {
            return .malformedKey("Failed to decode base64 payload")
        }
        
        guard let signatureData = Data(base64Encoded: signatureB64) else {
            return .malformedKey("Failed to decode base64 signature")
        }
        
        guard let publicKeyData = Data(base64Encoded: publicKeyBase64) else {
            return .malformedKey("Invalid base64 public key representation")
        }
        
        guard let publicKey = try? Curve25519.Signing.PublicKey(rawRepresentation: publicKeyData) else {
            return .malformedKey("Failed to initialize Ed25519 public key from raw bytes")
        }
        
        guard publicKey.isValidSignature(signatureData, for: payloadData) else {
            return .invalidSignature
        }
        
        guard let payload = try? JSONDecoder().decode(LicensePayload.self, from: payloadData) else {
            return .malformedKey("Failed to decode LicensePayload JSON")
        }
        
        return .valid(payload)
    }
}
