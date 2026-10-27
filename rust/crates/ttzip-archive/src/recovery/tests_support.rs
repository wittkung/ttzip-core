// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Deterministic fixtures shared by the archive-layer confirmation gates.
//!
//! Every fixture is built from fixed plaintexts and fixed encryption parameters
//! through the engine's own crypto primitives and archive writers, so the archives
//! are legally openable and byte-stable across runs.

use crate::crypto::crc32::crc32_fast;
use crate::crypto::password_recovery::verify_zipcrypto_candidate;
use crate::crypto::sha256::sha256_7z_kdf;
use crate::crypto::zipcrypto::ZipCryptoEngine;
use crate::sevenz::format::{build_sig_header_bytes, METHOD_COPY};
use crate::sevenz::writer::create_7z_solid_archive_bytes_with_password;
use crate::zip::parser::parse_local_file_header;
use crate::zip::reader::ZipArchive;
use crate::zip::writer::assemble_zip_archive;
use crate::zip::writer::types::{ZipCompressedItem, ZipInputItem};

/// Correct password of the synthesized ZipCrypto fixture.
pub(crate) const FIXTURE_PASSWORD: &str = "mm37-correct-password";

/// Wrong password that satisfies the 12-byte ZipCrypto screening header check.
///
/// Derived once from the deterministic fixture and pinned as a literal. The test
/// re-derives it and asserts that the derivation still reproduces this exact value,
/// so the fixture cannot silently drift.
pub(crate) const FIXTURE_SCREEN_BAIT: &str = "mm37-bait-00597";

/// The 11 deterministic plaintext bytes of the ZipCrypto encryption header.
pub(crate) const FIXTURE_HEADER_RANDOM: [u8; 11] = [
    0x21, 0x43, 0x65, 0x87, 0xA9, 0xCB, 0xED, 0x0F, 0x31, 0x53, 0x75,
];

/// Number of deterministic bait candidates explored by the derivation.
pub(crate) const BAIT_SEARCH_BOUND: u32 = 65_536;

/// Deterministic salt of the synthesized 7z fixtures.
pub(crate) const FIXTURE_7Z_SALT: [u8; 16] = [
    0x10, 0x32, 0x54, 0x76, 0x98, 0xBA, 0xDC, 0xFE, 0x0F, 0x21, 0x43, 0x65, 0x87, 0xA9, 0xCB, 0xED,
];

/// Deterministic initialization vector of the synthesized 7z fixtures.
pub(crate) const FIXTURE_7Z_IV: [u8; 16] = [
    0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54, 0x32, 0x10,
];

/// Key derivation cycle exponent of the synthesized 7z fixtures.
pub(crate) const FIXTURE_7Z_CYCLES: u32 = 6;

/// Deterministic salt of the synthesized WinZip AES fixtures.
pub(crate) const FIXTURE_AES_SALT: [u8; 16] = [
    0x02, 0x24, 0x46, 0x68, 0x8A, 0xAC, 0xCE, 0xE0, 0xF2, 0x14, 0x36, 0x58, 0x7A, 0x9C, 0xBE, 0xD0,
];

/// Deterministic plaintext payload of the fixtures.
pub(crate) fn fixture_plaintext() -> Vec<u8> {
    (0u32..512).map(|i| (i % 251) as u8).collect()
}

/// Builds the deterministic encrypted ZipCrypto archive bytes.
///
/// The ZipCrypto header is produced by the crypto crate's own `ZipCryptoEngine`, and
/// the container is produced by the archive crate's own assembler, so the fixture is a
/// legally openable encrypted archive rather than a hand-rolled blob.
pub(crate) fn build_zipcrypto_fixture() -> Vec<u8> {
    build_zipcrypto_fixture_for(&fixture_plaintext())
}

