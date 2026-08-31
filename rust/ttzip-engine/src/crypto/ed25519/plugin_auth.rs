// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! `.ttplugin` manifest authentication, 3-tier Ed25519 certificate chain hierarchy
//! (Root -> Marketplace -> Developer), and cryptographic key fingerprint computation.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::error::Ed25519Error;
use super::signing::SigningKey;
use super::verifying::{Signature, VerifyingKey, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
use crate::crypto::blake3;

/// Supported hashing algorithms for Ed25519 public key fingerprints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FingerprintAlgorithm {
    /// NIST SHA-256 fingerprint.
    Sha256,
    /// High-throughput BLAKE3 fingerprint.
    Blake3,
}

/// Three-tier certificate hierarchy levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CertificateLevel {
    /// Level 0: Offline TTZip Root Trust Anchor.
    Root,
    /// Level 1: TTZip Official Marketplace / App Store Authority.
    Marketplace,
    /// Level 2: Verified Plugin Developer.
    Developer,
}

impl core::fmt::Display for CertificateLevel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Root => write!(f, "Root"),
            Self::Marketplace => write!(f, "Marketplace"),
            Self::Developer => write!(f, "Developer"),
        }
    }
}

/// Canonical payload and structure for `.ttplugin` package manifests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifestAuth {
    /// Unique bundle reverse-DNS identifier (e.g. "com.wittkung.larksync").
    pub plugin_id: String,
    /// Semantic version string (e.g. "1.0.1").
    pub version: String,
    /// Developer subject identifier matching certificate chain.
    pub developer_id: String,
    /// Declared system capabilities and permissions.
    pub permissions: Vec<String>,
    /// SHA-256 checksum of the native executable binary.
    pub binary_sha256: String,
    /// Optional SHA-256 checksum of static resource assets.
    pub resources_sha256: Option<String>,
    /// Manifest creation timestamp (seconds since Unix epoch).
    pub issued_at: u64,
    /// Manifest expiration timestamp (seconds since Unix epoch).
    pub valid_until: u64,
}

impl PluginManifestAuth {
    /// Computes canonical byte representation for hashing and signature generation.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut sorted_perms = self.permissions.clone();
        sorted_perms.sort_unstable();

        let mut buf = Vec::with_capacity(256);
        buf.extend_from_slice(b"TTZIP-PLUGIN-MANIFEST-V1\n");
        buf.extend_from_slice(self.plugin_id.as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(self.version.as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(self.developer_id.as_bytes());
        buf.push(b'\n');
        for perm in &sorted_perms {
            buf.extend_from_slice(perm.as_bytes());
            buf.push(b',');
        }
        buf.push(b'\n');
        buf.extend_from_slice(self.binary_sha256.as_bytes());
        buf.push(b'\n');
        if let Some(ref res_hash) = self.resources_sha256 {
            buf.extend_from_slice(res_hash.as_bytes());
        }
        buf.push(b'\n');
        buf.extend_from_slice(&self.issued_at.to_le_bytes());
        buf.extend_from_slice(&self.valid_until.to_le_bytes());
        buf
    }

    /// Signs the manifest using the developer's private signing key.
    pub fn sign_manifest(&self, key: &SigningKey) -> Signature {
        let payload = self.canonical_bytes();
        key.sign(&payload)
    }

    /// Verifies manifest signature, time validity, and structure against developer's public key.
    pub fn verify_manifest(
        &self,
        key: &VerifyingKey,
        signature: &Signature,
        current_time: u64,
    ) -> Result<(), Ed25519Error> {
        if current_time < self.issued_at {
            return Err(Ed25519Error::CertificateNotYetValid {
                valid_from: self.issued_at,
                valid_until: self.valid_until,
                current_time,
            });
        }
        if current_time > self.valid_until {
            return Err(Ed25519Error::CertificateExpired {
                valid_from: self.issued_at,
                valid_until: self.valid_until,
                current_time,
            });
        }

        let payload = self.canonical_bytes();
        key.verify(&payload, signature)
    }
}

mod sig_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8; 64], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(bytes)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 64], D::Error>
    where
        D: Deserializer<'de>,
    {
        let v: Vec<u8> = Deserialize::deserialize(deserializer)?;
        if v.len() != 64 {
            return Err(serde::de::Error::invalid_length(v.len(), &"64 bytes"));
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&v);
        Ok(arr)
    }
}

