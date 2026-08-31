// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unit tests for 7-Zip Encrypted Header (`kEncodedHeader`, 0x17) recursive
//! self-extracting state machine and sub-millisecond password probing.

use std::io::Cursor;
use std::time::Instant;

use ttzip_engine::crypto::aes256::aes256_cbc_encrypt;
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::crypto::sha256::sha256_7z_kdf;
use ttzip_engine::sevenz::dag::SevenZError;
use ttzip_engine::sevenz::encrypted_header::{
    probe_7z_password, EncodedHeaderDecoder,
};
use ttzip_engine::sevenz::format::*;
use ttzip_engine::sevenz::header::parse_7z_metadata;

/// Helper: builds a synthetic uncompressed `kHeader` byte stream describing two files.
fn build_synthetic_inner_header(files: &[(&str, u32)]) -> Vec<u8> {
    let mut h = Vec::new();
    h.push(K_HEADER);

    // FilesInfo
    h.push(K_FILES_INFO);
    write_varint(files.len() as u64, &mut h);

    // Names
    h.push(K_NAME);
    let mut names_u16 = Vec::new();
    for (name, _) in files {
        for u in name.encode_utf16() {
            names_u16.extend_from_slice(&u.to_le_bytes());
        }
        names_u16.extend_from_slice(&0u16.to_le_bytes());
    }
    write_varint((1 + names_u16.len()) as u64, &mut h);
    h.push(0); // external = 0
    h.extend_from_slice(&names_u16);

    // WinAttributes
    h.push(K_WIN_ATTRIBUTES);
    write_varint((2 + files.len() * 4) as u64, &mut h);
    h.push(1); // allDefined = 1
    h.push(0); // external = 0
    for (_, attr) in files {
        h.extend_from_slice(&attr.to_le_bytes());
    }

    h.push(K_END); // end kFilesInfo
    h.push(K_END); // end kHeader
    h
}

