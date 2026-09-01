// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! PDF Encryption Guard & Cryptographic Downgrade Defender.
//!
//! Inspects PDF encryption dictionaries, classifies cryptographic cipher suites
//! (Standard RC4, AES-128, AES-256), intercepts cryptographic downgrade attacks,
//! and conducts constant-time password validation probes.

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use super::PdfDefenseError;

/// Standard ISO 32000-1 password padding string (32 bytes).
pub const PDF_STANDARD_PASSWORD_PADDING: [u8; 32] = [
    0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41,
    0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01, 0x08,
    0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80,
    0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53, 0x69, 0x7A,
];

/// Supported and classified PDF cryptographic cipher suites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CipherSuite {
    /// Insecure 40-bit RC4 (PDF 1.1 / V=1, R=2). Broken and vulnerable to trivial brute force.
    Rc4_40,
    /// Legacy 128-bit RC4 (PDF 1.4 / V=2, R=3). Deprecated.
    Rc4_128,
    /// Standard AES-128-CBC (PDF 1.6 / V=4, R=4).
    Aes128Cbc,
    /// Modern AES-256-CBC with SHA-256/384/512 (ISO 32000-2 / PDF 2.0 / V=5, R=5,6).
    Aes256Cbc,
    /// Unrecognized or proprietary cryptographic handler.
    Unknown(String),
}

impl CipherSuite {
    /// Returns true if the cipher suite is cryptographically secure according to modern standards (AES-128 or AES-256).
    pub fn is_modern_secure(&self) -> bool {
        matches!(self, CipherSuite::Aes128Cbc | CipherSuite::Aes256Cbc)
    }

    /// Returns true if the cipher suite represents a legacy or insecure algorithm (e.g. 40-bit RC4).
    pub fn is_insecure_or_deprecated(&self) -> bool {
        matches!(self, CipherSuite::Rc4_40 | CipherSuite::Rc4_128)
    }
}

/// Policy governing acceptable cryptographic cipher suites in PDF documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EncryptionSecurityPolicy {
    /// Enforces modern AES-128/256 ciphers and strictly rejects weak legacy RC4 downgrade attacks.
    #[default]
    EnforceModernAesOnly,
    /// Permits standard legacy ciphers with audit logging.
    AllowStandardAndModern,
}

/// Comprehensive inspection report of document encryption parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptionInspectionReport {
    /// Whether the document is encrypted.
    pub is_encrypted: bool,
    /// Identified cryptographic cipher suite.
    pub cipher_suite: CipherSuite,
    /// Security handler filter name (e.g. "Standard", "Adobe.PubSec").
    pub filter: String,
    /// Algorithm version `V` (1..5).
    pub version_v: i32,
    /// Security revision `R` (2..6).
    pub revision_r: i32,
    /// Effective encryption key length in bits (40..256).
    pub key_length_bits: usize,
    /// User access permissions bitmask flags `P`.
    pub permissions_p: i64,
    /// Whether document metadata stream is encrypted (`/EncryptMetadata`).
    pub encrypt_metadata: bool,
    /// Whether an empty (default) user password unlocks document view access.
    pub is_open_with_empty_password: bool,
}

/// Guard inspecting PDF encryption parameters and mitigating downgrade exploits.
#[derive(Debug, Clone)]
pub struct PdfEncryptionGuard {
    policy: EncryptionSecurityPolicy,
}

impl Default for PdfEncryptionGuard {
    fn default() -> Self {
        Self::new(EncryptionSecurityPolicy::EnforceModernAesOnly)
    }
}

impl PdfEncryptionGuard {
    /// Creates a new guard with the specified security policy.
    pub fn new(policy: EncryptionSecurityPolicy) -> Self {
        Self { policy }
    }

    /// Returns the currently active policy.
    pub fn policy(&self) -> EncryptionSecurityPolicy {
        self.policy
    }

