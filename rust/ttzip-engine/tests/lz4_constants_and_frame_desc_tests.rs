// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for LZ4 Frame constants, skippable magic matching,
//! descriptor bitfield serialization, and header checksum validation.

use std::hash::Hasher;
use ttzip_engine::checksum::{xxh32, Xxh32Hasher};
use ttzip_engine::codecs::lz4::{
    emit_header, header_checksum, is_lz4_frame_magic, is_lz4_legacy_magic, is_lz4_skippable_magic,
    parse_frame_header, parse_header, BlockIndependence, BlockMaxSize, FrameDescriptor,
    LZ4F_MAGICNUMBER, LZ4F_MAGIC_LEGACY, LZ4F_MAGIC_SKIPPABLE_END, LZ4F_MAGIC_SKIPPABLE_MASK,
    LZ4F_MAGIC_SKIPPABLE_START, LZ4F_VERSION_1,
};
use ttzip_engine::types::TTZipStatus;

// MARK: - 1. Magic Number and Skippable Matching Tests

#[test]
fn test_standard_and_legacy_magic_constants() {
    assert_eq!(LZ4F_MAGICNUMBER, 0x184D_2204);
    assert_eq!(LZ4F_MAGIC_LEGACY, 0x184C_2102);

    assert!(is_lz4_frame_magic(LZ4F_MAGICNUMBER));
    assert!(!is_lz4_frame_magic(LZ4F_MAGIC_LEGACY));
    assert!(!is_lz4_frame_magic(0x0000_0000));
    assert!(!is_lz4_frame_magic(0xFFFF_FFFF));

    assert!(is_lz4_legacy_magic(LZ4F_MAGIC_LEGACY));
    assert!(!is_lz4_legacy_magic(LZ4F_MAGICNUMBER));
}

#[test]
fn test_all_16_skippable_magic_numbers_matched() {
    assert_eq!(LZ4F_MAGIC_SKIPPABLE_START, 0x184D_2A50);
    assert_eq!(LZ4F_MAGIC_SKIPPABLE_END, 0x184D_2A5F);
    assert_eq!(LZ4F_MAGIC_SKIPPABLE_MASK, 0xFFFF_FFF0);

    // Exactly 16 skippable magic numbers from 0x184D2A50 to 0x184D2A5F
    for offset in 0..16u32 {
        let magic = LZ4F_MAGIC_SKIPPABLE_START + offset;
        assert!(
            is_lz4_skippable_magic(magic),
            "Magic 0x{:08X} should be identified as skippable",
            magic
        );
        assert!(!is_lz4_frame_magic(magic));
        assert!(!is_lz4_legacy_magic(magic));
    }

    // Boundary values outside skippable range
    assert!(!is_lz4_skippable_magic(0x184D_2A4F));
    assert!(!is_lz4_skippable_magic(0x184D_2A60));
    assert!(!is_lz4_skippable_magic(0x184D_2204));
    assert!(!is_lz4_skippable_magic(0x184C_2102));
}

// MARK: - 2. Block Max Size and Independence Tests

#[test]
fn test_block_max_size_mapping_and_bytes() {
    let sizes = [
        (BlockMaxSize::Max64KB, 4u8, 64 * 1024),
        (BlockMaxSize::Max256KB, 5u8, 256 * 1024),
        (BlockMaxSize::Max1MB, 6u8, 1024 * 1024),
        (BlockMaxSize::Max4MB, 7u8, 4 * 1024 * 1024),
    ];

    for (variant, id, bytes) in sizes {
        assert_eq!(variant.to_id(), id);
        assert_eq!(variant.max_bytes(), bytes);
        assert_eq!(BlockMaxSize::from_id(id).unwrap(), variant);
    }

    // Invalid block size IDs (0..=3 and 8..=255)
    for invalid_id in [0u8, 1, 2, 3, 8, 9, 15, 255] {
        assert!(BlockMaxSize::from_id(invalid_id).is_err());
    }
}

#[test]
fn test_block_independence_flags() {
    let linked = BlockIndependence::Linked;
    let indep = BlockIndependence::Independent;

    assert!(!linked.is_independent());
    assert!(indep.is_independent());

    assert_eq!(BlockIndependence::from_flag(false), linked);
    assert_eq!(BlockIndependence::from_flag(true), indep);
}

