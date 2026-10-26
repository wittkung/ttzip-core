// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! R3 Regression Suite: Q1 C3-04 ZipCrypto Fixture, Data Descriptor, Zero-CRC32 Integrity Boundary,
//! WinZip AE-1 vs AE-2 Version Contract, and Security/Cancellation/I/O Error Semantics.

use std::ffi::CString;
use std::fs;
use std::io::{self, Cursor, Write};
use tempfile::TempDir;
use ttzip_archive::archive::unified::inspect::inspect_archive;
use ttzip_archive::archive::unified::UnifiedArchiveOrchestrator;
use ttzip_archive::codecs::deflate::{deflate_compress, deflate_compress_bound};
use ttzip_archive::crypto::crc32::crc32_fast;
use ttzip_archive::crypto::zipcrypto::ZipCryptoKeys;
use ttzip_archive::fs::collision::staging::StagingRoot;
use ttzip_archive::types::{TTZipEncryptionMethod, TTZipExtractOptions, TTZipStatus};
use ttzip_archive::zip::extra::{
    TAG_WINZIP_AES, WINZIP_AES_VENDOR_ID, WINZIP_AES_VERSION_AE1, WINZIP_AES_VERSION_AE2,
};
use ttzip_archive::zip::reader::ZipArchive;
use ttzip_archive::zip::reader_stream::{
    extract_zip_entry_to_sink_from_reader, parse_entries_from_reader,
};
use ttzip_archive::zip::writer::{
    assemble_zip_archive, compress_items_parallel, unix_to_dos_time, ZipInputItem,
};
use ttzip_types::options::ProgressAtomicSheet;

const FIXED_MTIME_2023_NOV: i64 = 1700001234;

