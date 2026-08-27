// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Sub-millisecond Offline Ed25519 License Verification Engine (RFC 8032).
//!
//! Provides cross-platform deterministic license token validation without platform-specific dependencies.

use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Default TTZip embedded Ed25519 public key (32 bytes in Base64).
pub const DEFAULT_PUBLIC_KEY_BASE64: &str = "pOkv5VfIP3WVbXalJnc+OkkLGo1MazH4m0TMPw8dZrs=";

/// Structured payload contained within an Ed25519 signed license key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct UniFFILicensePayload {
    pub version: i32,
    pub email: String,
    pub tier: String,
    pub issued_at: String,
    pub order_id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct InternalLicensePayload {
    #[serde(alias = "version", default = "default_version")]
    pub v: i32,
    pub email: String,
    pub tier: String,
    pub issued_at: String,
    pub order_id: String,
}

fn default_version() -> i32 {
    1
}

/// Verification result enumeration exposed to UniFFI.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum UniFFILicenseResult {
    Valid { payload: UniFFILicensePayload },
    InvalidSignature,
    MalformedKey { reason: String },
}

/// Verifies a license key against embedded or custom Ed25519 public key.
#[uniffi::export]
pub fn verify_license_key(
    license_key: String,
    public_key_base64: Option<String>,
) -> UniFFILicenseResult {
    let trimmed = license_key.trim();
    if !trimmed.starts_with("TTZIP1-") {
        return UniFFILicenseResult::MalformedKey {
            reason: "Missing TTZIP1- protocol prefix".to_string(),
        };
    }

    let token = &trimmed["TTZIP1-".len()..];
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 2 {
        return UniFFILicenseResult::MalformedKey {
            reason: "Invalid token format, expected <payload>.<signature>".to_string(),
        };
    }

    let payload_b64 = parts[0];
    let sig_b64 = parts[1];

    let payload_bytes = match decode_base64(payload_b64) {
        Ok(b) if !b.is_empty() => b,
        _ => {
            return UniFFILicenseResult::MalformedKey {
                reason: "Failed to decode base64 payload".to_string(),
            }
        }
    };

    let sig_bytes = match decode_base64(sig_b64) {
        Ok(b) if b.len() == 64 => b,
        _ => {
            return UniFFILicenseResult::MalformedKey {
                reason: "Failed to decode base64 signature".to_string(),
            }
        }
    };

    let pub_key_str = public_key_base64.as_deref().unwrap_or(DEFAULT_PUBLIC_KEY_BASE64);
    let pub_key_bytes = match decode_base64(pub_key_str) {
        Ok(b) if b.len() == 32 => b,
        Ok(_) => {
            return UniFFILicenseResult::MalformedKey {
                reason: "Failed to initialize Ed25519 public key from raw bytes".to_string(),
            }
        }
        Err(_) => {
            return UniFFILicenseResult::MalformedKey {
                reason: "Invalid base64 public key representation".to_string(),
            }
        }
    };

    let mut pk_arr = [0u8; 32];
    pk_arr.copy_from_slice(&pub_key_bytes);

    let verifying_key = match VerifyingKey::from_bytes(&pk_arr) {
        Ok(vk) => vk,
        Err(_) => {
            return UniFFILicenseResult::MalformedKey {
                reason: "Failed to parse Ed25519 verifying key".to_string(),
            };
        }
    };

    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);
    let signature = Signature::from_bytes(&sig_arr);

    if verifying_key.verify_strict(&payload_bytes, &signature).is_err() {
        return UniFFILicenseResult::InvalidSignature;
    }

    let json_payload: InternalLicensePayload = match serde_json::from_slice(&payload_bytes) {
        Ok(p) => p,
        Err(_) => {
            return UniFFILicenseResult::MalformedKey {
                reason: "Failed to decode LicensePayload JSON".to_string(),
            }
        }
    };

    UniFFILicenseResult::Valid {
        payload: UniFFILicensePayload {
            version: json_payload.v,
            email: json_payload.email,
            tier: json_payload.tier,
            issued_at: json_payload.issued_at,
            order_id: json_payload.order_id,
        },
    }
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

fn decode_base64(input: &str) -> Result<Vec<u8>, ()> {
    let clean: Vec<u8> = input
        .bytes()
        .filter(|&b| !b.is_ascii_whitespace())
        .collect();
    if clean.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::with_capacity((clean.len() * 3) / 4);
    let mut buf = 0u32;
    let mut bits = 0;

    for &b in &clean {
        if b == b'=' {
            break;
        }
        let val = decode_base64_char(b).ok_or(())?;
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_decode() {
        assert_eq!(decode_base64("").unwrap(), b"");
        assert_eq!(decode_base64("AQID").unwrap(), vec![1, 2, 3]);
        assert_eq!(decode_base64("SGVsbG8=").unwrap(), b"Hello");
    }

    #[test]
    fn test_verify_license_key_malformed() {
        let res = verify_license_key("INVALID-KEY".to_string(), None);
        assert!(matches!(res, UniFFILicenseResult::MalformedKey { .. }));
    }
}
