// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for ZIP Pod fixed-size binary blocks and to_and_from_le! macro system.

use std::mem::{align_of, size_of};
use ttzip_engine::to_and_from_le;
use ttzip_engine::zip::blocks::{
    FixedSizeBlock, Pod, Zip32CDEBlock, Zip64CDEBlock, Zip64CDELocatorBlock,
    Zip64DataDescriptorBlock, ZipCentralEntryBlock, ZipDataDescriptorBlock, ZipLocalEntryBlock,
};

// =============================================================================
// 1. Compile-Time Layout & Exact Byte Size Contract Assertions
// =============================================================================

#[test]
fn test_fixed_size_blocks_size_and_alignment_contract() {
    // 1. ZipLocalEntryBlock: exactly 26 bytes
    assert_eq!(size_of::<ZipLocalEntryBlock>(), 26);
    assert_eq!(ZipLocalEntryBlock::SIZE, 26);
    assert_eq!(align_of::<ZipLocalEntryBlock>(), 1);
    assert_eq!(ZipLocalEntryBlock::MAGIC, 0x04034B50);

    // 2. ZipCentralEntryBlock: exactly 42 bytes
    assert_eq!(size_of::<ZipCentralEntryBlock>(), 42);
    assert_eq!(ZipCentralEntryBlock::SIZE, 42);
    assert_eq!(align_of::<ZipCentralEntryBlock>(), 1);
    assert_eq!(ZipCentralEntryBlock::MAGIC, 0x02014B50);

    // 3. ZipDataDescriptorBlock: exactly 12 bytes
    assert_eq!(size_of::<ZipDataDescriptorBlock>(), 12);
    assert_eq!(ZipDataDescriptorBlock::SIZE, 12);
    assert_eq!(align_of::<ZipDataDescriptorBlock>(), 1);
    assert_eq!(ZipDataDescriptorBlock::MAGIC, 0x08074B50);

    // 4. Zip64DataDescriptorBlock: exactly 20 bytes
    assert_eq!(size_of::<Zip64DataDescriptorBlock>(), 20);
    assert_eq!(Zip64DataDescriptorBlock::SIZE, 20);
    assert_eq!(align_of::<Zip64DataDescriptorBlock>(), 1);
    assert_eq!(Zip64DataDescriptorBlock::MAGIC, 0x08074B50);

    // 5. Zip32CDEBlock: exactly 18 bytes
    assert_eq!(size_of::<Zip32CDEBlock>(), 18);
    assert_eq!(Zip32CDEBlock::SIZE, 18);
    assert_eq!(align_of::<Zip32CDEBlock>(), 1);
    assert_eq!(Zip32CDEBlock::MAGIC, 0x06054B50);

    // 6. Zip64CDELocatorBlock: exactly 16 bytes
    assert_eq!(size_of::<Zip64CDELocatorBlock>(), 16);
    assert_eq!(Zip64CDELocatorBlock::SIZE, 16);
    assert_eq!(align_of::<Zip64CDELocatorBlock>(), 1);
    assert_eq!(Zip64CDELocatorBlock::MAGIC, 0x07064B50);

    // 7. Zip64CDEBlock: exactly 52 bytes
    assert_eq!(size_of::<Zip64CDEBlock>(), 52);
    assert_eq!(Zip64CDEBlock::SIZE, 52);
    assert_eq!(align_of::<Zip64CDEBlock>(), 1);
    assert_eq!(Zip64CDEBlock::MAGIC, 0x06064B50);
}

// =============================================================================
// 2. Endian Conversion & 100% Roundtrip Fidelity Tests
// =============================================================================