/// Embedded 507-byte Q1 C3-04 fixture (`ext_zip_zipcrypto.zip`, created by 7z with `-mem=ZipCrypto -pSecret2026!`).
const Q1_EXT_ZIP_ZIPCRYPTO_BYTES: &[u8; 507] = &[
    0x50, 0x4b, 0x03, 0x04, 0x14, 0x03, 0x01, 0x00, 0x00, 0x00, 0x39, 0xa4, 0x51, 0x5d, 0xb4, 0xcc,
    0x5b, 0xb3, 0x2e, 0x00, 0x00, 0x00, 0x22, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x68, 0x65,
    0x6c, 0x6c, 0x6f, 0x2e, 0x74, 0x78, 0x74, 0x38, 0xd9, 0x67, 0xde, 0x2e, 0x8d, 0xc9, 0x15, 0xe6,
    0x22, 0xb3, 0xcf, 0x84, 0x3a, 0xce, 0x38, 0xc6, 0x4b, 0x87, 0x5b, 0x11, 0x6b, 0xc5, 0x15, 0xd2,
    0x38, 0x23, 0x1e, 0xaf, 0x29, 0x2d, 0xce, 0xa5, 0x5d, 0x8a, 0x55, 0x3e, 0x6f, 0x72, 0xeb, 0x51,
    0x6b, 0xa8, 0xc6, 0x70, 0x62, 0x50, 0x4b, 0x03, 0x04, 0x14, 0x03, 0x00, 0x00, 0x00, 0x00, 0x39,
    0xa4, 0x51, 0x5d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04,
    0x00, 0x00, 0x00, 0x73, 0x75, 0x62, 0x2f, 0x50, 0x4b, 0x03, 0x04, 0x14, 0x03, 0x01, 0x00, 0x00,
    0x00, 0x39, 0xa4, 0x51, 0x5d, 0x04, 0xdf, 0xf2, 0x22, 0x31, 0x00, 0x00, 0x00, 0x25, 0x00, 0x00,
    0x00, 0x0e, 0x00, 0x00, 0x00, 0x73, 0x75, 0x62, 0x2f, 0x6e, 0x65, 0x73, 0x74, 0x65, 0x64, 0x2e,
    0x74, 0x78, 0x74, 0x94, 0x81, 0xc0, 0xbf, 0x69, 0xfe, 0xbd, 0x4b, 0xa2, 0xd8, 0xbe, 0x9a, 0x10,
    0x4b, 0x10, 0x6c, 0x34, 0x2c, 0x26, 0x2a, 0xb3, 0x92, 0xdb, 0xb2, 0x19, 0xe1, 0xb0, 0xf6, 0xa1,
    0x29, 0x4f, 0xb8, 0x20, 0x1a, 0xb2, 0xa5, 0xb7, 0xfa, 0x38, 0xbc, 0xfb, 0x6a, 0xaf, 0xe0, 0xc4,
    0x10, 0xd1, 0xa8, 0xda, 0x50, 0x4b, 0x01, 0x02, 0x3f, 0x03, 0x14, 0x03, 0x01, 0x00, 0x00, 0x00,
    0x39, 0xa4, 0x51, 0x5d, 0xb4, 0xcc, 0x5b, 0xb3, 0x2e, 0x00, 0x00, 0x00, 0x22, 0x00, 0x00, 0x00,
    0x09, 0x00, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x80, 0xa4, 0x81, 0x00, 0x00,
    0x00, 0x00, 0x68, 0x65, 0x6c, 0x6c, 0x6f, 0x2e, 0x74, 0x78, 0x74, 0x0a, 0x00, 0x20, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x18, 0x00, 0x80, 0x74, 0x78, 0xc2, 0x33, 0x5e, 0xdd, 0x01, 0x00,
    0x0b, 0x11, 0xc3, 0x33, 0x5e, 0xdd, 0x01, 0x80, 0x74, 0x78, 0xc2, 0x33, 0x5e, 0xdd, 0x01, 0x50,
    0x4b, 0x01, 0x02, 0x3f, 0x03, 0x14, 0x03, 0x00, 0x00, 0x00, 0x00, 0x39, 0xa4, 0x51, 0x5d, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x24, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x80, 0xed, 0x41, 0x55, 0x00, 0x00, 0x00, 0x73, 0x75, 0x62,
    0x2f, 0x0a, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x18, 0x00, 0x80, 0x74, 0x78,
    0xc2, 0x33, 0x5e, 0xdd, 0x01, 0x00, 0x0b, 0x11, 0xc3, 0x33, 0x5e, 0xdd, 0x01, 0x80, 0x74, 0x78,
    0xc2, 0x33, 0x5e, 0xdd, 0x01, 0x50, 0x4b, 0x01, 0x02, 0x3f, 0x03, 0x14, 0x03, 0x01, 0x00, 0x00,
    0x00, 0x39, 0xa4, 0x51, 0x5d, 0x04, 0xdf, 0xf2, 0x22, 0x31, 0x00, 0x00, 0x00, 0x25, 0x00, 0x00,
    0x00, 0x0e, 0x00, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x80, 0xa4, 0x81, 0x77,
    0x00, 0x00, 0x00, 0x73, 0x75, 0x62, 0x2f, 0x6e, 0x65, 0x73, 0x74, 0x65, 0x64, 0x2e, 0x74, 0x78,
    0x74, 0x0a, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x18, 0x00, 0x80, 0x74, 0x78,
    0xc2, 0x33, 0x5e, 0xdd, 0x01, 0x00, 0x0b, 0x11, 0xc3, 0x33, 0x5e, 0xdd, 0x01, 0x80, 0x74, 0x78,
    0xc2, 0x33, 0x5e, 0xdd, 0x01, 0x50, 0x4b, 0x05, 0x06, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x03,
    0x00, 0x11, 0x01, 0x00, 0x00, 0xd4, 0x00, 0x00, 0x00, 0x00, 0x00,
];

fn zip_item(rel_path: &str, data: Vec<u8>, mtime: i64, mode: u32) -> ZipInputItem {
    ZipInputItem {
        rel_path: rel_path.to_string(),
        data,
        mtime_epoch_secs: mtime as u32,
        mode,
        is_directory: false,
    }
}

fn make_extract_opts(password: Option<&CString>) -> TTZipExtractOptions {
    TTZipExtractOptions {
        password: password.map_or(std::ptr::null(), |c| c.as_ptr()),
        preserve_permissions: true,
        thread_budget: 2,
        ..Default::default()
    }
}

