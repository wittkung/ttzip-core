// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive Test Suite for the 7 Major ZIP Extra Field Families.
//!
//! Validates:
//! 1. Individual parsing and building roundtrips for each of the 7 Extra Field families.
//! 2. Mixed composite Extra Field stream parsing with preservation of unknown tags.
//! 3. `0x7075` Unicode Path CRC32 verification and safe fallback on mismatch.
//! 4. `0x000a` Windows NTFS 100ns FILETIME <-> Unix Epoch seconds and nanoseconds two-way conversion.
//! 5. `0x756e` ASi Unix POSIX mode, symlink target extraction, and CRC integrity check.
//! 6. Local File Header (LFH) vs Central Directory (CDFH) asymmetric serialization.
//! 7. Zero-panic defense against malformed, truncated, and corrupt TLV payloads.

use ttzip_engine::zip::extra::*;

#[test]
fn test_tag_constants() {
    assert_eq!(TAG_ZIP64, 0x0001);
    assert_eq!(TAG_NTFS, 0x000a);
    assert_eq!(TAG_EXT_TIMESTAMP, 0x5455);
    assert_eq!(TAG_UNICODE_COMMENT, 0x6375);
    assert_eq!(TAG_UNICODE_PATH, 0x7075);
    assert_eq!(TAG_ASI_UNIX, 0x756e);
    assert_eq!(TAG_INFOZIP_UNIX_NEW, 0x7875);
    assert_eq!(TAG_INFOZIP_UNIX, 0x7875);
    assert_eq!(TAG_WINZIP_AES, 0x9901);
    assert_eq!(TAG_DATA_STREAM_ALIGNMENT, 0xa11e);
}

// ----------------------------------------------------------------------------
// 1. Zip64 Extended Information (0x0001)
// ----------------------------------------------------------------------------
#[test]
fn test_zip64_extra_field_roundtrip() {
    let z64 = Zip64Extra {
        uncompressed_size: Some(10_000_000_000),
        compressed_size: Some(8_000_000_000),
        local_header_offset: Some(5_000_000_000),
        disk_start_number: Some(0),
    };

    let local_bytes = z64.build_local();
    assert_eq!(local_bytes.len(), 4 + 16);
    let parsed_local = Zip64Extra::parse(&local_bytes[4..], false, false, false, false);
    assert_eq!(parsed_local.uncompressed_size, Some(10_000_000_000));
    assert_eq!(parsed_local.compressed_size, Some(8_000_000_000));

    let central_bytes = z64.build_central();
    assert_eq!(central_bytes.len(), 4 + 28);
    let parsed_central = Zip64Extra::parse(&central_bytes[4..], true, true, true, true);
    assert_eq!(parsed_central.uncompressed_size, Some(10_000_000_000));
    assert_eq!(parsed_central.compressed_size, Some(8_000_000_000));
    assert_eq!(parsed_central.local_header_offset, Some(5_000_000_000));
    assert_eq!(parsed_central.disk_start_number, Some(0));
}

// ----------------------------------------------------------------------------
// 2. Extended Timestamp (0x5455)
// ----------------------------------------------------------------------------
#[test]
fn test_extended_timestamp_roundtrip_and_asymmetry() {
    let ts = ExtendedTimestampExtra {
        flags: EXT_TIME_FLAG_MTIME | EXT_TIME_FLAG_ATIME | EXT_TIME_FLAG_CTIME,
        mod_time: Some(1700000001),
        acc_time: Some(1700000002),
        create_time: Some(1700000003),
    };

    // Local extra contains all 3 timestamps
    let local_bytes = ts.build_local();
    assert_eq!(local_bytes.len(), 4 + 1 + 12);
    let parsed_local = ExtendedTimestampExtra::parse(&local_bytes[4..]).unwrap();
    assert_eq!(parsed_local.mod_time, Some(1700000001));
    assert_eq!(parsed_local.acc_time, Some(1700000002));
    assert_eq!(parsed_local.create_time, Some(1700000003));

    // Central extra ONLY contains mod_time (CDFH asymmetry per Info-ZIP spec)
    let central_bytes = ts.build_central();
    assert_eq!(central_bytes.len(), 4 + 5);
    let parsed_central = ExtendedTimestampExtra::parse(&central_bytes[4..]).unwrap();
    assert_eq!(parsed_central.flags, EXT_TIME_FLAG_MTIME);
    assert_eq!(parsed_central.mod_time, Some(1700000001));
    assert_eq!(parsed_central.acc_time, None);
    assert_eq!(parsed_central.create_time, None);
}