/// TTZip X.509-equivalent lightweight Ed25519 digital identity certificate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TTZipCertificate {
    /// Hierarchy tier level.
    pub level: CertificateLevel,
    /// Issuer entity identifier.
    pub issuer_id: String,
    /// Subject entity identifier.
    pub subject_id: String,
    /// 32-byte Ed25519 public key of the subject.
    pub subject_public_key: [u8; PUBLIC_KEY_LENGTH],
    /// Certificate validity start timestamp in seconds since Unix epoch.
    pub valid_from: u64,
    /// Certificate validity expiration timestamp in seconds since Unix epoch.
    pub valid_until: u64,
    /// 64-byte Ed25519 digital signature generated by the issuer.
    #[serde(with = "sig_serde")]
    pub signature: [u8; SIGNATURE_LENGTH],
}

impl TTZipCertificate {
    /// Generates canonical To-Be-Signed (TBS) byte payload for certificate issuance/verification.
    pub fn tbs_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(128);
        buf.extend_from_slice(b"TTZIP-CERTIFICATE-TBS-V1\n");
        buf.extend_from_slice(self.level.to_string().as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(self.issuer_id.as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(self.subject_id.as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(&self.subject_public_key);
        buf.extend_from_slice(&self.valid_from.to_le_bytes());
        buf.extend_from_slice(&self.valid_until.to_le_bytes());
        buf
    }

    /// Issues and signs a new `TTZipCertificate` using the issuer's private signing key.
    pub fn issue(
        level: CertificateLevel,
        issuer_id: String,
        subject_id: String,
        subject_public_key: [u8; PUBLIC_KEY_LENGTH],
        valid_from: u64,
        valid_until: u64,
        issuer_key: &SigningKey,
    ) -> Self {
        let mut cert = Self {
            level,
            issuer_id,
            subject_id,
            subject_public_key,
            valid_from,
            valid_until,
            signature: [0u8; SIGNATURE_LENGTH],
        };
        let tbs = cert.tbs_bytes();
        let sig = issuer_key.sign(&tbs);
        cert.signature = sig.to_bytes();
        cert
    }

    /// Verifies the certificate signature against an issuer public key and validates timestamp validity.
    pub fn verify_signature(
        &self,
        issuer_key: &VerifyingKey,
        current_time: u64,
    ) -> Result<(), Ed25519Error> {
        if current_time < self.valid_from {
            return Err(Ed25519Error::CertificateNotYetValid {
                valid_from: self.valid_from,
                valid_until: self.valid_until,
                current_time,
            });
        }
        if current_time > self.valid_until {
            return Err(Ed25519Error::CertificateExpired {
                valid_from: self.valid_from,
                valid_until: self.valid_until,
                current_time,
            });
        }

        let tbs = self.tbs_bytes();
        let sig = Signature::from_bytes(&self.signature);
        issuer_key.verify(&tbs, &sig)
    }
}

/// Complete 3-tier certificate chain connecting Root Trust Anchor to Developer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertificateChain {
    /// Level 0: Root certificate.
    pub root_cert: TTZipCertificate,
    /// Level 1: Marketplace authority certificate.
    pub marketplace_cert: TTZipCertificate,
    /// Level 2: Developer certificate.
    pub developer_cert: TTZipCertificate,
}

impl CertificateChain {
    /// Creates a new `CertificateChain`.
    pub fn new(
        root_cert: TTZipCertificate,
        marketplace_cert: TTZipCertificate,
        developer_cert: TTZipCertificate,
    ) -> Self {
        Self {
            root_cert,
            marketplace_cert,
            developer_cert,
        }
    }