fn write_single_entry_zip_frame(
    name: &[u8],
    flag: u16,
    method: u16,
    (dos_time, dos_date): (u16, u16),
    (crc, comp_len, uncomp_len): (u32, u32, u32),
    payload: &[u8],
    use_data_descriptor: bool,
) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&0x04034b50u32.to_le_bytes());
    buf.extend_from_slice(&20u16.to_le_bytes());
    buf.extend_from_slice(&flag.to_le_bytes());
    buf.extend_from_slice(&method.to_le_bytes());
    buf.extend_from_slice(&dos_time.to_le_bytes());
    buf.extend_from_slice(&dos_date.to_le_bytes());
    if use_data_descriptor {
        buf.extend_from_slice(&[0u8; 12]);
    } else {
        buf.extend_from_slice(&crc.to_le_bytes());
        buf.extend_from_slice(&comp_len.to_le_bytes());
        buf.extend_from_slice(&uncomp_len.to_le_bytes());
    }
    buf.extend_from_slice(&(name.len() as u16).to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(name);
    buf.extend_from_slice(payload);

    if use_data_descriptor {
        buf.extend_from_slice(&0x08074b50u32.to_le_bytes());
        buf.extend_from_slice(&crc.to_le_bytes());
        buf.extend_from_slice(&comp_len.to_le_bytes());
        buf.extend_from_slice(&uncomp_len.to_le_bytes());
    }

    let cd_off = buf.len() as u32;
    buf.extend_from_slice(&0x02014b50u32.to_le_bytes());
    buf.extend_from_slice(&0x031eu16.to_le_bytes());
    buf.extend_from_slice(&20u16.to_le_bytes());
    buf.extend_from_slice(&flag.to_le_bytes());
    buf.extend_from_slice(&method.to_le_bytes());
    buf.extend_from_slice(&dos_time.to_le_bytes());
    buf.extend_from_slice(&dos_date.to_le_bytes());
    buf.extend_from_slice(&crc.to_le_bytes());
    buf.extend_from_slice(&comp_len.to_le_bytes());
    buf.extend_from_slice(&uncomp_len.to_le_bytes());
    buf.extend_from_slice(&(name.len() as u16).to_le_bytes());
    buf.extend_from_slice(&[0u8; 8]);
    buf.extend_from_slice(&(0o100644u32 << 16).to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(name);

    let cd_size = (buf.len() as u32) - cd_off;
    buf.extend_from_slice(&0x06054b50u32.to_le_bytes());
    buf.extend_from_slice(&[0u8; 4]);
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&cd_size.to_le_bytes());
    buf.extend_from_slice(&cd_off.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf
}

fn build_unencrypted_store_archive(dos_time: u16, dos_date: u16, payload: &[u8]) -> Vec<u8> {
    let crc = crc32_fast(0, payload);
    let len = payload.len() as u32;
    write_single_entry_zip_frame(
        b"pure_dos.txt",
        0,
        0,
        (dos_time, dos_date),
        (crc, len, len),
        payload,
        false,
    )
}

fn build_zipcrypto_data_descriptor_archive(
    payload: &[u8],
    password: &str,
    mtime: u32,
) -> Vec<u8> {
    let crc = crc32_fast(0, payload);
    let (dos_time, dos_date) = unix_to_dos_time(mtime);
    let mut comp = vec![0u8; deflate_compress_bound(payload.len(), 6)];
    let comp_len = deflate_compress(payload, &mut comp, 6).unwrap();
    comp.truncate(comp_len);

    let mut keys = ZipCryptoKeys::from_password(password.as_bytes());
    let mut header = [
        0x11,
        0x22,
        0x33,
        0x44,
        0x55,
        0x66,
        0x77,
        0x88,
        0x99,
        0xaa,
        0xbb,
        (dos_time >> 8) as u8,
    ];
    keys.encrypt_slice(&mut header);
    keys.encrypt_slice(&mut comp);

    let mut enc_payload = Vec::with_capacity(12 + comp.len());
    enc_payload.extend_from_slice(&header);
    enc_payload.extend_from_slice(&comp);

    write_single_entry_zip_frame(
        b"dd_entry.txt",
        0x0009,
        8,
        (dos_time, dos_date),
        (crc, enc_payload.len() as u32, payload.len() as u32),
        &enc_payload,
        true,
    )
}

fn build_winzip_aes_archive_variant(
    payload: &[u8],
    password: &str,
    lfh_aes_version: u16,
    cdfh_aes_version: u16,
    vendor_id: u16,
    declared_crc: u32,
) -> Vec<u8> {
    let items = vec![zip_item(
        "aes_item.bin",
        payload.to_vec(),
        FIXED_MTIME_2023_NOV,
        0o100644,
    )];
    let compressed = compress_items_parallel(
        items,
        6,
        TTZipEncryptionMethod::Aes256,
        Some(password),
        1,
    )
    .unwrap();
    let mut zip = assemble_zip_archive(&compressed).unwrap();

    let name_len = "aes_item.bin".len();
    let lfh_extra_pos = 30 + name_len;
    assert_eq!(
        u16::from_le_bytes([zip[lfh_extra_pos], zip[lfh_extra_pos + 1]]),
        TAG_WINZIP_AES
    );
    zip[14..18].copy_from_slice(&declared_crc.to_le_bytes());
    zip[lfh_extra_pos + 4..lfh_extra_pos + 6].copy_from_slice(&lfh_aes_version.to_le_bytes());
    zip[lfh_extra_pos + 6..lfh_extra_pos + 8].copy_from_slice(&vendor_id.to_le_bytes());

    let eocd_pos = zip.len() - 22;
    let cd_off =
        u32::from_le_bytes(zip[eocd_pos + 16..eocd_pos + 20].try_into().unwrap()) as usize;
    let cd_extra_pos = cd_off + 46 + name_len;
    assert_eq!(
        u16::from_le_bytes([zip[cd_extra_pos], zip[cd_extra_pos + 1]]),
        TAG_WINZIP_AES
    );
    zip[cd_off + 16..cd_off + 20].copy_from_slice(&declared_crc.to_le_bytes());
    zip[cd_extra_pos + 4..cd_extra_pos + 6].copy_from_slice(&cdfh_aes_version.to_le_bytes());
    zip[cd_extra_pos + 6..cd_extra_pos + 8].copy_from_slice(&vendor_id.to_le_bytes());
    zip
}

fn assert_all_readers_ok(zip_bytes: &[u8], password: Option<&str>, expected: &[u8]) {
    let za = ZipArchive::open_slice(zip_bytes).unwrap();
    assert_eq!(za.extract_entry_bytes(0, password).unwrap(), expected);

    let mut sink = Vec::new();
    za.extract_entry_to_sink(0, password, &mut sink).unwrap();
    assert_eq!(sink, expected);

    let mut cur = Cursor::new(zip_bytes);
    let entries = parse_entries_from_reader(&mut cur, zip_bytes.len() as u64).unwrap();
    let mut r_sink = Vec::new();
    extract_zip_entry_to_sink_from_reader(
        &mut cur,
        zip_bytes.len() as u64,
        &entries[0],
        password,
        &mut r_sink,
        None,
    )
    .unwrap();
    assert_eq!(r_sink, expected);
}

fn assert_all_readers_err(zip_bytes: &[u8], password: Option<&str>, expected: TTZipStatus) {
    let za = ZipArchive::open_slice(zip_bytes).unwrap();
    assert_eq!(za.extract_entry_bytes(0, password), Err(expected));
    assert_eq!(
        za.extract_entry_to_sink(0, password, &mut Vec::new()),
        Err(expected)
    );

    let mut cur = Cursor::new(zip_bytes);
    let entries = parse_entries_from_reader(&mut cur, zip_bytes.len() as u64).unwrap();
    assert_eq!(
        extract_zip_entry_to_sink_from_reader(
            &mut cur,
            zip_bytes.len() as u64,
            &entries[0],
            password,
            &mut Vec::new(),
            None,
        ),
        Err(expected)
    );
}

fn four_bytes_with_zero_crc32() -> [u8; 4] {
    let mut state: u32 = 0xffff_ffff;
    for _ in 0..32 {
        if (state & 0x8000_0000) != 0 {
            state = ((state ^ 0xedb8_8320) << 1) | 1;
        } else {
            state <<= 1;
        }
    }
    (state ^ 0xffff_ffff).to_le_bytes()
}

#[test]
fn test_c3_04_q1_embedded_fixture_roundtrip_and_password_rejection() {
    let tmp = TempDir::new().unwrap();
    let local_zc = tmp.path().join("ext_zip_zipcrypto_embedded.zip");
    fs::write(&local_zc, Q1_EXT_ZIP_ZIPCRYPTO_BYTES).unwrap();

    let mut inspected = Vec::new();
    inspect_archive(&local_zc, Some("Secret2026!"), true, |e| {
        inspected.push((e.path.to_string(), e.is_encrypted));
        true
    })
    .expect("inspect_archive on embedded Q1 fixture must succeed");
    assert_eq!(inspected.len(), 3);
    assert!(inspected.iter().any(|(p, enc)| p == "hello.txt" && *enc));

    let pwd_ok = CString::new("Secret2026!").unwrap();
    let pwd_bad = CString::new("WrongPass123").unwrap();
    let out_ok = tmp.path().join("q1_zc_ok");
    UnifiedArchiveOrchestrator::extract_archive_with_sheet(
        &local_zc,
        &StagingRoot(out_ok.clone()),
        &make_extract_opts(Some(&pwd_ok)),
        None,
    )
    .expect("Embedded Q1 ext_zip_zipcrypto.zip with Secret2026! must succeed");
    assert_eq!(
        fs::read_to_string(out_ok.join("hello.txt")).unwrap(),
        "Hello from external tool archive!\n"
    );
    assert!(out_ok.join("sub/nested.txt").exists());

    let z_arch = ZipArchive::open_slice(Q1_EXT_ZIP_ZIPCRYPTO_BYTES).unwrap();
    assert_eq!(
        z_arch.extract_all(
            &tmp.path().join("q1_zc_bad"),
            &make_extract_opts(Some(&pwd_bad)),
        ),
        Err(TTZipStatus::ErrInvalidPassword)
    );
    assert_eq!(
        z_arch.extract_all(&tmp.path().join("q1_zc_nopw"), &make_extract_opts(None)),
        Err(TTZipStatus::ErrInvalidPassword)
    );
}

#[test]
fn test_c3_04_zipcrypto_data_descriptor_roundtrip_and_wrong_password() {
    let payload =
        b"ZipCrypto payload with data descriptor (bit 3) and historical mtime\n".repeat(16);
    let zc_dd = build_zipcrypto_data_descriptor_archive(
        &payload,
        "Secret2026!",
        FIXED_MTIME_2023_NOV as u32,
    );
    assert_all_readers_ok(&zc_dd, Some("Secret2026!"), &payload);
    assert_all_readers_err(
        &zc_dd,
        Some("wrong-password-999"),
        TTZipStatus::ErrInvalidPassword,
    );
}

#[test]
fn test_zero_crc32_non_empty_zipcrypto_and_unencrypted_verification() {
    let payload = four_bytes_with_zero_crc32();
    assert_eq!(
        crc32_fast(0, &payload),
        0,
        "Fixture must be non-empty with CRC32 == 0"
    );

    // 1. Unencrypted Store archive with CRC32 == 0
    let unenc_zip = build_unencrypted_store_archive(0x4000, 0x576e, &payload);
    assert_eq!(
        ZipArchive::open_slice(&unenc_zip).unwrap().entries()[0].crc32,
        0
    );
    assert_all_readers_ok(&unenc_zip, None, &payload);

    let mut unenc_corrupt = unenc_zip.clone();
    unenc_corrupt[30 + "pure_dos.txt".len()] ^= 0xff;
    assert_all_readers_err(&unenc_corrupt, None, TTZipStatus::ErrCorruptHeader);

    // 2. ZipCrypto Store & Deflate archives with CRC32 == 0
    let items = vec![zip_item(
        "zero_crc.bin",
        payload.to_vec(),
        FIXED_MTIME_2023_NOV,
        0o100644,
    )];
    let compressed = compress_items_parallel(
        items,
        0,
        TTZipEncryptionMethod::ZipCrypto,
        Some("ZeroCrcPass!"),
        1,
    )
    .unwrap();
    let zc_zip = assemble_zip_archive(&compressed).unwrap();
    let za_zc = ZipArchive::open_slice(&zc_zip).unwrap();
    assert_eq!(za_zc.entries()[0].crc32, 0);
    assert_all_readers_ok(&zc_zip, Some("ZeroCrcPass!"), &payload);
    assert_all_readers_err(
        &zc_zip,
        Some("WrongZeroPass"),
        TTZipStatus::ErrInvalidPassword,
    );

    let zc_deflate = build_zipcrypto_data_descriptor_archive(
        &payload,
        "ZeroCrcPass!",
        FIXED_MTIME_2023_NOV as u32,
    );
    assert_eq!(
        ZipArchive::open_slice(&zc_deflate).unwrap().entries()[0].crc32,
        0
    );
    assert_all_readers_ok(&zc_deflate, Some("ZeroCrcPass!"), &payload);

    // Corrupt last ciphertext byte (after 12-byte ZipCrypto header) -> CRC check must catch it
    let mut zc_corrupt = zc_zip.clone();
    let zc_payload_end =
        (za_zc.entries()[0].lfh_offset as usize) + 30 + "zero_crc.bin".len() + 9 + 16 - 1;
    zc_corrupt[zc_payload_end] ^= 0xff;
    assert_all_readers_err(
        &zc_corrupt,
        Some("ZeroCrcPass!"),
        TTZipStatus::ErrInvalidPassword,
    );
}

#[test]
fn test_winzip_aes_ae1_enforces_crc32_including_zero_crc_boundary() {
    let payload = b"WinZip AE-1 authenticated payload requiring strict CRC32 verification\n".repeat(8);
    let real_crc = crc32_fast(0, &payload);
    assert_ne!(real_crc, 0);

    // 1. Valid AE-1 with matching non-zero CRC32
    let ae1_valid = build_winzip_aes_archive_variant(
        &payload,
        "Aes2026!",
        WINZIP_AES_VERSION_AE1,
        WINZIP_AES_VERSION_AE1,
        WINZIP_AES_VENDOR_ID,
        real_crc,
    );
    let parsed_ae1 = ZipArchive::open_slice(&ae1_valid).unwrap();
    assert_eq!(parsed_ae1.entries()[0].aes_version, WINZIP_AES_VERSION_AE1);
    assert_all_readers_ok(&ae1_valid, Some("Aes2026!"), &payload);
    assert_all_readers_err(
        &ae1_valid,
        Some("WrongPassword!"),
        TTZipStatus::ErrInvalidPassword,
    );

    // 2. Valid AE-1 whose plaintext legitimately has CRC32 == 0 and header declares CRC32 == 0
    let zero_crc_payload = four_bytes_with_zero_crc32();
    let ae1_zero_valid = build_winzip_aes_archive_variant(
        &zero_crc_payload,
        "Aes2026!",
        WINZIP_AES_VERSION_AE1,
        WINZIP_AES_VERSION_AE1,
        WINZIP_AES_VENDOR_ID,
        0,
    );
    assert_all_readers_ok(&ae1_zero_valid, Some("Aes2026!"), &zero_crc_payload);

    // 3. AE-1 with valid encrypted payload & valid HMAC tag, but header erroneously declares CRC32 == 0
    let ae1_wrong_zero_crc = build_winzip_aes_archive_variant(
        &payload,
        "Aes2026!",
        WINZIP_AES_VERSION_AE1,
        WINZIP_AES_VERSION_AE1,
        WINZIP_AES_VENDOR_ID,
        0,
    );
    assert_all_readers_err(
        &ae1_wrong_zero_crc,
        Some("Aes2026!"),
        TTZipStatus::ErrCorruptHeader,
    );
}

#[test]
fn test_winzip_aes_ae2_and_invalid_aes_extra_contracts() {
    let payload = b"WinZip AE-2 authenticated payload with suppressed header CRC32\n".repeat(8);

    // 1. Valid AE-2 with header CRC32 == 0 succeeds and preserves HMAC authentication
    let ae2_valid = build_winzip_aes_archive_variant(
        &payload,
        "Aes2026!",
        WINZIP_AES_VERSION_AE2,
        WINZIP_AES_VERSION_AE2,
        WINZIP_AES_VENDOR_ID,
        0,
    );
    let parsed_ae2 = ZipArchive::open_slice(&ae2_valid).unwrap();
    assert_eq!(parsed_ae2.entries()[0].aes_version, WINZIP_AES_VERSION_AE2);
    assert_all_readers_ok(&ae2_valid, Some("Aes2026!"), &payload);
    assert_all_readers_err(
        &ae2_valid,
        Some("WrongPassword!"),
        TTZipStatus::ErrInvalidPassword,
    );

    // Tamper 1 bit in AE-2 10-byte HMAC-SHA1-80 authentication tag -> rejected as ErrInvalidPassword
    let mut ae2_bad_mac = ae2_valid.clone();
    let eocd_pos = ae2_bad_mac.len() - 22;
    let cd_off =
        u32::from_le_bytes(ae2_bad_mac[eocd_pos + 16..eocd_pos + 20].try_into().unwrap()) as usize;
    ae2_bad_mac[cd_off - 1] ^= 0x80;
    assert_all_readers_err(
        &ae2_bad_mac,
        Some("Aes2026!"),
        TTZipStatus::ErrInvalidPassword,
    );

    // 2. Invalid AES version (3), invalid vendor ID, or LFH/CDFH version mismatch must fail with ErrCorruptHeader
    let aes_bad_version = build_winzip_aes_archive_variant(
        &payload,
        "Aes2026!",
        3,
        3,
        WINZIP_AES_VENDOR_ID,
        0,
    );
    assert_all_readers_err(
        &aes_bad_version,
        Some("Aes2026!"),
        TTZipStatus::ErrCorruptHeader,
    );

    let aes_bad_vendor = build_winzip_aes_archive_variant(
        &payload,
        "Aes2026!",
        WINZIP_AES_VERSION_AE2,
        WINZIP_AES_VERSION_AE2,
        0x5a5a,
        0,
    );
    assert_all_readers_err(
        &aes_bad_vendor,
        Some("Aes2026!"),
        TTZipStatus::ErrCorruptHeader,
    );

    let aes_lfh_cdfh_mismatch = build_winzip_aes_archive_variant(
        &payload,
        "Aes2026!",
        WINZIP_AES_VERSION_AE1,
        WINZIP_AES_VERSION_AE2,
        WINZIP_AES_VENDOR_ID,
        0,
    );
    assert_all_readers_err(
        &aes_lfh_cdfh_mismatch,
        Some("Aes2026!"),
        TTZipStatus::ErrCorruptHeader,
    );
}

struct FailingWriteSink;

impl Write for FailingWriteSink {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("simulated disk full"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_zipcrypto_preserves_security_cancellation_and_io_errors() {
    let payload = vec![b'Z'; 12 * 1024];
    let mut zc_dd = build_zipcrypto_data_descriptor_archive(
        &payload,
        "Secret2026!",
        FIXED_MTIME_2023_NOV as u32,
    );
    let cd_uncomp_pos = zc_dd.len() - 22 - (46 + "dd_entry.txt".len()) + 24;
    zc_dd[cd_uncomp_pos..cd_uncomp_pos + 4].copy_from_slice(&16u32.to_le_bytes());
    assert_all_readers_err(
        &zc_dd,
        Some("Secret2026!"),
        TTZipStatus::ErrSecurityViolation,
    );

    let valid_zc = build_zipcrypto_data_descriptor_archive(
        &payload,
        "Secret2026!",
        FIXED_MTIME_2023_NOV as u32,
    );
    let za_ok = ZipArchive::open_slice(&valid_zc).unwrap();
    let cancel_sheet = ProgressAtomicSheet::new();
    cancel_sheet.cancel();

    assert_eq!(
        za_ok.extract_entry_to_sink_with_sheet(
            0,
            Some("Secret2026!"),
            &mut Vec::new(),
            Some(&cancel_sheet),
        ),
        Err(TTZipStatus::Cancelled)
    );
    assert_eq!(
        za_ok.extract_entry_to_sink(0, Some("Secret2026!"), &mut FailingWriteSink),
        Err(TTZipStatus::ErrExtractionFailed)
    );

    let mut cur_ok = Cursor::new(&valid_zc);
    let ok_entries = parse_entries_from_reader(&mut cur_ok, valid_zc.len() as u64).unwrap();
    assert_eq!(
        extract_zip_entry_to_sink_from_reader(
            &mut cur_ok,
            valid_zc.len() as u64,
            &ok_entries[0],
            Some("Secret2026!"),
            &mut Vec::new(),
            Some(&cancel_sheet),
        ),
        Err(TTZipStatus::Cancelled)
    );
    assert_eq!(
        extract_zip_entry_to_sink_from_reader(
            &mut cur_ok,
            valid_zc.len() as u64,
            &ok_entries[0],
            Some("Secret2026!"),
            &mut FailingWriteSink,
            None,
        ),
        Err(TTZipStatus::ErrExtractionFailed)
    );
}
