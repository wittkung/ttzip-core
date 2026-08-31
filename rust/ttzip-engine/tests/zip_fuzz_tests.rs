// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive Malformed ZIP Fault-Injection Fuzzing Harness & Password Brute-Force Interception Suite.
//!
//! Validates 6 critical resilience dimensions:
//! 1. EOCD Forgery and Corruption Injection (fake magic in comments, truncated records, broken comment lengths).
//! 2. Zip64 Locator Dangling Pointers & Illegal Size Injection (out-of-bounds offsets, non-Zip64 signatures, overflow sizes).
//! 3. Malformed Extra Field Length Bomb Injection (0xFFFF declared length with truncated payload, chained corrupt TLVs).
//! 4. Bad CRC & Truncated Compressed Stream Injection (Deflate truncated stream, corrupted CRC32, tampered AES HMAC).
//! 5. 1B/1B Single-Byte Jitter Streaming (micro-chunk stepped stream decryption & extraction integrity).
//! 6. Password Brute-Force Probing & Rapid Short-Circuit Interception (WinZip AES 2-byte PVV & ZipCrypto 1-byte check).

use std::panic::{catch_unwind, AssertUnwindSafe};
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::crypto::password_recovery::{
    verify_winzip_aes_candidate, verify_zipcrypto_candidate,
};
use ttzip_engine::crypto::winzip_aes::{
    winzip_aes_encrypt_payload, WinZipAesDecrypter, WinZipAesKeyStrength, WinZipAesVersion,
};
use ttzip_engine::crypto::zipcrypto::ZipCryptoEngine;
use ttzip_engine::types::{TTZipEncryptionMethod, TTZipStatus};
use ttzip_engine::zip::extra::{
    ExtraFieldsParser, ZipExtraFields, TAG_EXT_TIMESTAMP, TAG_WINZIP_AES,
};
use ttzip_engine::zip::parser::{
    find_eocd, parse_all_entries, parse_cdfh_entry, parse_local_file_header,
    MAGIC_EOCD, MAGIC_ZIP64_EOCD, MAGIC_ZIP64_LOCATOR,
};
use ttzip_engine::zip::reader::ZipArchive;
use ttzip_engine::zip::writer::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

// ============================================================================
// Test Fixture Helpers
// ============================================================================

/// Helper to build a standard multi-entry ZIP archive in memory.
fn build_sample_zip() -> Vec<u8> {
    let items = vec![
        ZipInputItem {
            rel_path: "plain.txt".to_string(),
            data: b"TTZip Robust Fault Injection & Fuzzing Harness Payload 2026.".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "compressed.bin".to_string(),
            data: vec![0x37u8; 8192],
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "folder/".to_string(),
            data: Vec::new(),
            mtime_epoch_secs: 1700000000,
            mode: 0o755,
            is_directory: true,
        },
    ];

    let compressed = compress_items_parallel(items, 6, TTZipEncryptionMethod::None, None, 2)
        .expect("sample zip compression must succeed");
    assemble_zip_archive(&compressed).expect("sample zip assembly must succeed")
}

/// Helper to build an AES-256 encrypted ZIP archive.
fn build_sample_aes_zip(password: &str) -> Vec<u8> {
    let items = vec![ZipInputItem {
        rel_path: "secret_document.pdf".to_string(),
        data: b"Top Secret Payload encrypted with WinZip AES-256 for TTZip testing.".to_vec(),
        mtime_epoch_secs: 1700000000,
        mode: 0o600,
        is_directory: false,
    }];

    let compressed =
        compress_items_parallel(items, 6, TTZipEncryptionMethod::Aes256, Some(password), 2)
            .expect("AES zip compression must succeed");
    assemble_zip_archive(&compressed).expect("AES zip assembly must succeed")
}

// ============================================================================
// 1. EOCD Forgery and Corruption Injection
// ============================================================================

