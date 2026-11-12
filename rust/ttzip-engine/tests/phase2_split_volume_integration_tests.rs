// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Phase 2 Integration Tests: Multi-Volume Split Container & Virtual Continuous Reader.

use libc::{c_char, c_void};
use std::ffi::{CStr, CString};
use std::fs;
use tempfile::tempdir;
use ttzip_engine::archive::split::VolumeNamingScheme;
use ttzip_engine::ffi::archive_ffi::split::*;
use ttzip_engine::types::TTZipStatus;

unsafe extern "C" fn test_progress_cb(
    _processed: u64,
    _total: u64,
    _entry: *const c_char,
    user_data: *mut c_void,
) -> bool {
    unsafe {
        if !user_data.is_null() {
            let counter = &mut *(user_data as *mut usize);
            *counter += 1;
        }
        true
    }
}

#[test]
fn test_split_writer_and_reader_ffi_roundtrip() {
    let dir = tempdir().unwrap();
    let base_archive = dir.path().join("split_test.7z");
    let c_base = CString::new(base_archive.to_str().unwrap()).unwrap();
    let volume_size: u64 = 65536; // 64 KB

    unsafe {
        let writer = ttzip_rust_split_writer_new(
            c_base.as_ptr(),
            volume_size,
            VolumeNamingScheme::NumberedExtension as i32,
            true,
        );
        assert!(!writer.is_null());

        // 180 KB payload -> 3 volumes (64KB, 64KB, 52KB)
        let payload: Vec<u8> = (0..184320).map(|i| (i % 251) as u8).collect();
        let write_res = ttzip_rust_split_writer_write(writer, payload.as_ptr(), payload.len());
        assert_eq!(write_res, 0);

        let total_bytes = ttzip_rust_split_writer_get_total_bytes(writer);
        assert_eq!(total_bytes, 184320);

        let close_status = ttzip_rust_split_writer_close(writer);
        assert_eq!(close_status, TTZipStatus::Ok);

        let vol_count = ttzip_rust_split_writer_get_volume_count(writer);
        assert_eq!(vol_count, 3);

        let mut buf = [0 as c_char; 1024];
        let p_res = ttzip_rust_split_writer_get_volume_path(writer, 0, buf.as_mut_ptr(), buf.len());
        assert_eq!(p_res, TTZipStatus::Ok);
        let path0 = CStr::from_ptr(buf.as_ptr()).to_str().unwrap();
        assert!(path0.ends_with(".001"));

        ttzip_rust_split_writer_free(writer);

        // Open reader from volume 2 (middle of chain) to verify auto-topology discovery
        let vol2_path = format!("{}.002", base_archive.to_str().unwrap());
        let c_vol2 = CString::new(vol2_path).unwrap();

        let reader = ttzip_rust_split_reader_open(c_vol2.as_ptr());
        assert!(!reader.is_null());

        let total_size = ttzip_rust_split_reader_get_total_size(reader);
        assert_eq!(total_size, 184320);

        let reader_vol_count = ttzip_rust_split_reader_get_volume_count(reader);
        assert_eq!(reader_vol_count, 3);

        // Read back all bytes across volume boundaries
        let mut read_data = vec![0u8; 184320];
        let mut bytes_read: usize = 0;
        let read_status = ttzip_rust_split_reader_read(
            reader,
            read_data.as_mut_ptr(),
            read_data.len(),
            &mut bytes_read,
        );
        assert_eq!(read_status, TTZipStatus::Ok);
        assert_eq!(bytes_read, 184320);
        assert_eq!(read_data, payload);

        // Test seek
        let mut new_offset: u64 = 0;
        let seek_status = ttzip_rust_split_reader_seek(reader, 70000, 0, &mut new_offset);
        assert_eq!(seek_status, TTZipStatus::Ok);
        assert_eq!(new_offset, 70000);

        let mut chunk = [0u8; 100];
        let mut chunk_read = 0;
        let chunk_status = ttzip_rust_split_reader_read(
            reader,
            chunk.as_mut_ptr(),
            chunk.len(),
            &mut chunk_read,
        );
        assert_eq!(chunk_status, TTZipStatus::Ok);
        assert_eq!(chunk_read, 100);
        assert_eq!(&chunk[..], &payload[70000..70100]);

        ttzip_rust_split_reader_free(reader);
    }
}