#[test]
fn test_zip_local_entry_block_endian_roundtrip() {
    let block = ZipLocalEntryBlock {
        version_needed: 0x1400,
        general_purpose_flag: 0x0808,
        compression_method: 0x0800,
        last_mod_time: 0x1234,
        last_mod_date: 0x5678,
        crc32: 0xDEADBEEF,
        compressed_size: 0x01020304,
        uncompressed_size: 0x05060708,
        file_name_length: 0x0010,
        extra_field_length: 0x0020,
    };

    let le_block = block.to_le();
    let restored = le_block.from_le();
    assert_eq!(block, restored);

    let mut dest = [0u8; 26];
    let written = block.write(&mut dest).expect("write failed");
    assert_eq!(written, 26);

    let (parsed, consumed) = ZipLocalEntryBlock::parse(&dest).expect("parse failed");
    assert_eq!(consumed, 26);
    assert_eq!(parsed, block);
}

#[test]
fn test_zip_central_entry_block_endian_roundtrip() {
    let block = ZipCentralEntryBlock {
        version_made_by: 0x031E,
        version_needed: 0x002D,
        general_purpose_flag: 0x0800,
        compression_method: 0x0008,
        last_mod_time: 0x4321,
        last_mod_date: 0x8765,
        crc32: 0xCAFEBABE,
        compressed_size: 0xA1B2C3D4,
        uncompressed_size: 0xE5F60718,
        file_name_length: 0x0018,
        extra_field_length: 0x0024,
        file_comment_length: 0x000A,
        disk_number_start: 0x0001,
        internal_file_attributes: 0x0002,
        external_file_attributes: 0x81A40000,
        relative_offset_of_local_header: 0x12345678,
    };

    let le_block = block.to_le();
    let restored = le_block.from_le();
    assert_eq!(block, restored);

    let mut dest = [0u8; 42];
    let written = block.write(&mut dest).expect("write failed");
    assert_eq!(written, 42);

    let (parsed, consumed) = ZipCentralEntryBlock::parse(&dest).expect("parse failed");
    assert_eq!(consumed, 42);
    assert_eq!(parsed, block);
}

#[test]
fn test_zip_data_descriptor_blocks_endian_roundtrip() {
    // 32-bit data descriptor
    let dd32 = ZipDataDescriptorBlock {
        crc32: 0x12345678,
        compressed_size: 0x87654321,
        uncompressed_size: 0xABCDEF01,
    };
    let mut buf32 = [0u8; 12];
    assert_eq!(dd32.write(&mut buf32), Some(12));
    let (parsed32, n32) = ZipDataDescriptorBlock::parse(&buf32).expect("parse 32 failed");
    assert_eq!(n32, 12);
    assert_eq!(parsed32, dd32);

    // 64-bit data descriptor
    let dd64 = Zip64DataDescriptorBlock {
        crc32: 0x98765432,
        compressed_size: 0x0102030405060708,
        uncompressed_size: 0x090A0B0C0D0E0F10,
    };
    let mut buf64 = [0u8; 20];
    assert_eq!(dd64.write(&mut buf64), Some(20));
    let (parsed64, n64) = Zip64DataDescriptorBlock::parse(&buf64).expect("parse 64 failed");
    assert_eq!(n64, 20);
    assert_eq!(parsed64, dd64);
}

#[test]
fn test_zip_cde_and_zip64_cde_blocks_endian_roundtrip() {
    // Zip32 EOCD
    let cde32 = Zip32CDEBlock {
        disk_number: 0x0001,
        disk_with_central_directory: 0x0002,
        total_entries_this_disk: 0x0040,
        total_entries: 0x0080,
        central_directory_size: 0x00002000,
        central_directory_offset: 0x00004000,
        comment_length: 0x0010,
    };
    let mut buf_cde32 = [0u8; 18];
    assert_eq!(cde32.write(&mut buf_cde32), Some(18));
    let (parsed_cde32, len32) = Zip32CDEBlock::parse(&buf_cde32).expect("parse cde32 failed");
    assert_eq!(len32, 18);
    assert_eq!(parsed_cde32, cde32);

    // Zip64 Locator
    let loc64 = Zip64CDELocatorBlock {
        disk_with_zip64_central_directory: 0x00000001,
        zip64_central_directory_offset: 0x00000000FFFF0000,
        total_number_of_disks: 0x00000002,
    };
    let mut buf_loc64 = [0u8; 16];
    assert_eq!(loc64.write(&mut buf_loc64), Some(16));
    let (parsed_loc64, len_loc64) =
        Zip64CDELocatorBlock::parse(&buf_loc64).expect("parse loc64 failed");
    assert_eq!(len_loc64, 16);
    assert_eq!(parsed_loc64, loc64);

    // Zip64 EOCD Record
    let cde64 = Zip64CDEBlock {
        record_size: 44,
        version_made_by: 0x031E,
        version_needed: 0x002D,
        disk_number: 0,
        disk_with_central_directory: 0,
        total_entries_this_disk: 100_000,
        total_entries: 100_000,
        central_directory_size: 0x00000001_00000000,
        central_directory_offset: 0x00000002_00000000,
    };
    let mut buf_cde64 = [0u8; 52];
    assert_eq!(cde64.write(&mut buf_cde64), Some(52));
    let (parsed_cde64, len_cde64) = Zip64CDEBlock::parse(&buf_cde64).expect("parse cde64 failed");
    assert_eq!(len_cde64, 52);
    assert_eq!(parsed_cde64, cde64);
}

