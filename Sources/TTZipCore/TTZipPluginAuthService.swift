// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation

/// Cryptographic authentication status for `.ttplugin` manifests, signatures, and certificates.
public enum PluginAuthStatus: String, Sendable, Codable, CaseIterable {
    /// Digital signature and certificate chain are cryptographically valid and active.
    case valid
    /// Digital signature mismatch, corrupt bits, or tampering detected.
    case invalidSignature
    /// Certificate issuer does not chain to any configured trusted root anchor.
    case untrustedRoot
    /// Malformed public key, invalid signature format, or corrupt certificate payload.
    case malformedCert
    /// Certificate or manifest validity window has expired or is in the future.
    case expired

    /// Converts UniFFI raw enum representation to Swift 6 domain enum.
    public init(from uniffiStatus: UniFfiAuthStatus) {
        switch uniffiStatus {
        case .valid: self = .valid
        case .invalidSignature: self = .invalidSignature
        case .untrustedRoot: self = .untrustedRoot
        case .malformedCert: self = .malformedCert
        case .expired: self = .expired
        }
    }

    /// Converts domain enum to UniFFI raw enum representation.
    public var uniffiStatus: UniFfiAuthStatus {
        switch self {
        case .valid: return .valid
        case .invalidSignature: return .invalidSignature
        case .untrustedRoot: return .untrustedRoot
        case .malformedCert: return .malformedCert
        case .expired: return .expired
        }
    }

    /// Whether the authentication verification was completely successful.
    public var isSuccess: Bool {
        return self == .valid
    }
}

/// Strongly-typed Swift 6 representation of an Ed25519 digital identity certificate.
public struct PluginCertificate: Sendable, Codable, Identifiable, Hashable {
    /// Certificate serial identifier matching `serialNumber`.
    public var id: String { serialNumber }
    /// Unique certificate serial number string.
    public let serialNumber: String
    /// Entity identifier of the certificate authority / issuer.
    public let issuerId: String
    /// Entity identifier of the subject / developer.
    public let subjectId: String
    /// 32-byte Ed25519 public key in standard Base64 representation.
    public let publicKeyBase64: String
    /// Validity commencement date.
    public let issuedAt: Date
    /// Validity expiration date.
    public let expiresAt: Date
    /// 64-byte Ed25519 digital signature in Base64 representation.
    public let signatureBase64: String
    /// Standardized SHA-256 public key fingerprint (e.g. `SHA256:...`).
    public let fingerprintSha256: String

    public init(
        serialNumber: String,
        issuerId: String,
        subjectId: String,
        publicKeyBase64: String,
        issuedAt: Date,
        expiresAt: Date,
        signatureBase64: String,
        fingerprintSha256: String
    ) {
        self.serialNumber = serialNumber
        self.issuerId = issuerId
        self.subjectId = subjectId
        self.publicKeyBase64 = publicKeyBase64
        self.issuedAt = issuedAt
        self.expiresAt = expiresAt
        self.signatureBase64 = signatureBase64
        self.fingerprintSha256 = fingerprintSha256
    }

    /// Initializes from UniFFI bridge record.
    public init(from uniffiRecord: UniFfiEd25519Cert) {
        self.serialNumber = uniffiRecord.serialNumber
        self.issuerId = uniffiRecord.issuerId
        self.subjectId = uniffiRecord.subjectId
        self.publicKeyBase64 = uniffiRecord.publicKeyBase64
        self.issuedAt = Date(timeIntervalSince1970: TimeInterval(uniffiRecord.issuedAtEpochSecs))
        self.expiresAt = Date(timeIntervalSince1970: TimeInterval(uniffiRecord.expiresAtEpochSecs))
        self.signatureBase64 = uniffiRecord.signatureBase64
        self.fingerprintSha256 = uniffiRecord.fingerprintSha256
    }

    /// Converts to UniFFI bridge record.
    public func toUniFFI() -> UniFfiEd25519Cert {
        return UniFfiEd25519Cert(
            serialNumber: serialNumber,
            issuerId: issuerId,
            subjectId: subjectId,
            publicKeyBase64: publicKeyBase64,
            issuedAtEpochSecs: Int64(issuedAt.timeIntervalSince1970),
            expiresAtEpochSecs: Int64(expiresAt.timeIntervalSince1970),
            signatureBase64: signatureBase64,
            fingerprintSha256: fingerprintSha256
        )
    }
}

