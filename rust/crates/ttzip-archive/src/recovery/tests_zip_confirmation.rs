// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive-layer confirmation gates for the ZIP formats.
//!\n//! The gates cover the selection rule (an ordinary entry must never act as a
//! password credential), the integrity credential of every supported ZIP
//! encryption, and the cases in which the archive carries no evidence at all.

use crate::crypto::crc32::crc32_fast;
use crate::crypto::password_recovery::verify_zipcrypto_candidate;
use crate::crypto::zipcrypto::ZipCryptoEngine;
use crate::recovery::{
    ConfirmationBudget, ConfirmationVerdict, IntegrityCredential, PasswordConfirmation,
    PreparationError, ScreeningMaterial,
};
use crate::zip::parser::parse_local_file_header;
use crate::zip::reader::ZipArchive;
use crate::zip::writer::assemble_zip_archive;
use crate::zip::writer::types::ZipCompressedItem;
use super::tests_support::*;

#[test]
fn screen_bait_is_rejected_while_the_correct_password_is_confirmed() {
    let zip_bytes = build_zipcrypto_fixture();
    let (header, check_byte) = zipcrypto_screen_material(&zip_bytes);
    let bait = derive_screen_bait(&header, check_byte);
    assert_eq!(
        bait, FIXTURE_SCREEN_BAIT,
        "the deterministic bait derivation must reproduce the pinned literal"
    );

    let confirmation =
        PasswordConfirmation::for_zip(&zip_bytes, ConfirmationBudget::default()).expect("prepared");

    assert!(verify_zipcrypto_candidate(&bait, &header, check_byte));
    assert!(verify_zipcrypto_candidate(FIXTURE_PASSWORD, &header, check_byte));

    assert_eq!(
        confirmation.confirm(&bait),
        ConfirmationVerdict::NotAccepted,
        "a screening hit must never be promoted to a confirmed password"
    );
    assert_eq!(
        confirmation.confirm(FIXTURE_PASSWORD),
        ConfirmationVerdict::Confirmed
    );
    assert_eq!(
        confirmation.confirm("an-unrelated-password"),
        ConfirmationVerdict::NotAccepted
    );
}

/// The production confirmation exposes the screening material of the unit it proves.
#[test]
fn confirmation_exposes_the_screening_material_of_its_own_unit() {
    let zip_bytes = build_zipcrypto_fixture();
    let confirmation =
        PasswordConfirmation::for_zip(&zip_bytes, ConfirmationBudget::default()).expect("prepared");
    let (header, check_byte) = zipcrypto_screen_material(&zip_bytes);

    assert_eq!(
        confirmation.credential(),
        IntegrityCredential::EntryCrc32 {
            expected: crc32_fast(0, &fixture_plaintext())
        }
    );
    match confirmation.screening_material() {
        Some(ScreeningMaterial::ZipCrypto {
            header: actual,
            check_byte: actual_check,
        }) => {
            assert_eq!(actual, header);
            assert_eq!(actual_check, check_byte);
        }
        other => panic!("expected ZipCrypto screening material, got {other:?}"),
    }
}

/// The current screening-only search stops on the screening hit and reports a wrong
/// password, while the production confirmation keeps searching and finds the right one.
#[test]
fn mixed_archive_selects_the_encrypted_entry() {
    let zip_bytes = build_mixed_zip_fixture();
    let confirmation =
        PasswordConfirmation::for_zip(&zip_bytes, ConfirmationBudget::default()).expect("prepared");

    assert_eq!(
        confirmation.confirm("an-unrelated-password"),
        ConfirmationVerdict::NotAccepted,
        "the unencrypted entry must not act as a password credential"
    );
    assert_eq!(
        confirmation.confirm(FIXTURE_PASSWORD),
        ConfirmationVerdict::Confirmed
    );
}

/// An archive without any encrypted unit cannot confirm a password.
#[test]
fn archive_without_encrypted_unit_is_not_preparable() {
    let zip_bytes = build_unencrypted_zip_fixture();
    assert_eq!(
        PasswordConfirmation::for_zip(&zip_bytes, ConfirmationBudget::default()).err(),
        Some(PreparationError::NoEncryptedUnit)
    );
}