// =============================================================================
// 3. Zero-Copy Slice Casting & Zero Allocation Tests
// =============================================================================

#[test]
fn test_zero_copy_pod_and_prefix_slicing() {
    let original = ZipLocalEntryBlock {
        version_needed: 20,
        general_purpose_flag: 0,
        compression_method: 8,
        last_mod_time: 0x4820,
        last_mod_date: 0x54A8,
        crc32: 0x12345678,
        compressed_size: 1024,
        uncompressed_size: 2048,
        file_name_length: 12,
        extra_field_length: 0,
    };

    let mut stream = Vec::new();
    original.write_to_vec(&mut stream);
    let trailing_payload = b"payload_bytes_here";
    stream.extend_from_slice(trailing_payload);

    // 1. parse_from_prefix
    let (parsed, remaining) =
        ZipLocalEntryBlock::parse_from_prefix(&stream).expect("parse_from_prefix failed");
    assert_eq!(parsed, original);
    assert_eq!(remaining, trailing_payload);

    // 2. Pod::ref_from_prefix zero-copy reference casting
    let (pod_ref, pod_remaining) =
        ZipLocalEntryBlock::ref_from_prefix(&stream).expect("ref_from_prefix failed");
    assert_eq!(pod_remaining, trailing_payload);
    assert_eq!(pod_ref.as_bytes().len(), 26);

    // 3. Pod::as_bytes byte slice conversion
    let bytes_view = original.as_bytes();
    assert_eq!(bytes_view.len(), 26);
    let (re_parsed, _) = ZipLocalEntryBlock::parse(bytes_view).expect("parse from as_bytes failed");
    assert_eq!(re_parsed, original);

    // 4. Truncated slice safety verification
    assert!(ZipLocalEntryBlock::parse(&stream[..25]).is_none());
    assert!(ZipLocalEntryBlock::parse_from_prefix(&stream[..10]).is_none());
    assert!(ZipLocalEntryBlock::ref_from_prefix(&stream[..20]).is_none());
}

// =============================================================================
// 4. Macro System Verification with Custom Block
// =============================================================================

to_and_from_le! {
    /// Custom test block verifying to_and_from_le! macro expansion outside blocks.rs.
    pub struct CustomZipTestBlock {
        magic: 0xAABBCCDD,
        pub field_u16: u16,
        pub field_u32: u32,
        pub field_u64: u64,
    }
}

#[test]
fn test_to_and_from_le_macro_custom_expansion() {
    assert_eq!(size_of::<CustomZipTestBlock>(), 2 + 4 + 8);
    assert_eq!(CustomZipTestBlock::MAGIC, 0xAABBCCDD);

    let item = CustomZipTestBlock::new(0x1234, 0x56789ABC, 0x0102030405060708);
    let mut buffer = [0u8; 14];
    assert_eq!(item.write(&mut buffer), Some(14));

    let (restored, consumed) = CustomZipTestBlock::parse(&buffer).expect("parse failed");
    assert_eq!(consumed, 14);
    assert_eq!(restored, item);
}