    /// Validates the entire certificate chain against a trusted Root public key
    /// and returns the verified Developer `VerifyingKey`.
    pub fn verify_chain(
        &self,
        trusted_root_key: &VerifyingKey,
        current_time: u64,
    ) -> Result<VerifyingKey, Ed25519Error> {
        // 1. Verify Root Tier
        if self.root_cert.level != CertificateLevel::Root {
            return Err(Ed25519Error::InvalidCertificateLevel {
                expected: "Root".to_string(),
                actual: self.root_cert.level.to_string(),
            });
        }
        if self.root_cert.subject_public_key != trusted_root_key.to_bytes() {
            return Err(Ed25519Error::CertificateChainBroken {
                expected_issuer: "TrustedRootPublicKey".to_string(),
                actual_issuer: "RootSubjectMismatch".to_string(),
            });
        }
        self.root_cert.verify_signature(trusted_root_key, current_time)?;

        // 2. Verify Marketplace Tier
        if self.marketplace_cert.level != CertificateLevel::Marketplace {
            return Err(Ed25519Error::InvalidCertificateLevel {
                expected: "Marketplace".to_string(),
                actual: self.marketplace_cert.level.to_string(),
            });
        }
        if self.marketplace_cert.issuer_id != self.root_cert.subject_id {
            return Err(Ed25519Error::CertificateChainBroken {
                expected_issuer: self.root_cert.subject_id.clone(),
                actual_issuer: self.marketplace_cert.issuer_id.clone(),
            });
        }
        let root_subject_key = VerifyingKey::from_bytes(&self.root_cert.subject_public_key)?;
        self.marketplace_cert.verify_signature(&root_subject_key, current_time)?;

        // 3. Verify Developer Tier
        if self.developer_cert.level != CertificateLevel::Developer {
            return Err(Ed25519Error::InvalidCertificateLevel {
                expected: "Developer".to_string(),
                actual: self.developer_cert.level.to_string(),
            });
        }
        if self.developer_cert.issuer_id != self.marketplace_cert.subject_id {
            return Err(Ed25519Error::CertificateChainBroken {
                expected_issuer: self.marketplace_cert.subject_id.clone(),
                actual_issuer: self.developer_cert.issuer_id.clone(),
            });
        }
        let marketplace_subject_key =
            VerifyingKey::from_bytes(&self.marketplace_cert.subject_public_key)?;
        self.developer_cert.verify_signature(&marketplace_subject_key, current_time)?;

        // Return verified developer public key
        VerifyingKey::from_bytes(&self.developer_cert.subject_public_key)
    }

    /// End-to-end verification of a plugin manifest against the certificate chain and trusted root.
    pub fn verify_plugin_manifest(
        &self,
        manifest: &PluginManifestAuth,
        manifest_signature: &Signature,
        trusted_root_key: &VerifyingKey,
        current_time: u64,
    ) -> Result<(), Ed25519Error> {
        let dev_key = self.verify_chain(trusted_root_key, current_time)?;
        if manifest.developer_id != self.developer_cert.subject_id {
            return Err(Ed25519Error::ManifestVerificationFailed(format!(
                "Manifest developer_id '{}' does not match certified subject '{}'",
                manifest.developer_id, self.developer_cert.subject_id
            )));
        }
        manifest.verify_manifest(&dev_key, manifest_signature, current_time)
    }
}

/// Encodes byte slice to standard RFC 4648 Base64 string with padding.
pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i];
        let b1 = if i + 1 < data.len() { data[i + 1] } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] } else { 0 };

        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);

        if i + 1 < data.len() {
            out.push(TABLE[((n >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }

        if i + 2 < data.len() {
            out.push(TABLE[(n & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }

        i += 3;
    }
    out
}

/// Computes formatted public key fingerprint string using SHA-256 in Base64 (e.g. `SHA256:...`).
pub fn compute_key_fingerprint_sha256(public_key_bytes: &[u8; PUBLIC_KEY_LENGTH]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(public_key_bytes);
    let hash_res = hasher.finalize();
    format!("SHA256:{}", base64_encode(&hash_res))
}

/// Computes formatted public key fingerprint string using BLAKE3 in Base64 (e.g. `BLAKE3:...`).
pub fn compute_key_fingerprint_blake3(public_key_bytes: &[u8; PUBLIC_KEY_LENGTH]) -> String {
    let hash_res = blake3::hash(public_key_bytes);
    format!("BLAKE3:{}", base64_encode(&hash_res))
}

/// Generates a standardized public key fingerprint based on selected algorithm.
pub fn compute_key_fingerprint(
    public_key_bytes: &[u8; PUBLIC_KEY_LENGTH],
    algorithm: FingerprintAlgorithm,
) -> String {
    match algorithm {
        FingerprintAlgorithm::Sha256 => compute_key_fingerprint_sha256(public_key_bytes),
        FingerprintAlgorithm::Blake3 => compute_key_fingerprint_blake3(public_key_bytes),
    }
}