// ----------------------------------------------------------------------------
// 3. Info-ZIP Unix New (0x7875)
// ----------------------------------------------------------------------------
#[test]
fn test_infozip_unix_new_roundtrip_and_asymmetry() {
    let ux = InfoZipUnixNewExtra {
        version: 1,
        uid: 1001,
        gid: 1002,
    };

    let local_bytes = ux.build_local();
    assert_eq!(local_bytes.len(), 4 + 11);
    let parsed_local = InfoZipUnixNewExtra::parse(&local_bytes[4..]).unwrap();
    assert_eq!(parsed_local.version, 1);
    assert_eq!(parsed_local.uid, 1001);
    assert_eq!(parsed_local.gid, 1002);

    // Central extra is stripped to 0 payload bytes per Info-ZIP spec
    let central_bytes = ux.build_central();
    assert_eq!(central_bytes.len(), 4);
    let parsed_central = InfoZipUnixNewExtra::parse(&central_bytes[4..]).unwrap();
    assert_eq!(parsed_central.version, 1);
}

// ----------------------------------------------------------------------------
// 4. Windows NTFS 100ns Timestamps (0x000a) & Conversion Precision
// ----------------------------------------------------------------------------
#[test]
fn test_ntfs_extra_field_and_two_way_timestamp_conversion() {
    // 1970-01-01 00:00:00 UTC = exact tick difference
    let epoch_ticks = WINDOWS_EPOCH_DIFF_TICKS;
    assert_eq!(NtfsExtra::filetime_to_unix_secs(epoch_ticks), 0);
    assert_eq!(NtfsExtra::unix_secs_to_filetime(0), epoch_ticks);

    // Modern timestamp: 2026-08-30 12:00:00 UTC with 543,210,000 nanoseconds
    let unix_secs = 1788091200i64;
    let unix_nanos = 543_210_000u32;
    let filetime = NtfsExtra::unix_nanos_to_filetime(unix_secs, unix_nanos);
    let (back_secs, back_nanos) = NtfsExtra::filetime_to_unix_nanos(filetime);
    assert_eq!(back_secs, unix_secs);
    // 100ns granularity truncation check: 543210000 is divisible by 100
    assert_eq!(back_nanos, 543_210_000);

    // NTFS Extra record roundtrip
    let ntfs = NtfsExtra::from_unix_secs(1788091200, 1788091201, 1788091202);
    let bytes = ntfs.build();
    assert_eq!(bytes.len(), 36);

    let parsed = NtfsExtra::parse(&bytes[4..]).expect("NTFS parse failed");
    assert_eq!(parsed.mtime_unix_secs(), 1788091200);
    assert_eq!(parsed.atime_unix_secs(), 1788091201);
    assert_eq!(parsed.ctime_unix_secs(), 1788091202);
}

// ----------------------------------------------------------------------------
// 5. Info-ZIP Unicode Path (0x7075) & Unicode Comment (0x6375)
// ----------------------------------------------------------------------------
#[test]
fn test_unicode_path_and_comment_validation_and_fallback() {
    let standard_filename = "archive_fallback.txt";
    let unicode_text = "文档/归档_🚀.txt";

    let upath = UnicodeFieldExtra::from_text(
        TAG_UNICODE_PATH,
        unicode_text,
        standard_filename.as_bytes(),
    );
    let bytes = upath.build();

    let parsed = UnicodeFieldExtra::parse(TAG_UNICODE_PATH, &bytes[4..]).expect("parse unicode path");
    assert_eq!(parsed.text, unicode_text);
    assert_eq!(parsed.version, 1);

    // CRC32 matches standard filename: valid
    assert!(parsed.is_valid_for(standard_filename.as_bytes()));

    // CRC32 mismatch: standard filename was modified by non-Unicode-aware tool -> fallback
    let modified_standard = "archive_renamed.txt";
    assert!(!parsed.is_valid_for(modified_standard.as_bytes()));

    // Unicode Comment (0x6375)
    let standard_comment = "legacy comment";
    let unicode_comment_text = "UTF-8 说明：完全兼容 2026";
    let ucomm = UnicodeFieldExtra::from_text(
        TAG_UNICODE_COMMENT,
        unicode_comment_text,
        standard_comment.as_bytes(),
    );
    let comm_bytes = ucomm.build();
    let parsed_comm = UnicodeFieldExtra::parse(TAG_UNICODE_COMMENT, &comm_bytes[4..]).unwrap();
    assert_eq!(parsed_comm.text, unicode_comment_text);
    assert!(parsed_comm.is_valid_for(standard_comment.as_bytes()));
    assert!(!parsed_comm.is_valid_for(b"tampered comment"));
}

