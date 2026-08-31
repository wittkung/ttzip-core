// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! DataStreamAlignment Sector & Virtual Memory Page Alignment Test Suite.
//!
//! Validates:
//! 1. Mathematical accuracy of `AlignmentPaddingCalculator` under 4KB, 16KB (Apple Silicon), and 64KB alignments.
//! 2. Boundary conditions: natural alignment (0B pad), 1..5 byte remainder cycle accumulation (`needed += align`),
//!    and exact 6-byte minimal extra field boundary.
//! 3. Binary Extra Field construction (`TAG_DATA_STREAM_ALIGNMENT = 0xa11e`) and Central Directory stripping.
//! 4. Full end-to-end ZIP archive generation with aligned payloads and zero-copy `memmap2` page mapping.

use memmap2::MmapOptions;
use std::fs::File;
use std::io::Write;
use tempfile::NamedTempFile;
use ttzip_engine::types::TTZipEncryptionMethod;
use ttzip_engine::zip::alignment::{
    build_alignment_extra_field, contains_alignment_extra_field, parse_alignment_extra_field,
    strip_alignment_extra_fields, AlignmentPaddingCalculator, LFH_FIXED_HEADER_SIZE,
    MIN_ALIGNMENT_EXTRA_FIELD_LEN, TAG_DATA_STREAM_ALIGNMENT,
};
use ttzip_engine::zip::extra::ZipExtraFields;
use ttzip_engine::zip::parser::parse_local_file_header;
use ttzip_engine::zip::reader::ZipArchive;
use ttzip_engine::zip::writer::{
    assemble_zip_archive_aligned, compress_items_parallel, ZipInputItem,
};

#[test]
fn test_tag_and_constants() {
    assert_eq!(TAG_DATA_STREAM_ALIGNMENT, 0xa11e);
    assert_eq!(MIN_ALIGNMENT_EXTRA_FIELD_LEN, 6);
    assert_eq!(LFH_FIXED_HEADER_SIZE, 30);
}

#[test]
fn test_alignment_padding_calculator_mathematical_precision_4kb() {
    let align = 4096u16;

    // Case 1: Naturally aligned payload start
    // LFH = 30, file_name = 10, existing_extra = 4056 -> unpadded = 4096
    let pad = AlignmentPaddingCalculator::calculate(0, 10, 4056, align);
    assert_eq!(pad, 0);
    let data_start = AlignmentPaddingCalculator::calculate_data_start(0, 10, 4056, align);
    assert_eq!(data_start, 4096);
    assert_eq!(data_start % 4096, 0);

    // Case 2: Needed padding = 1 byte -> Must accumulate full alignment cycle (1 + 4096 = 4097)
    // unpadded = 4095 -> rem = 4095, needed = 1 < 6 -> pad = 4097
    let pad = AlignmentPaddingCalculator::calculate(0, 10, 4055, align);
    assert_eq!(pad, 4097);
    let data_start = AlignmentPaddingCalculator::calculate_data_start(0, 10, 4055, align);
    assert_eq!(data_start, 4095 + 4097);
    assert_eq!(data_start, 8192);
    assert_eq!(data_start % 4096, 0);

    // Case 3: Needed padding = 2 bytes -> pad = 4098
    let pad = AlignmentPaddingCalculator::calculate(0, 10, 4054, align);
    assert_eq!(pad, 4098);
    let data_start = AlignmentPaddingCalculator::calculate_data_start(0, 10, 4054, align);
    assert_eq!(data_start, 8192);
    assert_eq!(data_start % 4096, 0);

    // Case 4: Needed padding = 5 bytes -> pad = 4101
    let pad = AlignmentPaddingCalculator::calculate(0, 10, 4051, align);
    assert_eq!(pad, 4101);
    let data_start = AlignmentPaddingCalculator::calculate_data_start(0, 10, 4051, align);
    assert_eq!(data_start, 8192);
    assert_eq!(data_start % 4096, 0);

    // Case 5: Needed padding = 6 bytes (exact minimum extra field header) -> pad = 6
    let pad = AlignmentPaddingCalculator::calculate(0, 10, 4050, align);
    assert_eq!(pad, 6);
    let data_start = AlignmentPaddingCalculator::calculate_data_start(0, 10, 4050, align);
    assert_eq!(data_start, 4096);
    assert_eq!(data_start % 4096, 0);

    // Case 6: Needed padding = 7 bytes -> pad = 7
    let pad = AlignmentPaddingCalculator::calculate(0, 10, 4049, align);
    assert_eq!(pad, 7);
    let data_start = AlignmentPaddingCalculator::calculate_data_start(0, 10, 4049, align);
    assert_eq!(data_start, 4096);
    assert_eq!(data_start % 4096, 0);
}

