// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Service and Pipeline Implementations for System Updates, Delta Patching, and Appcast Metadata.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use sha2::{Digest, Sha256};

use super::types::{
    UniFFIAppcastMetadata, UniFFIDeltaFormat, UniFFIDeltaPatchResult,
    UniFFISystemError,
};
use crate::crypto::ed25519::verifying::{Signature, VerifyingKey, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
use crate::uniffi_api::codecs::{
    uniffi_deflate_compress, uniffi_deflate_decompress, uniffi_zstd_compress,
    uniffi_zstd_decompress,
};

/// 15-byte Magic prefix for TTZip Delta Binary Format.
pub const TTZIP_DELTA_MAGIC: &[u8; 15] = b"TTZIP-DELTA-V1\0";
/// Total fixed header length in bytes: 15 (magic) + 1 (format) + 8 (target size) + 32 (base sha256) + 32 (target sha256).
pub const TTZIP_DELTA_HEADER_LEN: usize = 88;

// ============================================================================
// Exported Free Functions
// ============================================================================

/// Creates a binary delta patch package from a base byte buffer to target byte buffer.
#[uniffi::export]
pub fn uniffi_create_delta_patch(
    base_bytes: Vec<u8>,
    target_bytes: Vec<u8>,
    format: UniFFIDeltaFormat,
) -> Result<Vec<u8>, UniFFISystemError> {
    create_delta_patch_impl(&base_bytes, &target_bytes, format)
}

/// Applies a binary delta patch package onto base bytes, reconstructing target bytes in-memory.
#[uniffi::export]
pub fn uniffi_apply_delta_patch(
    base_bytes: Vec<u8>,
    patch_bytes: Vec<u8>,
    expected_target_hash: Option<String>,
) -> Result<UniFFIDeltaPatchResult, UniFFISystemError> {
    apply_delta_patch_impl(&base_bytes, &patch_bytes, expected_target_hash.as_deref())
}

/// Recursively computes deterministic cryptographic Merkle tree hash of a file or directory hierarchy.
#[uniffi::export]
pub fn uniffi_calculate_tree_hash(root_path: String) -> Result<String, UniFFISystemError> {
    calculate_tree_hash_impl(&root_path)
}

/// Verifies detached Ed25519 digital signature of an Appcast feed or artifact bytes.
#[uniffi::export]
pub fn uniffi_verify_appcast_signature(
    appcast_bytes: Vec<u8>,
    signature_base64: String,
    public_key_base64: String,
) -> Result<bool, UniFFISystemError> {
    verify_ed25519_signature_impl(&appcast_bytes, &signature_base64, &public_key_base64)
}

// ============================================================================
// Stateful UniFFI Service Object
// ============================================================================

/// Thread-safe system update, delta patch, and appcast verification engine service.
#[derive(uniffi::Object, Default)]
pub struct UniFFISystemService {}

#[uniffi::export]
impl UniFFISystemService {
    /// Constructs a new thread-safe system update service instance.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    /// Creates a binary delta patch from base bytes to target bytes.
    pub fn create_delta_patch(
        &self,
        base_bytes: Vec<u8>,
        target_bytes: Vec<u8>,
        format: UniFFIDeltaFormat,
    ) -> Result<Vec<u8>, UniFFISystemError> {
        create_delta_patch_impl(&base_bytes, &target_bytes, format)
    }

    /// Applies a binary delta patch onto base bytes directly in memory.
    pub fn apply_delta_patch(
        &self,
        base_bytes: Vec<u8>,
        patch_bytes: Vec<u8>,
        expected_target_hash: Option<String>,
    ) -> Result<UniFFIDeltaPatchResult, UniFFISystemError> {
        apply_delta_patch_impl(&base_bytes, &patch_bytes, expected_target_hash.as_deref())
    }

    /// Recursively computes deterministic Merkle tree hash for a local directory or file.
    pub fn calculate_tree_hash(&self, root_path: String) -> Result<String, UniFFISystemError> {
        calculate_tree_hash_impl(&root_path)
    }

    /// Verifies detached Ed25519 signature of Appcast bytes.
    pub fn verify_appcast_signature(
        &self,
        appcast_bytes: Vec<u8>,
        signature_base64: String,
        public_key_base64: String,
    ) -> Result<bool, UniFFISystemError> {
        verify_ed25519_signature_impl(&appcast_bytes, &signature_base64, &public_key_base64)
    }

    /// Parses JSON representation of an Appcast feed metadata and release items.
    pub fn parse_appcast_json(&self, json_content: String) -> Result<UniFFIAppcastMetadata, UniFFISystemError> {
        serde_json::from_str(&json_content).map_err(|e| UniFFISystemError::AppcastParseError {
            details: format!("Failed to parse JSON appcast: {e}"),
        })
    }

    /// Validates version monotonicity preventing downgrade attacks.
    pub fn check_version_monotonicity(
        &self,
        current_version: String,
        incoming_version: String,
    ) -> Result<bool, UniFFISystemError> {
        if compare_semver(&incoming_version, &current_version) < 0 {
            return Err(UniFFISystemError::VersionDowngradeForbidden {
                current_version,
                incoming_version,
            });
        }
        Ok(true)
    }
}

// ============================================================================
// Internal Implementation Details
// ============================================================================

/// Low-level binary delta patch generator.
fn create_delta_patch_impl(
    base: &[u8],
    target: &[u8],
    format: UniFFIDeltaFormat,
) -> Result<Vec<u8>, UniFFISystemError> {
    let base_hash = Sha256::digest(base);
    let target_hash = Sha256::digest(target);
    let target_len = target.len() as u64;

    let raw_payload = generate_delta_opcodes(base, target);

    let (format_byte, encoded_payload) = match format {
        UniFFIDeltaFormat::RawByteBlock => (0u8, raw_payload),
        UniFFIDeltaFormat::ZstdCompressed => {
            let compressed = uniffi_zstd_compress(raw_payload, 3)
                .map_err(|e| UniFFISystemError::patch_err(format!("Zstd compression failed: {e:?}")))?;
            (1u8, compressed)
        }
        UniFFIDeltaFormat::FlateCompressed => {
            let compressed = uniffi_deflate_compress(raw_payload, 6)
                .map_err(|e| UniFFISystemError::patch_err(format!("Flate compression failed: {e:?}")))?;
            (2u8, compressed)
        }
    };

    let mut output = Vec::with_capacity(TTZIP_DELTA_HEADER_LEN + encoded_payload.len());
    output.extend_from_slice(TTZIP_DELTA_MAGIC);
    output.push(format_byte);
    output.extend_from_slice(&target_len.to_le_bytes());
    output.extend_from_slice(&base_hash);
    output.extend_from_slice(&target_hash);
    output.extend_from_slice(&encoded_payload);

    Ok(output)
}

/// Generates rolling block COPY and INSERT opcodes from base to target.
fn generate_delta_opcodes(base: &[u8], target: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(target.len() / 2 + 64);
    let min_match = if base.len() >= 16 { 16 } else { 4.min(base.len()) };

    let mut base_index: HashMap<u32, Vec<usize>> = HashMap::new();
    if base.len() >= 4 {
        let stride = 4;
        let mut i = 0;
        while i + 4 <= base.len() {
            let chunk = u32::from_le_bytes(base[i..i + 4].try_into().unwrap());
            base_index.entry(chunk).or_default().push(i);
            i += stride;
        }
    }

    let mut target_pos = 0;
    let mut pending_literal = Vec::new();

    while target_pos < target.len() {
        let mut best_match: Option<(usize, usize)> = None;
        if target_pos + 4 <= target.len() && !base_index.is_empty() {
            let chunk = u32::from_le_bytes(target[target_pos..target_pos + 4].try_into().unwrap());
            if let Some(offsets) = base_index.get(&chunk) {
                for &offset in offsets {
                    let mut len = 0;
                    while target_pos + len < target.len()
                        && offset + len < base.len()
                        && base[offset + len] == target[target_pos + len]
                    {
                        len += 1;
                    }
                    if len >= min_match {
                        if let Some((_, best_len)) = best_match {
                            if len > best_len {
                                best_match = Some((offset, len));
                            }
                        } else {
                            best_match = Some((offset, len));
                        }
                    }
                }
            }
        }

        if let Some((base_offset, match_len)) = best_match {
            if !pending_literal.is_empty() {
                payload.push(0x02);
                payload.extend_from_slice(&(pending_literal.len() as u64).to_le_bytes());
                payload.extend_from_slice(&pending_literal);
                pending_literal.clear();
            }
            payload.push(0x01);
            payload.extend_from_slice(&(base_offset as u64).to_le_bytes());
            payload.extend_from_slice(&(match_len as u64).to_le_bytes());
            target_pos += match_len;
        } else {
            pending_literal.push(target[target_pos]);
            target_pos += 1;
        }
    }

    if !pending_literal.is_empty() {
        payload.push(0x02);
        payload.extend_from_slice(&(pending_literal.len() as u64).to_le_bytes());
        payload.extend_from_slice(&pending_literal);
    }

    payload.push(0x00);
    payload
}

/// Applies a binary delta patch onto base bytes directly in memory.
fn apply_delta_patch_impl(
    base: &[u8],
    patch: &[u8],
    expected_hash: Option<&str>,
) -> Result<UniFFIDeltaPatchResult, UniFFISystemError> {
    let start_time = Instant::now();

    if patch.len() < TTZIP_DELTA_HEADER_LEN {
        return Err(UniFFISystemError::CorruptData {
            details: format!("Patch size {} is smaller than header length {}", patch.len(), TTZIP_DELTA_HEADER_LEN),
        });
    }

    if &patch[0..15] != TTZIP_DELTA_MAGIC {
        return Err(UniFFISystemError::CorruptData {
            details: "Invalid TTZip delta header magic".to_string(),
        });
    }

    let format_byte = patch[15];
    let target_size = u64::from_le_bytes(patch[16..24].try_into().unwrap()) as usize;
    let expected_base_sha256 = &patch[24..56];
    let expected_target_sha256 = &patch[56..88];

    let actual_base_sha256 = Sha256::digest(base);
    if actual_base_sha256.as_slice() != expected_base_sha256 {
        return Err(UniFFISystemError::patch_err("Base byte buffer SHA-256 mismatch; wrong base version"));
    }

    let encoded_payload = &patch[88..];
    let raw_payload = match format_byte {
        0 => encoded_payload.to_vec(),
        1 => uniffi_zstd_decompress(encoded_payload.to_vec(), None)
            .map_err(|e| UniFFISystemError::patch_err(format!("Zstd decompression failed: {e:?}")))?,
        2 => uniffi_deflate_decompress(encoded_payload.to_vec(), (target_size * 2 + 1024) as u64)
            .map_err(|e| UniFFISystemError::patch_err(format!("Flate decompression failed: {e:?}")))?,
        unknown => {
            return Err(UniFFISystemError::CorruptData {
                details: format!("Unsupported delta format identifier: {unknown}"),
            });
        }
    };

    let patched_bytes = execute_delta_opcodes(base, &raw_payload, target_size)?;
    let actual_target_sha256 = Sha256::digest(&patched_bytes);

    if actual_target_sha256.as_slice() != expected_target_sha256 {
        return Err(UniFFISystemError::patch_err(
            "Reconstructed target payload failed internal SHA-256 verification",
        ));
    }

    let hex_hash = hex_encode(actual_target_sha256.as_slice());

    if let Some(expected) = expected_hash {
        let clean_expected = expected.trim().trim_start_matches("sha256:").trim_start_matches("SHA256:");
        if !clean_expected.is_empty() && !clean_expected.eq_ignore_ascii_case(&hex_hash) {
            return Err(UniFFISystemError::verify_err(format!(
                "Target SHA-256 verification failed: expected {clean_expected}, got {hex_hash}"
            )));
        }
    }

    let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;

    Ok(UniFFIDeltaPatchResult {
        success: true,
        patch_size: patch.len() as u64,
        target_size: target_size as u64,
        target_hash: hex_hash,
        applied_in_memory: true,
        duration_ms,
        patched_bytes,
    })
}

/// Executes delta COPY, INSERT, and END opcodes against base bytes.
fn execute_delta_opcodes(
    base: &[u8],
    payload: &[u8],
    target_size: usize,
) -> Result<Vec<u8>, UniFFISystemError> {
    let mut out = Vec::with_capacity(target_size);
    let mut cursor = 0;

    while cursor < payload.len() {
        let op = payload[cursor];
        cursor += 1;
        match op {
            0x00 => break,
            0x01 => {
                if cursor + 16 > payload.len() {
                    return Err(UniFFISystemError::patch_err("Truncated COPY instruction"));
                }
                let offset = u64::from_le_bytes(payload[cursor..cursor + 8].try_into().unwrap()) as usize;
                let len = u64::from_le_bytes(payload[cursor + 8..cursor + 16].try_into().unwrap()) as usize;
                cursor += 16;
                if offset.checked_add(len).is_none_or(|end| end > base.len()) {
                    return Err(UniFFISystemError::patch_err("COPY offset out of bounds of base data"));
                }
                if out.len().checked_add(len).is_none_or(|end| end > target_size) {
                    return Err(UniFFISystemError::patch_err("COPY exceeds target capacity"));
                }
                out.extend_from_slice(&base[offset..offset + len]);
            }
            0x02 => {
                if cursor + 8 > payload.len() {
                    return Err(UniFFISystemError::patch_err("Truncated INSERT length header"));
                }
                let len = u64::from_le_bytes(payload[cursor..cursor + 8].try_into().unwrap()) as usize;
                cursor += 8;
                if cursor.checked_add(len).is_none_or(|end| end > payload.len()) {
                    return Err(UniFFISystemError::patch_err("Truncated INSERT literal data"));
                }
                if out.len().checked_add(len).is_none_or(|end| end > target_size) {
                    return Err(UniFFISystemError::patch_err("INSERT exceeds target capacity"));
                }
                out.extend_from_slice(&payload[cursor..cursor + len]);
                cursor += len;
            }
            other => {
                return Err(UniFFISystemError::patch_err(format!("Unknown delta opcode 0x{:02X}", other)));
            }
        }
    }

    if out.len() != target_size {
        return Err(UniFFISystemError::patch_err(format!(
            "Target size mismatch: expected {target_size} bytes, got {} bytes",
            out.len()
        )));
    }

    Ok(out)
}

/// Recursively computes deterministic cryptographic Merkle tree hash of a file or directory hierarchy.
fn calculate_tree_hash_impl(root_path: &str) -> Result<String, UniFFISystemError> {
    let p = Path::new(root_path);
    if !p.exists() {
        return Err(UniFFISystemError::io_err(format!("Path does not exist: {root_path}")));
    }

    if p.is_file() {
        let content = fs::read(p).map_err(|e| UniFFISystemError::io_err(e.to_string()))?;
        let digest = Sha256::digest(&content);
        return Ok(hex_encode(digest.as_slice()));
    }

    let mut entries: Vec<(String, bool, u64, Vec<u8>)> = Vec::new();
    collect_directory_entries(p, p, &mut entries)?;
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut tree_hasher = Sha256::new();
    for (rel_path, is_dir, size, content_hash) in entries {
        tree_hasher.update(b"PATH:");
        tree_hasher.update(rel_path.as_bytes());
        tree_hasher.update(b"\nKIND:");
        tree_hasher.update(if is_dir { b"DIR\n".as_slice() } else { b"FILE\n".as_slice() });
        tree_hasher.update(b"SIZE:");
        tree_hasher.update(size.to_string().as_bytes());
        tree_hasher.update(b"\nHASH:");
        tree_hasher.update(&content_hash);
        tree_hasher.update(b"\n");
    }

    let final_digest = tree_hasher.finalize();
    Ok(hex_encode(final_digest.as_slice()))
}

/// Helper to recursively collect deterministic directory entries.
fn collect_directory_entries(
    root: &Path,
    current: &Path,
    entries: &mut Vec<(String, bool, u64, Vec<u8>)>,
) -> Result<(), UniFFISystemError> {
    let read_dir = fs::read_dir(current).map_err(|e| UniFFISystemError::io_err(e.to_string()))?;
    for entry_res in read_dir {
        let entry = entry_res.map_err(|e| UniFFISystemError::io_err(e.to_string()))?;
        let path = entry.path();
        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        let file_type = entry
            .file_type()
            .map_err(|e| UniFFISystemError::io_err(e.to_string()))?;

        if file_type.is_dir() {
            entries.push((rel_path, true, 0, Vec::new()));
            collect_directory_entries(root, &path, entries)?;
        } else if file_type.is_file() {
            let metadata = entry
                .metadata()
                .map_err(|e| UniFFISystemError::io_err(e.to_string()))?;
            let size = metadata.len();
            let content = fs::read(&path).map_err(|e| UniFFISystemError::io_err(e.to_string()))?;
            let hash = Sha256::digest(&content).to_vec();
            entries.push((rel_path, false, size, hash));
        }
    }
    Ok(())
}

/// Verifies Ed25519 digital signature of byte payload against Base64 public key and signature.
fn verify_ed25519_signature_impl(
    data: &[u8],
    sig_b64: &str,
    pub_b64: &str,
) -> Result<bool, UniFFISystemError> {
    let pub_bytes = decode_base64_str(pub_b64).ok_or_else(|| UniFFISystemError::InvalidSignature {
        reason: "Invalid Base64 in public key".to_string(),
    })?;
    if pub_bytes.len() != PUBLIC_KEY_LENGTH {
        return Err(UniFFISystemError::InvalidSignature {
            reason: format!("Expected 32-byte public key, got {}", pub_bytes.len()),
        });
    }

    let sig_bytes = decode_base64_str(sig_b64).ok_or_else(|| UniFFISystemError::InvalidSignature {
        reason: "Invalid Base64 in signature".to_string(),
    })?;
    if sig_bytes.len() != SIGNATURE_LENGTH {
        return Err(UniFFISystemError::InvalidSignature {
            reason: format!("Expected 64-byte signature, got {}", sig_bytes.len()),
        });
    }

    let mut pk_arr = [0u8; PUBLIC_KEY_LENGTH];
    pk_arr.copy_from_slice(&pub_bytes);
    let verifying_key = VerifyingKey::from_bytes(&pk_arr).map_err(|e| UniFFISystemError::InvalidSignature {
        reason: format!("Invalid Ed25519 verifying key: {e:?}"),
    })?;

    let mut sig_arr = [0u8; SIGNATURE_LENGTH];
    sig_arr.copy_from_slice(&sig_bytes);
    let signature = Signature::from_bytes(&sig_arr);

    match verifying_key.verify_strict(data, &signature) {
        Ok(()) => Ok(true),
        Err(e) => Err(UniFFISystemError::InvalidSignature {
            reason: format!("Ed25519 verification failed: {e:?}"),
        }),
    }
}

/// Semantic version string comparison: returns -1 if a < b, 0 if a == b, 1 if a > b.
fn compare_semver(a: &str, b: &str) -> i32 {
    let parse_tokens = |s: &str| -> Vec<u64> {
        let clean = s.trim().trim_start_matches('v').trim_start_matches('V');
        clean
            .split(['.', '-', '+'])
            .filter_map(|part| part.parse::<u64>().ok())
            .collect()
    };

    let tokens_a = parse_tokens(a);
    let tokens_b = parse_tokens(b);

    let max_len = tokens_a.len().max(tokens_b.len());
    for i in 0..max_len {
        let val_a = tokens_a.get(i).copied().unwrap_or(0);
        let val_b = tokens_b.get(i).copied().unwrap_or(0);
        if val_a < val_b {
            return -1;
        } else if val_a > val_b {
            return 1;
        }
    }
    0
}

/// Encodes byte slice into lowercase hexadecimal string.
fn hex_encode(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 2);
    for &b in data {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

/// Zero-dependency Base64 decoding helper.
fn decode_base64_str(input: &str) -> Option<Vec<u8>> {
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
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
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
