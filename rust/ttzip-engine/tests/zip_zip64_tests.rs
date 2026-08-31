// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and integration test suite for Zip64 Dual-Threshold Promotion
//! State Machine, LFH/CDFH Type-Safe Orchestrator, and EOCD Record/Locator.

use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::types::TTZipStatus;
use ttzip_engine::zip::parser::{
    find_eocd, parse_all_entries, parse_cdfh_entry, MAGIC_CDFH, MAGIC_EOCD, MAGIC_LFH,
    MAGIC_ZIP64_EOCD, MAGIC_ZIP64_LOCATOR,
};
use ttzip_engine::zip::reader::ZipArchive;
use ttzip_engine::zip::writer::{assemble_zip_archive, ZipCompressedItem};
use ttzip_engine::zip::zip64::{
    Zip64DecisionMatrix, Zip64EocdLocator, Zip64EocdRecord, Zip64ExtraField, SENTINEL_U16,
    SENTINEL_U32, TAG_ZIP64, ZIP64_BYTES_THR, ZIP64_ENTRY_THR, ZIP64_EOCD_MIN_SIZE,
    ZIP64_LFH_PAYLOAD_SIZE, ZIP64_LOCATOR_SIZE, ZIP64_VERSION_NEEDED,
};

#[test]
fn test_zip64_constants_and_sentinels() {
    assert_eq!(ZIP64_BYTES_THR, 0xFFFF_FFFF);
    assert_eq!(ZIP64_ENTRY_THR, 0xFFFF);
    assert_eq!(TAG_ZIP64, 0x0001);
    assert_eq!(MAGIC_ZIP64_EOCD, 0x06064B50);
    assert_eq!(MAGIC_ZIP64_LOCATOR, 0x07064B50);
    assert_eq!(ZIP64_EOCD_MIN_SIZE, 56);
    assert_eq!(ZIP64_LOCATOR_SIZE, 20);
    assert_eq!(ZIP64_LFH_PAYLOAD_SIZE, 16);
    assert_eq!(ZIP64_VERSION_NEEDED, 45);
    assert_eq!(SENTINEL_U32, 0xFFFF_FFFF);
    assert_eq!(SENTINEL_U16, 0xFFFF);
}

#[test]
fn test_zip64_eocd_record_and_locator_roundtrip() {
    // 1. Standard Zip64 EOCD Record without extensible data
    let total_entries = 120_000u64;
    let cd_size = 8_589_934_592u64; // 8GB
    let cd_offset = 17_179_869_184u64; // 16GB

    let record = Zip64EocdRecord::new(total_entries, cd_size, cd_offset);
    assert_eq!(record.size_of_record, 44);
    assert_eq!(record.version_made_by, 0x032D);
    assert_eq!(record.version_needed, 45);
    assert_eq!(record.total_entries, total_entries);
    assert_eq!(record.entries_on_this_disk, total_entries);
    assert_eq!(record.cd_size, cd_size);
    assert_eq!(record.cd_offset, cd_offset);

    let serialized = record.serialize();
    assert_eq!(serialized.len(), 56);

    let parsed = Zip64EocdRecord::parse(&serialized).expect("parsing Zip64 EOCD record failed");
    assert_eq!(record, parsed);

    // 2. Zip64 EOCD Record with custom extensible data
    let mut custom_record = record.clone();
    custom_record.extensible_data = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE];
    let custom_serialized = custom_record.serialize();
    assert_eq!(custom_serialized.len(), 56 + 8);

    let custom_parsed = Zip64EocdRecord::parse(&custom_serialized).expect("parsing custom Zip64 EOCD failed");
    assert_eq!(custom_parsed.size_of_record, 44 + 8);
    assert_eq!(custom_parsed.extensible_data, custom_record.extensible_data);

    // 3. Zip64 EOCD Locator
    let zip64_eocd_offset = cd_offset + cd_size;
    let locator = Zip64EocdLocator::new(zip64_eocd_offset);
    assert_eq!(locator.disk_with_zip64_eocd, 0);
    assert_eq!(locator.zip64_eocd_offset, zip64_eocd_offset);
    assert_eq!(locator.total_disks, 1);

    let loc_bytes = locator.serialize();
    assert_eq!(loc_bytes.len(), 20);

    let loc_parsed = Zip64EocdLocator::parse(&loc_bytes).expect("parsing Zip64 locator failed");
    assert_eq!(locator, loc_parsed);
}

