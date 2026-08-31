// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration tests for PKWARE Data Descriptor 16B/24B streaming pipeline,
//! Bit 3 injection, unseekable stream writer, and ZipCrypto check byte linkage.

use std::fs;
use std::io::Cursor;
use tempfile::tempdir;
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    TTZIP_ABI_VERSION_2,
};
use ttzip_engine::zip::reader::ZipArchive;
use ttzip_engine::zip::writer::{
    compute_zipcrypto_check_byte, create_zip_streaming_parallel, has_data_descriptor,
    inject_data_descriptor_flag, parse_data_descriptor, write_data_descriptor,
    ZipDataDescriptor32, ZipDataDescriptor64, FLAG_DATA_DESCRIPTOR, MAGIC_DATA_DESCRIPTOR,
};

#[test]
fn test_data_descriptor_32_and_64_serialization_roundtrip() {
    let crc = 0x87654321;
    let comp32 = 1024u32;
    let uncomp32 = 4096u32;

    // 1. Standard 16-byte Data Descriptor
    let desc32 = ZipDataDescriptor32::new(crc, comp32, uncomp32);
    let bytes32 = desc32.to_bytes();
    assert_eq!(bytes32.len(), 16);
    assert_eq!(&bytes32[0..4], &MAGIC_DATA_DESCRIPTOR.to_le_bytes());
    assert_eq!(&bytes32[4..8], &crc.to_le_bytes());
    assert_eq!(&bytes32[8..12], &comp32.to_le_bytes());
    assert_eq!(&bytes32[12..16], &uncomp32.to_le_bytes());

    let parsed32 = ZipDataDescriptor32::from_bytes(&bytes32).expect("from_bytes failed");
    assert_eq!(parsed32, desc32);

    let mut cursor32 = Cursor::new(bytes32);
    let read32 = ZipDataDescriptor32::read(&mut cursor32).expect("read failed");
    assert_eq!(read32, desc32);

    // 2. Zip64 24-byte Data Descriptor
    let comp64 = 0x1_0000_2000u64;
    let uncomp64 = 0x2_0000_4000u64;
    let desc64 = ZipDataDescriptor64::new(crc, comp64, uncomp64);
    let bytes64 = desc64.to_bytes();
    assert_eq!(bytes64.len(), 24);
    assert_eq!(&bytes64[0..4], &MAGIC_DATA_DESCRIPTOR.to_le_bytes());
    assert_eq!(&bytes64[4..8], &crc.to_le_bytes());
    assert_eq!(&bytes64[8..16], &comp64.to_le_bytes());
    assert_eq!(&bytes64[16..24], &uncomp64.to_le_bytes());

    let parsed64 = ZipDataDescriptor64::from_bytes(&bytes64).expect("from_bytes failed");
    assert_eq!(parsed64, desc64);

    let mut cursor64 = Cursor::new(bytes64);
    let read64 = ZipDataDescriptor64::read(&mut cursor64).expect("read failed");
    assert_eq!(read64, desc64);
}

#[test]
fn test_write_and_parse_data_descriptor_stream() {
    // 32-bit standard write and parse
    let mut sink = Vec::new();
    let written32 = write_data_descriptor(&mut sink, 0x11223344, 512, 1024, false).unwrap();
    assert_eq!(written32, 16);
    assert_eq!(sink.len(), 16);

    let (parsed_crc, parsed_comp, parsed_uncomp) =
        parse_data_descriptor(&mut Cursor::new(&sink), false).unwrap();
    assert_eq!(parsed_crc, 0x11223344);
    assert_eq!(parsed_comp, 512);
    assert_eq!(parsed_uncomp, 1024);

    // 64-bit Zip64 write and parse
    let mut sink64 = Vec::new();
    let written64 = write_data_descriptor(
        &mut sink64,
        0x55667788,
        0x1_5000_0000,
        0x3_0000_0000,
        true,
    )
    .unwrap();
    assert_eq!(written64, 24);
    assert_eq!(sink64.len(), 24);

    let (parsed_crc64, parsed_comp64, parsed_uncomp64) =
        parse_data_descriptor(&mut Cursor::new(&sink64), true).unwrap();
    assert_eq!(parsed_crc64, 0x55667788);
    assert_eq!(parsed_comp64, 0x1_5000_0000);
    assert_eq!(parsed_uncomp64, 0x3_0000_0000);

    // Legacy uncompressed descriptor stream without magic
    let mut raw_legacy = Vec::new();
    raw_legacy.extend_from_slice(&0xAABBCCDDu32.to_le_bytes()); // CRC32
    raw_legacy.extend_from_slice(&256u32.to_le_bytes()); // comp
    raw_legacy.extend_from_slice(&512u32.to_le_bytes()); // uncomp
    let (legacy_crc, legacy_comp, legacy_uncomp) =
        parse_data_descriptor(&mut Cursor::new(&raw_legacy), false).unwrap();
    assert_eq!(legacy_crc, 0xAABBCCDD);
    assert_eq!(legacy_comp, 256);
    assert_eq!(legacy_uncomp, 512);
}