#[test]
fn test_split_file_and_join_volumes_ffi() {
    let dir = tempdir().unwrap();
    let src_file = dir.path().join("original_payload.bin");
    let split_base = dir.path().join("split_archive.dat");
    let reassembled_file = dir.path().join("reassembled_payload.bin");

    let test_data: Vec<u8> = (0..150000).map(|i| (i * 37 % 256) as u8).collect();
    fs::write(&src_file, &test_data).unwrap();

    let c_src = CString::new(src_file.to_str().unwrap()).unwrap();
    let c_dst_base = CString::new(split_base.to_str().unwrap()).unwrap();
    let c_reass = CString::new(reassembled_file.to_str().unwrap()).unwrap();

    unsafe {
        let split_status = ttzip_rust_split_file(
            c_src.as_ptr(),
            c_dst_base.as_ptr(),
            65536,
            VolumeNamingScheme::NumberedExtension as i32,
            true,
        );
        assert_eq!(split_status, TTZipStatus::Ok);

        let part1 = format!("{}.001", split_base.to_str().unwrap());
        let part2 = format!("{}.002", split_base.to_str().unwrap());
        let part3 = format!("{}.003", split_base.to_str().unwrap());
        assert!(fs::metadata(&part1).is_ok());
        assert!(fs::metadata(&part2).is_ok());
        assert!(fs::metadata(&part3).is_ok());

        let mut progress_count: usize = 0;
        let c_part1 = CString::new(part1).unwrap();
        let join_status = ttzip_rust_join_split_volumes(
            c_part1.as_ptr(),
            c_reass.as_ptr(),
            Some(test_progress_cb),
            &mut progress_count as *mut usize as *mut c_void,
        );
        assert_eq!(join_status, TTZipStatus::Ok);
        assert!(progress_count > 0);

        let reassembled_data = fs::read(&reassembled_file).unwrap();
        assert_eq!(reassembled_data, test_data);
    }
}

#[test]
fn test_pkzip_spanned_ffi_roundtrip() {
    let dir = tempdir().unwrap();
    let src_file = dir.path().join("document.zip");
    let split_base = dir.path().join("document.zip");

    let test_data: Vec<u8> = (0..140000).map(|i| (i ^ 0x5A) as u8).collect();
    fs::write(&src_file, &test_data).unwrap();

    let c_src = CString::new(src_file.to_str().unwrap()).unwrap();
    let c_dst_base = CString::new(split_base.to_str().unwrap()).unwrap();

    unsafe {
        let split_status = ttzip_rust_split_file(
            c_src.as_ptr(),
            c_dst_base.as_ptr(),
            65536,
            VolumeNamingScheme::PkzipSpanned as i32,
            true,
        );
        assert_eq!(split_status, TTZipStatus::Ok);

        let parent_dir = dir.path();
        assert!(parent_dir.join("document.z01").exists());
        assert!(parent_dir.join("document.z02").exists());
        assert!(parent_dir.join("document.zip").exists());

        // Probe chain from the final .zip file
        let final_zip = parent_dir.join("document.zip");
        let c_final_zip = CString::new(final_zip.to_str().unwrap()).unwrap();
        let reader = ttzip_rust_split_reader_open(c_final_zip.as_ptr());
        assert!(!reader.is_null());

        let total_size = ttzip_rust_split_reader_get_total_size(reader);
        assert_eq!(total_size, 140000);
        let vol_count = ttzip_rust_split_reader_get_volume_count(reader);
        assert_eq!(vol_count, 3);

        let mut read_data = vec![0u8; 140000];
        let mut bytes_read: usize = 0;
        let read_status = ttzip_rust_split_reader_read(
            reader,
            read_data.as_mut_ptr(),
            read_data.len(),
            &mut bytes_read,
        );
        assert_eq!(read_status, TTZipStatus::Ok);
        assert_eq!(bytes_read, 140000);
        assert_eq!(read_data, test_data);

        ttzip_rust_split_reader_free(reader);
    }
}