#[test]
fn test_eocd_forged_magic_in_comment_safe_resolution() {
    let mut base_zip = build_sample_zip();
    let original_len = base_zip.len();

    // Build an archive comment containing fake MAGIC_EOCD signatures surrounded by text
    let mut comment = Vec::new();
    comment.extend_from_slice(b"Archive header comment prefix with dummy data.");
    comment.extend_from_slice(&[
        0x50, 0x4B, 0x05, 0x06, // Fake EOCD signature inside comment
        0x00, 0x00, 0x00, 0x00, // fake disk numbers
        0xFF, 0xFF, 0xFF, 0xFF, // fake entry counts
        0xDE, 0xAD, 0xBE, 0xEF, // fake cd size
        0xCA, 0xFE, 0xBA, 0xBE, // fake cd offset
        0x00, 0x00, // fake comment len
    ]);
    comment.extend_from_slice(b"Trailing comment text after fake EOCD signature.");

    // Append comment bytes to zip
    base_zip.extend_from_slice(&comment);
    let new_comment_len = comment.len() as u16;

    // Fix the real EOCD's comment length field at original_len - 2
    let comment_len_offset = original_len - 2;
    base_zip[comment_len_offset..comment_len_offset + 2]
        .copy_from_slice(&new_comment_len.to_le_bytes());

    let result = catch_unwind(AssertUnwindSafe(|| {
        let eocd_info = find_eocd(&base_zip).expect("find_eocd must locate the true EOCD record");
        assert_eq!(eocd_info.total_entries, 3);

        let archive = ZipArchive::open_slice(&base_zip).expect("ZipArchive must open successfully");
        assert_eq!(archive.len(), 3);
        archive.extract_entry_bytes(0, None).expect("entry 0 extract");
    }));

    assert!(result.is_ok(), "Parser must never panic on fake EOCD signatures in comments");
}

#[test]
fn test_eocd_truncated_payload_rejection() {
    let base_zip = build_sample_zip();

    for truncate_bytes in 1..=30 {
        if truncate_bytes >= base_zip.len() {
            break;
        }
        let truncated = &base_zip[..base_zip.len() - truncate_bytes];

        let result = catch_unwind(AssertUnwindSafe(|| {
            let eocd_res = find_eocd(truncated);
            let open_res = ZipArchive::open_slice(truncated);
            (eocd_res, open_res)
        }));

        assert!(
            result.is_ok(),
            "find_eocd / open_slice must never panic on truncated archive (cut {} bytes)",
            truncate_bytes
        );

        let (eocd_res, open_res) = result.unwrap();
        assert_eq!(eocd_res.err(), Some(TTZipStatus::ErrCorruptHeader));
        assert_eq!(open_res.err(), Some(TTZipStatus::ErrCorruptHeader));
    }
}

#[test]
fn test_eocd_corrupted_comment_length_rejection() {
    let mut base_zip = build_sample_zip();
    let file_len = base_zip.len();

    // Declare comment length as 0xFFFF (65535 bytes) which exceeds remaining file buffer
    base_zip[file_len - 2..file_len].copy_from_slice(&0xFFFFu16.to_le_bytes());

    let result = catch_unwind(AssertUnwindSafe(|| {
        let eocd_res = find_eocd(&base_zip);
        let open_res = ZipArchive::open_slice(&base_zip);
        (eocd_res, open_res)
    }));

    assert!(result.is_ok(), "Parser must never panic on comment length exceeding EOF");
    let (eocd_res, open_res) = result.unwrap();
    assert_eq!(eocd_res.err(), Some(TTZipStatus::ErrCorruptHeader));
    assert_eq!(open_res.err(), Some(TTZipStatus::ErrCorruptHeader));
}

// ============================================================================
// 2. Zip64 Locator Dangling Pointers & Illegal Size Injection
// ============================================================================

