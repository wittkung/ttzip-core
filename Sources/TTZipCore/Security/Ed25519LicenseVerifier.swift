// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// Verification result enumeration.
public enum LicenseVerificationResult: Sendable, Equatable {
    case valid(LicensePayload)
    case invalidSignature
    case malformedKey(String)
}

/// Zero-network, sub-millisecond Ed25519 offline license verifier powered by Rust microkernel.
public final class Ed25519LicenseVerifier: Sendable {
    
    /// Official TTZip embedded Ed25519 public key (32 bytes in Base64).
    public static let defaultPublicKeyBase64 = "pOkv5VfIP3WVbXalJnc+OkkLGo1MazH4m0TMPw8dZrs="
    
    /// Verifies a license key string against the official embedded public key or custom key.
    public static func verify(
        licenseKey: String,
        publicKeyBase64: String = defaultPublicKeyBase64
    ) -> LicenseVerificationResult {
        let rustResult = verifyLicenseKey(
            licenseKey: licenseKey,
            publicKeyBase64: publicKeyBase64
        )
        switch rustResult {
        case .valid(let payload):
            let tier = LicenseTier(rawValue: payload.tier) ?? .proLifetime
            let swiftPayload = LicensePayload(
                v: Int(payload.version),
                email: payload.email,
                tier: tier,
                issued_at: payload.issuedAt,
                order_id: payload.orderId
            )
            return .valid(swiftPayload)
        case .invalidSignature:
            return .invalidSignature
        case .malformedKey(let reason):
            return .malformedKey(reason)
        }
    }
}