fn lzma2_compress_raw(src: &[u8]) -> (Vec<u8>, u8) {
    use ttzip_engine::codecs::lzma2::{Fl2CParameter, Fl2CStream, Fl2InBuffer, Fl2OutBuffer};
    let mut cstream = Fl2CStream::new().expect("create Fl2CStream");
    cstream.set_parameter(Fl2CParameter::CompressionLevel, 3).expect("set level");
    cstream.set_parameter(Fl2CParameter::OmitProperties, 1).expect("omit props");
    cstream.init(0).expect("init Fl2CStream");

    let mut in_buf = Fl2InBuffer {
        src: src.as_ptr() as *const c_void,
        size: src.len(),
        pos: 0,
    };
    let mut out_chunk = vec![0u8; 8192];
    let mut comp = Vec::new();
    while in_buf.pos < in_buf.size {
        let mut out_buf = Fl2OutBuffer {
            dst: out_chunk.as_mut_ptr() as *mut c_void,
            size: out_chunk.len(),
            pos: 0,
        };
        cstream.compress_chunk(&mut in_buf, &mut out_buf).expect("compress chunk");
        comp.extend_from_slice(&out_chunk[..out_buf.pos]);
    }
    loop {
        let mut out_buf = Fl2OutBuffer {
            dst: out_chunk.as_mut_ptr() as *mut c_void,
            size: out_chunk.len(),
            pos: 0,
        };
        let rem = cstream.end_stream(&mut out_buf).expect("end stream");
        comp.extend_from_slice(&out_chunk[..out_buf.pos]);
        if rem == 0 {
            break;
        }
    }
    (comp, cstream.dict_property())
}

fn aes_encrypt_7z_padded(
    raw_comp: &[u8],
    password: &str,
    cycles_power: u32,
) -> (Vec<u8>, Vec<u8>, u64) {
    use ttzip_engine::crypto::aes256::aes256_cbc_encrypt;
    use ttzip_engine::crypto::sha256::sha256_7z_kdf;

    let mut padded = raw_comp.to_vec();
    let rem = padded.len() % 16;
    if rem != 0 || padded.is_empty() {
        padded.resize(padded.len() + (16 - rem), 0);
    }
    let salt = [0x10, 0x21, 0x32, 0x43, 0x54, 0x65, 0x76, 0x87, 0x98, 0xA9, 0xBA, 0xCB, 0xDC, 0xED, 0xFE, 0x0F];
    let iv = [0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6, 0x07, 0x18, 0x29, 0x3A, 0x4B, 0x5C, 0x6D, 0x7E, 0x8F, 0x90];
    let key = sha256_7z_kdf(password, &salt, cycles_power);
    let mut encrypted = vec![0u8; padded.len()];
    aes256_cbc_encrypt(&key, &iv, &padded, &mut encrypted).expect("AES-256-CBC encrypt");

    let mut aes_props = Vec::with_capacity(34);
    aes_props.push(((cycles_power as u8) & 0x3F) | 0xC0);
    aes_props.push(0xFF);
    aes_props.extend_from_slice(&salt);
    aes_props.extend_from_slice(&iv);
    (encrypted, aes_props, raw_comp.len() as u64)
}

