// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and integration test suite for APFS/Linux native physical hole punching,
//! SEEK_DATA/SEEK_HOLE hardware extent discovery, and GNU Sparse 0.1/1.0 TAR generation and restoration.

use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use tempfile::tempdir;

use ttzip_engine::archive::tar::pax::parse_pax_data;
use ttzip_engine::fs::sparse::is_zero_block;
use ttzip_engine::tar::sparse::{
    detect_file_sparse_extents, extract_sparse_file_with_hole_punching,
    parse_gnu_sparse_1_0_stream, parse_pax_sparse_0_1, punch_file_hole, write_sparse_file_to_tar,
    SparseExtent, SparseMap, TarSparseFormat,
};

/// Creates a synthetic 100MB sparse file on disk with 1MB data at beginning, 98MB hole, 1MB data at end.
fn create_synthetic_100mb_sparse_file(path: &Path) -> (Vec<u8>, Vec<u8>) {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .expect("Failed to create synthetic sparse file");

    let head_pattern = vec![0x5Au8; 1024 * 1024]; // 1MB pattern 0x5A
    let tail_pattern = vec![0xA5u8; 1024 * 1024]; // 1MB pattern 0xA5

    // 1. Write 1MB head data
    file.write_all(&head_pattern).expect("write head");

    // 2. Seek across 98MB hole to 99MB offset
    file.seek(SeekFrom::Start(99 * 1024 * 1024))
        .expect("seek hole");

    // 3. Write 1MB tail data (finalizing 100MB logical file)
    file.write_all(&tail_pattern).expect("write tail");
    file.flush().expect("flush");

    let logical_len = file.metadata().expect("meta").len();
    assert_eq!(logical_len, 100 * 1024 * 1024);

    (head_pattern, tail_pattern)
}

#[test]
fn test_sparse_extent_detection_synthetic_100mb() {
    let tmp = tempdir().expect("tempdir");
    let sparse_path = tmp.path().join("detect_100mb.bin");
    create_synthetic_100mb_sparse_file(&sparse_path);

    let file = File::open(&sparse_path).expect("open");
    let extents = detect_file_sparse_extents(&file, 100 * 1024 * 1024).expect("detect extents");

    assert_eq!(extents.len(), 2);
    assert_eq!(extents[0].offset, 0);
    assert_eq!(extents[0].numbytes, 1024 * 1024);

    assert_eq!(extents[1].offset, 99 * 1024 * 1024);
    assert_eq!(extents[1].numbytes, 1024 * 1024);
}

#[test]
fn test_sparse_map_serialization_and_parsing_roundtrip() {
    let extents = vec![
        SparseExtent::new(0, 1048576),
        SparseExtent::new(103809024, 1048576),
    ];
    let original_map = SparseMap::new(104857600, extents);

    // 1. Test GNU Sparse 0.1 Map String Roundtrip
    let gnu_0_1_str = original_map.to_gnu_0_1_map_string();
    assert_eq!(gnu_0_1_str, "0,1048576,103809024,1048576");

    let parsed_0_1 = parse_pax_sparse_0_1(&gnu_0_1_str, 104857600).expect("parse gnu 0.1 map");
    assert_eq!(parsed_0_1, original_map);

    // 2. Test GNU Sparse 1.0 Map Block Roundtrip
    let gnu_1_0_block = original_map.to_gnu_1_0_map_block();
    assert_eq!(gnu_1_0_block.len() % 512, 0);

    let mut cursor = Cursor::new(&gnu_1_0_block);
    let (parsed_1_0, consumed_bytes) =
        parse_gnu_sparse_1_0_stream(&mut cursor, 104857600).expect("parse gnu 1.0 map");
    assert_eq!(parsed_1_0, original_map);
    assert_eq!(consumed_bytes, 512);
}