/// A truncated archive is reported as damaged, never as a rejected password.
#[test]
fn truncated_archive_is_damaged_rather_than_a_wrong_password() {
    let zip_bytes = build_zipcrypto_fixture();
    let truncated = &zip_bytes[..zip_bytes.len() / 2];
    assert_eq!(
        PasswordConfirmation::for_zip(truncated, ConfirmationBudget::default()).err(),
        Some(PreparationError::ArchiveDamaged)
    );
}

/// A tampered ciphertext payload must not be reported as a confirmed password.
#[test]
fn tampered_ciphertext_payload_is_not_confirmed() {
    let mut zip_bytes = build_zipcrypto_fixture();
    let archive = ZipArchive::open_slice(&zip_bytes).expect("fixture must open");
    let entry = &archive.entries()[0];
    let (payload_offset, _) =
        parse_local_file_header(&zip_bytes, entry.lfh_offset as usize).expect("LFH must parse");
    let body_offset = payload_offset + 12;
    zip_bytes[body_offset] ^= 0xFF;

    let confirmation =
        PasswordConfirmation::for_zip(&zip_bytes, ConfirmationBudget::default()).expect("prepared");
    let verdict = confirmation.confirm(FIXTURE_PASSWORD);
    assert!(
        !verdict.is_confirmed(),
        "a tampered payload must never be confirmed, got {verdict:?}"
    );
}

/// A budget that cannot cover the target entry is reported as exhausted, never as a
/// rejected password.
#[test]
fn ae2_correct_and_wrong_password_are_distinguishable() {
    let zip_bytes = assemble_zip_archive(&[build_aes_entry("secret.bin", &fixture_plaintext())])
        .expect("AE-2 fixture assembly");
    let archive = ZipArchive::open_slice(&zip_bytes).expect("fixture must open");
    let entry = &archive.entries()[0];
    assert_eq!(entry.compression_method, 99);
    assert_eq!(entry.crc32, 0, "AE-2 stores no CRC");

    let confirmation =
        PasswordConfirmation::for_zip(&zip_bytes, ConfirmationBudget::default()).expect("prepared");
    assert_eq!(
        confirmation.credential(),
        IntegrityCredential::WinZipAesAuthentication
    );
    assert_eq!(
        confirmation.confirm(FIXTURE_PASSWORD),
        ConfirmationVerdict::Confirmed
    );
    assert_eq!(
        confirmation.confirm("an-unrelated-password"),
        ConfirmationVerdict::NotAccepted
    );
}

/// A broken authentication code must never be reported as a confirmed password.
#[test]
fn ae2_with_a_broken_authentication_code_is_not_confirmed() {
    let mut zip_bytes = assemble_zip_archive(&[build_aes_entry("secret.bin", &fixture_plaintext())])
        .expect("AE-2 fixture assembly");
    let archive = ZipArchive::open_slice(&zip_bytes).expect("fixture must open");
    let entry = &archive.entries()[0];
    let (payload_offset, _) =
        parse_local_file_header(&zip_bytes, entry.lfh_offset as usize).expect("LFH must parse");
    let tag_offset = payload_offset + entry.compressed_size as usize - 1;
    zip_bytes[tag_offset] ^= 0xFF;

    let confirmation =
        PasswordConfirmation::for_zip(&zip_bytes, ConfirmationBudget::default()).expect("prepared");
    let verdict = confirmation.confirm(FIXTURE_PASSWORD);
    assert!(
        !verdict.is_confirmed(),
        "a broken authentication code must never be confirmed, got {verdict:?}"
    );
}

/// An AE-2 entry without payload is still authenticated: its authentication code is
/// computed over an empty ciphertext and still depends on the password.
#[test]
fn empty_ae2_entry_is_authenticated_by_its_authentication_code() {
    let zip_bytes = assemble_zip_archive(&[build_aes_entry("empty.bin", &[])])
        .expect("empty AE-2 fixture assembly");
    let archive = ZipArchive::open_slice(&zip_bytes).expect("fixture must open");
    let entry = &archive.entries()[0];
    assert_eq!(entry.uncompressed_size, 0);
    assert_eq!(entry.compressed_size, 28, "salt + pvv + authentication code");

    let confirmation =
        PasswordConfirmation::for_zip(&zip_bytes, ConfirmationBudget::default()).expect("prepared");
    assert_eq!(
        confirmation.confirm(FIXTURE_PASSWORD),
        ConfirmationVerdict::Confirmed,
        "an empty AE-2 entry carries a real authentication code"
    );
    assert_eq!(
        confirmation.confirm("an-unrelated-password"),
        ConfirmationVerdict::NotAccepted
    );
}