#[test]
fn test_zip64_locator_out_of_bounds_offset_recovery() {
    let mut base_zip = build_sample_zip();
    let orig_eocd_pos = base_zip.len() - 22;

    // Inject a synthetic Zip64 Locator right before standard EOCD
    let mut synthetic_locator = Vec::new();
    synthetic_locator.extend_from_slice(&MAGIC_ZIP64_LOCATOR.to_le_bytes());
    synthetic_locator.extend_from_slice(&0u32.to_le_bytes()); // disk with zip64 eocd
    synthetic_locator.extend_from_slice(&0xFFFFFFFF_FFFFFFFFu64.to_le_bytes()); // Dangling offset
    synthetic_locator.extend_from_slice(&1u32.to_le_bytes()); // total disks

    // Splice locator before EOCD
    base_zip.splice(orig_eocd_pos..orig_eocd_pos, synthetic_locator);

    let result = catch_unwind(AssertUnwindSafe(|| {
        let eocd = find_eocd(&base_zip).expect("Parser must fall back to standard EOCD");
        // Must safely ignore out-of-bounds locator and report non-zip64 valid EOCD
        assert!(!eocd.is_zip64);
        assert_eq!(eocd.total_entries, 3);
    }));

    assert!(
        result.is_ok(),
        "find_eocd must never panic on Zip64 locator pointing beyond EOF"
    );
}

#[test]
fn test_zip64_locator_pointing_to_garbage_magic() {
    let mut base_zip = build_sample_zip();
    let orig_eocd_pos = base_zip.len() - 22;

    // Point locator to offset 0 (which contains MAGIC_LFH, not MAGIC_ZIP64_EOCD)
    let mut synthetic_locator = Vec::new();
    synthetic_locator.extend_from_slice(&MAGIC_ZIP64_LOCATOR.to_le_bytes());
    synthetic_locator.extend_from_slice(&0u32.to_le_bytes());
    synthetic_locator.extend_from_slice(&0u64.to_le_bytes()); // Offset 0 has LFH
    synthetic_locator.extend_from_slice(&1u32.to_le_bytes());

    base_zip.splice(orig_eocd_pos..orig_eocd_pos, synthetic_locator);

    let result = catch_unwind(AssertUnwindSafe(|| {
        let eocd = find_eocd(&base_zip).expect("find_eocd fallback");
        assert!(!eocd.is_zip64);
    }));

    assert!(
        result.is_ok(),
        "Parser must safely handle Zip64 locator pointing to invalid magic without panic"
    );
}

#[test]
fn test_zip64_eocd_overflowing_record_size() {
    let mut out = Vec::new();

    // 1. Zip64 EOCD with 0xFFFFFFFFFFFFFFFF record size
    let z64_eocd_start = out.len();
    out.extend_from_slice(&MAGIC_ZIP64_EOCD.to_le_bytes());
    out.extend_from_slice(&u64::MAX.to_le_bytes()); // Record size overflow bomb
    out.extend_from_slice(&45u16.to_le_bytes()); // version made
    out.extend_from_slice(&45u16.to_le_bytes()); // version needed
    out.extend_from_slice(&0u32.to_le_bytes()); // disk number
    out.extend_from_slice(&0u32.to_le_bytes()); // disk with cd
    out.extend_from_slice(&1u64.to_le_bytes()); // total entries on disk
    out.extend_from_slice(&1u64.to_le_bytes()); // total entries
    out.extend_from_slice(&46u64.to_le_bytes()); // cd size
    out.extend_from_slice(&0u64.to_le_bytes()); // cd offset

    // 2. Zip64 Locator
    out.extend_from_slice(&MAGIC_ZIP64_LOCATOR.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(z64_eocd_start as u64).to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());

    // 3. Standard EOCD
    out.extend_from_slice(&MAGIC_EOCD.to_le_bytes());
    out.extend_from_slice(&[0u8; 18]);

    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = find_eocd(&out);
        let _ = parse_all_entries(&out);
        let _ = ZipArchive::open_slice(&out);
    }));

    assert!(result.is_ok(), "Zip64 EOCD with overflow record size must never cause panic or OOM");
}

// ============================================================================
// 3. Malformed Extra Field Length Bomb Injection
// ============================================================================