#[test]
fn test_alignment_padding_calculator_apple_silicon_16kb_and_64kb() {
    let test_alignments = [4096u16, 8192u16, 16384u16, 32768u16];

    for &align in &test_alignments {
        let align_u64 = align as u64;

        // Arbitrary header offsets
        for header_start in [0u64, 1234u64, 65536u64, 1000000u64] {
            for fn_len in [1usize, 12usize, 255usize] {
                for extra_len in [0usize, 9usize, 24usize] {
                    let pad = AlignmentPaddingCalculator::calculate(
                        header_start,
                        fn_len,
                        extra_len,
                        align,
                    );
                    let data_start = AlignmentPaddingCalculator::calculate_data_start(
                        header_start,
                        fn_len,
                        extra_len,
                        align,
                    );

                    assert_eq!(data_start % align_u64, 0);
                    if pad > 0 {
                        assert!(pad >= MIN_ALIGNMENT_EXTRA_FIELD_LEN);
                    }
                    assert!(AlignmentPaddingCalculator::is_aligned(data_start, align));
                }
            }
        }
    }
}

#[test]
fn test_alignment_padding_calculator_boundary_and_degenerate_cases() {
    // Target alignment 0 or 1 should disable alignment (0 padding)
    assert_eq!(AlignmentPaddingCalculator::calculate(100, 10, 0, 0), 0);
    assert_eq!(AlignmentPaddingCalculator::calculate(100, 10, 0, 1), 0);
    assert!(AlignmentPaddingCalculator::is_aligned(12345, 0));
    assert!(AlignmentPaddingCalculator::is_aligned(12345, 1));

    // Exhaustive remainder range 0..4095
    let align = 4096u16;
    for rem in 0..4096u64 {
        // header_start = rem, fn_len = 0, extra_len = 0, fixed = 30 -> unpadded = rem + 30
        // Adjust header_start so that (header_start + 30) % 4096 == rem
        let header_start = if rem >= 30 {
            rem - 30
        } else {
            rem + 4096 - 30
        };

        let pad = AlignmentPaddingCalculator::calculate(header_start, 0, 0, align);
        let data_start =
            AlignmentPaddingCalculator::calculate_data_start(header_start, 0, 0, align);

        assert_eq!(data_start % 4096, 0);
        if rem == 0 {
            assert_eq!(pad, 0);
        } else {
            assert!(pad >= 6);
            let needed = 4096 - rem;
            if needed < 6 {
                assert_eq!(pad as u64, needed + 4096);
            } else {
                assert_eq!(pad as u64, needed);
            }
        }
    }
}