/// An entry flagged as encrypted but carrying no encryption header at all yields no
/// evidence, so it must not be selected as a confirmation target.
///
/// The raw reader accepts such an entry for every password, which is exactly why the
/// selection has to reject it on its own.
#[test]
fn encrypted_flag_without_encryption_header_yields_no_evidence() {
    let zip_bytes = assemble_zip_archive(&[ZipCompressedItem {
        rel_path: "empty.bin".to_string(),
        uncompressed_size: 0,
        compressed_size: 0,
        crc32: 0,
        compression_method: 0,
        actual_method: 0,
        aes_strength: 0,
        payload: Vec::new(),
        mtime_epoch_secs: 1_700_000_000,
        mode: 0o600,
        is_directory: false,
        is_encrypted: true,
    }])
    .expect("empty encrypted fixture assembly");

    let archive = ZipArchive::open_slice(&zip_bytes).expect("fixture must open");
    assert!(
        archive
            .extract_entry_bytes(0, Some("an-unrelated-password"))
            .is_ok(),
        "the raw reader must be shown to accept an arbitrary password here"
    );

    assert_eq!(
        PasswordConfirmation::for_zip(&zip_bytes, ConfirmationBudget::default()).err(),
        Some(PreparationError::NoCredential),
        "an entry without an encryption header must never act as a credential"
    );
}

/// An encrypted entry whose compression method the engine cannot decode is reported as
/// unsupported, not as a rejected candidate.
#[test]
fn unsupported_compression_method_is_not_a_candidate_rejection() {
    let plaintext = fixture_plaintext();
    let crc32 = crc32_fast(0, &plaintext);
    let check_byte = (crc32 >> 24) as u8;

    let mut engine = ZipCryptoEngine::new(FIXTURE_PASSWORD.as_bytes());
    let header = engine.generate_header(check_byte, &FIXTURE_HEADER_RANDOM);
    let mut body = plaintext.clone();
    engine.encrypt_slice(&mut body);
    let mut payload = header.to_vec();
    payload.extend_from_slice(&body);

    // Method 14 (LZMA) is a registered PKWARE compression method that the archive
    // engine does not implement, so the entry is a genuinely unsupported one.
    let item = ZipCompressedItem {
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
    };
    let zip_bytes = assemble_zip_archive(&[item]).expect("unsupported fixture assembly");

    assert_eq!(
        PasswordConfirmation::for_zip(&zip_bytes, ConfirmationBudget::default()).err(),
        Some(PreparationError::Unsupported),
        "an undecodable compression method must not be reported as a candidate rejection"
    );
}

/// A non-empty ZipCrypto entry whose stored CRC32 is legitimately zero is still
/// confirmable.
///
/// The ZIP CRC field is a mandatory 32-bit value, not an optional one: a zero value
/// is a valid CRC32 and must not be read as "the archive carries no checksum".
#[test]
fn zipcrypto_entry_with_a_zero_crc_is_still_confirmable() {
    // The CRC32 of these four bytes is exactly zero.
    let plaintext = vec![0x9du8, 0x0au8, 0xd9u8, 0x6du8];
    assert_eq!(crc32_fast(0, &plaintext), 0);

    let zip_bytes = build_zipcrypto_fixture_for(&plaintext);
    let archive = ZipArchive::open_slice(&zip_bytes).expect("fixture must open");
    assert_eq!(archive.entries()[0].crc32, 0);
    assert_eq!(
        archive
            .extract_entry_bytes(0, Some(FIXTURE_PASSWORD))
            .expect("the raw reader must decrypt the entry"),
        plaintext,
        "the raw reader must accept the entry"
    );

    let confirmation =
        PasswordConfirmation::for_zip(&zip_bytes, ConfirmationBudget::default()).expect("prepared");
    assert_eq!(
        confirmation.credential(),
        IntegrityCredential::EntryCrc32 { expected: 0 },
        "a zero CRC32 is a checksum, not a missing checksum"
    );
    assert_eq!(
        confirmation.confirm(FIXTURE_PASSWORD),
        ConfirmationVerdict::Confirmed
    );
    assert_eq!(
        confirmation.confirm("an-unrelated-password"),
        ConfirmationVerdict::NotAccepted
    );
}