fn append_files_info_section(entries: &[(&str, &[u8])], h: &mut Vec<u8>) {
    use ttzip_engine::sevenz::format::*;
    h.push(K_FILES_INFO);
    write_varint(entries.len() as u64, h);

    h.push(K_NAME);
    let mut names_u16 = Vec::new();
    for (name, _) in entries {
        for u in name.encode_utf16() {
            names_u16.extend_from_slice(&u.to_le_bytes());
        }
        names_u16.extend_from_slice(&0u16.to_le_bytes());
    }
    write_varint((1 + names_u16.len()) as u64, h);
    h.push(0);
    h.extend_from_slice(&names_u16);

    h.push(K_WIN_ATTRIBUTES);
    write_varint((2 + entries.len() * 4) as u64, h);
    h.push(1);
    h.push(0);
    for _ in entries {
        h.extend_from_slice(&0x81A4_0020u32.to_le_bytes());
    }

    h.push(K_END);
}

/// Builds a synthetic external-`7zz` (`[LZMA2, AES]` coder order) archive with plaintext header.
fn build_external_7zz_lzma2_aes_archive(
    entries: &[(&str, &[u8])],
    password: &str,
) -> Vec<u8> {
    use ttzip_engine::crypto::crc32::crc32_fast;
    use ttzip_engine::sevenz::format::*;

    let mut solid_plain = Vec::new();
    for (_, data) in entries {
        solid_plain.extend_from_slice(data);
    }
    let (raw_comp, dict_prop) = lzma2_compress_raw(&solid_plain);
    let (encrypted_payload, aes_props, raw_comp_len) = aes_encrypt_7z_padded(&raw_comp, password, 4);

    let mut h = Vec::new();
    h.push(K_HEADER);
    h.push(K_MAIN_STREAMS_INFO);

    h.push(K_PACK_INFO);
    write_varint(0, &mut h);
    write_varint(1, &mut h);
    h.push(K_SIZE);
    write_varint(encrypted_payload.len() as u64, &mut h);
    h.push(K_END);

    h.push(K_UNPACK_INFO);
    h.push(K_FOLDER);
    write_varint(1, &mut h);
    h.push(0);
    write_varint(2, &mut h);

    // Coder 0: LZMA2 (final output coder comes FIRST in external 7zz)
    h.push(0x21);
    h.push(METHOD_LZMA2 as u8);
    write_varint(1, &mut h);
    h.push(dict_prop);

    // Coder 1: AES-256 (feeds into Coder 0 via bind pair (0, 1))
    h.push(0x24);
    h.extend_from_slice(&[0x06, 0xF1, 0x07, 0x01]);
    write_varint(aes_props.len() as u64, &mut h);
    h.extend_from_slice(&aes_props);

    write_varint(0, &mut h); // in_idx = 0 (LZMA2 input)
    write_varint(1, &mut h); // out_idx = 1 (AES output)

    // Unpack sizes in coder order: [uncompressed_len (Coder 0), raw_comp_len (Coder 1)]
    h.push(K_CODERS_UNPACK_SIZE);
    write_varint(solid_plain.len() as u64, &mut h);
    write_varint(raw_comp_len, &mut h);
    h.push(K_END);

    h.push(K_SUB_STREAMS_INFO);
    if entries.len() > 1 {
        h.push(K_NUM_UNPACK_STREAM);
        write_varint(entries.len() as u64, &mut h);
        h.push(K_SIZE);
        for (_, data) in &entries[..entries.len() - 1] {
            write_varint(data.len() as u64, &mut h);
        }
    }
    h.push(K_CRC);
    h.push(1);
    for (_, data) in entries {
        h.extend_from_slice(&crc32_fast(0, data).to_le_bytes());
    }
    h.push(K_END);
    h.push(K_END); // end MainStreamsInfo

    append_files_info_section(entries, &mut h);
    h.push(K_END); // end Header

    let sig = SevenZSignatureHeader {
        major_version: 0,
        minor_version: 4,
        start_header_crc: 0,
        next_header_offset: encrypted_payload.len() as u64,
        next_header_size: h.len() as u64,
        next_header_crc: crc32_fast(0, &h),
    };
    let mut out = Vec::new();
    out.extend_from_slice(&sig.serialize());
    out.extend_from_slice(&encrypted_payload);
    out.extend_from_slice(&h);
    out
}