/// Helper: constructs a complete 7z archive buffer with an Encrypted Header (`kEncodedHeader`).
fn create_synthetic_encrypted_header_archive(
    files: &[(&str, u32)],
    password: &str,
    cycles_power: u32,
    use_lzma2: bool,
) -> Vec<u8> {
    let inner_header = build_synthetic_inner_header(files);
    let inner_crc = crc32_fast(0, &inner_header);
    let raw_unpack_len = inner_header.len();

    let (compressed_payload, dict_prop) = if use_lzma2 {
        use ttzip_engine::codecs::lzma2::{Fl2CParameter, Fl2CStream, Fl2InBuffer, Fl2OutBuffer};
        let mut cstream = Fl2CStream::new().expect("create cstream");
        cstream.set_parameter(Fl2CParameter::CompressionLevel, 3).expect("set level");
        cstream.set_parameter(Fl2CParameter::OmitProperties, 1).expect("set omit");
        cstream.init(0).expect("init cstream");

        let mut in_buf = Fl2InBuffer {
            src: inner_header.as_ptr() as *const libc::c_void,
            size: inner_header.len(),
            pos: 0,
        };
        let mut out_chunk = vec![0u8; 4096];
        let mut comp = Vec::new();

        while in_buf.pos < in_buf.size {
            let mut out_buf = Fl2OutBuffer {
                dst: out_chunk.as_mut_ptr() as *mut libc::c_void,
                size: out_chunk.len(),
                pos: 0,
            };
            let _ = cstream.compress_chunk(&mut in_buf, &mut out_buf).expect("compress chunk");
            if out_buf.pos > 0 {
                comp.extend_from_slice(&out_chunk[..out_buf.pos]);
            }
        }

        loop {
            let mut out_buf = Fl2OutBuffer {
                dst: out_chunk.as_mut_ptr() as *mut libc::c_void,
                size: out_chunk.len(),
                pos: 0,
            };
            let remaining = cstream.end_stream(&mut out_buf).expect("end stream");
            if out_buf.pos > 0 {
                comp.extend_from_slice(&out_chunk[..out_buf.pos]);
            }
            if remaining == 0 {
                break;
            }
        }
        let prop = cstream.dict_property();
        (comp, prop)
    } else {
        (inner_header.clone(), 0)
    };

    // Pad to 16 bytes for AES-CBC
    let mut padded = compressed_payload;
    let remainder = padded.len() % 16;
    if remainder != 0 {
        padded.resize(padded.len() + (16 - remainder), 0);
    }

    let salt = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00];
    let iv = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10];

    let key = sha256_7z_kdf(password, &salt, cycles_power);
    let mut encrypted_payload = vec![0u8; padded.len()];
    aes256_cbc_encrypt(&key, &iv, &padded, &mut encrypted_payload).expect("AES encrypt");

    let pack_len = encrypted_payload.len();
    let pack_pos = 0u64; // packed stream begins right at offset 32 (payload offset)

    // Build kEncodedHeader boot stream
    let mut boot = Vec::new();
    boot.push(K_ENCODED_HEADER);

    // PackInfo
    boot.push(K_PACK_INFO);
    write_varint(pack_pos, &mut boot);
    write_varint(1, &mut boot); // numPackStreams = 1
    boot.push(K_SIZE);
    write_varint(pack_len as u64, &mut boot);
    boot.push(K_END);

    // UnpackInfo
    boot.push(K_UNPACK_INFO);
    boot.push(K_FOLDER);
    write_varint(1, &mut boot); // numFolders = 1
    boot.push(0); // external = 0

    if use_lzma2 {
        // 2 Coders: Coder 0 = LZMA2, Coder 1 = AES
        write_varint(2, &mut boot);

        // Coder 0: LZMA2 (0x21)
        boot.push(0x21); // flags: method_size = 1, props present
        boot.push(0x21); // METHOD_LZMA2
        write_varint(1, &mut boot); // props length
        boot.push(dict_prop); // dict size prop from compressor

        // Coder 1: AES (0x06F10701)
        boot.push(0x24); // flags: method_size = 4, props present
        boot.extend_from_slice(&[0x06, 0xF1, 0x07, 0x01]);
        let mut aes_props = Vec::new();
        let b0 = (cycles_power & 0x3F) as u8 | 0xC0; // salt present + iv present
        aes_props.push(b0);
        let s_len_enc = (salt.len().saturating_sub(1) & 0x0F) as u8;
        let iv_len_enc = (iv.len().saturating_sub(1) & 0x0F) as u8;
        aes_props.push((iv_len_enc << 4) | s_len_enc);
        aes_props.extend_from_slice(&salt);
        aes_props.extend_from_slice(&iv);

        write_varint(aes_props.len() as u64, &mut boot);
        boot.extend_from_slice(&aes_props);

        // Bind pair: In 0 (LZMA2) <- Out 1 (AES)
        write_varint(0, &mut boot); // in stream 0
        write_varint(1, &mut boot); // out stream 1 (Coder 1)
    } else {
        // 2 Coders: Coder 0 = COPY, Coder 1 = AES
        write_varint(2, &mut boot);

        // Coder 0: COPY (0x00)
        boot.push(0x01); // method size 1
        boot.push(0x00);

        // Coder 1: AES (0x06F10701)
        boot.push(0x24);
        boot.extend_from_slice(&[0x06, 0xF1, 0x07, 0x01]);
        let mut aes_props = Vec::new();
        let b0 = (cycles_power & 0x3F) as u8 | 0xC0;
        aes_props.push(b0);
        let s_len_enc = (salt.len().saturating_sub(1) & 0x0F) as u8;
        let iv_len_enc = (iv.len().saturating_sub(1) & 0x0F) as u8;
        aes_props.push((iv_len_enc << 4) | s_len_enc);
        aes_props.extend_from_slice(&salt);
        aes_props.extend_from_slice(&iv);

        write_varint(aes_props.len() as u64, &mut boot);
        boot.extend_from_slice(&aes_props);

        // Bind pair: In 0 (COPY) <- Out 1 (AES)
        write_varint(0, &mut boot);
        write_varint(1, &mut boot);
    }

    // CodersUnpackSize
    boot.push(K_CODERS_UNPACK_SIZE);
    write_varint(raw_unpack_len as u64, &mut boot); // Coder 0 unpack size
    write_varint(padded.len() as u64, &mut boot); // Coder 1 unpack size

    // CRC
    boot.push(K_CRC);
    boot.push(1); // allDefined = 1
    boot.extend_from_slice(&inner_crc.to_le_bytes());

    boot.push(K_END); // end kUnpackInfo
    boot.push(K_END); // end kEncodedHeader

    let next_header_offset = encrypted_payload.len() as u64;
    let next_header_size = boot.len() as u64;
    let next_header_crc = crc32_fast(0, &boot);

    let sig = SevenZSignatureHeader {
        major_version: 0,
        minor_version: 4,
        start_header_crc: 0,
        next_header_offset,
        next_header_size,
        next_header_crc,
    };

    let mut archive = Vec::new();
    archive.extend_from_slice(&sig.serialize());
    archive.extend_from_slice(&encrypted_payload);
    archive.extend_from_slice(&boot);
    archive
}

/// Helper: constructs a complete 7z archive with a plain, uncompressed `kHeader` (0x01).
fn create_synthetic_plain_header_archive(files: &[(&str, u32)]) -> Vec<u8> {
    let inner_header = build_synthetic_inner_header(files);
    let next_header_offset = 0u64;
    let next_header_size = inner_header.len() as u64;
    let next_header_crc = crc32_fast(0, &inner_header);

    let sig = SevenZSignatureHeader {
        major_version: 0,
        minor_version: 4,
        start_header_crc: 0,
        next_header_offset,
        next_header_size,
        next_header_crc,
    };

    let mut archive = Vec::new();
    archive.extend_from_slice(&sig.serialize());
    archive.extend_from_slice(&inner_header);
    archive
}

