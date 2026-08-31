// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Sub-millisecond Ed25519 Cryptographic Engine (RFC 8032 / Zip215).
//!
//! Provides deterministic signing, strict constant-time verification, 128-bit
//! scalar-folded batch verification, binary search fault isolation, and 3-tier
//! PKI certificate chain hierarchy for secure `.ttplugin` package distribution.

pub mod batch;
pub mod error;
pub mod plugin_auth;
pub mod signing;
pub mod verifying;

pub use batch::{locate_faulty_signatures, BatchItem, BatchVerifier};
pub use error::Ed25519Error;
pub use plugin_auth::{
    base64_encode, compute_key_fingerprint, compute_key_fingerprint_blake3,
    compute_key_fingerprint_sha256, CertificateChain, CertificateLevel, FingerprintAlgorithm,
    PluginManifestAuth, TTZipCertificate,
};
pub use signing::{
    derive_deterministic_nonce, expand_secret_key, scalar_clamp, sign, SigningKey,
    SECRET_KEY_LENGTH,
};
pub use verifying::{
    verify, Signature, VerifyingKey, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH,
};

#[cfg(test)]
mod tests {
    use super::*;
    use zeroize::Zeroize;

    fn hex_to_bytes(hex: &str) -> Vec<u8> {
        let clean = hex.trim();
        (0..clean.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&clean[i..i + 2], 16).expect("Valid hex string"))
            .collect()
    }

    #[test]
    fn test_rfc8032_official_vector_1() {
        let secret_seed = hex_to_bytes("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
        let expected_public = hex_to_bytes("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a");
        let expected_sig = hex_to_bytes("e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b");

        let signing_key = SigningKey::from_slice(&secret_seed).unwrap();
        let verifying_key = signing_key.verifying_key();
        assert_eq!(verifying_key.as_bytes(), expected_public.as_slice());

        let msg = b"";
        let sig = signing_key.sign(msg);
        assert_eq!(sig.as_bytes(), expected_sig.as_slice());

        assert!(verifying_key.verify(msg, &sig).is_ok());
        assert!(verifying_key.verify_strict(msg, &sig).is_ok());
    }

    #[test]
    fn test_rfc8032_official_vector_2() {
        let secret_seed = hex_to_bytes("4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb");
        let expected_public = hex_to_bytes("3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c");
        let expected_sig = hex_to_bytes("92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00");

        let signing_key = SigningKey::from_slice(&secret_seed).unwrap();
        let verifying_key = signing_key.verifying_key();
        assert_eq!(verifying_key.as_bytes(), expected_public.as_slice());

        let msg = b"\x72";
        let sig = signing_key.sign(msg);
        assert_eq!(sig.as_bytes(), expected_sig.as_slice());

        assert!(verifying_key.verify(msg, &sig).is_ok());
    }

    #[test]
    fn test_rfc8032_official_vector_3() {
        let secret_seed = hex_to_bytes("c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7");
        let expected_public = hex_to_bytes("fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025");
        let expected_sig = hex_to_bytes("6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a");

        let signing_key = SigningKey::from_slice(&secret_seed).unwrap();
        let verifying_key = signing_key.verifying_key();
        assert_eq!(verifying_key.as_bytes(), expected_public.as_slice());

        let msg = b"\xaf\x82";
        let sig = signing_key.sign(msg);
        assert_eq!(sig.as_bytes(), expected_sig.as_slice());

        assert!(verifying_key.verify(msg, &sig).is_ok());
    }

    #[test]
    fn test_scalar_clamping_and_zeroize() {
        let mut raw_bytes = [0xffu8; 32];
        scalar_clamp(&mut raw_bytes);
        assert_eq!(raw_bytes[0] & 0b0000_0111, 0);
        assert_eq!(raw_bytes[31] & 0b1000_0000, 0);
        assert_eq!(raw_bytes[31] & 0b0100_0000, 0b0100_0000);

        raw_bytes.zeroize();
        assert_eq!(raw_bytes, [0u8; 32]);
    }

    #[test]
    fn test_strict_verification_rejects_non_canonical_scalar() {
        let seed = [42u8; 32];
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();
        let msg = b"TTZip Strict Verification Test";
        let sig = signing_key.sign(msg);

        // Valid signature passes
        assert!(verifying_key.verify(msg, &sig).is_ok());

        // Construct non-canonical S (>= curve order \ell = 2^252 + 27742317777372353535851937790883648493)
        let mut non_canonical_sig_bytes = sig.to_bytes();
        // Setting high bytes of S to exceed curve order \ell
        non_canonical_sig_bytes[63] = 0xFF;
        let non_canonical_sig = Signature::from_bytes(&non_canonical_sig_bytes);

        assert_eq!(
            verifying_key.verify(msg, &non_canonical_sig),
            Err(Ed25519Error::NonCanonicalScalar)
        );
    }

    #[test]
    fn test_batch_verification_success() {
        let mut verifier = BatchVerifier::new();
        let mut keys = Vec::new();
        let mut sigs = Vec::new();
        let messages: Vec<&[u8]> = vec![
            b"Batch Item 1: Archive Segment A",
            b"Batch Item 2: Core Microkernel Chunk B",
            b"Batch Item 3: Stream Pipeline Block C",
            b"Batch Item 4: Blake3 Verified Node D",
        ];

        for i in 0..messages.len() {
            let seed = [i as u8 + 10; 32];
            let sk = SigningKey::from_bytes(&seed);
            let vk = sk.verifying_key();
            let sig = sk.sign(messages[i]);
            keys.push(vk);
            sigs.push(sig);
        }

        for i in 0..messages.len() {
            verifier.add(&keys[i], messages[i], &sigs[i]);
        }

        assert_eq!(verifier.len(), 4);
        assert!(verifier.verify().is_ok());
    }

    #[test]
    fn test_batch_verification_locate_faulty_signatures() {
        let mut keys = Vec::new();
        let mut sigs = Vec::new();
        let messages: Vec<&[u8]> = vec![
            b"Block 0: Valid",
            b"Block 1: Tampered Msg",
            b"Block 2: Valid",
            b"Block 3: Tampered Sig",
            b"Block 4: Valid",
        ];

        for i in 0..messages.len() {
            let seed = [i as u8 + 50; 32];
            let sk = SigningKey::from_bytes(&seed);
            let vk = sk.verifying_key();
            let sig = sk.sign(messages[i]);
            keys.push(vk);
            sigs.push(sig);
        }

        // Tamper signature 1 by modifying message passed to verifier
        let mut tampered_messages = messages.clone();
        tampered_messages[1] = b"Block 1: CORRUPTED DATA";

        // Tamper signature 3 by corrupting signature byte
        let mut corrupted_sig3_bytes = sigs[3].to_bytes();
        corrupted_sig3_bytes[10] ^= 0x55;
        sigs[3] = Signature::from_bytes(&corrupted_sig3_bytes);

        // Batch verifier should fail
        let mut verifier = BatchVerifier::new();
        for i in 0..keys.len() {
            verifier.add(&keys[i], tampered_messages[i], &sigs[i]);
        }
        assert!(verifier.verify().is_err());

        // Fault locator must isolate exactly indices [1, 3]
        let faulty = locate_faulty_signatures(&keys, &tampered_messages, &sigs);
        assert_eq!(faulty, vec![1, 3]);
    }

    #[test]
    fn test_three_tier_certificate_chain_full_lifecycle() {
        let current_time = 1_772_000_000u64;

        // 1. Root Tier
        let root_sk = SigningKey::from_bytes(&[1u8; 32]);
        let root_vk = root_sk.verifying_key();
        let root_cert = TTZipCertificate::issue(
            CertificateLevel::Root,
            "TTZip-Root-CA".to_string(),
            "TTZip-Root-CA".to_string(),
            root_vk.to_bytes(),
            current_time - 1000,
            current_time + 100000,
            &root_sk,
        );

        // 2. Marketplace Tier
        let market_sk = SigningKey::from_bytes(&[2u8; 32]);
        let market_vk = market_sk.verifying_key();
        let market_cert = TTZipCertificate::issue(
            CertificateLevel::Marketplace,
            "TTZip-Root-CA".to_string(),
            "TTZip-Official-Marketplace".to_string(),
            market_vk.to_bytes(),
            current_time - 500,
            current_time + 50000,
            &root_sk,
        );

        // 3. Developer Tier
        let dev_sk = SigningKey::from_bytes(&[3u8; 32]);
        let dev_vk = dev_sk.verifying_key();
        let dev_cert = TTZipCertificate::issue(
            CertificateLevel::Developer,
            "TTZip-Official-Marketplace".to_string(),
            "DEV-WITT-KUNG-001".to_string(),
            dev_vk.to_bytes(),
            current_time - 100,
            current_time + 20000,
            &market_sk,
        );

        let chain = CertificateChain::new(root_cert.clone(), market_cert.clone(), dev_cert.clone());
        let verified_dev_key = chain.verify_chain(&root_vk, current_time).unwrap();
        assert_eq!(verified_dev_key.to_bytes(), dev_vk.to_bytes());

        // 4. Plugin Manifest Authorization
        let manifest = PluginManifestAuth {
            plugin_id: "com.wittkung.larksync".to_string(),
            version: "1.0.1".to_string(),
            developer_id: "DEV-WITT-KUNG-001".to_string(),
            permissions: vec!["keychain".to_string(), "compression".to_string()],
            binary_sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            resources_sha256: None,
            issued_at: current_time - 10,
            valid_until: current_time + 5000,
        };

        let manifest_sig = manifest.sign_manifest(&dev_sk);
        assert!(chain.verify_plugin_manifest(&manifest, &manifest_sig, &root_vk, current_time).is_ok());

        // Tamper test: Manifest developer mismatch
        let mut malicious_manifest = manifest.clone();
        malicious_manifest.developer_id = "DEV-ATTACKER-999".to_string();
        let malicious_sig = malicious_manifest.sign_manifest(&dev_sk);
        assert!(chain.verify_plugin_manifest(&malicious_manifest, &malicious_sig, &root_vk, current_time).is_err());

        // Tamper test: Broken certificate chain issuer
        let mut broken_market_cert = market_cert.clone();
        broken_market_cert.issuer_id = "Untrusted-Rogue-CA".to_string();
        let broken_chain = CertificateChain::new(root_cert, broken_market_cert, dev_cert);
        assert!(broken_chain.verify_chain(&root_vk, current_time).is_err());
    }

    #[test]
    fn test_public_key_fingerprint_generation() {
        let seed = [7u8; 32];
        let sk = SigningKey::from_bytes(&seed);
        let vk = sk.verifying_key();
        let vk_bytes = vk.to_bytes();

        let sha256_fp = compute_key_fingerprint(&vk_bytes, FingerprintAlgorithm::Sha256);
        assert!(sha256_fp.starts_with("SHA256:"));

        let blake3_fp = compute_key_fingerprint(&vk_bytes, FingerprintAlgorithm::Blake3);
        assert!(blake3_fp.starts_with("BLAKE3:"));

        assert_ne!(sha256_fp, blake3_fp);
    }
}