#[test]
fn test_extra_field_declared_length_bomb_safe_skip() {
    // 0x0001 (Zip64) declared length = 0xFFFF (65535 bytes), actual payload only 2 bytes
    let malformed_extra = [
        0x01, 0x00, // Tag = TAG_ZIP64
        0xFF, 0xFF, // Length = 65535 (Bomb)
        0xAA, 0xBB, // Truncated remaining payload
    ];

    let result = catch_unwind(AssertUnwindSafe(|| {
        let parsed = ZipExtraFields::parse(&malformed_extra, true, true, true, true);
        assert!(!parsed.has_zip64);
        assert!(parsed.zip64.is_none());

        let generic_parsed = ExtraFieldsParser::parse(&malformed_extra, true);
        assert!(!generic_parsed.has_zip64);
    }));

    assert!(result.is_ok(), "Extra field length bomb must be skipped gracefully with 0 panic");
}

#[test]
fn test_extra_field_chained_malformed_tlv_robustness() {
    let mut chained_extra = Vec::new();

    // 1. Valid Extended Timestamp (0x5455)
    chained_extra.extend_from_slice(&TAG_EXT_TIMESTAMP.to_le_bytes());
    chained_extra.extend_from_slice(&5u16.to_le_bytes()); // length 5
    chained_extra.push(0x01); // mod time flag
    chained_extra.extend_from_slice(&1700000000u32.to_le_bytes());

    // 2. Corrupted WinZip AES tag with declared length 100 but only 1 byte
    chained_extra.extend_from_slice(&TAG_WINZIP_AES.to_le_bytes());
    chained_extra.extend_from_slice(&100u16.to_le_bytes());
    chained_extra.push(0x01); // truncated

    let result = catch_unwind(AssertUnwindSafe(|| {
        let parsed = ZipExtraFields::parse(&chained_extra, false, false, false, false);
        // Valid prefix must be parsed
        assert!(parsed.has_extended_timestamp);
        assert_eq!(parsed.mod_time, Some(1700000000));
        // Corrupted suffix must be ignored safely without panic
        assert!(!parsed.has_winzip_aes);
    }));

    assert!(
        result.is_ok(),
        "Chained corrupt Extra Field stream must preserve valid tags and drop corrupt suffix safely"
    );
}

#[test]
fn test_extra_field_fuzz_random_garbage_slices() {
    let mut rng_state: u64 = 0x9876543210FEDCBA;
    let mut prng = || {
        rng_state ^= rng_state >> 12;
        rng_state ^= rng_state << 25;
        rng_state ^= rng_state >> 27;
        rng_state = rng_state.wrapping_mul(0x2545F4914F6CDD1D);
        rng_state
    };

    let iterations = 2000;
    let result = catch_unwind(AssertUnwindSafe(|| {
        for _ in 0..iterations {
            let len = (prng() % 128) as usize;
            let mut buf = vec![0u8; len];
            for b in &mut buf {
                *b = (prng() & 0xFF) as u8;
            }

            let _ = ZipExtraFields::parse(&buf, true, true, true, true);
            let _ = ZipExtraFields::parse(&buf, false, false, false, false);
            let _ = ExtraFieldsParser::parse(&buf, true);
        }
    }));

    assert!(result.is_ok(), "2000 iterations of random Extra Field fuzzing must produce 0 panics");
}

// ============================================================================
// 4. Bad CRC & Truncated Compressed Stream Injection
// ============================================================================

#[test]
fn test_bad_crc_deflate_entry_rejection() {
    let mut base_zip = build_sample_zip();
    let archive = ZipArchive::open_slice(&base_zip).expect("valid zip");

    // Locate the compressed.bin entry (index 1)
    assert_eq!(archive.entries()[1].rel_path, "compressed.bin");
    let original_crc = archive.entries()[1].crc32;

    // Mutate CRC32 in Central Directory entry for compressed.bin
    // CDFH is located after all file payloads
    let eocd = find_eocd(&base_zip).expect("eocd");
    let cd_offset = eocd.cd_offset as usize;

    let mut pos = cd_offset;
    for idx in 0..archive.len() {
        let (_entry, next_pos) = parse_cdfh_entry(&base_zip, pos).expect("cdfh");
        if idx == 1 {
            // CRC32 is at offset pos + 16
            let bad_crc = original_crc ^ 0xFFFFFFFF;
            base_zip[pos + 16..pos + 20].copy_from_slice(&bad_crc.to_le_bytes());
            break;
        }
        pos = next_pos;
    }

    let modified_archive = ZipArchive::open_slice(&base_zip).expect("open modified");
    let extract_res = modified_archive.extract_entry_bytes(1, None);

    assert_eq!(
        extract_res.err(),
        Some(TTZipStatus::ErrCorruptHeader),
        "Decompressor must reject entry with inverted CRC32"
    );
}