/// Comprehensive offline verification report for `.ttplugin` archives and manifests.
public struct PluginVerificationReport: Sendable, Codable, Hashable {
    /// Cryptographic authentication result.
    public let status: PluginAuthStatus
    /// Whether the package is completely verified and trusted for execution.
    public let isValid: Bool
    /// Developer subject identity extracted from verified certificate or manifest.
    public let developerId: String?
    /// Verified cryptographic fingerprint of the publisher key.
    public let fingerprint: String?
    /// Verified developer certificate if certificate-based signing was utilized.
    public let certificate: PluginCertificate?
    /// Verification execution timestamp.
    public let verifiedAt: Date
    /// Localized diagnostic error details if verification failed.
    public let errorDetails: String?

    public init(
        status: PluginAuthStatus,
        developerId: String? = nil,
        fingerprint: String? = nil,
        certificate: PluginCertificate? = nil,
        verifiedAt: Date = Date(),
        errorDetails: String? = nil
    ) {
        self.status = status
        self.isValid = status.isSuccess
        self.developerId = developerId
        self.fingerprint = fingerprint
        self.certificate = certificate
        self.verifiedAt = verifiedAt
        self.errorDetails = errorDetails
    }
}

/// Supported hashing algorithms for public key fingerprints in Swift.
public enum PluginFingerprintAlgorithm: String, Sendable, CaseIterable {
    case sha256
    case blake3
}

/// Swift 6 `@Observable` and `Sendable` offline plugin authentication and verification service.
///
/// Wraps the high-performance Rust Ed25519 microkernel to provide zero-network manifest
/// validation, 3-tier certificate verification, and developer fingerprint resolution.
@Observable
public final class TTZipPluginAuthService: @unchecked Sendable {

    /// Shared singleton instance.
    public static let shared = TTZipPluginAuthService()

    /// Default embedded TTZip Root Public Key (32 bytes in Base64).
    public static let defaultRootPublicKeyBase64 = "pOkv5VfIP3WVbXalJnc+OkkLGo1MazH4m0TMPw8dZrs="

    private let lock = NSLock()
    private var verifier: UniFfiPluginVerifier

    // MARK: - Published Observable Metrics

    /// Total cumulative plugin verification operations executed.
    public private(set) var totalVerificationsCount: Int = 0
    /// Total successful signature/certificate verifications.
    public private(set) var successfulVerificationsCount: Int = 0
    /// Total failed or rejected verification attempts.
    public private(set) var failedVerificationsCount: Int = 0
    /// Active number of trusted root certificate authorities configured in memory.
    public private(set) var trustedRootsCount: Int = 1

    /// Initializes service preloaded with default embedded TTZip root trust anchor.
    public init() {
        self.verifier = UniFfiPluginVerifier.defaultVerifier()
    }

    /// Initializes service with custom list of trusted root public keys in Base64.
    public init(rootPublicKeysBase64: [String]) throws {
        self.verifier = try UniFfiPluginVerifier(rootPublicKeysBase64: rootPublicKeysBase64)
        self.trustedRootsCount = rootPublicKeysBase64.count
    }

    // MARK: - Trust Anchor Management

    /// Appends a trusted root public key in Base64 representation to the verification trust store.
    public func addTrustedRoot(publicKeyBase64: String) throws {
        lock.lock()
        defer { lock.unlock() }
        try verifier.addTrustedRootBase64(rootPublicKeyBase64: publicKeyBase64)
        trustedRootsCount += 1
    }

    /// Resets the verifier instance back to the embedded default TTZip root public key.
    public func resetToDefaultRoots() {
        lock.lock()
        defer { lock.unlock() }
        self.verifier = UniFfiPluginVerifier.defaultVerifier()
        self.trustedRootsCount = 1
    }

    // MARK: - Offline Manifest & Package Verification

    /// Verifies `.ttplugin` manifest string against a detached Base64 signature and developer public key.
    public func verifyManifest(
        rawManifestJson: String,
        signatureBase64: String,
        developerPublicKeyBase64: String
    ) async -> PluginVerificationReport {
        let rustStatus = verifier.verifyManifest(
            manifestContent: rawManifestJson,
            signatureBase64: signatureBase64,
            developerPublicKeyBase64: developerPublicKeyBase64
        )
        let status = PluginAuthStatus(from: rustStatus)
        let fp = try? verifier.extractFingerprintSha256(publicKeyBase64: developerPublicKeyBase64)

        recordVerificationMetrics(isSuccess: status.isSuccess)

        return PluginVerificationReport(
            status: status,
            fingerprint: fp,
            errorDetails: status.isSuccess ? nil : "Manifest Ed25519 signature verification failed with status: \(status.rawValue)"
        )
    }

