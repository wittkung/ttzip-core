// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use super::*;
use crate::crypto::crc32::crc32_fast;
use crate::sevenz::header::parse_solid_preview_metadata;

fn valid_lzma2_header_bytes() -> Vec<u8> {
    let mut header = Vec::new();
    header.extend_from_slice(&[
        K_HEADER,
        K_MAIN_STREAMS_INFO,
        K_PACK_INFO,
        0x00, // pack_pos = 0
        0x01, // num_pack_streams = 1
        K_SIZE,
        0x04, // pack_size = 4
        K_END,
        K_UNPACK_INFO,
        K_FOLDER,
        0x01, // num_folders = 1
        0x00, // external = 0
        0x01, // num_coders = 1
        0x21, // flags: method_size = 1, has_props = 1
        METHOD_LZMA2 as u8,
        0x01, // prop_size = 1
        0x00, // dict prop
        K_CODERS_UNPACK_SIZE,
        0x04, // unpack_size = 4
        K_END,
        K_SUB_STREAMS_INFO,
        K_CRC,
        0x01, // all_defined = 1
        0x78,
        0x56,
        0x34,
        0x12,
        K_END,
        K_END, // close MainStreamsInfo
        K_FILES_INFO,
        0x01, // num_files = 1
        K_NAME,
        0x05, // prop_size: 1 external + 4 bytes ("a\0" UTF-16LE)
        0x00, // external = 0
        b'a',
        0x00,
        0x00,
        0x00,
        K_END, // close FilesInfo
        K_END, // close Header
    ]);
    header
}

fn wrap_plain_archive(payload: &[u8], header: &[u8]) -> Vec<u8> {
    let signature = SevenZSignatureHeader {
        major_version: 0,
        minor_version: 4,
        start_header_crc: 0,
        next_header_offset: payload.len() as u64,
        next_header_size: header.len() as u64,
        next_header_crc: crc32_fast(0, header),
    }
    .serialize();
    let mut out = Vec::with_capacity(signature.len() + payload.len() + header.len());
    out.extend_from_slice(&signature);
    out.extend_from_slice(payload);
    out.extend_from_slice(header);
    out
}

fn parse_with_admission(header: &[u8], admission: HeaderAdmission) -> Result<SevenZHeaderInfo, TTZipStatus> {
    let mut info = SevenZHeaderInfo::default();
    parse_7z_header_stream_with_admission(header, &mut info, admission)?;
    Ok(info)
}

#[test]
fn test_read_optional_crc_list_and_substreams_reject_oversized_counts_on_short_header() {
    let mut hpos = 0usize;
    assert_eq!(
        read_optional_crc_list(&[0x01u8, 0x00], &mut hpos, 10_000_000),
        Err(TTZipStatus::ErrCorruptHeader)
    );
    let mut hpos_mask = 0usize;
    assert_eq!(
        read_optional_crc_list(&[0x00u8, 0xFF], &mut hpos_mask, 500_000),
        Err(TTZipStatus::ErrCorruptHeader)
    );
    let header_bytes = [K_NUM_UNPACK_STREAM, 0xE0, 0x40, 0x4B, 0x4C, K_CRC, 0x01, K_END];
    let mut info = SevenZHeaderInfo {
        folders: vec![SevenZFolder::default()],
        ..Default::default()
    };
    let mut pos = 0usize;
    assert_eq!(
        parse_substreams_info_section(&header_bytes, &mut pos, &mut info, HeaderAdmission::Standard),
        Err(TTZipStatus::ErrCorruptHeader)
    );
}

#[test]
fn test_skip_pack_crcs_handles_sparse_bitmask_and_rejects_truncation() {
    let sparse_pack_header = [
        K_HEADER,
        K_MAIN_STREAMS_INFO,
        K_PACK_INFO,
        0x00, // pack_pos = 0
        0x02, // num_pack_streams = 2
        K_SIZE,
        0x04,
        0x05,
        K_CRC,
        0x00, // all_defined = 0
        0x80, // bitmask: stream 0 defined, stream 1 omitted
        0x11,
        0x22,
        0x33,
        0x44, // CRC for stream 0
        K_END,
        K_END,
        K_FILES_INFO,
        0x01,
        K_NAME,
        0x05,
        0x00,
        b'x',
        0x00,
        0x00,
        0x00,
        K_END,
        K_END,
    ];
    let parsed = parse_with_admission(&sparse_pack_header, HeaderAdmission::Standard)
        .expect("sparse pack CRC bitmask must be consumed cleanly");
    assert_eq!(parsed.pack_sizes, vec![4, 5]);
    assert_eq!(parsed.files.len(), 1);
    assert_eq!(parsed.files[0].rel_path, "x");

    let mut pos = 0usize;
    assert_eq!(
        skip_pack_crcs(&[0x00], &mut pos, 2),
        Err(TTZipStatus::ErrCorruptHeader)
    );
    let mut pos_trunc_crc = 0usize;
    assert_eq!(
        skip_pack_crcs(&[0x00, 0x80, 0x11, 0x22], &mut pos_trunc_crc, 2),
        Err(TTZipStatus::ErrCorruptHeader)
    );
}