/// Builds a synthetic 7z archive with `[BCJ_X86, LZMA2, AES]` filtered encrypted coder chain.
fn build_filtered_bcj_lzma2_aes_archive(
    entry_name: &str,
    plaintext: &[u8],
    password: &str,
) -> Vec<u8> {
    use ttzip_engine::codecs::branch::x86_encode;
    use ttzip_engine::crypto::crc32::crc32_fast;
    use ttzip_engine::sevenz::format::*;

    let mut prefiltered = plaintext.to_vec();
    let converted = x86_encode(&mut prefiltered, 0);
    assert!(converted > 0 && prefiltered != plaintext, "BCJ_X86 must transform x86 relative calls");

    let (raw_comp, dict_prop) = lzma2_compress_raw(&prefiltered);
    let (encrypted_payload, aes_props, raw_comp_len) = aes_encrypt_7z_padded(&raw_comp, password, 4);

    let mut h = Vec::new();
    h.push(K_HEADER);
    h.push(K_MAIN_STREAMS_INFO);

    h.push(K_PACK_INFO);
    write_varint(0, &mut h);
    write_varint(1, &mut h);
    h.push(K_SIZE);
    write_varint(encrypted_payload.len() as u64, &mut h);
    h.push(K_END);

    h.push(K_UNPACK_INFO);
    h.push(K_FOLDER);
    write_varint(1, &mut h);
    h.push(0);
    write_varint(3, &mut h);

    // Coder 0: BCJ_X86 (0x03030103)
    h.push(0x04);
    h.extend_from_slice(&[0x03, 0x03, 0x01, 0x03]);
    // Coder 1: LZMA2 (0x21)
    h.push(0x21);
    h.push(METHOD_LZMA2 as u8);
    write_varint(1, &mut h);
    h.push(dict_prop);
    // Coder 2: AES (0x06F10701)
    h.push(0x24);
    h.extend_from_slice(&[0x06, 0xF1, 0x07, 0x01]);
    write_varint(aes_props.len() as u64, &mut h);
    h.extend_from_slice(&aes_props);

    // Bind pairs: BCJ_X86(in 0) <- LZMA2(out 1), LZMA2(in 1) <- AES(out 2)
    write_varint(0, &mut h);
    write_varint(1, &mut h);
    write_varint(1, &mut h);
    write_varint(2, &mut h);

    h.push(K_CODERS_UNPACK_SIZE);
    write_varint(plaintext.len() as u64, &mut h);
    write_varint(prefiltered.len() as u64, &mut h);
    write_varint(raw_comp_len, &mut h);
    h.push(K_END);

    h.push(K_SUB_STREAMS_INFO);
    h.push(K_CRC);
    h.push(1);
    h.extend_from_slice(&crc32_fast(0, plaintext).to_le_bytes());
    h.push(K_END);
    h.push(K_END);

    append_files_info_section(&[(entry_name, plaintext)], &mut h);
    h.push(K_END);

    let sig = SevenZSignatureHeader {
        major_version: 0,
        minor_version: 4,
        start_header_crc: 0,
        next_header_offset: encrypted_payload.len() as u64,
        next_header_size: h.len() as u64,
        next_header_crc: crc32_fast(0, &h),
    };
    let mut out = Vec::new();
    out.extend_from_slice(&sig.serialize());
    out.extend_from_slice(&encrypted_payload);
    out.extend_from_slice(&h);
    out
}

fn write_two_volume_split(dir: & std::path::Path, stem: &str, bytes: &[u8]) -> (String, String) {
    let mid = bytes.len() / 2;
    assert!(mid > 32 && mid < bytes.len(), "split boundary must divide archive across 2 volumes");
    let part1 = dir.join(format!("{stem}.7z.001"));
    let part2 = dir.join(format!("{stem}.7z.002"));
    fs::write(&part1, &bytes[..mid]).unwrap();
    fs::write(&part2, &bytes[mid..]).unwrap();
    (
        part1.to_string_lossy().into_owned(),
        part2.to_string_lossy().into_owned(),
    )
}