/// Builds the deterministic encrypted ZipCrypto archive around the given plaintext.
pub(crate) fn build_zipcrypto_fixture_for(plaintext: &[u8]) -> Vec<u8> {
    let plaintext = plaintext.to_vec();
    let crc32 = crc32_fast(0, &plaintext);
    let check_byte = (crc32 >> 24) as u8;

    let mut engine = ZipCryptoEngine::new(FIXTURE_PASSWORD.as_bytes());
    let header = engine.generate_header(check_byte, &FIXTURE_HEADER_RANDOM);
    let mut body = plaintext.clone();
    engine.encrypt_slice(&mut body);

    let mut payload = header.to_vec();
    payload.extend_from_slice(&body);

    assemble_zip_archive(&[zipcrypto_item(&plaintext, crc32, payload)]).expect("assembly")
}

/// Builds the item of the deterministic ZipCrypto fixture.
pub(crate) fn zipcrypto_item(plaintext: &[u8], crc32: u32, payload: Vec<u8>) -> ZipCompressedItem {
    ZipCompressedItem {
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
    }
}

/// Builds a ZIP archive holding an ordinary entry in front of an encrypted one.
///
/// The ordinary entry comes first on purpose: a confirmation target that ignores the
/// encryption fact would pick it and accept every candidate.
pub(crate) fn build_mixed_zip_fixture() -> Vec<u8> {
    let plaintext = fixture_plaintext();
    let crc32 = crc32_fast(0, &plaintext);
    let check_byte = (crc32 >> 24) as u8;

    let mut engine = ZipCryptoEngine::new(FIXTURE_PASSWORD.as_bytes());
    let header = engine.generate_header(check_byte, &FIXTURE_HEADER_RANDOM);
    let mut body = plaintext.clone();
    engine.encrypt_slice(&mut body);
    let mut payload = header.to_vec();
    payload.extend_from_slice(&body);

    let ordinary = ZipCompressedItem {
        rel_path: "readme.txt".to_string(),
        uncompressed_size: 16,
        compressed_size: 16,
        crc32: crc32_fast(0, b"plain ordinary text"),
        compression_method: 0,
        actual_method: 0,
        aes_strength: 0,
        payload: b"plain ordinary text".to_vec(),
        mtime_epoch_secs: 1_700_000_000,
        mode: 0o644,
        is_directory: false,
        is_encrypted: false,
    };

    assemble_zip_archive(&[
        ordinary,
        zipcrypto_item(&plaintext, crc32, payload),
    ])
    .expect("mixed fixture assembly")
}

/// Builds a ZIP archive that holds no encrypted entry at all.
pub(crate) fn build_unencrypted_zip_fixture() -> Vec<u8> {
    assemble_zip_archive(&[ZipCompressedItem {
        rel_path: "readme.txt".to_string(),
        uncompressed_size: 16,
        compressed_size: 16,
        crc32: crc32_fast(0, b"plain ordinary text"),
        compression_method: 0,
        actual_method: 0,
        aes_strength: 0,
        payload: b"plain ordinary text".to_vec(),
        mtime_epoch_secs: 1_700_000_000,
        mode: 0o644,
        is_directory: false,
        is_encrypted: false,
    }])
    .expect("unencrypted fixture assembly")
}

/// Builds an AE-2 encrypted ZIP entry with the engine's own AE primitives.
///
/// AE-2 stores no CRC, so the entry carries `crc32 = 0` exactly like a real AE-2
/// archive. The credential of such an entry is its authentication code, never its CRC.
pub(crate) fn build_aes_entry(rel_path: &str, plaintext: &[u8]) -> ZipCompressedItem {
    let mut payload = Vec::new();
    crate::crypto::winzip_aes256_encrypt_and_tag(
        FIXTURE_PASSWORD,
        &FIXTURE_AES_SALT,
        plaintext,
        &mut payload,
    )
    .expect("AES encryption");
    ZipCompressedItem {
        rel_path: rel_path.to_string(),
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
    }
}