#[test]
fn test_parse_single_coder_rejects_reserved_flags_and_malformed_props() {
    let base = valid_lzma2_header_bytes();
    let coder_pos = base
        .windows(4)
        .position(|w| w == [0x21, METHOD_LZMA2 as u8, 0x01, 0x00])
        .expect("locate LZMA2 coder");

    for reserved_flags in [0x41u8, 0x81, 0xC1] {
        let mut header = base.clone();
        header[coder_pos] = reserved_flags;
        for admission in [HeaderAdmission::Standard, HeaderAdmission::SolidPreview] {
            assert_eq!(
                parse_with_admission(&header, admission).err(),
                Some(TTZipStatus::ErrCorruptHeader),
                "reserved coder flags {reserved_flags:#04x}"
            );
        }
    }

    let mut overflow = base[..coder_pos + 2].to_vec();
    overflow.extend_from_slice(&[0xFF; 9]);
    overflow.extend_from_slice(&base[coder_pos + 3..]);

    let mut truncated = base[..coder_pos + 2].to_vec();
    truncated.push(0x7F);
    truncated.extend_from_slice(&base[coder_pos + 3..]);

    for malformed in [&overflow, &truncated] {
        assert_eq!(
            parse_with_admission(malformed, HeaderAdmission::Standard).err(),
            Some(TTZipStatus::ErrCorruptHeader)
        );
        assert_eq!(
            parse_with_admission(malformed, HeaderAdmission::SolidPreview).err(),
            Some(TTZipStatus::ErrCorruptHeader)
        );
    }
}

#[test]
fn test_solid_preview_rejects_unclosed_sections_bad_scope_and_trailing_bytes() {
    let valid = valid_lzma2_header_bytes();
    assert!(parse_with_admission(&valid, HeaderAdmission::SolidPreview).is_ok());

    let mut missing_header_end = valid.clone();
    missing_header_end.pop();
    assert_eq!(
        parse_with_admission(&missing_header_end, HeaderAdmission::SolidPreview).err(),
        Some(TTZipStatus::ErrCorruptHeader)
    );

    let mut trailing_byte = valid.clone();
    trailing_byte.push(K_END);
    assert_eq!(
        parse_with_admission(&trailing_byte, HeaderAdmission::SolidPreview).err(),
        Some(TTZipStatus::ErrCorruptHeader)
    );

    let unclosed_pack = [K_HEADER, K_MAIN_STREAMS_INFO, K_PACK_INFO, 0x00, 0x01, K_SIZE, 0x04];
    assert_eq!(
        parse_with_admission(&unclosed_pack, HeaderAdmission::SolidPreview).err(),
        Some(TTZipStatus::ErrCorruptHeader)
    );

    let unclosed_main_streams = [
        K_HEADER,
        K_MAIN_STREAMS_INFO,
        K_PACK_INFO,
        0x00,
        0x01,
        K_SIZE,
        0x04,
        K_END,
        K_FILES_INFO,
        0x00,
        K_END,
        K_END,
    ];
    assert_eq!(
        parse_with_admission(&unclosed_main_streams, HeaderAdmission::SolidPreview).err(),
        Some(TTZipStatus::ErrCorruptHeader)
    );

    let pack_outside_main = [K_HEADER, K_PACK_INFO, 0x00, 0x01, K_END, K_END];
    assert_eq!(
        parse_with_admission(&pack_outside_main, HeaderAdmission::SolidPreview).err(),
        Some(TTZipStatus::ErrCorruptHeader)
    );

    let unclosed_files = [K_HEADER, K_FILES_INFO, 0x01, K_NAME, 0x05, 0x00, b'a', 0x00, 0x00, 0x00];
    assert_eq!(
        parse_with_admission(&unclosed_files, HeaderAdmission::SolidPreview).err(),
        Some(TTZipStatus::ErrCorruptHeader)
    );
}

