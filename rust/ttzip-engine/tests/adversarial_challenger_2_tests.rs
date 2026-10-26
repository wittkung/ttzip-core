// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Adversarial Verification Test Suite for Challenger 2.
//!
//! Stress-tests two core microkernel architectural domains:
//! 1. Multi-volume splitting/chaining with missing volumes, out-of-order sequences, zero-byte chunks.
//! 2. Charset auto-detection heuristics under mixed encoding inputs (GBK, Big5, Shift-JIS, CP437, EUC-KR).

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use tempfile::tempdir;

use ttzip_engine::archive::split::{detect_volume_chain, VirtualMultiVolumeReader};
use ttzip_engine::charset::{
    detect_charset_with_confidence, sanitize_filename, sanitize_filename_to_slice,
};
use ttzip_engine::types::TTZipStatus;

// ============================================================================
// Domain 1: Multi-Volume Splitting & Chaining
// ============================================================================

#[test]
fn test_adv_multivol_missing_middle_volume_chain_truncation() {
    let dir = tempdir().unwrap();
    let vol1 = dir.path().join("archive.7z.001");
    let vol2 = dir.path().join("archive.7z.002");
    // Missing vol3: "archive.7z.003"
    let vol4 = dir.path().join("archive.7z.004");

    fs::write(&vol1, b"PART1_AAA").unwrap();
    fs::write(&vol2, b"PART2_BBB").unwrap();
    fs::write(&vol4, b"PART4_DDD").unwrap();

    // 1. Detection from vol1 rejects missing intermediate volume instead of silently truncating
    let err_from_1 = detect_volume_chain(&vol1).expect_err("Missing middle volume must be rejected");
    assert_eq!(err_from_1.kind(), std::io::ErrorKind::NotFound);
    assert!(
        err_from_1.to_string().contains("archive.7z.003"),
        "Expected missing volume name 'archive.7z.003' in error, got: {}",
        err_from_1
    );

    // 2. Detection from vol4 also rejects the missing intermediate volume
    let err_from_4 = detect_volume_chain(&vol4).expect_err("Missing middle volume from part 4 must be rejected");
    assert_eq!(err_from_4.kind(), std::io::ErrorKind::NotFound);
    assert!(
        err_from_4.to_string().contains("archive.7z.003"),
        "Expected missing volume name 'archive.7z.003' in error, got: {}",
        err_from_4
    );

    // 3. Passing an explicit volume list directly to VirtualMultiVolumeReader reads the provided segments
    let mut reader =
        VirtualMultiVolumeReader::from_volumes(vec![vol1.clone(), vol2.clone(), vol4.clone()])
            .unwrap();
    assert_eq!(reader.total_size(), 27); // 9 + 9 + 9
    let mut out = String::new();
    reader.read_to_string(&mut out).unwrap();
    assert_eq!(out, "PART1_AAAPART2_BBBPART4_DDD");
}

#[test]
fn test_adv_multivol_out_of_order_chain_stitching_and_seeks() {
    let dir = tempdir().unwrap();
    let p_a = dir.path().join("chunk_A.bin");
    let p_b = dir.path().join("chunk_B.bin");
    let p_c = dir.path().join("chunk_C.bin");

    fs::write(&p_a, b"1111").unwrap(); // 4 bytes
    fs::write(&p_b, b"222222").unwrap(); // 6 bytes
    fs::write(&p_c, b"33333333").unwrap(); // 8 bytes

    // Out-of-order list: C (8), A (4), B (6) -> Total 18 bytes
    let mut reader =
        VirtualMultiVolumeReader::from_volumes(vec![p_c.clone(), p_a.clone(), p_b.clone()]).unwrap();
    assert_eq!(reader.total_size(), 18);

    let mut all_bytes = Vec::new();
    reader.read_to_end(&mut all_bytes).unwrap();
    assert_eq!(&all_bytes[..], b"333333331111222222");

    // Seek backward across multiple segments
    reader.seek(SeekFrom::Start(10)).unwrap(); // offset 10 = in segment A (offset 8..12, intra=2)
    let mut small = [0u8; 4];
    reader.read_exact(&mut small).unwrap();
    // In A: 1111 (read 2 bytes: "11"), in B: 222222 (read 2 bytes: "22") -> "1122"
    assert_eq!(&small, b"1122");

    // Seek to very start
    reader.seek(SeekFrom::Start(0)).unwrap();
    let mut first = [0u8; 2];
    reader.read_exact(&mut first).unwrap();
    assert_eq!(&first, b"33");
}