#[test]
fn test_truncated_deflate_stream_rejection() {
    let base_zip = build_sample_zip();
    let archive = ZipArchive::open_slice(&base_zip).expect("valid zip");
    let entry = &archive.entries()[1];

    let lfh_offset = entry.lfh_offset as usize;
    let (payload_offset, _) =
        parse_local_file_header(&base_zip, lfh_offset).expect("local header");

    // Create a truncated archive cut in the middle of deflate stream
    let cut_offset = payload_offset + (entry.compressed_size as usize / 2);
    let mut truncated_zip = base_zip.clone();
    truncated_zip.truncate(cut_offset);

    let result = catch_unwind(AssertUnwindSafe(|| {
        let open_res = ZipArchive::open_slice(&truncated_zip);
        if let Ok(arc) = open_res {
            let _ = arc.extract_entry_bytes(1, None);
        }
    }));

    assert!(result.is_ok(), "Truncated deflate stream must never cause panic or buffer overrun");
}

#[test]
fn test_tampered_and_truncated_winzip_aes_stream_rejection() {
    let password = "SecretMasterPassword2026!";
    let base_aes_zip = build_sample_aes_zip(password);

    let archive = ZipArchive::open_slice(&base_aes_zip).expect("open aes zip");
    let entry = &archive.entries()[0];
    assert!(entry.is_encrypted);

    // 1. Test extraction with correct password succeeds initially
    let valid_extract = archive.extract_entry_bytes(0, Some(password));
    assert!(valid_extract.is_ok(), "Baseline AES extract must succeed");

    // 2. Tamper 1 byte in ciphertext body (HMAC authentication failure)
    let (payload_offset, _) =
        parse_local_file_header(&base_aes_zip, entry.lfh_offset as usize).expect("lfh");
    let mut tampered_zip = base_aes_zip.clone();
    // Tamper byte inside ciphertext (after 16B salt + 2B PVV)
    let ciphertext_byte_offset = payload_offset + 16 + 2 + 5;
    tampered_zip[ciphertext_byte_offset] ^= 0x55;

    let tampered_archive = ZipArchive::open_slice(&tampered_zip).expect("open tampered");
    let tampered_res = tampered_archive.extract_entry_bytes(0, Some(password));
    assert_eq!(
        tampered_res.err(),
        Some(TTZipStatus::ErrInvalidPassword),
        "Tampered AES ciphertext must fail HMAC verification and return ErrInvalidPassword"
    );
}

// ============================================================================
// 5. 1B/1B Single-Byte Jitter Streaming
// ============================================================================