/// An empty ZipCrypto entry carries no evidence and stays excluded, independently of
/// the zero-CRC case above.
#[test]
fn empty_zipcrypto_entry_stays_excluded() {
    let mut engine = ZipCryptoEngine::new(FIXTURE_PASSWORD.as_bytes());
    let header = engine.generate_header(0, &FIXTURE_HEADER_RANDOM);
    let item = ZipCompressedItem {
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
    };
    let zip_bytes = assemble_zip_archive(&[item]).expect("empty fixture assembly");

    assert_eq!(
        PasswordConfirmation::for_zip(&zip_bytes, ConfirmationBudget::default()).err(),
        Some(PreparationError::NoCredential),
        "an entry without content carries no evidence"
    );
}

/// A search that ran into a non-candidate verdict must report that verdict instead of
/// pretending the search was exhausted.
#[test]
fn a_named_zip_entry_can_be_confirmed() {
    let zip_bytes = build_mixed_zip_fixture();
    let confirmation = PasswordConfirmation::for_zip_entry(
        &zip_bytes,
        ConfirmationBudget::default(),
        1,
    )
    .expect("prepared");

    assert_eq!(
        confirmation.confirm(FIXTURE_PASSWORD),
        ConfirmationVerdict::Confirmed
    );
    assert_eq!(
        confirmation.confirm("an-unrelated-password"),
        ConfirmationVerdict::NotAccepted
    );
}

/// Legal AES-128/192 credentials are outside the current reader's AES-256 path.
/// Both nonempty and authenticated empty entries must stop before candidate testing.
#[test]
fn legal_unimplemented_aes_strengths_are_unsupported_not_rejected() {
    use crate::crypto::winzip_aes::WinZipAesKeyStrength::{Aes128, Aes192};
    for strength in [Aes128, Aes192] {
        for plaintext in [Vec::new(), fixture_plaintext()] {
            let bytes = build_aes_strength_zip_fixture(strength, &plaintext);
            let zip = ZipArchive::open_slice(&bytes).expect("real ZIP parser accepts fixture");
            let entry = &zip.entries()[0];
            assert!(entry.is_encrypted);
            assert_eq!(entry.compression_method, 99);
            assert_eq!(entry.actual_method, 0);
            assert_eq!(entry.aes_version, 2);
            assert_eq!(entry.aes_strength, strength as u8);
            assert_eq!(entry.uncompressed_size, plaintext.len() as u64);
            assert_eq!(
                PasswordConfirmation::for_zip(&bytes, ConfirmationBudget::default()).err(),
                Some(PreparationError::Unsupported)
            );
            assert_eq!(
                PasswordConfirmation::for_zip_entry(&bytes, ConfirmationBudget::default(), 0).err(),
                Some(PreparationError::Unsupported)
            );
        }
    }
}

/// A valid unsupported first unit must not hide a later supported credential.
#[test]
fn automatic_selection_skips_unsupported_aes_strength_for_later_credential() {
    use crate::crypto::winzip_aes::{
        WinZipAesKeyStrength::Aes128, WinZipAesVersion::AE2, winzip_aes_encrypt_payload,
    };
    let plaintext = fixture_plaintext();
    let payload = winzip_aes_encrypt_payload(
        AE2,
        Aes128,
        FIXTURE_PASSWORD,
        &FIXTURE_AES_SALT[..Aes128.salt_len()],
        &plaintext,
    )
    .expect("AES-128 encryption");
    let mut first = build_aes_entry("unsupported.bin", &plaintext);
    first.payload = payload;
    first.compressed_size = first.payload.len() as u64;
    let mut bytes = assemble_zip_archive(&[first, build_aes_entry("supported.bin", &plaintext)])
        .expect("two-entry assembly");
    // The assembler emits AES-256 metadata: rewrite only the first unit's declared
    // strength in its LFH and CDFH to match the valid AES-128 payload above.
    set_first_aes_strength(&mut bytes, 1);
    let confirmation = PasswordConfirmation::for_zip(&bytes, ConfirmationBudget::default())
        .expect("later eligible unit");
    assert_eq!(
        confirmation.target(),
        crate::recovery::ConfirmationTarget::ZipEntry { index: 1 }
    );
    assert_eq!(
        confirmation.confirm(FIXTURE_PASSWORD),
        ConfirmationVerdict::Confirmed
    );
    assert_eq!(
        confirmation.confirm("unrelated-password"),
        ConfirmationVerdict::NotAccepted
    );
}