#[test]
fn test_solid_preview_rejects_unknown_properties_and_empty_name_property() {
    let pack_unknown = [
        K_HEADER,
        K_MAIN_STREAMS_INFO,
        K_PACK_INFO,
        0x00,
        0x01,
        0x1A,
        0x01,
        0x00,
        K_END,
        K_END,
        K_END,
    ];
    assert_eq!(
        parse_with_admission(&pack_unknown, HeaderAdmission::SolidPreview).err(),
        Some(TTZipStatus::ErrUnsupportedFeature)
    );

    let unpack_unknown = [
        K_HEADER,
        K_MAIN_STREAMS_INFO,
        K_UNPACK_INFO,
        0x1A,
        0x01,
        0x00,
        K_END,
        K_END,
        K_END,
    ];
    assert_eq!(
        parse_with_admission(&unpack_unknown, HeaderAdmission::SolidPreview).err(),
        Some(TTZipStatus::ErrUnsupportedFeature)
    );

    let substreams_unknown = [
        K_HEADER,
        K_MAIN_STREAMS_INFO,
        K_UNPACK_INFO,
        K_FOLDER,
        0x01,
        0x00,
        0x01,
        0x21,
        METHOD_LZMA2 as u8,
        0x01,
        0x00,
        K_CODERS_UNPACK_SIZE,
        0x04,
        K_END,
        K_SUB_STREAMS_INFO,
        0x1A,
        0x01,
        0x00,
        K_END,
        K_END,
        K_END,
    ];
    assert_eq!(
        parse_with_admission(&substreams_unknown, HeaderAdmission::SolidPreview).err(),
        Some(TTZipStatus::ErrUnsupportedFeature)
    );

    let files_unknown = [K_HEADER, K_FILES_INFO, 0x01, K_ANTI, 0x01, 0x00, K_END, K_END];
    assert_eq!(
        parse_with_admission(&files_unknown, HeaderAdmission::SolidPreview).err(),
        Some(TTZipStatus::ErrUnsupportedFeature)
    );

    let empty_name_prop = [K_HEADER, K_FILES_INFO, 0x01, K_NAME, 0x00, K_DUMMY, 0x00, K_END, K_END];
    for admission in [HeaderAdmission::Standard, HeaderAdmission::SolidPreview] {
        assert_eq!(
            parse_with_admission(&empty_name_prop, admission).err(),
            Some(TTZipStatus::ErrCorruptHeader)
        );
    }
}

#[test]
fn test_solid_preview_rejects_empty_folder_substreams_and_truncated_unpack_sizes() {
    let empty_folder_substreams = [
        K_HEADER,
        K_MAIN_STREAMS_INFO,
        K_SUB_STREAMS_INFO,
        K_END,
        K_END,
        K_END,
    ];
    assert_eq!(
        parse_with_admission(&empty_folder_substreams, HeaderAdmission::SolidPreview).err(),
        Some(TTZipStatus::ErrCorruptHeader)
    );

    let truncated_unpack_sizes = [
        K_HEADER,
        K_MAIN_STREAMS_INFO,
        K_UNPACK_INFO,
        K_FOLDER,
        0x01,
        0x00,
        0x01,
        0x21,
        METHOD_LZMA2 as u8,
        0x01,
        0x00,
        K_CODERS_UNPACK_SIZE,
    ];
    assert_eq!(
        parse_with_admission(&truncated_unpack_sizes, HeaderAdmission::SolidPreview).err(),
        Some(TTZipStatus::ErrCorruptHeader)
    );
}

#[test]
fn test_files_info_ignores_truncated_defined_bitmask_without_misreading_mtime() {
    // 81 files require 11 bitmask bytes, so a 10-byte property payload cannot carry a complete
    // bitmask; the parser must discard it instead of treating the remaining bytes as external=0
    // plus an 8-byte timestamp.
    let truncated_mtime_bitmask = [
        K_HEADER,
        K_FILES_INFO,
        81,   // num_files = 81 -> mask_bytes = 11
        K_MTIME,
        10,   // prop_size = 10 (< 1 + 11)
        0x00, // all_defined = 0
        0x00, // would be misread as external = 0 if bitmask bounds check fell through
        0x00,
        0x80,
        0x3E,
        0xD5,
        0xDE,
        0xB1,
        0x9D,
        0x01, // valid NT FILETIME (1970-01-01)
        K_END,
        K_END,
    ];
    let parsed = parse_with_admission(&truncated_mtime_bitmask, HeaderAdmission::Standard)
        .expect("truncated mtime bitmask in standard mode must not panic");
    assert!(parsed.files.iter().all(|file| file.mtime_epoch_secs.is_none()));
}

#[test]
fn test_parse_solid_preview_metadata_accepts_benign_file_props_and_sparse_pack_crc() {
    let mut header = valid_lzma2_header_bytes();
    let files_end = header.len() - 2; // K_END closing FilesInfo
    header.splice(
        files_end..files_end,
        [
            K_CTIME, 0x02, 0x01, 0x00, // benign ctime
            K_ATIME, 0x02, 0x01, 0x00, // benign atime
            K_DUMMY, 0x02, 0x00, 0x00, // benign dummy padding
        ],
    );
    let pack_end = header
        .windows(4)
        .position(|w| w == [K_SIZE, 0x04, K_END, K_UNPACK_INFO])
        .expect("locate PackInfo K_END")
        + 2;
    header.splice(pack_end..pack_end, [K_CRC, 0x00, 0x00]); // sparse pack CRC with 0 bits set

    let archive = wrap_plain_archive(&[0u8; 4], &header);
    let parsed = parse_solid_preview_metadata(&archive, &|| false)
        .expect("valid plain LZMA2 archive with benign props and sparse pack CRC must pass");
    assert_eq!(parsed.files.len(), 1);
    assert_eq!(parsed.files[0].rel_path, "a");
    assert_eq!(parsed.stream_crcs, vec![0x1234_5678]);
}
