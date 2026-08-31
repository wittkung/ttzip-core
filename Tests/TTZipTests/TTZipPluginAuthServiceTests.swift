// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
import CryptoKit
@testable import TTZipCore

final class TTZipPluginAuthServiceTests: XCTestCase {

    func testSignerAndVerifierRoundtrip() async throws {
        let signer = try TTZipPluginAuthService.generateRandomSigner()
        let pubBase64 = signer.getPublicKeyBase64()
        let service = TTZipPluginAuthService.shared

        let sampleData = "Manifest content for TTZip Plugin".data(using: .utf8)!
        let signatureBase64 = signer.signBase64(data: sampleData)

        let status = await service.verifyRawData(
            data: sampleData,
            signatureBase64: signatureBase64,
            publicKeyBase64: pubBase64
        )
        XCTAssertEqual(status, .valid)

        let tamperedData = "Tampered manifest content".data(using: .utf8)!
        let tamperedStatus = await service.verifyRawData(
            data: tamperedData,
            signatureBase64: signatureBase64,
            publicKeyBase64: pubBase64
        )
        XCTAssertEqual(tamperedStatus, .invalidSignature)
    }

    func testManifestVerificationReport() async throws {
        let signer = try TTZipPluginAuthService.generateRandomSigner()
        let pubBase64 = signer.getPublicKeyBase64()
        let service = TTZipPluginAuthService.shared

        let manifestJson = "{\"id\":\"com.wittkung.larksync\",\"version\":\"1.0.1\",\"author\":\"Witt Kung\"}"
        let signatureBase64 = signer.signManifestString(manifestContent: manifestJson)

        let report = await service.verifyManifest(
            rawManifestJson: manifestJson,
            signatureBase64: signatureBase64,
            developerPublicKeyBase64: pubBase64
        )

        XCTAssertTrue(report.isValid)
        XCTAssertEqual(report.status, .valid)
        XCTAssertNotNil(report.fingerprint)
        XCTAssertTrue(report.fingerprint?.hasPrefix("SHA256:") == true)
        XCTAssertNil(report.errorDetails)
    }

    func testCertificateChainedManifestVerification() async throws {
        // 1. Setup root authority and verifier service
        let rootSigner = try TTZipPluginAuthService.generateRandomSigner()
        let rootPubBase64 = rootSigner.getPublicKeyBase64()
        let service = try TTZipPluginAuthService(rootPublicKeysBase64: [rootPubBase64])

        // 2. Setup developer keypair and issue certificate from root CA
        let devSigner = try TTZipPluginAuthService.generateRandomSigner()
        let devPubBase64 = devSigner.getPublicKeyBase64()

        let cert = try TTZipPluginAuthService.issueCertificate(
            issuerSigner: rootSigner,
            issuerId: "TTZip Official Marketplace Authority",
            subjectId: "com.wittkung.larksync",
            subjectPublicKeyBase64: devPubBase64,
            validityDays: 365,
            serialNumber: "CERT-TEST-001"
        )

        XCTAssertEqual(cert.serialNumber, "CERT-TEST-001")
        XCTAssertEqual(cert.subjectId, "com.wittkung.larksync")
        XCTAssertEqual(cert.publicKeyBase64, devPubBase64)

        // 3. Verify certificate alone
        let certStatus = await service.verifyCertificate(certificate: cert)
        XCTAssertEqual(certStatus, .valid)

        // 4. Sign and verify manifest with certificate
        let manifest = "{\"id\":\"com.wittkung.larksync\",\"version\":\"1.0.1\"}"
        let sig = devSigner.signManifestString(manifestContent: manifest)

        let report = await service.verifyManifest(
            rawManifestJson: manifest,
            signatureBase64: sig,
            certificate: cert
        )

        XCTAssertTrue(report.isValid)
        XCTAssertEqual(report.status, .valid)
        XCTAssertEqual(report.developerId, "com.wittkung.larksync")
        XCTAssertEqual(report.certificate?.serialNumber, "CERT-TEST-001")
    }

    func testExpiredCertificateRejection() async throws {
        let rootSigner = try TTZipPluginAuthService.generateRandomSigner()
        let rootPubBase64 = rootSigner.getPublicKeyBase64()
        let service = try TTZipPluginAuthService(rootPublicKeysBase64: [rootPubBase64])

        let devSigner = try TTZipPluginAuthService.generateRandomSigner()
        let cert = try TTZipPluginAuthService.issueCertificate(
            issuerSigner: rootSigner,
            issuerId: "TTZip Root CA",
            subjectId: "com.example.expired",
            subjectPublicKeyBase64: devSigner.getPublicKeyBase64(),
            validityDays: 1,
            serialNumber: "CERT-EXPIRED-001"
        )

        // Reference date 10 days in the future
        let futureDate = Date(timeIntervalSinceNow: 10 * 86400)
        let status = await service.verifyCertificate(certificate: cert, referenceDate: futureDate)
        XCTAssertEqual(status, .expired)
    }

    func testFingerprintDerivation() throws {
        let signer = try TTZipPluginAuthService.generateRandomSigner()
        let pubBase64 = signer.getPublicKeyBase64()
        let service = TTZipPluginAuthService.shared

        let sha256Fp = try service.computeFingerprint(fromPublicKeyBase64: pubBase64, algorithm: .sha256)
        let blake3Fp = try service.computeFingerprint(fromPublicKeyBase64: pubBase64, algorithm: .blake3)

        XCTAssertTrue(sha256Fp.hasPrefix("SHA256:"))
        XCTAssertTrue(blake3Fp.hasPrefix("BLAKE3:"))
    }

    func testCrossLanguageCryptoKitCompatibility() throws {
        // Verify that signatures generated by Swift CryptoKit can be verified by Rust engine
        let privateKey = Curve25519.Signing.PrivateKey()
        let publicKey = privateKey.publicKey
        let pubBase64 = publicKey.rawRepresentation.base64EncodedString()

        let testData = "Cross-language verification test data".data(using: .utf8)!
        let signatureData = try privateKey.signature(for: testData)
        let sigBase64 = signatureData.base64EncodedString()

        let verifier = UniFfiPluginVerifier.defaultVerifier()
        let status = verifier.verifySignatureBase64(
            data: testData,
            signatureBase64: sigBase64,
            publicKeyBase64: pubBase64
        )
        XCTAssertEqual(status, .valid)
    }
}