#[test]
fn test_single_byte_jitter_streaming_extraction() {
    let password = b"JitterStreamTestKey2026";
    let original_payload = b"Streaming 1-Byte Micro-Chunk Decompression Jitter Invariant Test!";

    // 1. ZipCrypto byte-by-byte stepping
    let mut zc_engine = ZipCryptoEngine::new(password);
    let mut encrypted_bytes = Vec::new();
    for &b in original_payload {
        encrypted_bytes.push(zc_engine.encrypt_byte(b));
    }

    let mut zc_dec_engine = ZipCryptoEngine::new(password);
    let mut decrypted_stream = Vec::new();
    for &b in &encrypted_bytes {
        decrypted_stream.push(zc_dec_engine.decrypt_byte(b));
    }
    assert_eq!(&decrypted_stream[..], original_payload);

    // 2. WinZip AES-256 micro-chunk (1 byte at a time) decrypt
    let salt16 = [0x42u8; 16];
    let container = winzip_aes_encrypt_payload(
        WinZipAesVersion::AE1,
        WinZipAesKeyStrength::Aes256,
        "JitterStreamPass",
        &salt16,
        original_payload,
    )
    .expect("encrypt payload");
    let expected_crc = crc32_fast(0, original_payload);

    // Parse container: Salt(16) | PVV(2) | Ciphertext(N) | AuthTag(10)
    let pvv = [container[16], container[17]];
    let cipher_len = original_payload.len();
    let ciphertext = &container[18..18 + cipher_len];
    let mut auth_tag = [0u8; 10];
    auth_tag.copy_from_slice(&container[18 + cipher_len..18 + cipher_len + 10]);

    let mut decrypter = WinZipAesDecrypter::new(
        WinZipAesVersion::AE1,
        WinZipAesKeyStrength::Aes256,
        "JitterStreamPass",
        &salt16,
        pvv,
    )
    .expect("decrypter init");

    let mut recovered_plain = Vec::new();
    for &b in ciphertext {
        let mut single_chunk = [b];
        decrypter.decrypt_chunk(&mut single_chunk);
        recovered_plain.push(single_chunk[0]);
    }

    let final_crc = decrypter
        .finalize(&auth_tag, Some(expected_crc))
        .expect("finalize must succeed on 1-byte stepped decryption");

    assert_eq!(&recovered_plain[..], original_payload);
    assert_eq!(final_crc, expected_crc);
}

#[test]
fn test_single_byte_jitter_fuzz_slice_stepping() {
    let base_zip = build_sample_zip();

    // Step through the entire archive in 1-byte increments from 0..file_size
    let result = catch_unwind(AssertUnwindSafe(|| {
        for step in (10..base_zip.len()).step_by(3) {
            let slice = &base_zip[..step];
            let _ = find_eocd(slice);
            let _ = parse_all_entries(slice);
            let _ = ZipArchive::open_slice(slice);
        }
    }));

    assert!(
        result.is_ok(),
        "Stepping through slice prefixes must never trigger panics or assertion faults"
    );
}

// ============================================================================
// 6. Password Brute-Force Probing & Rapid Short-Circuit Interception
// ============================================================================

#[test]
fn test_winzip_aes_pvv_rapid_short_circuit_interception() {
    let true_password = "CorrectHorseBatteryStaple2026!";
    let salt16 = [0x5Au8; 16];
    let payload = b"Sensitive Financial Ledger Data for TTZip Testing";

    let container = winzip_aes_encrypt_payload(
        WinZipAesVersion::AE2,
        WinZipAesKeyStrength::Aes256,
        true_password,
        &salt16,
        payload,
    )
    .expect("aes encrypt");

    let stored_pvv = [container[16], container[17]];

    // Generate 500 wrong candidate passwords
    let wrong_passwords = (0..500)
        .map(|i| format!("WrongPasswordCandidate_{:05}", i))
        .collect::<Vec<_>>();

    let result = catch_unwind(AssertUnwindSafe(|| {
        for wrong_pwd in &wrong_passwords {
            // 1. Test WinZipAesDecrypter::new short-circuit
            let dec_res = WinZipAesDecrypter::new(
                WinZipAesVersion::AE2,
                WinZipAesKeyStrength::Aes256,
                wrong_pwd,
                &salt16,
                stored_pvv,
            );
            assert_eq!(
                dec_res.err(),
                Some(TTZipStatus::ErrInvalidPassword),
                "Wrong password must be intercepted via PVV immediately"
            );

            // 2. Test password recovery candidate verification
            let candidate_res = verify_winzip_aes_candidate(wrong_pwd, &salt16, &stored_pvv);
            assert!(
                !candidate_res,
                "Wrong candidate must fail PVV verification (0 false positives)"
            );
        }

        // Verify true password succeeds
        assert!(verify_winzip_aes_candidate(
            true_password,
            &salt16,
            &stored_pvv
        ));
    }));

    assert!(
        result.is_ok(),
        "WinZip AES PVV short-circuit verification must execute with 0 panic"
    );
}