#[test]
fn test_unseekable_pipe_stream_simulation_with_zero_lfh_and_descriptor() {
    // Simulate writing to a non-seekable stdout or network pipe stream:
    // 1. Write Local File Header with CRC32=0, CompSize=0, UncompSize=0 and Bit 3 set
    // 2. Stream uncompressed/compressed payload
    // 3. Write trailing 16-byte Data Descriptor
    // 4. Write Central Directory and EOCD

    let filename = "pipe_streamed_file.txt";
    let payload = b"Simulated unseekable pipe stream content for Data Descriptor validation.";
    let crc = crc32_fast(0, payload);
    let comp_size = payload.len() as u64;
    let uncomp_size = payload.len() as u64;

    let mut pipe = Vec::new();
    let lfh_offset = pipe.len() as u64;

    // LFH with Bit 3 flag (0x0808) and 0 for CRC/sizes
    pipe.extend_from_slice(&0x04034B50u32.to_le_bytes()); // MAGIC_LFH
    pipe.extend_from_slice(&20u16.to_le_bytes()); // version needed
    pipe.extend_from_slice(&0x0808u16.to_le_bytes()); // flag (Bit 3 + Bit 11)
    pipe.extend_from_slice(&0u16.to_le_bytes()); // Store method
    pipe.extend_from_slice(&0u16.to_le_bytes()); // dos time
    pipe.extend_from_slice(&0x5C21u16.to_le_bytes()); // dos date
    pipe.extend_from_slice(&0u32.to_le_bytes()); // CRC = 0
    pipe.extend_from_slice(&0u32.to_le_bytes()); // Comp size = 0
    pipe.extend_from_slice(&0u32.to_le_bytes()); // Uncomp size = 0
    pipe.extend_from_slice(&(filename.len() as u16).to_le_bytes());
    pipe.extend_from_slice(&0u16.to_le_bytes()); // extra len = 0
    pipe.extend_from_slice(filename.as_bytes());

    // Stream payload
    pipe.extend_from_slice(payload);

    // Append 16-byte trailing Data Descriptor
    let desc_len = write_data_descriptor(&mut pipe, crc, comp_size, uncomp_size, false).unwrap();
    assert_eq!(desc_len, 16);

    // Central Directory
    let cd_offset = pipe.len() as u64;
    pipe.extend_from_slice(&0x02014B50u32.to_le_bytes()); // MAGIC_CDFH
    pipe.extend_from_slice(&0x031Eu16.to_le_bytes()); // version made by
    pipe.extend_from_slice(&20u16.to_le_bytes()); // version needed
    pipe.extend_from_slice(&0x0808u16.to_le_bytes()); // flag
    pipe.extend_from_slice(&0u16.to_le_bytes()); // Store method
    pipe.extend_from_slice(&0u16.to_le_bytes()); // dos time
    pipe.extend_from_slice(&0x5C21u16.to_le_bytes()); // dos date
    pipe.extend_from_slice(&crc.to_le_bytes()); // real CRC in CDFH
    pipe.extend_from_slice(&(comp_size as u32).to_le_bytes()); // real comp size in CDFH
    pipe.extend_from_slice(&(uncomp_size as u32).to_le_bytes()); // real uncomp size in CDFH
    pipe.extend_from_slice(&(filename.len() as u16).to_le_bytes());
    pipe.extend_from_slice(&0u16.to_le_bytes()); // extra len
    pipe.extend_from_slice(&0u16.to_le_bytes()); // comment len
    pipe.extend_from_slice(&0u16.to_le_bytes()); // disk num start
    pipe.extend_from_slice(&0u16.to_le_bytes()); // internal attr
    pipe.extend_from_slice(&(0o100644u32 << 16).to_le_bytes()); // external attr
    pipe.extend_from_slice(&(lfh_offset as u32).to_le_bytes());
    pipe.extend_from_slice(filename.as_bytes());

    let cd_size = (pipe.len() as u64) - cd_offset;

    // EOCD
    pipe.extend_from_slice(&0x06054B50u32.to_le_bytes()); // MAGIC_EOCD
    pipe.extend_from_slice(&0u16.to_le_bytes());
    pipe.extend_from_slice(&0u16.to_le_bytes());
    pipe.extend_from_slice(&1u16.to_le_bytes());
    pipe.extend_from_slice(&1u16.to_le_bytes());
    pipe.extend_from_slice(&(cd_size as u32).to_le_bytes());
    pipe.extend_from_slice(&(cd_offset as u32).to_le_bytes());
    pipe.extend_from_slice(&0u16.to_le_bytes());

    // Validate archive parsing and extraction
    let archive = ZipArchive::open_slice(&pipe).expect("open pipe archive slice failed");
    assert_eq!(archive.len(), 1);
    assert_eq!(archive.entries()[0].rel_path, filename);
    assert_eq!(archive.entries()[0].crc32, crc);
    assert_eq!(archive.entries()[0].uncompressed_size, uncomp_size);
    assert!(has_data_descriptor(archive.entries()[0].flag));

    let extracted = archive.extract_entry_bytes(0, None).expect("extract entry bytes failed");
    assert_eq!(extracted, payload);
}

