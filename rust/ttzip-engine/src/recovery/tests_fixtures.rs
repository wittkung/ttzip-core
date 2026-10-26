// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Deterministic fixtures for the engine-level recovery gates.
//!
//! Every fixture is built from fixed plaintexts and fixed encryption parameters
//! through the engine's own crypto primitives and the archive crate's writers, so
//! the archives are legally openable and byte-stable across runs.

use ttzip_archive::sevenz::writer::create_7z_solid_archive_bytes_with_password;
use ttzip_archive::zip::writer::assemble_zip_archive;
use ttzip_archive::zip::writer::types::{ZipCompressedItem, ZipInputItem};
use ttzip_crypto::crc32::crc32_fast;
use ttzip_crypto::sha1::winzip::winzip_aes256_encrypt_and_tag;
use ttzip_crypto::zipcrypto::ZipCryptoEngine;

/// Correct password of the synthesized fixtures.
pub const FIXTURE_PASSWORD: &str = "mm37-correct-password";

/// Wrong password that satisfies the ZipCrypto screening header check.
///
/// Derived once from the deterministic fixture and pinned as a literal. The gate
/// re-derives it and asserts that the derivation still reproduces this exact
/// value, so the fixture cannot silently drift.
pub const FIXTURE_SCREEN_BAIT: &str = "mm37-bait-00597";

/// The 11 deterministic plaintext bytes of the ZipCrypto encryption header.
pub const FIXTURE_HEADER_RANDOM: [u8; 11] = [
    0x21, 0x43, 0x65, 0x87, 0xA9, 0xCB, 0xED, 0x0F, 0x31, 0x53, 0x75,
];

/// Deterministic salt of the WinZip AES fixture.
pub const FIXTURE_AES_SALT: [u8; 16] = [
    0x02, 0x24, 0x46, 0x68, 0x8A, 0xAC, 0xCE, 0xE0, 0xF2, 0x14, 0x36, 0x58, 0x7A, 0x9C, 0xBE, 0xD0,
];

/// Wrong password that satisfies the WinZip AES screening check.
///
/// The WinZip AES screen compares a two-byte password verification value, so a
/// wrong password collides with the right one only once in 65,536 tries. The
/// collision was derived once against the fixed salt above and is pinned here; the
/// gate verifies it with a single key derivation instead of repeating the search.
pub const FIXTURE_AES_BAIT: &str = "mm37-aes-bait-038123";

/// Number of deterministic bait candidates explored by the derivation.
pub const BAIT_SEARCH_BOUND: u32 = 65_536;

/// Deterministic plaintext payload of the fixtures.
pub fn fixture_plaintext() -> Vec<u8> {
    (0u32..512).map(|i| (i % 251) as u8).collect()
}

/// Builds the deterministic encrypted ZipCrypto archive bytes.
pub fn build_zipcrypto_fixture() -> Vec<u8> {
    build_zipcrypto_fixture_with_password(FIXTURE_PASSWORD)
}

/// Builds the same owned fixture with a small brute-force test password.
pub fn build_zipcrypto_fixture_with_password(password: &str) -> Vec<u8> {
    let plaintext = fixture_plaintext();
    let crc32 = crc32_fast(0, &plaintext);
    let check_byte = (crc32 >> 24) as u8;

    let mut engine = ZipCryptoEngine::new(password.as_bytes());
    let header = engine.generate_header(check_byte, &FIXTURE_HEADER_RANDOM);
    let mut body = plaintext.clone();
    engine.encrypt_slice(&mut body);
    let mut payload = header.to_vec();
    payload.extend_from_slice(&body);

    assemble_zip_archive(&[ZipCompressedItem {
        rel_path: "payload.bin".to_string(),
        uncompressed_size: plaintext.len() as u64,
        compressed_size: payload.len() as u64,
        crc32,
        compression_method: 0,
        actual_method: 0,
        aes_strength: 0,
        payload,
        mtime_epoch_secs: 1_700_000_000,
        mode: 0o600,
        is_directory: false,
        is_encrypted: true,
    }])
    .expect("zip fixture assembly")
}