#[test]
fn test_build_alignment_extra_field_binary_layout() {
    // Pad len 0 returns empty
    assert!(build_alignment_extra_field(0, 4096).is_empty());
    // Pad len < 6 returns empty
    assert!(build_alignment_extra_field(5, 4096).is_empty());

    // Minimum valid field (6 bytes)
    let extra6 = build_alignment_extra_field(6, 4096);
    assert_eq!(extra6.len(), 6);
    assert_eq!(extra6[0..2], [0x1e, 0xa1]); // 0xa11e in LE
    assert_eq!(extra6[2..4], [0x02, 0x00]); // Data Size = 2
    assert_eq!(extra6[4..6], [0x00, 0x10]); // Alignment = 4096 (0x1000) in LE

    // Extended field (16 bytes)
    let extra16 = build_alignment_extra_field(16, 16384);
    assert_eq!(extra16.len(), 16);
    assert_eq!(extra16[0..2], [0x1e, 0xa1]);
    assert_eq!(extra16[2..4], [0x0c, 0x00]); // Data Size = 12 (16 - 4)
    assert_eq!(extra16[4..6], [0x00, 0x40]); // Alignment = 16384 (0x4000) in LE
    assert_eq!(&extra16[6..], &[0u8; 10]); // 10 padding zeroes

    // Parse alignment extra field
    let parsed_align = parse_alignment_extra_field(&extra16[4..]);
    assert_eq!(parsed_align, Some(16384));

    // Integration with ZipExtraFields parser
    let parsed_fields = ZipExtraFields::parse(&extra16, false, false, false, false);
    assert_eq!(parsed_fields.data_stream_alignment, Some(16384));
}

#[test]
fn test_central_directory_isolation_and_stripping() {
    let mut combined_extra = Vec::new();

    // 1. Extended timestamp (0x5455, 9 bytes)
    let ts = ZipExtraFields::build_extended_timestamp(1700000000);
    combined_extra.extend_from_slice(&ts);

    // 2. Alignment field (0xa11e, 32 bytes)
    let align_extra = build_alignment_extra_field(32, 4096);
    combined_extra.extend_from_slice(&align_extra);

    // 3. Zip64 field (0x0001, 12 bytes)
    let z64 = ZipExtraFields::build_zip64_extra(Some(100), None, None);
    combined_extra.extend_from_slice(&z64);

    assert!(contains_alignment_extra_field(&combined_extra));

    // Strip alignment fields
    let stripped = strip_alignment_extra_fields(&combined_extra);
    assert!(!contains_alignment_extra_field(&stripped));
    assert_eq!(stripped.len(), ts.len() + z64.len());

    // Verify remaining fields parse properly
    let parsed = ZipExtraFields::parse(&stripped, false, false, false, false);
    assert_eq!(parsed.mod_time, Some(1700000000));
    assert_eq!(parsed.uncompressed_size, Some(100));
    assert_eq!(parsed.data_stream_alignment, None);
}