#[test]
fn test_adv_multivol_zero_sized_segments_interleaved() {
    let dir = tempdir().unwrap();
    let p1 = dir.path().join("vol.001");
    let p2 = dir.path().join("vol.002"); // 0 bytes
    let p3 = dir.path().join("vol.003");
    let p4 = dir.path().join("vol.004"); // 0 bytes
    let p5 = dir.path().join("vol.005");

    fs::write(&p1, b"HELLO_").unwrap(); // 6 bytes
    fs::write(&p2, b"").unwrap(); // 0 bytes
    fs::write(&p3, b"WORLD_").unwrap(); // 6 bytes
    fs::write(&p4, b"").unwrap(); // 0 bytes
    fs::write(&p5, b"2026").unwrap(); // 4 bytes

    let mut reader =
        VirtualMultiVolumeReader::from_volumes(vec![p1, p2, p3, p4, p5]).unwrap();
    assert_eq!(reader.total_size(), 16);

    let mut buf = String::new();
    reader.read_to_string(&mut buf).unwrap();
    assert_eq!(buf, "HELLO_WORLD_2026");

    // Seek directly to offset 6 (at zero-sized segment boundary)
    reader.seek(SeekFrom::Start(6)).unwrap();
    let mut part = [0u8; 6];
    reader.read_exact(&mut part).unwrap();
    assert_eq!(&part, b"WORLD_");
}

#[test]
fn test_adv_multivol_extreme_boundary_seeks_and_overflows() {
    let dir = tempdir().unwrap();
    let p1 = dir.path().join("seg.001");
    let p2 = dir.path().join("seg.002");
    fs::write(&p1, vec![0x11u8; 100]).unwrap();
    fs::write(&p2, vec![0x22u8; 100]).unwrap();

    let mut reader = VirtualMultiVolumeReader::from_volumes(vec![p1, p2]).unwrap();
    assert_eq!(reader.total_size(), 200);

    // Seek to exact boundary (offset 100)
    reader.seek(SeekFrom::Start(100)).unwrap();
    let mut b = [0u8; 1];
    reader.read_exact(&mut b).unwrap();
    assert_eq!(b[0], 0x22);

    // Seek to EOF (offset 200)
    reader.seek(SeekFrom::Start(200)).unwrap();
    let mut eof_buf = [0u8; 10];
    let n = reader.read(&mut eof_buf).unwrap();
    assert_eq!(n, 0);

    // Seek beyond EOF (offset 500)
    reader.seek(SeekFrom::Start(500)).unwrap();
    assert_eq!(reader.read(&mut eof_buf).unwrap(), 0);

    // Negative seek error
    assert!(reader.seek(SeekFrom::Current(-600)).is_err());
    assert!(reader.seek(SeekFrom::End(-300)).is_err());
}

#[test]
fn test_adv_multivol_cross_volume_spanning_read() {
    let dir = tempdir().unwrap();
    let p1 = dir.path().join("span.001");
    let p2 = dir.path().join("span.002");
    let p3 = dir.path().join("span.003");

    // 40 bytes each = 120 bytes total
    let data1 = vec![0x11u8; 40];
    let data2 = vec![0x22u8; 40];
    let data3 = vec![0x33u8; 40];

    fs::write(&p1, &data1).unwrap();
    fs::write(&p2, &data2).unwrap();
    fs::write(&p3, &data3).unwrap();

    let mut reader = VirtualMultiVolumeReader::from_volumes(vec![p1, p2, p3]).unwrap();
    assert_eq!(reader.total_size(), 120);

    // Read 100 bytes starting from offset 10:
    // This spans 30 bytes of vol 1, all 40 bytes of vol 2, and 30 bytes of vol 3
    reader.seek(SeekFrom::Start(10)).unwrap();
    let mut span_buf = [0u8; 100];
    reader.read_exact(&mut span_buf).unwrap();

    assert_eq!(&span_buf[0..30], &vec![0x11u8; 30][..]);
    assert_eq!(&span_buf[30..70], &vec![0x22u8; 40][..]);
    assert_eq!(&span_buf[70..100], &vec![0x33u8; 30][..]);
    assert_eq!(reader.current_offset(), 110);
}