/// Builds the deterministic encrypted 7z fixture through the archive crate's writer.
pub(crate) fn build_sevenz_fixture() -> Vec<u8> {
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

/// Builds a deterministic encrypted 7z fixture with two entries in one solid folder.
///
/// The second entry can only be reached by decoding the first one, so it carries a
/// real preceding decode cost.
pub(crate) fn build_sevenz_two_entry_fixture() -> Vec<u8> {
    let items = vec![
        ZipInputItem {
            rel_path: "first.bin".to_string(),
            data: fixture_plaintext(),
            mtime_epoch_secs: 1_700_000_000,
            mode: 0o600,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "second.bin".to_string(),
            data: vec![0x5Au8; 64],
            mtime_epoch_secs: 1_700_000_000,
            mode: 0o600,
            is_directory: false,
        },
    ];
    create_7z_solid_archive_bytes_with_password(&items, 0, 1, Some(FIXTURE_PASSWORD))
        .expect("7z fixture creation")
}

/// Builds a 7z archive whose single stream carries no digest at all.
///
/// The 7z format treats the digest section as optional, so an archive without one is
/// legal. The payload is produced by the same primitives the engine's writer uses and
/// the header by the engine's own header builder, which is the only way to obtain a
/// genuinely digest-less encrypted 7z.
pub(crate) fn build_sevenz_fixture_without_digest() -> Vec<u8> {
    let plaintext = fixture_plaintext();
    let key = sha256_7z_kdf(FIXTURE_PASSWORD, &FIXTURE_7Z_SALT, FIXTURE_7Z_CYCLES);
    let padded_len = plaintext.len().div_ceil(16) * 16;
    let mut padded = plaintext.clone();
    padded.resize(padded_len, 0);
    let mut payload = vec![0u8; padded_len];
    crate::crypto::aes256::aes256_cbc_encrypt(&key, &FIXTURE_7Z_IV, &padded, &mut payload)
        .expect("7z payload encryption");

    let items = vec![ZipInputItem {
        rel_path: "secret.bin".to_string(),
        data: plaintext.clone(),
        mtime_epoch_secs: 1_700_000_000,
        mode: 0o600,
        is_directory: false,
    }];
    let aes_config = crate::sevenz::format::SevenZAesWriterConfig {
        salt: FIXTURE_7Z_SALT,
        iv: FIXTURE_7Z_IV,
        num_cycles_power: FIXTURE_7Z_CYCLES,
        raw_comp_len: plaintext.len() as u64,
    };
    let header = crate::sevenz::writer::build_7z_metadata_header_full(
        &items,
        &[plaintext.len() as u64],
        &[],
        payload.len() as u64,
        plaintext.len() as u64,
        METHOD_COPY,
        &[],
        Some(&aes_config),
    );
    let sig = build_sig_header_bytes(payload.len() as u64, &header);

    let mut out = Vec::new();
    out.extend_from_slice(&sig);
    out.extend_from_slice(&payload);
    out.extend_from_slice(&header);
    out
}

/// Extracts the screening material of a ZipCrypto fixture the way the crypto crate
/// expects it, so the screening stage and the confirmation describe the same unit.
pub(crate) fn zipcrypto_screen_material(zip_bytes: &[u8]) -> ([u8; 12], u8) {
    let archive = ZipArchive::open_slice(zip_bytes).expect("fixture must open");
    let entry = &archive.entries()[0];
    let (payload_offset, _) =
        parse_local_file_header(zip_bytes, entry.lfh_offset as usize).expect("LFH must parse");
    let comp_size = entry.compressed_size as usize;
    let raw_payload = &zip_bytes[payload_offset..payload_offset + comp_size];
    let header: [u8; 12] = raw_payload[..12].try_into().expect("header must fit");
    (header, (entry.crc32 >> 24) as u8)
}

/// Derives the first wrong password, in a fixed candidate order, that passes the screen.
pub(crate) fn derive_screen_bait(header: &[u8; 12], check_byte: u8) -> String {
    for i in 0..BAIT_SEARCH_BOUND {
        let candidate = format!("mm37-bait-{i:05}");
        if verify_zipcrypto_candidate(&candidate, header, check_byte) {
            return candidate;
        }
    }
    panic!("no screening-bait candidate found within the deterministic bound")
}


/// Synthesizes a single stored AE-2 entry through the general crypto primitive.
/// The archive assembler always emits strength 3, so this fixture writes the small
/// standard LFH/CDFH/EOCD records explicitly and supplies the actual strength in both
/// 0x9901 extras. No malformed metadata stands in for an unsupported algorithm.
pub(crate) fn build_aes_strength_zip_fixture(
    strength: crate::crypto::winzip_aes::WinZipAesKeyStrength,
    plaintext: &[u8],
) -> Vec<u8> {
    use crate::crypto::winzip_aes::{
        WinZipAesVersion, winzip_aes_decrypt_payload, winzip_aes_encrypt_payload,
    };
    let salt = &FIXTURE_AES_SALT[..strength.salt_len()];
    let payload = winzip_aes_encrypt_payload(
        WinZipAesVersion::AE2,
        strength,
        FIXTURE_PASSWORD,
        salt,
        plaintext,
    )
    .expect("valid AE-2 payload");
    let (decoded, _) = winzip_aes_decrypt_payload(
        WinZipAesVersion::AE2,
        strength,
        FIXTURE_PASSWORD,
        &payload,
        None,
    )
    .expect("the declared strength and credential must actually decrypt");
    assert_eq!(decoded, plaintext);
    assert!(
        winzip_aes_decrypt_payload(
            WinZipAesVersion::AE2,
            strength,
            "unrelated-password",
            &payload,
            None,
        )
        .is_err()
    );

    let name = b"strength.bin";
    let extra = [0x01, 0x99, 7, 0, 2, 0, b'A', b'E', strength as u8, 0, 0];
    let mut zip = Vec::new();
    zip.extend_from_slice(&0x04034b50u32.to_le_bytes());
    for field in [51u16, 0x0801, 99, 0, 0] {
        zip.extend_from_slice(&field.to_le_bytes());
    }
    for field in [0u32, payload.len() as u32, plaintext.len() as u32] {
        zip.extend_from_slice(&field.to_le_bytes());
    }
    for field in [name.len() as u16, extra.len() as u16] {
        zip.extend_from_slice(&field.to_le_bytes());
    }
    zip.extend_from_slice(name);
    zip.extend_from_slice(&extra);
    zip.extend_from_slice(&payload);
    let cd_offset = zip.len() as u32;
    zip.extend_from_slice(&0x02014b50u32.to_le_bytes());
    for field in [0x0333u16, 51, 0x0801, 99, 0, 0] {
        zip.extend_from_slice(&field.to_le_bytes());
    }
    for field in [0u32, payload.len() as u32, plaintext.len() as u32] {
        zip.extend_from_slice(&field.to_le_bytes());
    }
    for field in [name.len() as u16, extra.len() as u16, 0, 0, 0] {
        zip.extend_from_slice(&field.to_le_bytes());
    }
    zip.extend_from_slice(&0u32.to_le_bytes()); // External attributes.
    zip.extend_from_slice(&0u32.to_le_bytes()); // Local header offset.
    zip.extend_from_slice(name);
    zip.extend_from_slice(&extra);
    let cd_size = zip.len() as u32 - cd_offset;
    zip.extend_from_slice(&0x06054b50u32.to_le_bytes());
    for field in [0u16, 0, 1, 1] {
        zip.extend_from_slice(&field.to_le_bytes());
    }
    zip.extend_from_slice(&cd_size.to_le_bytes());
    zip.extend_from_slice(&cd_offset.to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes());
    zip
}