#[test]
fn test_streaming_archive_creation_with_data_descriptors_roundtrip() {
    let tmp = tempdir().unwrap();
    let src_dir = tmp.path().join("streaming_src");
    let out_zip = tmp.path().join("streaming_out.zip");

    fs::create_dir_all(src_dir.join("nested")).unwrap();
    fs::write(
        src_dir.join("file_small.txt"),
        b"Small payload with Bit 3 Data Descriptor streaming.",
    )
    .unwrap();
    fs::write(
        src_dir.join("nested/file_large.bin"),
        vec![0x77u8; 65536],
    )
    .unwrap();

    let options = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: TTZIP_ABI_VERSION_2,
        format: TTZipArchiveFormat::Zip,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 4,
        solid_block_size_mb: 0,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };

    let report = create_zip_streaming_parallel(&out_zip, std::slice::from_ref(&src_dir), &options)
        .expect("create streaming zip failed");
    assert_eq!(report.total_entries, 4);

    let zip_bytes = fs::read(&out_zip).expect("read zip bytes failed");
    let archive = ZipArchive::open_slice(&zip_bytes).expect("open archive slice failed");
    assert_eq!(archive.len(), 4);

    // Verify each file has Bit 3 set and accurately decompresses
    for (i, entry) in archive.entries().iter().enumerate() {
        if !entry.is_directory {
            assert!(
                has_data_descriptor(entry.flag),
                "Entry {} must have Bit 3 Data Descriptor set",
                entry.rel_path
            );
            let bytes = archive.extract_entry_bytes(i, None).expect("extract failed");
            assert_eq!(bytes.len() as u64, entry.uncompressed_size);
            assert_eq!(crc32_fast(0, &bytes), entry.crc32);
        }
    }
}

#[test]
fn test_zipcrypto_streaming_bit3_check_byte_linkage() {
    let crc = 0xFEEDBEEF;
    let dos_time = 0x4A21;

    // In standard mode (Bit 3 = 0): verify byte is CRC32 high byte (0xFE)
    let check_standard = compute_zipcrypto_check_byte(crc, dos_time, false);
    assert_eq!(check_standard, 0xFE);

    // In streaming mode (Bit 3 = 1): verify byte is DOS time high byte (0x4A)
    let check_streaming = compute_zipcrypto_check_byte(crc, dos_time, true);
    assert_eq!(check_streaming, 0x4A);

    // Flag injection verification
    let base = 0x0001; // encrypted
    let injected = inject_data_descriptor_flag(base);
    assert_eq!(injected, 0x0809);
    assert!(has_data_descriptor(injected));
    assert_eq!(injected & FLAG_DATA_DESCRIPTOR, FLAG_DATA_DESCRIPTOR);
}