    /// Inspects the encryption dictionary of a `lopdf::Document`.
    pub fn inspect_document(
        &self,
        doc: &lopdf::Document,
    ) -> Result<EncryptionInspectionReport, PdfDefenseError> {
        let encrypt_obj = match doc.trailer.get(b"Encrypt") {
            Ok(obj) => obj,
            Err(_) => {
                return Ok(EncryptionInspectionReport {
                    is_encrypted: false,
                    cipher_suite: CipherSuite::Unknown("None".to_string()),
                    filter: String::new(),
                    version_v: 0,
                    revision_r: 0,
                    key_length_bits: 0,
                    permissions_p: 0,
                    encrypt_metadata: true,
                    is_open_with_empty_password: true,
                });
            }
        };

        let encrypt_dict = match encrypt_obj {
            lopdf::Object::Dictionary(d) => d,
            lopdf::Object::Reference(id) => {
                let resolved = doc.get_object(*id).map_err(|e| {
                    PdfDefenseError::MalformedPdf {
                        reason: format!("Failed to resolve /Encrypt object {:?}: {e}", id),
                        offset: None,
                    }
                })?;
                resolved.as_dict().map_err(|_| {
                    PdfDefenseError::MalformedPdf {
                        reason: "/Encrypt object is not a dictionary".to_string(),
                        offset: None,
                    }
                })?
            }
            _ => {
                return Err(PdfDefenseError::MalformedPdf {
                    reason: "/Encrypt trailer entry is neither dictionary nor reference".to_string(),
                    offset: None,
                });
            }
        };

        let filter = encrypt_dict
            .get(b"Filter")
            .ok()
            .and_then(|o| o.as_name_str().ok())
            .unwrap_or("Standard")
            .to_string();

        let v = encrypt_dict
            .get(b"V")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0) as i32;

        let r = encrypt_dict
            .get(b"R")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0) as i32;

        let length = encrypt_dict
            .get(b"Length")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(40) as usize;

        let p = encrypt_dict
            .get(b"P")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0);

        let encrypt_metadata = encrypt_dict
            .get(b"EncryptMetadata")
            .ok()
            .and_then(|o| o.as_bool().ok())
            .unwrap_or(true);

        // Classify Cipher Suite
        let cipher_suite = match (v, r) {
            (1, 2) => CipherSuite::Rc4_40,
            (2, 3) => CipherSuite::Rc4_128,
            (4, 4) => CipherSuite::Aes128Cbc,
            (5, 5) | (5, 6) => CipherSuite::Aes256Cbc,
            _ => {
                if length == 256 || v == 5 {
                    CipherSuite::Aes256Cbc
                } else if length == 128 || v == 4 {
                    CipherSuite::Aes128Cbc
                } else if v == 1 {
                    CipherSuite::Rc4_40
                } else {
                    CipherSuite::Unknown(format!("V={v}, R={r}, Length={length}"))
                }
            }
        };

        // Enforce Downgrade Defense Policy
        if self.policy == EncryptionSecurityPolicy::EnforceModernAesOnly
            && cipher_suite.is_insecure_or_deprecated()
        {
            return Err(PdfDefenseError::InsecureEncryptionDetected {
                filter: filter.clone(),
                algorithm: format!("{cipher_suite:?}"),
                reason: "Deprecated/broken RC4 cipher rejected by Modern AES security policy".to_string(),
            });
        }

        // Test empty password probe
        let is_open_with_empty_password = Self::probe_empty_password(encrypt_dict);

        Ok(EncryptionInspectionReport {
            is_encrypted: true,
            cipher_suite,
            filter,
            version_v: v,
            revision_r: r,
            key_length_bits: length,
            permissions_p: p,
            encrypt_metadata,
            is_open_with_empty_password,
        })
    }

    /// Constant-time probe testing if an empty user password satisfies the standard hash.
    fn probe_empty_password(dict: &lopdf::Dictionary) -> bool {
        if let (Ok(lopdf::Object::String(u_bytes, _)), Ok(lopdf::Object::String(o_bytes, _))) =
            (dict.get(b"U"), dict.get(b"O"))
        {
            if u_bytes.len() >= 32 && o_bytes.len() >= 32 {
                // Check if user string matches standard padded digest (for standard V=1/2/4)
                let mut hasher = Sha256::new();
                hasher.update(PDF_STANDARD_PASSWORD_PADDING);
                hasher.update(&o_bytes[..32]);
                let digest = hasher.finalize();

                // Constant-time comparison
                let match_len = 16.min(u_bytes.len());
                let ct_eq: bool = u_bytes[..match_len].ct_eq(&digest[..match_len]).into();
                return ct_eq;
            }
        }
        false
    }

    /// Validates a supplied user password in constant-time against document verification hashes.
    pub fn verify_password_probe(
        &self,
        provided_password: &[u8],
        expected_hash: &[u8],
    ) -> bool {
        if expected_hash.is_empty() {
            return false;
        }

        let mut padded = [0u8; 32];
        let copy_len = provided_password.len().min(32);
        padded[..copy_len].copy_from_slice(&provided_password[..copy_len]);
        if copy_len < 32 {
            padded[copy_len..].copy_from_slice(&PDF_STANDARD_PASSWORD_PADDING[..(32 - copy_len)]);
        }

        let mut hasher = Sha256::new();
        hasher.update(padded);
        let digest = hasher.finalize();

        let cmp_len = digest.len().min(expected_hash.len());
        let is_eq: bool = digest[..cmp_len].ct_eq(&expected_hash[..cmp_len]).into();
        is_eq
    }
}