// ----------------------------------------------------------------------------
// 6. WinZip AES (0x9901)
// ----------------------------------------------------------------------------
#[test]
fn test_winzip_aes_extra_field_roundtrip() {
    let aes = WinZipAesExtra::new(0x0008, WINZIP_AES_STRENGTH_256);
    let bytes = aes.build();
    assert_eq!(bytes.len(), 11);

    let parsed = WinZipAesExtra::parse(&bytes[4..]).expect("parse aes extra");
    assert_eq!(parsed.version, WINZIP_AES_VERSION_AE2);
    assert_eq!(parsed.vendor_id, WINZIP_AES_VENDOR_ID);
    assert_eq!(parsed.strength, WINZIP_AES_STRENGTH_256);
    assert_eq!(parsed.actual_compression_method, 8); // Deflate
}

// ----------------------------------------------------------------------------
// 7. ASi Unix Metadata & Symlinks (0x756e)
// ----------------------------------------------------------------------------
#[test]
fn test_asi_unix_symlink_and_permissions() {
    // Symbolic link case
    let symlink = AsiUnixExtra::new_symlink(0o755, 501, 20, "/opt/ttzip/bin/engine");
    assert!(symlink.is_symlink());
    assert!(!symlink.is_regular());
    assert_eq!(symlink.permissions(), 0o755);
    assert_eq!(symlink.symlink_target.as_deref(), Some("/opt/ttzip/bin/engine"));

    let bytes = symlink.build();
    let parsed = AsiUnixExtra::parse(&bytes[4..]).expect("parse asi unix symlink");
    assert!(parsed.is_symlink());
    assert_eq!(parsed.permissions(), 0o755);
    assert_eq!(parsed.uid, 501);
    assert_eq!(parsed.gid, 20);
    assert_eq!(parsed.symlink_target.as_deref(), Some("/opt/ttzip/bin/engine"));

    // Regular file case
    let reg_file = AsiUnixExtra::new_file(0o644, 4096, 1000, 1000);
    assert!(reg_file.is_regular());
    assert!(!reg_file.is_symlink());
    assert_eq!(reg_file.sizdev, 4096);
    assert_eq!(reg_file.symlink_target, None);

    let reg_bytes = reg_file.build();
    let parsed_reg = AsiUnixExtra::parse(&reg_bytes[4..]).expect("parse asi regular file");
    assert!(parsed_reg.is_regular());
    assert_eq!(parsed_reg.sizdev, 4096);
    assert_eq!(parsed_reg.permissions(), 0o644);

    // Corrupted CRC check
    let mut corrupt_bytes = reg_bytes.clone();
    corrupt_bytes[10] ^= 0xFF; // Corrupt payload byte
    assert!(AsiUnixExtra::parse(&corrupt_bytes[4..]).is_none());
}

// ----------------------------------------------------------------------------
// 8. Data Stream Alignment (0xa11e)
// ----------------------------------------------------------------------------
#[test]
fn test_data_stream_alignment_extra_field() {
    let align = DataStreamAlignmentExtra {
        alignment: 4096,
        padding_len: 16,
    };

    let local_bytes = align.build_local();
    assert_eq!(local_bytes.len(), 16);
    let parsed = DataStreamAlignmentExtra::parse(&local_bytes[4..], 16).unwrap();
    assert_eq!(parsed.alignment, 4096);
    assert_eq!(parsed.padding_len, 16);

    // Central Directory stripping
    let central_bytes = align.build_central();
    assert!(central_bytes.is_empty());
}