#[test]
fn test_adv_multivol_corrupt_boundary_truncation_recovery() {
    let dir = tempdir().unwrap();
    let p1 = dir.path().join("trunc.001");
    let p2 = dir.path().join("trunc.002");

    fs::write(&p1, b"HEADER_METADATA_BLOCK_1").unwrap(); // 23 bytes
    fs::write(&p2, b"DATA_CHUNK_2").unwrap();            // 12 bytes

    let mut reader = VirtualMultiVolumeReader::from_volumes(vec![p1.clone(), p2.clone()]).unwrap();
    assert_eq!(reader.total_size(), 35); // 23 + 12 = 35

    // Read through boundary
    let mut full_buf = Vec::new();
    reader.read_to_end(&mut full_buf).unwrap();
    assert_eq!(full_buf, b"HEADER_METADATA_BLOCK_1DATA_CHUNK_2");
}

#[test]
fn test_adv_multivol_empty_and_nonexistent_error_handling() {
    // Empty path vector
    let empty_res = VirtualMultiVolumeReader::from_volumes(vec![]);
    assert!(empty_res.is_err());

    // Non-existent volume in list
    let fake_path = Path::new("/definitely/nonexistent/file.7z.001").to_path_buf();
    let fake_res = VirtualMultiVolumeReader::from_volumes(vec![fake_path]);
    assert!(fake_res.is_err());

    // Non-existent seed for detection
    let missing_seed = Path::new("/tmp/missing_archive_volume.z01");
    assert!(detect_volume_chain(missing_seed).is_err());
}

// ============================================================================
// Domain 2: Charset Auto-Detection Heuristics Under Mixed Encodings
// ============================================================================

#[test]
fn test_adv_charset_cjk_disambiguation_matrix() {
    // 1. Simplified Chinese (GB18030 / GBK)
    let gbk_text = "工程技术架构规范与微内核设计文档.pdf";
    let (gbk_bytes, _, _) = encoding_rs::GB18030.encode(gbk_text);
    let (gbk_det, gbk_conf) = detect_charset_with_confidence(&gbk_bytes);
    assert!(gbk_det == "GB18030" || gbk_det == "GBK", "Got {}", gbk_det);
    assert!(gbk_conf >= 0.50);
    assert_eq!(sanitize_filename(&gbk_bytes), gbk_text);

    // 2. Traditional Chinese (Big5)
    let big5_text = "繁體中文軟體專案需求與介面設計.docx";
    let (big5_bytes, _, _) = encoding_rs::BIG5.encode(big5_text);
    let (big5_det, big5_conf) = detect_charset_with_confidence(&big5_bytes);
    assert_eq!(big5_det, "Big5");
    assert!(big5_conf >= 0.50);
    assert_eq!(sanitize_filename(&big5_bytes), big5_text);

    // 3. Japanese (Shift-JIS)
    let sjis_text = "日本語アーカイブ圧縮解凍処理.zip";
    let (sjis_bytes, _, _) = encoding_rs::SHIFT_JIS.encode(sjis_text);
    let (sjis_det, sjis_conf) = detect_charset_with_confidence(&sjis_bytes);
    assert_eq!(sjis_det, "Shift_JIS");
    assert!(sjis_conf >= 0.50);
    assert_eq!(sanitize_filename(&sjis_bytes), sjis_text);

    // 4. Korean (EUC-KR)
    let euckr_text = "한국어데이터베이스백업파일.tar";
    let (euckr_bytes, _, _) = encoding_rs::EUC_KR.encode(euckr_text);
    let (euckr_det, euckr_conf) = detect_charset_with_confidence(&euckr_bytes);
    assert_eq!(euckr_det, "EUC-KR");
    assert!(euckr_conf >= 0.50);
    assert_eq!(sanitize_filename(&euckr_bytes), euckr_text);
}