// MARK: - 3. Header Checksum and XXH32 Calculation Tests

#[test]
fn test_header_checksum_official_algorithm() {
    // Test case from official LZ4 frame header: FLG = 0x64 (Indep, ContentChecksum), BD = 0x40 (64KB)
    let descriptor = [0x64u8, 0x40u8];
    let computed_hc = header_checksum(&descriptor);

    let raw_xxh32 = xxh32(&descriptor, 0);
    let expected_hc = ((raw_xxh32 >> 8) & 0xFF) as u8;
    assert_eq!(computed_hc, expected_hc);

    // Verify consistency with streaming Xxh32Hasher
    let mut hasher = Xxh32Hasher::new();
    hasher.write(&descriptor);
    assert_eq!(hasher.digest(), raw_xxh32);
}

// MARK: - 4. Frame Descriptor Roundtrip Matrix Tests

#[test]
fn test_minimal_default_descriptor_roundtrip() {
    let desc = FrameDescriptor::default();
    assert_eq!(desc.version, LZ4F_VERSION_1);
    assert_eq!(desc.block_independence, BlockIndependence::Independent);
    assert!(!desc.block_checksum);
    assert!(!desc.content_checksum);
    assert!(desc.content_size.is_none());
    assert!(desc.dict_id.is_none());
    assert_eq!(desc.block_max_size, BlockMaxSize::Max64KB);

    let encoded = desc.emit_to_vec(false).expect("emit default header");
    assert_eq!(encoded.len(), 3); // FLG + BD + HC

    let mut direct_buf = [0u8; 8];
    let written = emit_header(&desc, &mut direct_buf).expect("emit_header free function");
    assert_eq!(written, 3);
    assert_eq!(&direct_buf[..written], &encoded[..]);

    let (parsed, consumed) = parse_header(&encoded).expect("parse default header");
    assert_eq!(consumed, 3);
    assert_eq!(parsed, desc);
}

#[test]
fn test_full_options_descriptor_roundtrip() {
    let desc = FrameDescriptor {
        version: 1,
        block_independence: BlockIndependence::Linked,
        block_checksum: true,
        content_checksum: true,
        content_size: Some(0x0123_4567_89AB_CDEF),
        dict_id: Some(0xDEAD_BEEF),
        block_max_size: BlockMaxSize::Max4MB,
    };

    let encoded_with_magic = desc.emit_to_vec(true).expect("emit full header with magic");
    assert_eq!(encoded_with_magic.len(), 4 + 2 + 8 + 4 + 1); // 4 magic + 2 flg/bd + 8 size + 4 dict + 1 hc = 19

    let (parsed, consumed) = parse_frame_header(&encoded_with_magic).expect("parse full header");
    assert_eq!(consumed, encoded_with_magic.len());
    assert_eq!(parsed, desc);
}

#[test]
fn test_all_block_sizes_and_flags_matrix() {
    let block_sizes = [
        BlockMaxSize::Max64KB,
        BlockMaxSize::Max256KB,
        BlockMaxSize::Max1MB,
        BlockMaxSize::Max4MB,
    ];
    let independences = [BlockIndependence::Linked, BlockIndependence::Independent];
    let bool_flags = [false, true];

    for &bs in &block_sizes {
        for &bi in &independences {
            for &bc in &bool_flags {
                for &cc in &bool_flags {
                    for &has_cs in &bool_flags {
                        for &has_dict in &bool_flags {
                            let desc = FrameDescriptor {
                                version: 1,
                                block_independence: bi,
                                block_checksum: bc,
                                content_checksum: cc,
                                content_size: if has_cs { Some(1048576) } else { None },
                                dict_id: if has_dict { Some(42) } else { None },
                                block_max_size: bs,
                            };

                            let mut buf = [0u8; 32];
                            let written = desc.emit(&mut buf).expect("emit header");
                            let (parsed, consumed) = parse_header(&buf[..written]).expect("parse header");

                            assert_eq!(consumed, written);
                            assert_eq!(parsed, desc);
                        }
                    }
                }
            }
        }
    }
}

// MARK: - 5. Error Rejection and Defensive Validation Tests

