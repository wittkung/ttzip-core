// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Cross-Language Ed25519 Plugin Authentication & Certificate Hierarchy Layer.
//!
//! Provides typed, memory-safe, and Swift 6 Sendable bindings for `.ttplugin` manifest signing,
//! offline Ed25519 signature verification, 3-tier certificate chain validation, and cryptographic
//! developer key fingerprint derivation (NIST SHA-256 and BLAKE3).

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use super::types::TTZipError;
use crate::crypto::ed25519::signing::SigningKey;
use crate::crypto::ed25519::verifying::{Signature, VerifyingKey, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};

/// Official TTZip Default Embedded Ed25519 Root Public Key (32 bytes in Base64).
pub const DEFAULT_TTZIP_ROOT_PUBLIC_KEY_BASE64: &str = "pOkv5VfIP3WVbXalJnc+OkkLGo1MazH4m0TMPw8dZrs=";

/// Strongly typed authentication and verification status enumeration exposed via UniFFI.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum UniFFIAuthStatus {
    /// Digital signature and certificate chain are cryptographically valid and active.
    Valid,
    /// Digital signature mismatch or tampering detected.
    InvalidSignature,
    /// Certificate issuer does not chain to any configured trusted root anchor.
    UntrustedRoot,
    /// Malformed public key, signature format, or corrupt certificate payload.
    MalformedCert,
    /// Certificate or manifest has expired or is not yet valid.
    Expired,
}

/// Lightweight Ed25519 Digital Identity Certificate record exposed via UniFFI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct UniFFIEd25519Cert {
    /// Unique certificate serial number.
    pub serial_number: String,
    /// Entity identifier of the certificate authority / issuer.
    pub issuer_id: String,
    /// Entity identifier of the subject / developer.
    pub subject_id: String,
    /// 32-byte Ed25519 public key in standard Base64 representation.
    pub public_key_base64: String,
    /// Validity commencement timestamp in seconds since Unix epoch.
    pub issued_at_epoch_secs: i64,
    /// Validity expiration timestamp in seconds since Unix epoch.
    pub expires_at_epoch_secs: i64,
    /// 64-byte Ed25519 digital signature in Base64 representation.
    pub signature_base64: String,
    /// Standardized SHA-256 public key fingerprint (e.g. `SHA256:...`).
    pub fingerprint_sha256: String,
}