#[test]
fn test_adv_charset_cp437_dos_legacy_fallback() {
    // 1. CP437 accented Latin containing 0x81 (ü in CP437; unassigned in Windows-1252):
    // "München_Grätz_Bericht.doc" encoded in DOS CP437:
    let cp437_accent = b"M\x81nchen_Gr\x84tz_Bericht.doc";
    let sanitized_accent = sanitize_filename(cp437_accent);
    assert_eq!(sanitized_accent, "München_Grätz_Bericht.doc");

    // 2. Direct CP437 zero-allocation decoder on DOS box-drawing symbols
    let cp437_box = b"\xC9\xCD\xBB_frame.dat";
    let decoded_box = ttzip_engine::zip::cp437::decode_cp437(cp437_box);
    assert_eq!(decoded_box, "╔═╗_frame.dat");

    // 3. Windows-1252 extended ASCII single-byte decoding
    let win1252_bytes = b"\xDA\xC4\xBF\xB3\xC0\xC4\xD9.txt";
    let (det, conf) = detect_charset_with_confidence(win1252_bytes);
    assert_eq!(det, "windows-1252");
    assert!(conf >= 0.20);
    let sanitized_win = sanitize_filename(win1252_bytes);
    assert_eq!(sanitized_win, "ÚÄ¿³ÀÄÙ.txt");
}

#[test]
fn test_adv_charset_extreme_length_and_special_delimiters() {
    // 1. 1024-byte long GB18030 filename with mixed ASCII digits and symbols
    let mut long_chinese = String::with_capacity(2048);
    for i in 0..100 {
        long_chinese.push_str(&format!("项目归档分卷数据_第{i}批次_"));
    }
    long_chinese.push_str(".tar.gz");

    let (encoded_long, _, _) = encoding_rs::GB18030.encode(&long_chinese);
    let decoded_long = sanitize_filename(&encoded_long);
    assert_eq!(decoded_long, long_chinese);

    // 2. Filename containing internal null bytes and forward slashes in non-UTF8
    let raw_with_slashes = b"subfolder/\xD6\xD0\xCE\xC4/data.bin"; // "subfolder/中文/data.bin" in GBK
    let decoded_path = sanitize_filename(raw_with_slashes);
    assert_eq!(decoded_path, "subfolder/中文/data.bin");
}

#[test]
fn test_adv_charset_truncated_multibyte_safety() {
    // Truncated multi-byte byte sequences must NOT panic
    let truncated_gbk = b"Prefix_\x81"; // lead byte without trail
    let sanitized_gbk = sanitize_filename(truncated_gbk);
    assert!(sanitized_gbk.starts_with("Prefix_"));

    let truncated_utf8 = b"Hello_\xE4\xBD"; // 2 bytes of a 3-byte sequence
    let sanitized_utf8 = sanitize_filename(truncated_utf8);
    assert!(sanitized_utf8.starts_with("Hello_"));

    // Buffer slicing with small buffer
    let mut small_buf = [0u8; 3];
    let res = sanitize_filename_to_slice(b"VeryLongFilename.txt", &mut small_buf);
    assert_eq!(res, Err(TTZipStatus::ErrPathTooLong));
}

#[test]
fn test_adv_charset_mixed_ascii_cjk_complex_transcode() {
    let mixed_gbk = "2026_Q3_财务预算分析报表_Final_v1.0.xlsx";
    let (encoded, _, _) = encoding_rs::GB18030.encode(mixed_gbk);
    let result = sanitize_filename(&encoded);
    assert_eq!(result, mixed_gbk);

    let mixed_sjis = "Project_TTZip_設定ファイル_release.json";
    let (sjis_encoded, _, _) = encoding_rs::SHIFT_JIS.encode(mixed_sjis);
    let sjis_result = sanitize_filename(&sjis_encoded);
    assert_eq!(sjis_result, mixed_sjis);
}

#[test]
fn test_adv_charset_utf8_bom_astral_plane_emojis() {
    // UTF-8 with BOM
    let with_bom = b"\xEF\xBB\xBFUTF8_Document_With_BOM.md";
    let (det, conf) = detect_charset_with_confidence(with_bom);
    assert_eq!(det, "UTF-8");
    assert_eq!(conf, 1.0);

    // UTF-8 with 4-byte astral symbols: 🦀 🚀 📦 ⚡
    let emoji_str = "Release_🦀_v2.0_🚀_Archive_📦.7z";
    let emoji_bytes = emoji_str.as_bytes();
    let sanitized = sanitize_filename(emoji_bytes);
    assert_eq!(sanitized, emoji_str);
}