/// Builds the deterministic AE-2 encrypted archive bytes.
///
/// AE-2 stores no CRC, so the entry carries `crc32 = 0` exactly like a real AE-2
/// archive and its credential is the authentication code.
pub fn build_aes_fixture() -> Vec<u8> {
    let plaintext = fixture_plaintext();
    let mut payload = Vec::new();
    winzip_aes256_encrypt_and_tag(
        FIXTURE_PASSWORD,
        &FIXTURE_AES_SALT,
        &plaintext,
        &mut payload,
    )
    .expect("aes fixture encryption");

    assemble_zip_archive(&[ZipCompressedItem {
        rel_path: "secret.bin".to_string(),
        uncompressed_size: plaintext.len() as u64,
        compressed_size: payload.len() as u64,
        crc32: 0,
        compression_method: 99,
        actual_method: 0,
        aes_strength: 3,
        payload,
        mtime_epoch_secs: 1_700_000_000,
        mode: 0o600,
        is_directory: false,
        is_encrypted: true,
    }])
    .expect("aes fixture assembly")
}

/// Builds the deterministic encrypted 7z archive bytes.
pub fn build_sevenz_fixture() -> Vec<u8> {
    let items = vec![ZipInputItem {
        rel_path: "secret.bin".to_string(),
        data: fixture_plaintext(),
        mtime_epoch_secs: 1_700_000_000,
        mode: 0o600,
        is_directory: false,
    }];
    create_7z_solid_archive_bytes_with_password(&items, 0, 1, Some(FIXTURE_PASSWORD))
        .expect("7z fixture creation")
}

/// Builds a ZIP archive whose only encrypted entry has no content and therefore no
/// integrity credential.
pub fn build_credential_less_fixture() -> Vec<u8> {
    let mut engine = ZipCryptoEngine::new(FIXTURE_PASSWORD.as_bytes());
    let header = engine.generate_header(0, &FIXTURE_HEADER_RANDOM);
    assemble_zip_archive(&[ZipCompressedItem {
        rel_path: "empty.bin".to_string(),
        uncompressed_size: 0,
        compressed_size: header.len() as u64,
        crc32: 0,
        compression_method: 0,
        actual_method: 0,
        aes_strength: 0,
        payload: header.to_vec(),
        mtime_epoch_secs: 1_700_000_000,
        mode: 0o600,
        is_directory: false,
        is_encrypted: true,
    }])
    .expect("credential-less fixture assembly")
}

/// Builds a ZIP archive whose only encrypted entry uses a compression method the
/// engine cannot decode.
pub fn build_unsupported_fixture() -> Vec<u8> {
    let plaintext = fixture_plaintext();
    let crc32 = crc32_fast(0, &plaintext);
    let check_byte = (crc32 >> 24) as u8;

    let mut engine = ZipCryptoEngine::new(FIXTURE_PASSWORD.as_bytes());
    let header = engine.generate_header(check_byte, &FIXTURE_HEADER_RANDOM);
    let mut body = plaintext.clone();
    engine.encrypt_slice(&mut body);
    let mut payload = header.to_vec();
    payload.extend_from_slice(&body);

    assemble_zip_archive(&[ZipCompressedItem {
        rel_path: "unsupported.bin".to_string(),
        uncompressed_size: plaintext.len() as u64,
        compressed_size: payload.len() as u64,
        crc32,
        compression_method: 14,
        actual_method: 14,
        aes_strength: 0,
        payload,
        mtime_epoch_secs: 1_700_000_000,
        mode: 0o600,
        is_directory: false,
        is_encrypted: true,
    }])
    .expect("unsupported fixture assembly")
}

/// Builds a ZIP archive that holds no encrypted entry at all.
pub fn build_unencrypted_fixture() -> Vec<u8> {
    assemble_zip_archive(&[ZipCompressedItem {
        rel_path: "readme.txt".to_string(),
        uncompressed_size: 5,
        compressed_size: 5,
        crc32: crc32_fast(0, b"plain"),
        compression_method: 0,
        actual_method: 0,
        aes_strength: 0,
        payload: b"plain".to_vec(),
        mtime_epoch_secs: 1_700_000_000,
        mode: 0o644,
        is_directory: false,
        is_encrypted: false,
    }])
    .expect("unencrypted fixture assembly")
}

/// Derives the first wrong password, in a fixed candidate order, that passes the
/// ZipCrypto screening check.
pub fn derive_screen_bait(header: &[u8; 12], check_byte: u8) -> String {
    for i in 0..BAIT_SEARCH_BOUND {
        let candidate = format!("mm37-bait-{i:05}");
        if ttzip_crypto::password_recovery::verify_zipcrypto_candidate(
            &candidate, header, check_byte,
        ) {
            return candidate;
        }
    }
    panic!("no screening-bait candidate found within the deterministic bound")
}
