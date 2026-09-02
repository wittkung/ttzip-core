// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI System Metadata, Delta Patching, and Appcast Update Module.

mod service;
mod types;

pub use service::{
    uniffi_apply_delta_patch, uniffi_calculate_tree_hash, uniffi_create_delta_patch,
    uniffi_verify_appcast_signature, UniFFISystemService, TTZIP_DELTA_HEADER_LEN, TTZIP_DELTA_MAGIC,
};
pub use types::{
    UniFFIAppcastItem, UniFFIAppcastMetadata, UniFFIDeltaFormat, UniFFIDeltaPatchResult,
    UniFFISystemError,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::ed25519::signing::SigningKey;
    use std::fs;

    #[test]
    fn test_delta_patch_roundtrip_all_formats() {
        let base_data = b"Hello World! This is the TTZip baseline binary package v1.0.0. Features include ultra-fast decompression, VFS, and robust safety invariants.".to_vec();
        let target_data = b"Hello World! This is the TTZip baseline binary package v1.1.0 with new delta engine updates! Features include ultra-fast decompression, VFS, and robust safety invariants and new Appcast support.".to_vec();

        let formats = [
            UniFFIDeltaFormat::RawByteBlock,
            UniFFIDeltaFormat::ZstdCompressed,
            UniFFIDeltaFormat::FlateCompressed,
        ];

        for &fmt in &formats {
            let patch = uniffi_create_delta_patch(base_data.clone(), target_data.clone(), fmt)
                .expect("Failed to create delta patch");
            assert!(patch.len() >= TTZIP_DELTA_HEADER_LEN);

            let res = uniffi_apply_delta_patch(base_data.clone(), patch.clone(), None)
                .expect("Failed to apply delta patch");
            assert!(res.success);
            assert_eq!(res.patched_bytes, target_data);
            assert_eq!(res.target_size, target_data.len() as u64);
            assert!(res.applied_in_memory);

            // Test hash verification
            let res_with_hash = uniffi_apply_delta_patch(
                base_data.clone(),
                patch,
                Some(res.target_hash.clone()),
            )
            .expect("Failed with matching hash");
            assert_eq!(res_with_hash.patched_bytes, target_data);
        }
    }

    #[test]
    fn test_delta_patch_wrong_base_rejection() {
        let base_data = b"Original baseline data A".to_vec();
        let target_data = b"Original baseline data B with modification".to_vec();
        let patch = uniffi_create_delta_patch(base_data, target_data, UniFFIDeltaFormat::RawByteBlock).unwrap();

        let wrong_base = b"Wrong baseline data entirely".to_vec();
        let err = uniffi_apply_delta_patch(wrong_base, patch, None);
        assert!(err.is_err());
    }

    #[test]
    fn test_tree_hash_calculation() {
        let temp_dir = std::env::temp_dir().join(format!("ttzip_tree_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("sub")).unwrap();

        fs::write(temp_dir.join("file1.txt"), b"File 1 content").unwrap();
        fs::write(temp_dir.join("sub").join("file2.txt"), b"File 2 content in subdir").unwrap();

        let hash1 = uniffi_calculate_tree_hash(temp_dir.to_string_lossy().to_string()).unwrap();
        assert_eq!(hash1.len(), 64);

        // Determinism test
        let hash2 = uniffi_calculate_tree_hash(temp_dir.to_string_lossy().to_string()).unwrap();
        assert_eq!(hash1, hash2);

        // Mutation test
        fs::write(temp_dir.join("file1.txt"), b"File 1 mutated").unwrap();
        let hash3 = uniffi_calculate_tree_hash(temp_dir.to_string_lossy().to_string()).unwrap();
        assert_ne!(hash1, hash3);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_appcast_signature_verification() {
        let signing_key = SigningKey::from_bytes(&[42u8; 32]);
        let verifying_key = signing_key.verifying_key();

        let appcast_content = br#"{
            "channel": "stable",
            "title": "TTZip macOS",
            "feed_url": "https://ttzip.io/appcast.json",
            "latest_version": "1.2.0",
            "latest_build": 10200,
            "items": [],
            "signature_valid": true,
            "checked_at_epoch_secs": 1740000000
        }"#;

        let sig = signing_key.sign(appcast_content);

        // Helper to base64 encode
        let pub_b64 = to_base64_string(verifying_key.as_bytes());
        let sig_b64 = to_base64_string(&sig.to_bytes());

        let ok = uniffi_verify_appcast_signature(appcast_content.to_vec(), sig_b64.clone(), pub_b64.clone()).unwrap();
        assert!(ok);

        let tampered_err = uniffi_verify_appcast_signature(b"Tampered".to_vec(), sig_b64, pub_b64);
        assert!(tampered_err.is_err());
    }

    #[test]
    fn test_system_service_facade_and_monotonicity() {
        let service = UniFFISystemService::new();
        assert!(service.check_version_monotonicity("1.0.0".into(), "1.1.0".into()).unwrap());
        assert!(service.check_version_monotonicity("1.2.3".into(), "1.2.3".into()).unwrap());
        assert!(service.check_version_monotonicity("1.2.0".into(), "1.1.9".into()).is_err());

        let json_data = r#"{
            "channel": "stable",
            "title": "TTZip Desktop",
            "feed_url": "https://updates.ttzip.io/appcast.json",
            "latest_version": "2.0.0",
            "latest_build": 20000,
            "items": [
                {
                    "version": "2.0.0",
                    "build_number": 20000,
                    "min_os_version": "14.0",
                    "release_notes_url": "https://ttzip.io/notes/2.0.0",
                    "download_url": "https://dl.ttzip.io/TTZip-2.0.0.dmg",
                    "download_size": 25000000,
                    "signature_ed25519": "sig_placeholder",
                    "sha256": "abcdef",
                    "delta_patch_url": null,
                    "delta_base_version": null,
                    "delta_signature_ed25519": null,
                    "delta_size": null,
                    "is_critical": true,
                    "published_at_epoch_secs": 1740000000
                }
            ],
            "signature_valid": true,
            "checked_at_epoch_secs": 1740000000
        }"#;

        let meta = service.parse_appcast_json(json_data.to_string()).unwrap();
        assert_eq!(meta.latest_version, "2.0.0");
        assert_eq!(meta.items.len(), 1);
        assert!(meta.items[0].is_critical);
    }

    fn to_base64_string(data: &[u8]) -> String {
        const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        let mut i = 0;
        while i < data.len() {
            let b0 = data[i] as u32;
            let b1 = if i + 1 < data.len() { data[i + 1] as u32 } else { 0 };
            let b2 = if i + 2 < data.len() { data[i + 2] as u32 } else { 0 };
            let n = (b0 << 16) | (b1 << 8) | b2;
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
}