#[test]
fn test_write_and_extract_gnu_sparse_0_1_archive() {
    let tmp = tempdir().expect("tempdir");
    let src_path = tmp.path().join("source_100mb.bin");
    let (head_pat, tail_pat) = create_synthetic_100mb_sparse_file(&src_path);

    let mut tar_stream = Vec::new();
    let mut file = File::open(&src_path).expect("open src");

    // 1. Archive sparse file using GNU Sparse 0.1 format
    let written_bytes = write_sparse_file_to_tar(
        &mut tar_stream,
        &mut file,
        "payload/sparse_100mb.bin",
        TarSparseFormat::Gnu0_1,
    )
    .expect("write sparse tar");

    assert_eq!(written_bytes, tar_stream.len() as u64);

    // Hard verification: Archival size of 100MB sparse file must be ~2MB (not 100MB!)
    // 2MB data + PAX headers + 512-byte blocks < 2.1 MB
    assert!(
        tar_stream.len() < 2200000,
        "TAR size {} exceeds 2.2MB threshold for 100MB sparse file!",
        tar_stream.len()
    );

    // 2. Inspect PAX extended header structure in archive stream
    let pax_header_bytes = &tar_stream[0..512];
    assert!(pax_header_bytes.starts_with(b"PaxHeaders.0/"));

    let pax_payload = &tar_stream[512..1024];
    let pax_attrs = parse_pax_data(pax_payload);
    let map_val = pax_attrs
        .raw_map
        .get("GNU.sparse.map")
        .expect("GNU.sparse.map in PAX");
    assert_eq!(map_val, "0,1048576,103809024,1048576");

    let realsize_val = pax_attrs
        .raw_map
        .get("GNU.sparse.realsize")
        .expect("GNU.sparse.realsize in PAX");
    assert_eq!(realsize_val, "104857600");

    // 3. Extract sparse file with dual-mode hole preservation
    let sparse_map = parse_pax_sparse_0_1(map_val, 104857600).expect("parse map");
    let dest_path = tmp.path().join("extracted_0_1.bin");

    // Skip PAX header (512B) + PAX payload (512B) + main TAR header (512B) = 1536B
    let mut payload_cursor = Cursor::new(&tar_stream[1536..]);
    let extracted_size =
        extract_sparse_file_with_hole_punching(&mut payload_cursor, &dest_path, &sparse_map)
            .expect("extract sparse file");

    assert_eq!(extracted_size, 100 * 1024 * 1024);

    // 4. Verify physical disk allocation on APFS / Linux
    let meta = fs::metadata(&dest_path).expect("dest meta");
    assert_eq!(meta.len(), 100 * 1024 * 1024);

    #[cfg(target_os = "macos")]
    {
        let physical_bytes = meta.blocks() * 512;
        // On APFS, 2MB data + block metadata should consume < 5MB physical disk space
        assert!(
            physical_bytes < 5 * 1024 * 1024,
            "Physical disk allocation {} should be < 5MB on APFS (logical size is 100MB)",
            physical_bytes
        );
    }

    // 5. Verify 100% Bit-Exact data content integrity across holes and extents
    let mut restored_file = File::open(&dest_path).expect("open restored");

    // Read 1MB head data
    let mut head_buf = vec![0u8; 1024 * 1024];
    restored_file
        .read_exact(&mut head_buf)
        .expect("read head buf");
    assert_eq!(&head_buf[..], &head_pat[..]);

    // Read 1MB sample from middle hole (e.g. 50MB offset) -> MUST BE ALL ZEROS
    restored_file
        .seek(SeekFrom::Start(50 * 1024 * 1024))
        .expect("seek hole");
    let mut hole_buf = vec![0u8; 1024 * 1024];
    restored_file
        .read_exact(&mut hole_buf)
        .expect("read hole buf");
    assert!(is_zero_block(&hole_buf));

    // Read 1MB tail data (99MB offset)
    restored_file
        .seek(SeekFrom::Start(99 * 1024 * 1024))
        .expect("seek tail");
    let mut tail_buf = vec![0u8; 1024 * 1024];
    restored_file
        .read_exact(&mut tail_buf)
        .expect("read tail buf");
    assert_eq!(&tail_buf[..], &tail_pat[..]);
}