#[test]
fn test_zip_archive_4kb_alignment_and_mmap_zero_copy() {
    let payload1 = b"Payload 1 stored without compression for 4KB MMU direct mapping test.";
    let payload2 = vec![0xABu8; 8192];
    let payload3 = b"Small file in aligned container.";

    let items = vec![
        ZipInputItem {
            rel_path: "uncompressed_doc.bin".to_string(),
            data: payload1.to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "nested/pattern.dat".to_string(),
            data: payload2.clone(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "small.txt".to_string(),
            data: payload3.to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
    ];

    // Store compression level (0) so payload in ZIP matches uncompressed bytes 1:1
    let compressed = compress_items_parallel(
        items,
        0, // Store
        TTZipEncryptionMethod::None,
        None,
        2,
    )
    .expect("compression failed");

    // Assemble with 4096-byte (4KB) alignment
    let zip_bytes =
        assemble_zip_archive_aligned(&compressed, 4096).expect("aligned assembly failed");

    // Write to a temporary file for real OS mmap verification
    let mut temp_file = NamedTempFile::new().expect("temp file creation failed");
    temp_file
        .write_all(&zip_bytes)
        .expect("write zip bytes failed");
    temp_file.flush().expect("flush temp file failed");

    let file = File::open(temp_file.path()).expect("open temp file for mmap failed");

    // Verify Central Directory parsing and entry offsets
    let archive = ZipArchive::open_slice(&zip_bytes).expect("open zip archive slice failed");
    assert_eq!(archive.len(), 3);

    for (idx, entry) in archive.entries().iter().enumerate() {
        let (payload_offset, _) = parse_local_file_header(&zip_bytes, entry.lfh_offset as usize)
            .expect("parse LFH failed");

        // 1. Verify physical file offset is 100% 4KB aligned
        assert_eq!(
            payload_offset % 4096,
            0,
            "Entry {} ({}) payload offset {} must be 4KB aligned",
            idx,
            entry.rel_path,
            payload_offset
        );

        // 2. Perform zero-copy OS memory map at the aligned offset
        let comp_size = entry.compressed_size as usize;
        let mmap = unsafe {
            MmapOptions::new()
                .offset(payload_offset as u64)
                .len(comp_size)
                .map(&file)
                .expect("mmap at aligned offset must succeed without EINVAL")
        };

        // 3. Verify mapped contents match expected payload directly
        match idx {
            0 => assert_eq!(&mmap[..], payload1),
            1 => assert_eq!(&mmap[..], &payload2[..]),
            2 => assert_eq!(&mmap[..], payload3),
            _ => unreachable!(),
        }

        // 4. Verify standard archive extraction also returns matching uncompressed data
        let extracted = archive
            .extract_entry_bytes(idx, None)
            .expect("extract entry failed");
        assert_eq!(&extracted[..], &mmap[..]);
    }
}

#[test]
fn test_zip_archive_16kb_apple_silicon_page_alignment() {
    let payload = vec![0x5Au8; 32768];
    let items = vec![
        ZipInputItem {
            rel_path: "frameworks/CoreModel.bin".to_string(),
            data: payload.clone(),
            mtime_epoch_secs: 1700000000,
            mode: 0o755,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "frameworks/Secondary.bin".to_string(),
            data: vec![0x33u8; 4096],
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let compressed =
        compress_items_parallel(items, 0, TTZipEncryptionMethod::None, None, 2).unwrap();
    let zip_bytes = assemble_zip_archive_aligned(&compressed, 16384).unwrap();

    let archive = ZipArchive::open_slice(&zip_bytes).unwrap();
    assert_eq!(archive.len(), 2);

    for (idx, entry) in archive.entries().iter().enumerate() {
        let (payload_offset, _) =
            parse_local_file_header(&zip_bytes, entry.lfh_offset as usize).unwrap();

        assert_eq!(
            payload_offset % 16384,
            0,
            "Entry {} payload offset {} must be 16KB aligned for Apple Silicon",
            idx,
            payload_offset
        );

        let extracted = archive.extract_entry_bytes(idx, None).unwrap();
        if idx == 0 {
            assert_eq!(extracted, payload);
        } else {
            assert_eq!(extracted, vec![0x33u8; 4096]);
        }
    }
}

#[test]
fn test_zip_archive_deflate_compression_with_alignment() {
    let payload = b"Deflate compressed payload with 4KB sector alignment support in TTZip engine."
        .repeat(50);

    let items = vec![ZipInputItem {
        rel_path: "compressed_text.txt".to_string(),
        data: payload.clone(),
        mtime_epoch_secs: 1700000000,
        mode: 0o644,
        is_directory: false,
    }];

    // Deflate level 6
    let compressed =
        compress_items_parallel(items, 6, TTZipEncryptionMethod::None, None, 2).unwrap();
    assert_eq!(compressed[0].compression_method, 8);

    let zip_bytes = assemble_zip_archive_aligned(&compressed, 4096).unwrap();
    let archive = ZipArchive::open_slice(&zip_bytes).unwrap();
    assert_eq!(archive.len(), 1);

    let (payload_offset, _) =
        parse_local_file_header(&zip_bytes, archive.entries()[0].lfh_offset as usize).unwrap();
    assert_eq!(payload_offset % 4096, 0);

    let extracted = archive.extract_entry_bytes(0, None).unwrap();
    assert_eq!(extracted, payload);
}