// ----------------------------------------------------------------------------
// 9. Mixed Composite Extra Field Stream Parsing
// ----------------------------------------------------------------------------
#[test]
fn test_mixed_composite_extra_fields_stream() {
    let mut composite_stream = Vec::new();

    // 1. Zip64
    composite_stream.extend_from_slice(&ZipExtraFields::build_zip64_extra(Some(12345), Some(6789), None));
    // 2. Ext Timestamp
    composite_stream.extend_from_slice(&ExtendedTimestampExtra {
        flags: EXT_TIME_FLAG_MTIME | EXT_TIME_FLAG_ATIME,
        mod_time: Some(1700000000),
        acc_time: Some(1700000010),
        create_time: None,
    }.build_local());
    // 3. Info-ZIP Unix New
    composite_stream.extend_from_slice(&InfoZipUnixNewExtra { version: 1, uid: 500, gid: 500 }.build_local());
    // 4. NTFS
    composite_stream.extend_from_slice(&NtfsExtra::from_unix_secs(1700000000, 1700000001, 1700000002).build());
    // 5. Unicode Path
    composite_stream.extend_from_slice(&UnicodeFieldExtra::from_text(TAG_UNICODE_PATH, "复合流测试.txt", b"test.txt").build());
    // 6. WinZip AES
    composite_stream.extend_from_slice(&WinZipAesExtra::new(8, WINZIP_AES_STRENGTH_256).build());
    // 7. ASi Unix
    composite_stream.extend_from_slice(&AsiUnixExtra::new_symlink(0o777, 0, 0, "/tmp/target").build());
    // 8. Alignment
    composite_stream.extend_from_slice(&DataStreamAlignmentExtra { alignment: 4096, padding_len: 12 }.build_local());
    // 9. Unknown Tag
    composite_stream.extend_from_slice(&0xcafeu16.to_le_bytes());
    composite_stream.extend_from_slice(&4u16.to_le_bytes());
    composite_stream.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]);

    // Parse all extra fields in one unified pass
    let parsed = ExtraFieldsParser::parse(&composite_stream, false);

    assert!(parsed.has_zip64);
    assert_eq!(parsed.uncompressed_size, Some(12345));
    assert_eq!(parsed.compressed_size, Some(6789));

    assert!(parsed.has_extended_timestamp);
    assert_eq!(parsed.mod_time, Some(1700000000));
    assert_eq!(parsed.acc_time, Some(1700000010));

    assert!(parsed.has_posix_permissions);
    assert_eq!(parsed.uid, Some(500));
    assert_eq!(parsed.gid, Some(500));

    assert!(parsed.ntfs.is_some());
    assert_eq!(parsed.ntfs.as_ref().unwrap().mtime_unix_secs(), 1700000000);

    assert!(parsed.unicode_path.is_some());
    assert_eq!(parsed.unicode_path_str.as_deref(), Some("复合流测试.txt"));

    assert!(parsed.has_winzip_aes);
    assert_eq!(parsed.aes_actual_method, 8);
    assert_eq!(parsed.aes_strength, 3);

    assert!(parsed.asi_unix.is_some());
    let asi = parsed.asi_unix.as_ref().unwrap();
    assert!(asi.is_symlink());
    assert_eq!(asi.symlink_target.as_deref(), Some("/tmp/target"));

    assert_eq!(parsed.data_stream_alignment, Some(4096));

    // Unknown field preserved
    assert_eq!(parsed.unknown_fields.len(), 1);
    assert_eq!(parsed.unknown_fields[0].0, 0xcafe);
    assert_eq!(parsed.unknown_fields[0].1, vec![0xaa, 0xbb, 0xcc, 0xdd]);

    // Test serializing back to local extra and re-parsing
    let rebuilt_local = parsed.build_local_extra();
    let reparsed_local = ExtraFieldsParser::parse(&rebuilt_local, false);
    assert_eq!(reparsed_local.uncompressed_size, Some(12345));
    assert_eq!(reparsed_local.mod_time, Some(1700000000));
    assert_eq!(reparsed_local.unknown_fields.len(), 1);
}

// ----------------------------------------------------------------------------
// 10. Robustness & Zero-Panic Defense on Malformed TLVs
// ----------------------------------------------------------------------------
#[test]
fn test_malformed_tlv_robustness_zero_panic() {
    // Case 1: Empty byte slice
    let p1 = ExtraFieldsParser::parse(&[], false);
    assert_eq!(p1, ZipExtraFields::default());

    // Case 2: Truncated header (1..3 bytes)
    let p2 = ExtraFieldsParser::parse(&[0x01, 0x00, 0x08], false);
    assert_eq!(p2, ZipExtraFields::default());

    // Case 3: Declared payload length exceeds remaining slice
    let p3 = ExtraFieldsParser::parse(&[0x01, 0x00, 0xFF, 0x00, 0x01, 0x02], false);
    assert_eq!(p3, ZipExtraFields::default());

    // Case 4: Zero-length extra field tag
    let p4 = ExtraFieldsParser::parse(&[0x55, 0x54, 0x00, 0x00], false);
    assert!(!p4.has_extended_timestamp);

    // Case 5: WinZip AES truncated payload (< 7 bytes)
    let p5 = ExtraFieldsParser::parse(&[0x01, 0x99, 0x03, 0x00, 0x01, 0x02, 0x03], false);
    assert!(!p5.has_winzip_aes);

    // Case 6: Unicode Path truncated payload (< 5 bytes)
    let p6 = ExtraFieldsParser::parse(&[0x75, 0x70, 0x03, 0x00, 0x01, 0x02, 0x03], false);
    assert!(p6.unicode_path.is_none());

    // Case 7: ASi Unix truncated payload (< 14 bytes)
    let p7 = ExtraFieldsParser::parse(&[0x6e, 0x75, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04], false);
    assert!(p7.asi_unix.is_none());

    // Case 8: Huge data size
    let p8 = ExtraFieldsParser::parse(&[0x75, 0x78, 0xFF, 0xFF], false);
    assert_eq!(p8, ZipExtraFields::default());
}