#[test]
fn test_reject_corrupted_header_checksum() {
    let desc = FrameDescriptor::default();
    let mut encoded = desc.emit_to_vec(false).expect("emit header");

    // Corrupt the header checksum byte (last byte)
    let hc_idx = encoded.len() - 1;
    encoded[hc_idx] ^= 0xFF;

    let res = parse_header(&encoded);
    assert_eq!(res.unwrap_err(), TTZipStatus::ErrCorruptHeader);
}

#[test]
fn test_reject_corrupted_descriptor_payload() {
    let desc = FrameDescriptor {
        content_size: Some(99999),
        ..FrameDescriptor::default()
    };
    let mut encoded = desc.emit_to_vec(false).expect("emit header");

    // Tamper with content size payload byte without updating HC
    encoded[3] ^= 0x01;

    let res = parse_header(&encoded);
    assert_eq!(res.unwrap_err(), TTZipStatus::ErrCorruptHeader);
}

#[test]
fn test_reject_invalid_versions() {
    for invalid_version in [0u8, 2, 3] {
        let flg = (invalid_version & 0x03) << 6 | 0x20; // with Indep bit
        let bd = 0x40; // 64KB
        let mut raw = [flg, bd, 0u8];
        raw[2] = header_checksum(&raw[..2]);

        let res = parse_header(&raw);
        assert_eq!(
            res.unwrap_err(),
            TTZipStatus::ErrCorruptHeader,
            "Version {} must be rejected",
            invalid_version
        );
    }
}

#[test]
fn test_reject_non_zero_reserved_bits() {
    // 1. FLG bit 1 must be 0
    {
        let flg = 0x40 | 0x02; // Version 1 + Reserved bit 1 set
        let bd = 0x40;
        let mut raw = [flg, bd, 0u8];
        raw[2] = header_checksum(&raw[..2]);

        let res = parse_header(&raw);
        assert_eq!(res.unwrap_err(), TTZipStatus::ErrCorruptHeader);
    }

    // 2. BD bit 7 must be 0
    {
        let flg = 0x40; // Version 1
        let bd = 0x40 | 0x80; // 64KB + Reserved bit 7 set
        let mut raw = [flg, bd, 0u8];
        raw[2] = header_checksum(&raw[..2]);

        let res = parse_header(&raw);
        assert_eq!(res.unwrap_err(), TTZipStatus::ErrCorruptHeader);
    }

    // 3. BD bits 3-0 must be 0
    for low_bit in [0x01u8, 0x02, 0x04, 0x08, 0x0F] {
        let flg = 0x40;
        let bd = 0x40 | low_bit; // 64KB + low reserved bits set
        let mut raw = [flg, bd, 0u8];
        raw[2] = header_checksum(&raw[..2]);

        let res = parse_header(&raw);
        assert_eq!(res.unwrap_err(), TTZipStatus::ErrCorruptHeader);
    }
}

#[test]
fn test_reject_invalid_bd_block_size_id() {
    // IDs 0, 1, 2, 3 in BD bit 6-4 are invalid
    for invalid_id in [0u8, 1, 2, 3] {
        let flg = 0x40;
        let bd = invalid_id << 4;
        let mut raw = [flg, bd, 0u8];
        raw[2] = header_checksum(&raw[..2]);

        let res = parse_header(&raw);
        assert_eq!(res.unwrap_err(), TTZipStatus::ErrCorruptHeader);
    }
}

#[test]
fn test_reject_truncated_buffer_underflow() {
    let desc = FrameDescriptor {
        content_size: Some(12345),
        dict_id: Some(67890),
        ..FrameDescriptor::default()
    };
    let encoded = desc.emit_to_vec(false).expect("emit header");

    for trunc_len in 0..encoded.len() {
        let truncated = &encoded[..trunc_len];
        let res = parse_header(truncated);
        assert_eq!(res.unwrap_err(), TTZipStatus::ErrCorruptHeader);
    }
}

#[test]
fn test_parse_with_magic_rejects_wrong_magic() {
    let desc = FrameDescriptor::default();
    let mut encoded = desc.emit_to_vec(true).expect("emit with magic");

    // Corrupt magic bytes
    encoded[0] ^= 0x01;
    let res = parse_frame_header(&encoded);
    assert_eq!(res.unwrap_err(), TTZipStatus::ErrCorruptHeader);
}