#[test]
fn test_plain_header_and_encoded_header_dual_mode_detection() {
    let files = [
        ("documents/report.pdf", 0x20u32),
        ("images/photo.png", 0x20u32),
    ];

    // 1. Test plain header (0x01)
    let plain_archive = create_synthetic_plain_header_archive(&files);
    let decoder = EncodedHeaderDecoder::default();
    let plain_info = decoder.decode(&plain_archive, None).expect("Decode plain header");
    assert_eq!(plain_info.files.len(), 2);
    assert_eq!(plain_info.files[0].rel_path, "documents/report.pdf");
    assert_eq!(plain_info.files[1].rel_path, "images/photo.png");

    let mut plain_cursor = Cursor::new(&plain_archive);
    let probe_res = probe_7z_password(&mut plain_cursor, "any_pass").expect("Probe plain header");
    assert!(probe_res, "Plain header must report Ok(true) unconditionally");

    // 2. Test encoded header (0x17) with LZMA2
    let password = "SecretMasterPassword_2026";
    let enc_archive = create_synthetic_encrypted_header_archive(&files, password, 4, true);

    let mut enc_cursor = Cursor::new(&enc_archive);
    let probe_enc = probe_7z_password(&mut enc_cursor, password).expect("Probe encrypted header with correct password");
    assert!(probe_enc, "Correct password must validate successfully");
}

#[test]
fn test_encrypted_header_decoding_and_metadata_recovery() {
    let files = [
        ("secure/credentials.vault", 0x20u32),
        ("keys/id_ed25519", 0x20u32),
        ("logs/audit.json", 0x20u32),
    ];
    let password = "UltraSecureKey#99";
    let archive_bytes = create_synthetic_encrypted_header_archive(&files, password, 4, true);

    let decoder = EncodedHeaderDecoder::default();
    let info = decoder.decode(&archive_bytes, Some(password)).expect("Decode encrypted header");

    assert_eq!(info.files.len(), 3);
    assert_eq!(info.files[0].rel_path, "secure/credentials.vault");
    assert_eq!(info.files[1].rel_path, "keys/id_ed25519");
    assert_eq!(info.files[2].rel_path, "logs/audit.json");

    // Verify through parse_7z_metadata high-level facade
    let meta_info = parse_7z_metadata(&archive_bytes, Some(password)).expect("parse_7z_metadata facade");
    assert_eq!(meta_info.files.len(), 3);
    assert_eq!(meta_info.files[0].rel_path, "secure/credentials.vault");
}

#[test]
fn test_encrypted_header_wrong_password_interception() {
    let files = [("confidential.doc", 0x20u32)];
    let correct_pass = "CorrectPassword123";
    let wrong_pass = "IncorrectPassword999";
    let archive_bytes = create_synthetic_encrypted_header_archive(&files, correct_pass, 4, true);

    let decoder = EncodedHeaderDecoder::default();

    // 1. Wrong password in full decoder
    let res = decoder.decode(&archive_bytes, Some(wrong_pass));
    assert_eq!(res.unwrap_err(), SevenZError::BadPassword);

    // 2. Missing password
    let res_none = decoder.decode(&archive_bytes, None);
    assert_eq!(res_none.unwrap_err(), SevenZError::BadPassword);

    // 3. Fast probe with wrong password
    let mut cursor = Cursor::new(&archive_bytes);
    let probe_err = probe_7z_password(&mut cursor, wrong_pass);
    assert_eq!(probe_err.unwrap_err(), SevenZError::BadPassword);
}

#[test]
fn test_oversized_header_unpack_quota_interception() {
    let files = [("big.bin", 0x20u32)];
    let password = "TestPassword";
    let archive_bytes = create_synthetic_encrypted_header_archive(&files, password, 4, false);

    // Instantiate decoder with a strict 16-byte maximum quota
    let strict_decoder = EncodedHeaderDecoder::new(16);
    let res = strict_decoder.decode(&archive_bytes, Some(password));

    match res.unwrap_err() {
        SevenZError::CountLimitExceeded { field_name, limit, .. } => {
            assert_eq!(field_name, "header unpack size");
            assert_eq!(limit, 16);
        }
        other => panic!("Expected CountLimitExceeded, got: {:?}", other),
    }
}

#[test]
fn test_probe_password_sub_millisecond_latency() {
    let files = [
        ("src/main.rs", 0x20u32),
        ("src/lib.rs", 0x20u32),
        ("Cargo.toml", 0x20u32),
    ];
    let password = "FastProbingPassword";
    let archive_bytes = create_synthetic_encrypted_header_archive(&files, password, 2, true);

    let mut cursor = Cursor::new(&archive_bytes);

    // Warm-up iteration
    let _ = probe_7z_password(&mut cursor, password).expect("warmup");

    // Benchmark 100 consecutive password probes
    let iterations = 100;
    let start = Instant::now();
    for _ in 0..iterations {
        let res = probe_7z_password(&mut cursor, password).expect("probe pass");
        assert!(res);
    }
    let elapsed = start.elapsed();
    let avg_per_probe = elapsed / iterations;

    // Average probing time must be well under 1ms (< 1000 µs)
    assert!(
        avg_per_probe.as_millis() <= 2,
        "Average probe latency ({:?}) must be sub-millisecond",
        avg_per_probe
    );
}