#[test]
#[expect(deprecated, reason = "regression coverage for legacy inspect_archive_entries API")]
fn test_split_7z_external_lzma2_aes_coder_order_inspect_and_preview() {
    use ttzip_engine::uniffi_api::archive::{
        extract_single_entry_stream_guarded, inspect_archive_entries,
    };
    use ttzip_engine::uniffi_api::extraction::extract_single_entry_by_path;
    use ttzip_engine::uniffi_api::types::TTZipError;

    let dir = tempdir().unwrap();
    let payload_a: Vec<u8> = (0..4096).map(|i| ((i * 73 + 19) % 251) as u8).collect();
    let payload_b: Vec<u8> = (0..6144).map(|i| ((i * 41 + 7) % 253) as u8).collect();
    let password = "Split7zPassword!2026";

    let archive_bytes = build_external_7zz_lzma2_aes_archive(
        &[("docs/alpha.bin", &payload_a), ("docs/beta.bin", &payload_b)],
        password,
    );
    let (part1_path, _part2_path) = write_two_volume_split(dir.path(), "ext7zz", &archive_bytes);

    let entries = inspect_archive_entries(part1_path.clone(), None)
        .expect("plaintext header split 7z must inspect with password = None");
    assert_eq!(entries.len(), 2);
    assert!(entries[0].is_encrypted);
    assert!(entries[1].is_encrypted);
    assert_eq!(entries[0].uncompressed_size, payload_a.len() as u64);
    assert_eq!(entries[1].uncompressed_size, payload_b.len() as u64);

    for bad_pwd in [None, Some("WrongPassword!999".to_string())] {
        let err_by_path = extract_single_entry_by_path(
            part1_path.clone(),
            "docs/alpha.bin".to_string(),
            bad_pwd.clone(),
        )
        .expect_err("missing/wrong password must fail on split encrypted 7z");
        assert!(
            matches!(err_by_path, TTZipError::InvalidPassword),
            "expected InvalidPassword from extract_single_entry_by_path, got {err_by_path:?}"
        );

        let err_guarded =
            extract_single_entry_stream_guarded(part1_path.clone(), 1, bad_pwd, 64)
                .expect_err("missing/wrong password must fail on guarded stream");
        assert!(
            matches!(err_guarded, TTZipError::InvalidPassword),
            "expected InvalidPassword from extract_single_entry_stream_guarded, got {err_guarded:?}"
        );
    }

    let got_a = extract_single_entry_by_path(
        part1_path.clone(),
        "docs/alpha.bin".to_string(),
        Some(password.to_string()),
    )
    .expect("correct password must extract entry 0 by path across split volumes");
    assert_eq!(got_a, payload_a);

    let got_b = extract_single_entry_stream_guarded(
        part1_path,
        1,
        Some(password.to_string()),
        64,
    )
    .expect("correct password must extract entry 1 via guarded stream across split volumes");
    assert_eq!(got_b, payload_b);
}