#[test]
fn test_write_and_extract_gnu_sparse_1_0_archive() {
    let tmp = tempdir().expect("tempdir");
    let src_path = tmp.path().join("source_1_0_100mb.bin");
    let (head_pat, tail_pat) = create_synthetic_100mb_sparse_file(&src_path);

    let mut tar_stream = Vec::new();
    let mut file = File::open(&src_path).expect("open src");

    // 1. Archive sparse file using GNU Sparse 1.0 format
    let written_bytes = write_sparse_file_to_tar(
        &mut tar_stream,
        &mut file,
        "payload/sparse_1_0_100mb.bin",
        TarSparseFormat::Gnu1_0,
    )
    .expect("write sparse tar 1.0");

    assert_eq!(written_bytes, tar_stream.len() as u64);

    // Archival size of 100MB sparse file must be ~2MB
    assert!(
        tar_stream.len() < 2200000,
        "TAR size {} exceeds 2.2MB threshold for 100MB sparse file in 1.0 format!",
        tar_stream.len()
    );

    // 2. Inspect GNU 1.0 headers and payload structure
    // Offset 0: PAX Header (512B)
    // Offset 512: PAX Payload (512B)
    // Offset 1024: Main TAR Header (512B)
    // Offset 1536: GNU 1.0 Map Block (512B)
    // Offset 2048: Non-zero data stream (2MB)
    let map_slice = &tar_stream[1536..];
    let mut map_cursor = Cursor::new(map_slice);
    let (sparse_map, map_block_len) =
        parse_gnu_sparse_1_0_stream(&mut map_cursor, 100 * 1024 * 1024).expect("parse 1.0 map");

    assert_eq!(map_block_len, 512);
    assert_eq!(sparse_map.extents.len(), 2);
    assert_eq!(sparse_map.extents[0].offset, 0);
    assert_eq!(sparse_map.extents[0].numbytes, 1024 * 1024);
    assert_eq!(sparse_map.extents[1].offset, 99 * 1024 * 1024);
    assert_eq!(sparse_map.extents[1].numbytes, 1024 * 1024);

    // 3. Extract sparse file with dual-mode hole preservation
    let dest_path = tmp.path().join("extracted_1_0.bin");
    let mut payload_cursor = Cursor::new(&tar_stream[2048..]);
    let extracted_size =
        extract_sparse_file_with_hole_punching(&mut payload_cursor, &dest_path, &sparse_map)
            .expect("extract sparse file 1.0");

    assert_eq!(extracted_size, 100 * 1024 * 1024);

    // 4. Verify physical disk allocation and bit-exact data fidelity
    let meta = fs::metadata(&dest_path).expect("dest meta");
    assert_eq!(meta.len(), 100 * 1024 * 1024);

    #[cfg(target_os = "macos")]
    {
        let physical_bytes = meta.blocks() * 512;
        assert!(
            physical_bytes < 5 * 1024 * 1024,
            "Physical disk allocation {} should be < 5MB on APFS (logical size is 100MB)",
            physical_bytes
        );
    }

    let mut restored_file = File::open(&dest_path).expect("open restored 1.0");

    let mut head_buf = vec![0u8; 1024 * 1024];
    restored_file.read_exact(&mut head_buf).expect("read head");
    assert_eq!(&head_buf[..], &head_pat[..]);

    restored_file
        .seek(SeekFrom::Start(50 * 1024 * 1024))
        .expect("seek hole");
    let mut hole_buf = vec![0u8; 1024 * 1024];
    restored_file.read_exact(&mut hole_buf).expect("read hole");
    assert!(is_zero_block(&hole_buf));

    restored_file
        .seek(SeekFrom::Start(99 * 1024 * 1024))
        .expect("seek tail");
    let mut tail_buf = vec![0u8; 1024 * 1024];
    restored_file.read_exact(&mut tail_buf).expect("read tail");
    assert_eq!(&tail_buf[..], &tail_pat[..]);
}

#[test]
fn test_completely_hollow_zero_file_archival_and_extraction() {
    let tmp = tempdir().expect("tempdir");
    let hollow_path = tmp.path().join("hollow_50mb.bin");

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&hollow_path)
        .expect("create hollow");
    file.set_len(50 * 1024 * 1024).expect("set len 50MB");
    drop(file);

    let mut tar_stream = Vec::new();
    let mut file = File::open(&hollow_path).expect("open hollow");

    let written = write_sparse_file_to_tar(
        &mut tar_stream,
        &mut file,
        "hollow.bin",
        TarSparseFormat::Gnu0_1,
    )
    .expect("write hollow tar");

    // Completely hollow 50MB file has 0 data bytes, only PAX + main headers (< 4KB)
    assert!(
        written < 4096,
        "Hollow file tar size {} should be < 4KB",
        written
    );

    let map = SparseMap::new(50 * 1024 * 1024, Vec::new());
    let dest_path = tmp.path().join("restored_hollow.bin");
    let mut empty_reader = Cursor::new(&[]);
    let restored_len =
        extract_sparse_file_with_hole_punching(&mut empty_reader, &dest_path, &map)
            .expect("extract hollow");

    assert_eq!(restored_len, 50 * 1024 * 1024);
    let meta = fs::metadata(&dest_path).expect("meta");
    assert_eq!(meta.len(), 50 * 1024 * 1024);

    #[cfg(target_os = "macos")]
    {
        let physical = meta.blocks() * 512;
        // Hollow file on APFS occupies 0 physical blocks
        assert_eq!(
            physical, 0,
            "Completely hollow file should have 0 physical blocks"
        );
    }
}

#[test]
fn test_native_hole_punching_api() {
    let tmp = tempdir().expect("tempdir");
    let punch_path = tmp.path().join("punch_test.bin");

    // Create a 1MB file filled with 0xFF
    let pattern = vec![0xFFu8; 1024 * 1024];
    fs::write(&punch_path, &pattern).expect("write punch file");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&punch_path)
        .expect("open punch file");

    // Punch a 512KB hole in the middle (256KB .. 768KB)
    let hole_offset = 256 * 1024;
    let hole_len = 512 * 1024;
    let res = punch_file_hole(&file, hole_offset, hole_len);
    assert!(res.is_ok());

    #[cfg(target_os = "macos")]
    {
        // On APFS, verify that reading punched range returns zeroes
        let mut read_file = File::open(&punch_path).expect("open read");
        read_file
            .seek(SeekFrom::Start(hole_offset))
            .expect("seek punched");
        let mut hole_buf = vec![0u8; hole_len as usize];
        read_file.read_exact(&mut hole_buf).expect("read punched");
        assert!(is_zero_block(&hole_buf));
    }
}
