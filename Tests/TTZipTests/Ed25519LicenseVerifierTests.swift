// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
import CryptoKit
@testable import TTZipCore

final class Ed25519LicenseVerifierTests: XCTestCase {
    
    func testValidLicenseKeyVerification() throws {
        // Generate ephemeral keypair
        let privateKey = Curve25519.Signing.PrivateKey()
        let publicKey = privateKey.publicKey
        let pubB64 = publicKey.rawRepresentation.base64EncodedString()
        
        let payload = LicensePayload(
            email: "verified@ttzip.app",
            tier: .proLifetime,
            issued_at: "2026-08-25T00:00:00Z",
            order_id: "ORD-9988-OK"
        )
        
        let payloadData = try JSONEncoder().encode(payload)
        let signatureData = try privateKey.signature(for: payloadData)
        
        let keyString = "TTZIP1-\(payloadData.base64EncodedString()).\(signatureData.base64EncodedString())"
        
        let result = Ed25519LicenseVerifier.verify(licenseKey: keyString, publicKeyBase64: pubB64)
        switch result {
        case .valid(let verified):
            XCTAssertEqual(verified.email, "verified@ttzip.app")
            XCTAssertEqual(verified.order_id, "ORD-9988-OK")
            XCTAssertEqual(verified.tier, .proLifetime)
        default:
            XCTFail("Expected valid license verification, got \(result)")
        }
    }
    
    func testForgedPayloadSignatureMismatch() throws {
        let privateKey = Curve25519.Signing.PrivateKey()
        let publicKey = privateKey.publicKey
        let pubB64 = publicKey.rawRepresentation.base64EncodedString()
        
        let payload1 = LicensePayload(email: "orig@ttzip.app", tier: .proLifetime, issued_at: "2026-08-25T00:00:00Z", order_id: "ORD-1")
        let payload2 = LicensePayload(email: "attacker@ttzip.app", tier: .proLifetime, issued_at: "2026-08-25T00:00:00Z", order_id: "ORD-1")
        
        let payloadData1 = try JSONEncoder().encode(payload1)
        let signatureData1 = try privateKey.signature(for: payloadData1)
        
        let forgedPayloadData = try JSONEncoder().encode(payload2)
        let forgedKeyString = "TTZIP1-\(forgedPayloadData.base64EncodedString()).\(signatureData1.base64EncodedString())"
        
        let result = Ed25519LicenseVerifier.verify(licenseKey: forgedKeyString, publicKeyBase64: pubB64)
        XCTAssertEqual(result, .invalidSignature)
    }
    
    func testMalformedKeyStringHandling() {
        let malformedStrings = [
            "",
            "AURA-PRO1-KEY8-2026",
            "TTZIP1-invalidbase64.signature",
            "TTZIP1-validbase64.",
            "SOMEPREFIX-abc.def",
            "TTZIP1-abc"
        ]
        
        for key in malformedStrings {
            let result = Ed25519LicenseVerifier.verify(licenseKey: key)
            switch result {
            case .malformedKey:
                // Success
                break
            default:
                XCTFail("Expected malformedKey for string '\(key)', got \(result)")
            }
        }
    }
}