    /// Verifies `.ttplugin` manifest string using a verified developer certificate.
    public func verifyManifest(
        rawManifestJson: String,
        signatureBase64: String,
        certificate: PluginCertificate,
        referenceDate: Date = Date()
    ) async -> PluginVerificationReport {
        let currentSecs = Int64(referenceDate.timeIntervalSince1970)
        let uniffiCert = certificate.toUniFFI()

        let rustStatus = verifier.verifyManifestWithCert(
            manifestContent: rawManifestJson,
            signatureBase64: signatureBase64,
            cert: uniffiCert,
            currentTimestampSecs: currentSecs
        )
        let status = PluginAuthStatus(from: rustStatus)

        recordVerificationMetrics(isSuccess: status.isSuccess)

        return PluginVerificationReport(
            status: status,
            developerId: certificate.subjectId,
            fingerprint: certificate.fingerprintSha256,
            certificate: certificate,
            errorDetails: status.isSuccess ? nil : "Certificate-chained manifest validation failed with status: \(status.rawValue)"
        )
    }

    /// Verifies raw data bytes against a detached Ed25519 signature and public key.
    public func verifyRawData(
        data: Data,
        signatureBase64: String,
        publicKeyBase64: String
    ) async -> PluginAuthStatus {
        let rustStatus = verifier.verifySignatureBase64(
            data: data,
            signatureBase64: signatureBase64,
            publicKeyBase64: publicKeyBase64
        )
        let status = PluginAuthStatus(from: rustStatus)
        recordVerificationMetrics(isSuccess: status.isSuccess)
        return status
    }

    /// Verifies a `PluginCertificate` against configured root anchors and validity timestamps.
    public func verifyCertificate(
        certificate: PluginCertificate,
        referenceDate: Date = Date()
    ) async -> PluginAuthStatus {
        let currentSecs = Int64(referenceDate.timeIntervalSince1970)
        let rustStatus = verifier.verifyCertificate(
            cert: certificate.toUniFFI(),
            currentTimestampSecs: currentSecs
        )
        let status = PluginAuthStatus(from: rustStatus)
        recordVerificationMetrics(isSuccess: status.isSuccess)
        return status
    }

    // MARK: - Cryptographic Fingerprint Resolution

    /// Derives the cryptographic public key fingerprint for a Base64-encoded public key.
    public func computeFingerprint(
        fromPublicKeyBase64 publicKeyBase64: String,
        algorithm: PluginFingerprintAlgorithm = .sha256
    ) throws -> String {
        switch algorithm {
        case .sha256:
            return try verifier.extractFingerprintSha256(publicKeyBase64: publicKeyBase64)
        case .blake3:
            return try verifier.extractFingerprintBlake3(publicKeyBase64: publicKeyBase64)
        }
    }

    /// Derives the SHA-256 fingerprint for raw 32-byte public key data.
    public func computeFingerprint(fromPublicKeyBytes bytes: Data) -> String {
        let base64 = bytes.base64EncodedString()
        if let fp = try? verifier.extractFingerprintSha256(publicKeyBase64: base64) {
            return fp
        }
        return "SHA256:\(bytes.base64EncodedString())"
    }

    // MARK: - Signer & Tooling Utilities

    /// Creates an offline signer instance from a 32-byte secret seed in Base64 representation.
    public static func createSigner(fromSeedBase64 seedBase64: String) throws -> UniFfiPluginSigner {
        return try UniFfiPluginSigner.fromSeedBase64(seedBase64: seedBase64)
    }

    /// Creates an offline signer instance from raw 32-byte secret seed data.
    public static func createSigner(fromSeedBytes seedBytes: Data) throws -> UniFfiPluginSigner {
        return try UniFfiPluginSigner.fromSeedBytes(seedBytes: seedBytes)
    }

    /// Generates a fresh random Ed25519 private signing key.
    public static func generateRandomSigner() throws -> UniFfiPluginSigner {
        return try UniFfiPluginSigner.generate()
    }

    /// Issues and signs a new `PluginCertificate` using an issuer private signing key.
    public static func issueCertificate(
        issuerSigner: UniFfiPluginSigner,
        issuerId: String,
        subjectId: String,
        subjectPublicKeyBase64: String,
        validityDays: Int = 365,
        serialNumber: String? = nil
    ) throws -> PluginCertificate {
        let uniffiCert = try issuerSigner.issueCertificate(
            issuerId: issuerId,
            subjectId: subjectId,
            subjectPublicKeyBase64: subjectPublicKeyBase64,
            validityDays: UInt32(max(1, validityDays)),
            serialNumber: serialNumber
        )
        return PluginCertificate(from: uniffiCert)
    }

    // MARK: - Private Metrics Helper

    private func recordVerificationMetrics(isSuccess: Bool) {
        lock.lock()
        defer { lock.unlock() }
        totalVerificationsCount += 1
        if isSuccess {
            successfulVerificationsCount += 1
        } else {
            failedVerificationsCount += 1
        }
    }
}