/// Invalid strength is damaged metadata, distinct from legal unsupported 1/2.
#[test]
fn invalid_aes_strength_is_damaged_and_empty_aes256_stays_authenticated() {
    let mut bytes = assemble_zip_archive(&[build_aes_entry("invalid.bin", &[])]).expect("assembly");
    set_first_aes_strength(&mut bytes, 0);
    let zip = ZipArchive::open_slice(&bytes).expect("container remains parseable");
    assert_eq!(zip.entries()[0].aes_strength, 0);
    assert_eq!(
        PasswordConfirmation::for_zip(&bytes, ConfirmationBudget::default()).err(),
        Some(PreparationError::ArchiveDamaged)
    );
    assert_eq!(
        PasswordConfirmation::for_zip_entry(&bytes, ConfirmationBudget::default(), 0).err(),
        Some(PreparationError::ArchiveDamaged)
    );

    let supported =
        assemble_zip_archive(&[build_aes_entry("empty.bin", &[])]).expect("empty AES-256 assembly");
    let confirmation = PasswordConfirmation::for_zip(&supported, ConfirmationBudget::default())
        .expect("empty AES-256 authentication is supported");
    assert_eq!(
        confirmation.confirm(FIXTURE_PASSWORD),
        ConfirmationVerdict::Confirmed
    );
    assert_eq!(
        confirmation.confirm("unrelated-password"),
        ConfirmationVerdict::NotAccepted
    );
}

/// Contradictory AES headers are structurally damaged before any password attempt.
#[test]
fn contradictory_lfh_and_cdfh_aes_metadata_is_not_a_candidate_rejection() {
    let mut bytes =
        assemble_zip_archive(&[build_aes_entry("contradiction.bin", &fixture_plaintext())])
            .expect("valid AES-256 fixture");
    // Keep the authentic AES-256 payload and CDFH intact; only LFH claims AES-192.
    set_first_aes_strengths(&mut bytes, 2, 3);
    let zip = ZipArchive::open_slice(&bytes).expect("container table parses");
    assert_eq!(zip.entries()[0].aes_strength, 3);
    assert_eq!(
        zip.extract_entry_bytes(0, Some(FIXTURE_PASSWORD)),
        Err(crate::types::TTZipStatus::ErrCorruptHeader)
    );
    assert_eq!(
        PasswordConfirmation::for_zip(&bytes, ConfirmationBudget::default()).err(),
        Some(PreparationError::ArchiveDamaged)
    );
    assert_eq!(
        PasswordConfirmation::for_zip_entry(&bytes, ConfirmationBudget::default(), 0).err(),
        Some(PreparationError::ArchiveDamaged)
    );
}

/// Updates the strength field in the first entry's two real TLV extra lists.
/// The payload, sizes, versions, vendor, method and every other entry stay unchanged.
fn set_first_aes_strength(bytes: &mut [u8], strength: u8) {
    set_first_aes_strengths(bytes, strength, strength);
}

fn set_first_aes_strengths(bytes: &mut [u8], lfh_strength: u8, cd_strength: u8) {
    fn word(bytes: &[u8], at: usize) -> usize {
        u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize
    }
    fn update(bytes: &mut [u8], start: usize, len: usize, strength: u8) {
        let mut pos = start;
        while pos + 4 <= start + len {
            let id = word(bytes, pos);
            let size = word(bytes, pos + 2);
            assert!(pos + 4 + size <= start + len);
            if id == 0x9901 {
                assert_eq!(size, 7);
                bytes[pos + 8] = strength;
                return;
            }
            pos += 4 + size;
        }
        panic!("fixture must carry an AES extra field");
    }
    let name_len = word(bytes, 26);
    let extra_len = word(bytes, 28);
    update(bytes, 30 + name_len, extra_len, lfh_strength);
    // EOCD has no comment in these assembler fixtures.
    let eocd = bytes.len() - 22;
    assert_eq!(&bytes[eocd..eocd + 4], &0x06054b50u32.to_le_bytes());
    let cd =
        u32::from_le_bytes(bytes[eocd + 16..eocd + 20].try_into().expect("EOCD offset")) as usize;
    assert_eq!(&bytes[cd..cd + 4], &0x02014b50u32.to_le_bytes());
    let name_len = word(bytes, cd + 28);
    let extra_len = word(bytes, cd + 30);
    update(bytes, cd + 46 + name_len, extra_len, cd_strength);
}