#[test]
fn test_split_7z_encrypted_header_preview_and_password_verification() {
    use ttzip_engine::sevenz::writer::create_7z_archive_explicit_split_with_header_encryption;
    use ttzip_engine::types::{
        TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    };
    use ttzip_engine::uniffi_api::archive::extract_single_entry_stream_guarded;
    use ttzip_engine::uniffi_api::extraction::extract_single_entry_by_path;
    use ttzip_engine::uniffi_api::types::TTZipError;

    let dir = tempdir().unwrap();
    let src_file = dir.path().join("vault_secret.bin");
    let payload: Vec<u8> = (0..16384).map(|i| ((i * 97 + 31) % 256) as u8).collect();
    fs::write(&src_file, &payload).unwrap();

    let password = "HeaderLockedSplit!2026";
    let pwd_c = CString::new(password).unwrap();
    let options = TTZipCreateOptions {
        format: TTZipArchiveFormat::SevenZip,
        level: TTZipCompressionLevel::Store,
        encryption: TTZipEncryptionMethod::Aes256,
        password: pwd_c.as_ptr(),
        thread_budget: 1,
        ..Default::default()
    };
    let base_out = dir.path().join("mhe_split.7z");
    create_7z_archive_explicit_split_with_header_encryption(
        &base_out,
        &[(src_file, "vault_secret.bin".to_string())],
        &options,
        8192,
        Some(true),
    )
    .expect("create split encrypted-header 7z");

    let part1 = dir.path().join("mhe_split.7z.001");
    let part2 = dir.path().join("mhe_split.7z.002");
    assert!(part1.exists() && part2.exists());
    let part1_str = part1.to_string_lossy().into_owned();

    for bad_pwd in [None, Some("WrongHeaderPass".to_string())] {
        let err_path = extract_single_entry_by_path(
            part1_str.clone(),
            "vault_secret.bin".to_string(),
            bad_pwd.clone(),
        )
        .expect_err("encrypted header split 7z must reject missing/wrong password");
        assert!(matches!(err_path, TTZipError::InvalidPassword));

        let err_guarded =
            extract_single_entry_stream_guarded(part1_str.clone(), 0, bad_pwd, 64)
                .expect_err("encrypted header split 7z guarded stream must reject missing/wrong password");
        assert!(matches!(err_guarded, TTZipError::InvalidPassword));
    }

    let dec_path = extract_single_entry_by_path(
        part1_str.clone(),
        "vault_secret.bin".to_string(),
        Some(password.to_string()),
    )
    .expect("correct password must decrypt split encrypted-header 7z by path");
    assert_eq!(dec_path, payload);

    let dec_guarded = extract_single_entry_stream_guarded(
        part1_str,
        0,
        Some(password.to_string()),
        64,
    )
    .expect("correct password must decrypt split encrypted-header 7z via guarded stream");
    assert_eq!(dec_guarded, payload);
}

#[test]
fn test_split_7z_filtered_bcj_x86_lzma2_aes_folder_preview() {
    use ttzip_engine::uniffi_api::archive::extract_single_entry_stream_guarded;
    use ttzip_engine::uniffi_api::extraction::extract_single_entry_by_path;
    use ttzip_engine::uniffi_api::types::TTZipError;

    let dir = tempdir().unwrap();
    let mut x86_code = Vec::new();
    for step in 0..120u32 {
        x86_code.extend_from_slice(&[0x55, 0x48, 0x89, 0xE5]);
        x86_code.push(0xE8);
        x86_code.extend_from_slice(&((step * 16) as i32).to_le_bytes());
        x86_code.push(0xE9);
        x86_code.extend_from_slice(&((step * 32) as i32).to_le_bytes());
        x86_code.extend_from_slice(&[0x5D, 0xC3]);
    }
    let password = "FilteredSplitPassword!2026";
    let archive_bytes = build_filtered_bcj_lzma2_aes_archive("bin/app.exe", &x86_code, password);
    let (part1_path, _part2_path) = write_two_volume_split(dir.path(), "filtered", &archive_bytes);

    let wrong = extract_single_entry_by_path(
        part1_path.clone(),
        "bin/app.exe".to_string(),
        Some("WrongPass".to_string()),
    )
    .expect_err("wrong password must fail on filtered split 7z");
    assert!(matches!(wrong, TTZipError::InvalidPassword));

    let by_path = extract_single_entry_by_path(
        part1_path.clone(),
        "bin/app.exe".to_string(),
        Some(password.to_string()),
    )
    .expect("filtered split 7z must decode post-filter bytes by path");
    assert_eq!(by_path, x86_code);

    let by_guarded = extract_single_entry_stream_guarded(
        part1_path,
        0,
        Some(password.to_string()),
        64,
    )
    .expect("filtered split 7z must decode post-filter bytes via guarded stream");
    assert_eq!(by_guarded, x86_code);
}