#[test]
fn test_zip64_eocd_boundary_errors() {
    // Truncated EOCD record
    let short_record = vec![0u8; 40];
    assert_eq!(Zip64EocdRecord::parse(&short_record), Err(TTZipStatus::ErrCorruptHeader));

    // Invalid magic in EOCD record
    let mut bad_magic_record = Zip64EocdRecord::new(10, 100, 200).serialize();
    bad_magic_record[0] = 0x00;
    assert_eq!(Zip64EocdRecord::parse(&bad_magic_record), Err(TTZipStatus::ErrCorruptHeader));

    // Truncated locator
    let short_loc = vec![0u8; 15];
    assert_eq!(Zip64EocdLocator::parse(&short_loc), Err(TTZipStatus::ErrCorruptHeader));

    // Invalid magic in locator
    let mut bad_magic_loc = Zip64EocdLocator::new(100).serialize_vec();
    bad_magic_loc[0] = 0xFF;
    assert_eq!(Zip64EocdLocator::parse(&bad_magic_loc), Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_zip64_extra_field_lfh_strict_16_bytes() {
    let uncomp_size = 5_000_000_000u64; // ~5GB
    let comp_size = 4_500_000_000u64; // ~4.5GB

    let lfh_extra = Zip64ExtraField::build_lfh(uncomp_size, comp_size);
    // Header (4B) + Payload (16B) = 20B
    assert_eq!(lfh_extra.len(), 20);

    let tag = u16::from_le_bytes([lfh_extra[0], lfh_extra[1]]);
    let size = u16::from_le_bytes([lfh_extra[2], lfh_extra[3]]);
    assert_eq!(tag, TAG_ZIP64);
    assert_eq!(size, 16);

    let payload = &lfh_extra[4..];
    let (parsed_uncomp, parsed_comp) = Zip64ExtraField::parse_lfh_payload(payload).expect("parse LFH payload failed");
    assert_eq!(parsed_uncomp, uncomp_size);
    assert_eq!(parsed_comp, comp_size);

    // Parsing truncated payload fails
    assert_eq!(Zip64ExtraField::parse_lfh_payload(&payload[..10]), Err(TTZipStatus::ErrCorruptHeader));

    // Enum serialize matches builder
    let enum_lfh = Zip64ExtraField::Lfh {
        uncompressed_size: uncomp_size,
        compressed_size: comp_size,
    };
    assert_eq!(enum_lfh.serialize(), lfh_extra);
}

#[test]
fn test_zip64_extra_field_cdfh_dynamic_variable_length() {
    let uncomp = 6_000_000_000u64;
    let comp = 5_000_000_000u64;
    let offset = 10_000_000_000u64;
    let disk = 2u32;

    // Case 1: 8-byte payload (only uncompressed)
    let cdfh_8b = Zip64ExtraField::build_cdfh(Some(uncomp), None, None, None);
    assert_eq!(cdfh_8b.len(), 4 + 8);
    let parsed_8b = Zip64ExtraField::parse_cdfh_payload(&cdfh_8b[4..], true, false, false, false).unwrap();
    assert_eq!(
        parsed_8b,
        Zip64ExtraField::Cdfh {
            uncompressed_size: Some(uncomp),
            compressed_size: None,
            local_header_offset: None,
            disk_start_number: None,
        }
    );

    // Case 2: 8-byte payload (only offset >= 4GB)
    let cdfh_off_8b = Zip64ExtraField::build_cdfh(None, None, Some(offset), None);
    assert_eq!(cdfh_off_8b.len(), 4 + 8);
    let parsed_off = Zip64ExtraField::parse_cdfh_payload(&cdfh_off_8b[4..], false, false, true, false).unwrap();
    assert_eq!(
        parsed_off,
        Zip64ExtraField::Cdfh {
            uncompressed_size: None,
            compressed_size: None,
            local_header_offset: Some(offset),
            disk_start_number: None,
        }
    );

    // Case 3: 16-byte payload (uncomp + comp)
    let cdfh_16b = Zip64ExtraField::build_cdfh(Some(uncomp), Some(comp), None, None);
    assert_eq!(cdfh_16b.len(), 4 + 16);
    let parsed_16b = Zip64ExtraField::parse_cdfh_payload(&cdfh_16b[4..], true, true, false, false).unwrap();
    assert_eq!(
        parsed_16b,
        Zip64ExtraField::Cdfh {
            uncompressed_size: Some(uncomp),
            compressed_size: Some(comp),
            local_header_offset: None,
            disk_start_number: None,
        }
    );

    // Case 4: 24-byte payload (uncomp + comp + offset)
    let cdfh_24b = Zip64ExtraField::build_cdfh(Some(uncomp), Some(comp), Some(offset), None);
    assert_eq!(cdfh_24b.len(), 4 + 24);
    let parsed_24b = Zip64ExtraField::parse_cdfh_payload(&cdfh_24b[4..], true, true, true, false).unwrap();
    assert_eq!(
        parsed_24b,
        Zip64ExtraField::Cdfh {
            uncompressed_size: Some(uncomp),
            compressed_size: Some(comp),
            local_header_offset: Some(offset),
            disk_start_number: None,
        }
    );

    // Case 5: 28-byte payload (uncomp + comp + offset + disk)
    let cdfh_28b = Zip64ExtraField::build_cdfh(Some(uncomp), Some(comp), Some(offset), Some(disk));
    assert_eq!(cdfh_28b.len(), 4 + 28);
    let parsed_28b = Zip64ExtraField::parse_cdfh_payload(&cdfh_28b[4..], true, true, true, true).unwrap();
    assert_eq!(
        parsed_28b,
        Zip64ExtraField::Cdfh {
            uncompressed_size: Some(uncomp),
            compressed_size: Some(comp),
            local_header_offset: Some(offset),
            disk_start_number: Some(disk),
        }
    );

    // Empty fields return empty vector
    assert!(Zip64ExtraField::build_cdfh(None, None, None, None).is_empty());
}

#[test]
fn test_zip64_decision_matrix_lfh_thresholds() {
    // 1. Small file (< 4GB)
    let small_dec = Zip64DecisionMatrix::evaluate_lfh(1024, 512);
    assert!(!small_dec.is_zip64);
    assert_eq!(small_dec.uncompressed_size_field, 1024);
    assert_eq!(small_dec.compressed_size_field, 512);
    assert_eq!(small_dec.version_needed, 20);
    assert!(small_dec.extra_bytes.is_empty());
    assert_eq!(small_dec.extra_field, None);

    // 2. Exact Boundary - 1 (0xFFFF_FFFE)
    let b_minus_1 = Zip64DecisionMatrix::evaluate_lfh(0xFFFF_FFFE, 100);
    assert!(!b_minus_1.is_zip64);
    assert_eq!(b_minus_1.uncompressed_size_field, 0xFFFF_FFFE);

    // 3. Exact Boundary (0xFFFF_FFFF = 4GB - 1 sentinel threshold)
    let b_sentinel = Zip64DecisionMatrix::evaluate_lfh(0xFFFF_FFFF, 100);
    assert!(b_sentinel.is_zip64);
    assert_eq!(b_sentinel.uncompressed_size_field, SENTINEL_U32);
    assert_eq!(b_sentinel.compressed_size_field, SENTINEL_U32);
    assert_eq!(b_sentinel.version_needed, 45);
    assert_eq!(b_sentinel.extra_bytes.len(), 20);

    // 4. Single large uncompressed file (5GB, 1GB compressed)
    let large_uncomp = Zip64DecisionMatrix::evaluate_lfh(5_000_000_000, 1_000_000_000);
    assert!(large_uncomp.is_zip64);
    assert_eq!(large_uncomp.uncompressed_size_field, SENTINEL_U32);
    assert_eq!(large_uncomp.compressed_size_field, SENTINEL_U32);
    assert_eq!(large_uncomp.version_needed, 45);
    assert_eq!(large_uncomp.extra_bytes.len(), 20);

    // 5. Large compressed size (incompressible data >= 4GB)
    let large_comp = Zip64DecisionMatrix::evaluate_lfh(1_000_000_000, 5_000_000_000);
    assert!(large_comp.is_zip64);
    assert_eq!(large_comp.uncompressed_size_field, SENTINEL_U32);
    assert_eq!(large_comp.compressed_size_field, SENTINEL_U32);
}

#[test]
fn test_zip64_decision_matrix_cdfh_thresholds() {
    // 1. Small file with small offset (< 4GB)
    let small = Zip64DecisionMatrix::evaluate_cdfh(2048, 1024, 5000);
    assert!(!small.is_zip64);
    assert_eq!(small.uncompressed_size_field, 2048);
    assert_eq!(small.compressed_size_field, 1024);
    assert_eq!(small.local_header_offset_field, 5000);
    assert_eq!(small.version_needed, 20);
    assert!(small.extra_bytes.is_empty());

    // 2. Small file but local offset >= 4GB (archive preceding data > 4GB)
    let large_offset = Zip64DecisionMatrix::evaluate_cdfh(2048, 1024, 6_000_000_000);
    assert!(large_offset.is_zip64);
    assert_eq!(large_offset.uncompressed_size_field, 2048);
    assert_eq!(large_offset.compressed_size_field, 1024);
    assert_eq!(large_offset.local_header_offset_field, SENTINEL_U32);
    assert_eq!(large_offset.version_needed, 45);
    // Payload should be 8 bytes (local_header_offset only) -> Total 12 bytes
    assert_eq!(large_offset.extra_bytes.len(), 12);

    // 3. Large uncompressed & compressed, small offset
    let large_both = Zip64DecisionMatrix::evaluate_cdfh(5_000_000_000, 4_500_000_000, 1000);
    assert!(large_both.is_zip64);
    assert_eq!(large_both.uncompressed_size_field, SENTINEL_U32);
    assert_eq!(large_both.compressed_size_field, SENTINEL_U32);
    assert_eq!(large_both.local_header_offset_field, 1000);
    // Payload should be 16 bytes -> Total 20 bytes
    assert_eq!(large_both.extra_bytes.len(), 20);

    // 4. Large uncompressed, compressed, and offset
    let all_large = Zip64DecisionMatrix::evaluate_cdfh(5_000_000_000, 4_500_000_000, 10_000_000_000);
    assert!(all_large.is_zip64);
    assert_eq!(all_large.uncompressed_size_field, SENTINEL_U32);
    assert_eq!(all_large.compressed_size_field, SENTINEL_U32);
    assert_eq!(all_large.local_header_offset_field, SENTINEL_U32);
    // Payload should be 24 bytes -> Total 28 bytes
    assert_eq!(all_large.extra_bytes.len(), 28);
}

#[test]
fn test_zip64_decision_matrix_eocd_thresholds() {
    // 1. Small archive
    let small = Zip64DecisionMatrix::evaluate_eocd(500, 20_000, 100_000);
    assert!(!small.is_zip64);
    assert_eq!(small.total_entries_field, 500);
    assert_eq!(small.cd_size_field, 20_000);
    assert_eq!(small.cd_offset_field, 100_000);
    assert_eq!(small.zip64_eocd, None);
    assert_eq!(small.zip64_locator, None);

    // 2. Entry count boundary - 1 (65534)
    let e_minus_1 = Zip64DecisionMatrix::evaluate_eocd(65534, 1000, 2000);
    assert!(!e_minus_1.is_zip64);
    assert_eq!(e_minus_1.total_entries_field, 65534);

    // 3. Entry count boundary (65535 = 0xFFFF)
    let e_sentinel = Zip64DecisionMatrix::evaluate_eocd(65535, 1000, 2000);
    assert!(e_sentinel.is_zip64);
    assert_eq!(e_sentinel.total_entries_field, SENTINEL_U16);
    let rec = e_sentinel.zip64_eocd.unwrap();
    assert_eq!(rec.total_entries, 65535);
    let loc = e_sentinel.zip64_locator.unwrap();
    assert_eq!(loc.zip64_eocd_offset, 3000); // 2000 + 1000

    // 4. Massive catalog (1,000,000 files)
    let million = Zip64DecisionMatrix::evaluate_eocd(1_000_000, 80_000_000, 500_000_000);
    assert!(million.is_zip64);
    assert_eq!(million.total_entries_field, SENTINEL_U16);
    assert_eq!(million.zip64_eocd.unwrap().total_entries, 1_000_000);

    // 5. CD offset >= 4GB
    let large_cd_off = Zip64DecisionMatrix::evaluate_eocd(10, 500, 5_000_000_000);
    assert!(large_cd_off.is_zip64);
    assert_eq!(large_cd_off.cd_offset_field, SENTINEL_U32);
    assert_eq!(large_cd_off.total_entries_field, 10);
    assert_eq!(large_cd_off.zip64_eocd.unwrap().cd_offset, 5_000_000_000);

    // 6. CD size >= 4GB
    let large_cd_sz = Zip64DecisionMatrix::evaluate_eocd(10, 5_000_000_000, 1000);
    assert!(large_cd_sz.is_zip64);
    assert_eq!(large_cd_sz.cd_size_field, SENTINEL_U32);
    assert_eq!(large_cd_sz.zip64_eocd.unwrap().cd_size, 5_000_000_000);
}

#[test]
fn test_zip64_synthetic_archive_parser_roundtrip() {
    // Construct a synthetic ZIP archive binary layout containing Zip64 structures:
    // Entry 1: uncompressed 5GB, compressed 1GB, lfh_offset 0
    // Entry 2: uncompressed 100B, compressed 100B, lfh_offset 5GB (0x1_2A05F200)

    let mut buf = Vec::new();

    // 1. Entry 1 LFH (mocked payload omitted for memory efficiency in unit test)
    let lfh1_dec = Zip64DecisionMatrix::evaluate_lfh(5_000_000_000, 1_000_000_000);
    assert!(lfh1_dec.is_zip64);
    let lfh1_off = buf.len() as u64;

    buf.extend_from_slice(&MAGIC_LFH.to_le_bytes());
    buf.extend_from_slice(&lfh1_dec.version_needed.to_le_bytes());
    buf.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8
    buf.extend_from_slice(&0u16.to_le_bytes()); // Store
    buf.extend_from_slice(&0u16.to_le_bytes()); // dos time
    buf.extend_from_slice(&0u16.to_le_bytes()); // dos date
    buf.extend_from_slice(&0x11223344u32.to_le_bytes()); // crc32
    buf.extend_from_slice(&lfh1_dec.compressed_size_field.to_le_bytes());
    buf.extend_from_slice(&lfh1_dec.uncompressed_size_field.to_le_bytes());
    let name1 = b"huge_file.bin";
    buf.extend_from_slice(&(name1.len() as u16).to_le_bytes());
    buf.extend_from_slice(&(lfh1_dec.extra_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(name1);
    buf.extend_from_slice(&lfh1_dec.extra_bytes);

    // 2. Central Directory
    let cd_start = buf.len() as u64;

    // CDFH 1: Uncompressed size >= 4GB (8B extra field payload)
    let cdfh1_dec = Zip64DecisionMatrix::evaluate_cdfh(5_000_000_000, 1_000_000_000, lfh1_off);
    assert!(cdfh1_dec.is_zip64);
    buf.extend_from_slice(&MAGIC_CDFH.to_le_bytes());
    buf.extend_from_slice(&0x032Du16.to_le_bytes()); // Unix + 4.5
    buf.extend_from_slice(&cdfh1_dec.version_needed.to_le_bytes());
    buf.extend_from_slice(&0x0800u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&0x11223344u32.to_le_bytes());
    buf.extend_from_slice(&cdfh1_dec.compressed_size_field.to_le_bytes());
    buf.extend_from_slice(&cdfh1_dec.uncompressed_size_field.to_le_bytes());
    buf.extend_from_slice(&(name1.len() as u16).to_le_bytes());
    buf.extend_from_slice(&(cdfh1_dec.extra_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes()); // comment len
    buf.extend_from_slice(&0u16.to_le_bytes()); // disk
    buf.extend_from_slice(&0u16.to_le_bytes()); // int attr
    buf.extend_from_slice(&0o1006440000u32.to_le_bytes()); // ext attr
    buf.extend_from_slice(&cdfh1_dec.local_header_offset_field.to_le_bytes());
    buf.extend_from_slice(name1);
    buf.extend_from_slice(&cdfh1_dec.extra_bytes);

    // CDFH 2: Offset >= 4GB (8B extra field payload for offset)
    let fake_large_offset = 6_000_000_000u64;
    let cdfh2_dec = Zip64DecisionMatrix::evaluate_cdfh(100, 100, fake_large_offset);
    assert!(cdfh2_dec.is_zip64);
    let name2 = b"at_huge_offset.txt";
    buf.extend_from_slice(&MAGIC_CDFH.to_le_bytes());
    buf.extend_from_slice(&0x032Du16.to_le_bytes());
    buf.extend_from_slice(&cdfh2_dec.version_needed.to_le_bytes());
    buf.extend_from_slice(&0x0800u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&0x55667788u32.to_le_bytes());
    buf.extend_from_slice(&cdfh2_dec.compressed_size_field.to_le_bytes());
    buf.extend_from_slice(&cdfh2_dec.uncompressed_size_field.to_le_bytes());
    buf.extend_from_slice(&(name2.len() as u16).to_le_bytes());
    buf.extend_from_slice(&(cdfh2_dec.extra_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&0o1006440000u32.to_le_bytes());
    buf.extend_from_slice(&cdfh2_dec.local_header_offset_field.to_le_bytes());
    buf.extend_from_slice(name2);
    buf.extend_from_slice(&cdfh2_dec.extra_bytes);

    let cd_size = (buf.len() as u64) - cd_start;

    // 3. Zip64 EOCD Record & Locator
    let _eocd_dec = Zip64DecisionMatrix::evaluate_eocd(2, cd_size, cd_start);
    // Here cd_start is small, but let's force Zip64 EOCD emission
    let z64_rec = Zip64EocdRecord::new(2, cd_size, cd_start);
    let z64_rec_pos = buf.len() as u64;
    buf.extend_from_slice(&z64_rec.serialize());

    let z64_loc = Zip64EocdLocator::new(z64_rec_pos);
    buf.extend_from_slice(&z64_loc.serialize());

    // 4. Standard EOCD Record
    buf.extend_from_slice(&MAGIC_EOCD.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes()); // disk
    buf.extend_from_slice(&0u16.to_le_bytes()); // cd disk
    buf.extend_from_slice(&2u16.to_le_bytes()); // entries disk
    buf.extend_from_slice(&2u16.to_le_bytes()); // total entries
    buf.extend_from_slice(&(cd_size as u32).to_le_bytes());
    buf.extend_from_slice(&(cd_start as u32).to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes()); // comment len

    // 5. Zero-copy parser verification
    let eocd_info = find_eocd(&buf).expect("find_eocd failed");
    assert!(eocd_info.is_zip64);
    assert_eq!(eocd_info.total_entries, 2);
    assert_eq!(eocd_info.cd_size, cd_size);
    assert_eq!(eocd_info.cd_offset, cd_start);

    // Verify individual CDFH entry parsing
    let (cdfh1_entry, next_pos) = parse_cdfh_entry(&buf, cd_start as usize).expect("parse_cdfh_entry 1 failed");
    assert_eq!(cdfh1_entry.rel_path, "huge_file.bin");
    assert_eq!(cdfh1_entry.uncompressed_size, 5_000_000_000);
    assert_eq!(cdfh1_entry.compressed_size, 1_000_000_000);

    let (cdfh2_entry, _) = parse_cdfh_entry(&buf, next_pos).expect("parse_cdfh_entry 2 failed");
    assert_eq!(cdfh2_entry.rel_path, "at_huge_offset.txt");
    assert_eq!(cdfh2_entry.lfh_offset, fake_large_offset);

    let entries = parse_all_entries(&buf).expect("parse_all_entries failed");
    assert_eq!(entries.len(), 2);

    // Entry 1 verification
    assert_eq!(entries[0].rel_path, "huge_file.bin");
    assert_eq!(entries[0].uncompressed_size, 5_000_000_000);
    assert_eq!(entries[0].compressed_size, 1_000_000_000);
    assert_eq!(entries[0].lfh_offset, 0);

    // Entry 2 verification (promoted offset)
    assert_eq!(entries[1].rel_path, "at_huge_offset.txt");
    assert_eq!(entries[1].uncompressed_size, 100);
    assert_eq!(entries[1].compressed_size, 100);
    assert_eq!(entries[1].lfh_offset, fake_large_offset);
}

#[test]
fn test_zip64_compact_non_zip64_pure_fallback() {
    let payload1 = b"Hello, World!".to_vec();
    let crc1 = crc32_fast(0, &payload1);

    let payload2 = b"Second Payload".to_vec();
    let crc2 = crc32_fast(0, &payload2);

    let items = vec![
        ZipCompressedItem {
            rel_path: "file1.txt".to_string(),
            uncompressed_size: payload1.len() as u64,
            compressed_size: payload1.len() as u64,
            crc32: crc1,
            compression_method: 0,
            actual_method: 0,
            aes_strength: 0,
            payload: payload1,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        ZipCompressedItem {
            rel_path: "file2.txt".to_string(),
            uncompressed_size: payload2.len() as u64,
            compressed_size: payload2.len() as u64,
            crc32: crc2,
            compression_method: 0,
            actual_method: 0,
            aes_strength: 0,
            payload: payload2,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
    ];

    let zip_bytes = assemble_zip_archive(&items).expect("assemble failed");

    // Must NOT contain Zip64 EOCD or Locator signatures
    let z64_eocd_bytes = MAGIC_ZIP64_EOCD.to_le_bytes();
    let z64_loc_bytes = MAGIC_ZIP64_LOCATOR.to_le_bytes();
    assert!(!zip_bytes.windows(4).any(|w| w == z64_eocd_bytes));
    assert!(!zip_bytes.windows(4).any(|w| w == z64_loc_bytes));

    let archive = ZipArchive::open_slice(&zip_bytes).expect("open slice failed");
    assert_eq!(archive.len(), 2);
    assert_eq!(archive.entries()[0].uncompressed_size, 13);
    assert_eq!(archive.entries()[1].uncompressed_size, 14);

    let e0 = archive.extract_entry_bytes(0, None).expect("extract e0 failed");
    assert_eq!(e0, b"Hello, World!");
    let e1 = archive.extract_entry_bytes(1, None).expect("extract e1 failed");
    assert_eq!(e1, b"Second Payload");
}