impl UniFFIEd25519Cert {
    /// Computes canonical To-Be-Signed (TBS) byte payload for certificate issuance and verification.
    pub fn canonical_tbs_bytes(&self) -> Result<Vec<u8>, TTZipError> {
        let pub_bytes = decode_base64(&self.public_key_base64)
            .ok_or_else(|| TTZipError::SecurityViolation {
                reason: "Invalid Base64 in certificate public key".to_string(),
            })?;
        if pub_bytes.len() != PUBLIC_KEY_LENGTH {
            return Err(TTZipError::SecurityViolation {
                reason: format!("Expected 32-byte public key, got {}", pub_bytes.len()),
            });
        }

        let mut buf = Vec::with_capacity(160);
        buf.extend_from_slice(b"TTZIP-CERT-TBS-V1\n");
        buf.extend_from_slice(self.serial_number.as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(self.issuer_id.as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(self.subject_id.as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(&pub_bytes);
        buf.extend_from_slice(&self.issued_at_epoch_secs.to_le_bytes());
        buf.extend_from_slice(&self.expires_at_epoch_secs.to_le_bytes());
        Ok(buf)
    }
}

// ============================================================================
// UniFFIPluginSigner
// ============================================================================

/// Thread-safe cryptographic Ed25519 signer for `.ttplugin` manifests and certificates.
#[derive(uniffi::Object)]
pub struct UniFFIPluginSigner {
    signing_key: SigningKey,
}

#[uniffi::export]
impl UniFFIPluginSigner {
    /// Instantiates a signer from a 32-byte secret seed in standard Base64 representation.
    #[uniffi::constructor]
    pub fn from_seed_base64(seed_base64: String) -> Result<Arc<Self>, TTZipError> {
        let mut seed_bytes = decode_base64(seed_base64.trim()).ok_or_else(|| TTZipError::SecurityViolation {
            reason: "Invalid Base64 secret seed encoding".to_string(),
        })?;
        if seed_bytes.len() != 32 {
            seed_bytes.zeroize();
            return Err(TTZipError::SecurityViolation {
                reason: format!("Ed25519 secret seed must be 32 bytes, got {}", seed_bytes.len()),
            });
        }

        let key = SigningKey::from_slice(&seed_bytes).map_err(|e| {
            seed_bytes.zeroize();
            TTZipError::SecurityViolation {
                reason: format!("Failed to parse signing key: {:?}", e),
            }
        })?;
        seed_bytes.zeroize();

        Ok(Arc::new(Self { signing_key: key }))
    }

    /// Instantiates a signer from raw 32-byte secret seed bytes.
    #[uniffi::constructor]
    pub fn from_seed_bytes(mut seed_bytes: Vec<u8>) -> Result<Arc<Self>, TTZipError> {
        if seed_bytes.len() != 32 {
            seed_bytes.zeroize();
            return Err(TTZipError::SecurityViolation {
                reason: format!("Ed25519 secret seed must be 32 bytes, got {}", seed_bytes.len()),
            });
        }

        let key = SigningKey::from_slice(&seed_bytes).map_err(|e| {
            seed_bytes.zeroize();
            TTZipError::SecurityViolation {
                reason: format!("Failed to parse signing key: {:?}", e),
            }
        })?;
        seed_bytes.zeroize();

        Ok(Arc::new(Self { signing_key: key }))
    }

    /// Generates a cryptographically secure random private signing key.
    #[uniffi::constructor]
    pub fn generate() -> Result<Arc<Self>, TTZipError> {
        let mut seed = [0u8; 32];
        if crate::crypto::vault::get_random_bytes(&mut seed).is_err() {
            return Err(TTZipError::EngineError { code: -101 });
        }
        let key = SigningKey::from_slice(&seed).map_err(|e| {
            seed.zeroize();
            TTZipError::SecurityViolation {
                reason: format!("Failed to initialize random signing key: {:?}", e),
            }
        })?;
        seed.zeroize();

        Ok(Arc::new(Self { signing_key: key }))
    }

    /// Returns the associated 32-byte Ed25519 public key encoded in standard Base64.
    pub fn get_public_key_base64(&self) -> String {
        let vk = self.signing_key.verifying_key();
        base64_encode(vk.as_bytes())
    }

    /// Returns the raw 32-byte Ed25519 public key.
    pub fn get_public_key_bytes(&self) -> Vec<u8> {
        let vk = self.signing_key.verifying_key();
        vk.as_bytes().to_vec()
    }

    /// Derives the standard SHA-256 public key fingerprint in format `SHA256:<base64>`.
    pub fn get_fingerprint_sha256(&self) -> String {
        let vk = self.signing_key.verifying_key();
        compute_sha256_fingerprint(vk.as_bytes())
    }

    /// Derives the high-performance BLAKE3 public key fingerprint in format `BLAKE3:<base64>`.
    pub fn get_fingerprint_blake3(&self) -> String {
        let vk = self.signing_key.verifying_key();
        compute_blake3_fingerprint(vk.as_bytes())
    }

    /// Generates a deterministic 64-byte Ed25519 digital signature over raw binary payload.
    pub fn sign(&self, data: Vec<u8>) -> Vec<u8> {
        let sig = self.signing_key.sign(&data);
        sig.to_bytes().to_vec()
    }

    /// Generates a 64-byte Ed25519 digital signature over raw binary payload encoded in Base64.
    pub fn sign_base64(&self, data: Vec<u8>) -> String {
        let sig = self.signing_key.sign(&data);
        base64_encode(&sig.to_bytes())
    }

    /// Generates a detached Base64 signature for a UTF-8 manifest payload string.
    pub fn sign_manifest_string(&self, manifest_content: String) -> String {
        self.sign_base64(manifest_content.as_bytes().to_vec())
    }

    /// Issues and signs a new `UniFFIEd25519Cert` for a subject developer public key.
    pub fn issue_certificate(
        &self,
        issuer_id: String,
        subject_id: String,
        subject_public_key_base64: String,
        validity_days: u32,
        serial_number: Option<String>,
    ) -> Result<UniFFIEd25519Cert, TTZipError> {
        let pub_clean = subject_public_key_base64.trim();
        let pub_bytes = decode_base64(pub_clean).ok_or_else(|| TTZipError::SecurityViolation {
            reason: "Invalid Base64 subject public key".to_string(),
        })?;
        if pub_bytes.len() != PUBLIC_KEY_LENGTH {
            return Err(TTZipError::SecurityViolation {
                reason: format!("Subject public key must be 32 bytes, got {}", pub_bytes.len()),
            });
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let expires = now + (validity_days as i64 * 86400);

        let serial = serial_number.unwrap_or_else(|| {
            let mut rand_bytes = [0u8; 8];
            let _ = crate::crypto::vault::get_random_bytes(&mut rand_bytes);
            let val = u64::from_be_bytes(rand_bytes);
            format!("TTZIP-CERT-{:016X}", val)
        });

        let mut cert = UniFFIEd25519Cert {
            serial_number: serial,
            issuer_id,
            subject_id,
            public_key_base64: pub_clean.to_string(),
            issued_at_epoch_secs: now,
            expires_at_epoch_secs: expires,
            signature_base64: String::new(),
            fingerprint_sha256: compute_sha256_fingerprint(&pub_bytes),
        };

        let tbs = cert.canonical_tbs_bytes()?;
        let sig = self.signing_key.sign(&tbs);
        cert.signature_base64 = base64_encode(&sig.to_bytes());

        Ok(cert)
    }
}

// ============================================================================
// UniFFIPluginVerifier
// ============================================================================

/// Thread-safe cryptographic Ed25519 verifier for `.ttplugin` archives, manifests, and certificates.
#[derive(uniffi::Object)]
pub struct UniFFIPluginVerifier {
    trusted_roots: RwLock<Vec<[u8; PUBLIC_KEY_LENGTH]>>,
}

#[uniffi::export]
impl UniFFIPluginVerifier {
    /// Creates a verifier configured with custom trusted root public keys in Base64.
    #[uniffi::constructor]
    pub fn new(root_public_keys_base64: Vec<String>) -> Result<Arc<Self>, TTZipError> {
        let mut roots = Vec::with_capacity(root_public_keys_base64.len());
        for pk_b64 in root_public_keys_base64 {
            let b = decode_base64(pk_b64.trim()).ok_or_else(|| TTZipError::SecurityViolation {
                reason: format!("Malformed root public key base64: {}", pk_b64),
            })?;
            if b.len() != PUBLIC_KEY_LENGTH {
                return Err(TTZipError::SecurityViolation {
                    reason: format!("Root public key must be 32 bytes, got {}", b.len()),
                });
            }
            let mut arr = [0u8; PUBLIC_KEY_LENGTH];
            arr.copy_from_slice(&b);
            roots.push(arr);
        }
        Ok(Arc::new(Self {
            trusted_roots: RwLock::new(roots),
        }))
    }

    /// Creates a verifier preloaded with the official embedded TTZip root public key.
    #[uniffi::constructor]
    pub fn default_verifier() -> Arc<Self> {
        let mut roots = Vec::new();
        if let Some(b) = decode_base64(DEFAULT_TTZIP_ROOT_PUBLIC_KEY_BASE64) {
            if b.len() == PUBLIC_KEY_LENGTH {
                let mut arr = [0u8; PUBLIC_KEY_LENGTH];
                arr.copy_from_slice(&b);
                roots.push(arr);
            }
        }
        Arc::new(Self {
            trusted_roots: RwLock::new(roots),
        })
    }

    /// Appends a trusted root public key in Base64 representation to the verifier's trust store.
    pub fn add_trusted_root_base64(&self, root_public_key_base64: String) -> Result<(), TTZipError> {
        let b = decode_base64(root_public_key_base64.trim()).ok_or_else(|| TTZipError::SecurityViolation {
            reason: "Invalid Base64 in root public key".to_string(),
        })?;
        if b.len() != PUBLIC_KEY_LENGTH {
            return Err(TTZipError::SecurityViolation {
                reason: format!("Root public key must be 32 bytes, got {}", b.len()),
            });
        }
        let mut arr = [0u8; PUBLIC_KEY_LENGTH];
        arr.copy_from_slice(&b);

        let mut guard = self.trusted_roots.write();
        if !guard.contains(&arr) {
            guard.push(arr);
        }
        Ok(())
    }

    /// Verifies raw binary data against raw Ed25519 signature and public key bytes.
    pub fn verify_raw(&self, data: Vec<u8>, signature_bytes: Vec<u8>, public_key_bytes: Vec<u8>) -> UniFFIAuthStatus {
        if public_key_bytes.len() != PUBLIC_KEY_LENGTH || signature_bytes.len() != SIGNATURE_LENGTH {
            return UniFFIAuthStatus::MalformedCert;
        }

        let vk = match VerifyingKey::from_slice(&public_key_bytes) {
            Ok(k) => k,
            Err(_) => return UniFFIAuthStatus::MalformedCert,
        };

        let sig = match Signature::from_slice(&signature_bytes) {
            Ok(s) => s,
            Err(_) => return UniFFIAuthStatus::MalformedCert,
        };

        match vk.verify_strict(&data, &sig) {
            Ok(_) => UniFFIAuthStatus::Valid,
            Err(_) => UniFFIAuthStatus::InvalidSignature,
        }
    }

    /// Verifies raw data against Base64-encoded signature and public key strings.
    pub fn verify_signature_base64(
        &self,
        data: Vec<u8>,
        signature_base64: String,
        public_key_base64: String,
    ) -> UniFFIAuthStatus {
        let sig_bytes = match decode_base64(signature_base64.trim()) {
            Some(b) if b.len() == SIGNATURE_LENGTH => b,
            _ => return UniFFIAuthStatus::MalformedCert,
        };

        let pub_bytes = match decode_base64(public_key_base64.trim()) {
            Some(b) if b.len() == PUBLIC_KEY_LENGTH => b,
            _ => return UniFFIAuthStatus::MalformedCert,
        };

        self.verify_raw(data, sig_bytes, pub_bytes)
    }

    /// Verifies a `UniFFIEd25519Cert` against configured trusted root anchors and validity timestamps.
    pub fn verify_certificate(
        &self,
        cert: UniFFIEd25519Cert,
        current_timestamp_secs: Option<i64>,
    ) -> UniFFIAuthStatus {
        let now = current_timestamp_secs.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        });

        // 1. Temporal validity check
        if cert.issued_at_epoch_secs > cert.expires_at_epoch_secs {
            return UniFFIAuthStatus::MalformedCert;
        }
        if now < cert.issued_at_epoch_secs || now > cert.expires_at_epoch_secs {
            return UniFFIAuthStatus::Expired;
        }

        // 2. Format decoding
        let sig_bytes = match decode_base64(cert.signature_base64.trim()) {
            Some(b) if b.len() == SIGNATURE_LENGTH => b,
            _ => return UniFFIAuthStatus::MalformedCert,
        };
        let pub_bytes = match decode_base64(cert.public_key_base64.trim()) {
            Some(b) if b.len() == PUBLIC_KEY_LENGTH => b,
            _ => return UniFFIAuthStatus::MalformedCert,
        };

        let tbs = match cert.canonical_tbs_bytes() {
            Ok(b) => b,
            Err(_) => return UniFFIAuthStatus::MalformedCert,
        };

        let sig = match Signature::from_slice(&sig_bytes) {
            Ok(s) => s,
            Err(_) => return UniFFIAuthStatus::MalformedCert,
        };

        // 3. Verify against trusted root store
        let roots = self.trusted_roots.read();
        if roots.is_empty() {
            // Direct self-signed certificate fallback check
            let vk = match VerifyingKey::from_slice(&pub_bytes) {
                Ok(k) => k,
                Err(_) => return UniFFIAuthStatus::MalformedCert,
            };
            return match vk.verify_strict(&tbs, &sig) {
                Ok(_) => UniFFIAuthStatus::Valid,
                Err(_) => UniFFIAuthStatus::InvalidSignature,
            };
        }

        let mut root_matched = false;
        for root in roots.iter() {
            if let Ok(vk) = VerifyingKey::from_slice(root) {
                if vk.verify_strict(&tbs, &sig).is_ok() {
                    root_matched = true;
                    break;
                }
            }
        }

        if root_matched {
            UniFFIAuthStatus::Valid
        } else {
            UniFFIAuthStatus::UntrustedRoot
        }
    }

    /// Verifies `.ttplugin` manifest string against a detached Base64 signature and developer public key.
    pub fn verify_manifest(
        &self,
        manifest_content: String,
        signature_base64: String,
        developer_public_key_base64: String,
    ) -> UniFFIAuthStatus {
        self.verify_signature_base64(
            manifest_content.as_bytes().to_vec(),
            signature_base64,
            developer_public_key_base64,
        )
    }

    /// Verifies `.ttplugin` manifest string using a developer certificate chain.
    pub fn verify_manifest_with_cert(
        &self,
        manifest_content: String,
        signature_base64: String,
        cert: UniFFIEd25519Cert,
        current_timestamp_secs: Option<i64>,
    ) -> UniFFIAuthStatus {
        let cert_status = self.verify_certificate(cert.clone(), current_timestamp_secs);
        if cert_status != UniFFIAuthStatus::Valid {
            return cert_status;
        }

        self.verify_manifest(
            manifest_content,
            signature_base64,
            cert.public_key_base64,
        )
    }

    /// Computes standardized SHA-256 fingerprint for a Base64 public key string.
    pub fn extract_fingerprint_sha256(&self, public_key_base64: String) -> Result<String, TTZipError> {
        let b = decode_base64(public_key_base64.trim()).ok_or_else(|| TTZipError::SecurityViolation {
            reason: "Invalid Base64 public key encoding".to_string(),
        })?;
        if b.len() != PUBLIC_KEY_LENGTH {
            return Err(TTZipError::SecurityViolation {
                reason: format!("Expected 32-byte public key, got {}", b.len()),
            });
        }
        Ok(compute_sha256_fingerprint(&b))
    }

    /// Computes standardized BLAKE3 fingerprint for a Base64 public key string.
    pub fn extract_fingerprint_blake3(&self, public_key_base64: String) -> Result<String, TTZipError> {
        let b = decode_base64(public_key_base64.trim()).ok_or_else(|| TTZipError::SecurityViolation {
            reason: "Invalid Base64 public key encoding".to_string(),
        })?;
        if b.len() != PUBLIC_KEY_LENGTH {
            return Err(TTZipError::SecurityViolation {
                reason: format!("Expected 32-byte public key, got {}", b.len()),
            });
        }
        Ok(compute_blake3_fingerprint(&b))
    }
}

// ============================================================================
// Cryptographic Fingerprint & Encoding Utilities
// ============================================================================

/// Computes SHA-256 fingerprint formatted as `SHA256:<base64>`.
pub fn compute_sha256_fingerprint(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!("SHA256:{}", base64_encode(&digest))
}

/// Computes BLAKE3 fingerprint formatted as `BLAKE3:<base64>`.
pub fn compute_blake3_fingerprint(bytes: &[u8]) -> String {
    let digest = crate::crypto::blake3::hash(bytes);
    format!("BLAKE3:{}", base64_encode(&digest))
}

/// Base64 encoding table and implementation.
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

fn decode_base64_char(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}

/// Robust zero-dependency Base64 decoding supporting standard and URL-safe variants.
pub fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let clean: Vec<u8> = input
        .bytes()
        .filter(|&b| !b.is_ascii_whitespace())
        .collect();
    if clean.is_empty() {
        return Some(Vec::new());
    }

    let mut out = Vec::with_capacity((clean.len() * 3) / 4);
    let mut buf = 0u32;
    let mut bits = 0;

    for &b in &clean {
        if b == b'=' {
            break;
        }
        let val = decode_base64_char(b)?;
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signer_verifier_roundtrip() {
        let signer = UniFFIPluginSigner::generate().unwrap();
        let pub_b64 = signer.get_public_key_base64();
        let msg = b"Plugin Payload Manifest V1";

        let sig_b64 = signer.sign_base64(msg.to_vec());
        let verifier = UniFFIPluginVerifier::default_verifier();

        let status = verifier.verify_signature_base64(msg.to_vec(), sig_b64.clone(), pub_b64.clone());
        assert_eq!(status, UniFFIAuthStatus::Valid);

        let tampered_status = verifier.verify_signature_base64(b"Tampered".to_vec(), sig_b64, pub_b64);
        assert_eq!(tampered_status, UniFFIAuthStatus::InvalidSignature);
    }

    #[test]
    fn test_certificate_issuance_and_verification() {
        let root_signer = UniFFIPluginSigner::generate().unwrap();
        let root_pub = root_signer.get_public_key_base64();

        let dev_signer = UniFFIPluginSigner::generate().unwrap();
        let dev_pub = dev_signer.get_public_key_base64();

        let cert = root_signer
            .issue_certificate(
                "TTZip Root CA".to_string(),
                "com.wittkung.larksync".to_string(),
                dev_pub.clone(),
                365,
                None,
            )
            .unwrap();

        assert_eq!(cert.subject_id, "com.wittkung.larksync");
        assert_eq!(cert.public_key_base64, dev_pub);

        let verifier = UniFFIPluginVerifier::new(vec![root_pub]).unwrap();
        let cert_status = verifier.verify_certificate(cert.clone(), None);
        assert_eq!(cert_status, UniFFIAuthStatus::Valid);

        let manifest = "{\"plugin_id\": \"com.wittkung.larksync\", \"version\": \"1.0.1\"}";
        let sig = dev_signer.sign_manifest_string(manifest.to_string());

        let manifest_status = verifier.verify_manifest_with_cert(
            manifest.to_string(),
            sig,
            cert,
            None,
        );
        assert_eq!(manifest_status, UniFFIAuthStatus::Valid);
    }

    #[test]
    fn test_fingerprint_derivation() {
        let signer = UniFFIPluginSigner::generate().unwrap();
        let fp_sha256 = signer.get_fingerprint_sha256();
        let fp_blake3 = signer.get_fingerprint_blake3();

        assert!(fp_sha256.starts_with("SHA256:"));
        assert!(fp_blake3.starts_with("BLAKE3:"));

        let verifier = UniFFIPluginVerifier::default_verifier();
        let ext_sha = verifier.extract_fingerprint_sha256(signer.get_public_key_base64()).unwrap();
        assert_eq!(ext_sha, fp_sha256);
    }
}