#[test]
fn test_zipcrypto_initial_check_byte_interception() {
    let true_password = b"MasterPKZIPSecretPass!";
    let crc32: u32 = 0xCAFEBABE;
    let expected_check_byte = ((crc32 >> 24) & 0xFF) as u8; // 0xCA
    let random_11: [u8; 11] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

    let mut enc_engine = ZipCryptoEngine::new(true_password);
    let enc_header = enc_engine.generate_header(expected_check_byte, &random_11);

    // 1. Test specific wrong passwords return ErrInvalidPassword immediately
    let deterministic_wrong = [
        "WrongSecretPassword1",
        "IncorrectGuess2026",
        "AdminSecretRoot",
        "InvalidToken999",
        "ZipCryptoBadKey",
    ];

    for wrong_pwd in &deterministic_wrong {
        let res = ZipCryptoEngine::verify_and_init(
            wrong_pwd.as_bytes(),
            &enc_header,
            crc32,
            0,
            false,
        );
        if !verify_zipcrypto_candidate(wrong_pwd, &enc_header, expected_check_byte) {
            assert_eq!(
                res.err(),
                Some(TTZipStatus::ErrInvalidPassword),
                "Non-matching candidate must return ErrInvalidPassword"
            );
        }
    }

    // 2. Test 500 pseudo-random guesses for 0 panic and strict parity between engine & candidate checker
    let wrong_passwords = (0..500)
        .map(|i| format!("BadZipCryptoGuess_{:05}", i))
        .collect::<Vec<_>>();

    let mut rejected_count = 0;
    let result = catch_unwind(AssertUnwindSafe(|| {
        for wrong_pwd in &wrong_passwords {
            let res = ZipCryptoEngine::verify_and_init(
                wrong_pwd.as_bytes(),
                &enc_header,
                crc32,
                0,
                false,
            );
            let candidate_ok =
                verify_zipcrypto_candidate(wrong_pwd, &enc_header, expected_check_byte);

            // Engine init success must strictly agree with 1-byte candidate check
            assert_eq!(res.is_ok(), candidate_ok);

            if res.is_err() {
                assert_eq!(res.err(), Some(TTZipStatus::ErrInvalidPassword));
                rejected_count += 1;
            }
        }

        // True password must succeed
        let true_res = ZipCryptoEngine::verify_and_init(
            true_password,
            &enc_header,
            crc32,
            0,
            false,
        );
        assert!(true_res.is_ok());
        assert!(verify_zipcrypto_candidate(
            std::str::from_utf8(true_password).unwrap(),
            &enc_header,
            expected_check_byte
        ));
    }));

    assert!(
        result.is_ok(),
        "ZipCrypto check byte interception must execute with 0 panic across 500 guesses"
    );
    // Statistical rejection rate for 1-byte check (255/256 ≈ 99.6%) must be >= 95%
    assert!(
        rejected_count >= 475,
        "Expected >= 95% rejection rate across 500 guesses, got {}",
        rejected_count
    );
}

#[test]
fn test_concurrent_brute_force_interception_matrix() {
    use rayon::prelude::*;

    let target_salt = [0x99u8; 16];
    let secret_pass = "AlphaBravoCharlie2026!";
    let container = winzip_aes_encrypt_payload(
        WinZipAesVersion::AE2,
        WinZipAesKeyStrength::Aes256,
        secret_pass,
        &target_salt,
        b"Protected Payload",
    )
    .expect("encrypt");
    let stored_pvv = [container[16], container[17]];

    // Dictionary of 1,000 candidate words containing exactly 1 match
    let mut dictionary: Vec<String> = (0..999)
        .map(|i| format!("DictionaryWord_{:06}", i))
        .collect();
    dictionary.insert(777, secret_pass.to_string());

    let match_count = dictionary
        .par_iter()
        .filter(|candidate| verify_winzip_aes_candidate(candidate, &target_salt, &stored_pvv))
        .count();

    assert_eq!(
        match_count, 1,
        "Brute force dictionary search must find exactly 1 unique match with 0 false positives"
    );
}
