// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration and verification tests for `ArchiveCorpusGenerator`.

use ttzip_engine::sevenz::format::SevenZSignatureHeader;
use ttzip_engine::sevenz::header::models::SevenZHeaderInfo;
use ttzip_engine::sevenz::header::stream::parse_7z_header_stream;
use ttzip_engine::testing::archive_corpus_generator::*;
use ttzip_engine::zip::parser::{find_eocd, parse_cdfh_entry};

#[test]
fn test_archive_corpus_generator_prng_determinism() {
    let mut gen1 = ArchiveCorpusGenerator::new(0x1234_5678_9ABC_DEF0);
    let mut gen2 = ArchiveCorpusGenerator::new(0x1234_5678_9ABC_DEF0);

    let config = ZipSynthesisConfig {
        num_entries: 3,
        variants: vec![
            ZipExtremeVariant::ZeroLengthFileName,
            ZipExtremeVariant::OversizedExtraField,
        ],
        extra_field_bytes: 512,
        virtual_uncompressed_size: 2048,
        compression_method: 0,
        payload_len: 128,
    };

    let a1 = gen1.generate_extreme_zip(&config);
    let a2 = gen2.generate_extreme_zip(&config);

    assert_eq!(a1.data, a2.data, "Generated archive bytes must be bit-exact deterministic");
    assert_eq!(a1.seed, a2.seed);
}

#[test]
fn test_extreme_zip_zero_length_and_oversized_extra() {
    let mut gen = ArchiveCorpusGenerator::new(0xCAFE_BABE_0000_0001);
    let config = ZipSynthesisConfig {
        num_entries: 2,
        variants: vec![
            ZipExtremeVariant::ZeroLengthFileName,
            ZipExtremeVariant::OversizedExtraField,
        ],
        extra_field_bytes: 1024,
        virtual_uncompressed_size: 4096,
        compression_method: 0,
        payload_len: 256,
    };

    let archive = gen.generate_extreme_zip(&config);
    assert_eq!(archive.kind, ArchiveContainerKind::Zip);
    assert!(!archive.data.is_empty());

    // Verify ZIP parser can find EOCD and parse CDFH entries
    let eocd = find_eocd(&archive.data).expect("EOCD must be valid");
    assert_eq!(eocd.total_entries, 2);

    let mut cursor = eocd.cd_offset as usize;
    for i in 0..eocd.total_entries {
        let (entry, next_cursor) = parse_cdfh_entry(&archive.data, cursor).expect("CDFH parse");
        cursor = next_cursor;
        if i == 0 {
            assert_eq!(entry.rel_path, "", "Entry 0 should have zero-length filename");
        } else {
            assert!(entry.rel_path.starts_with("extreme_corpus_entry_"));
        }
    }
}

#[test]
fn test_zip64_boundary_crossing_synthesis() {
    let mut gen = ArchiveCorpusGenerator::new(0x9999_8888_7777_6666);
    let virtual_4gb_plus = (1u64 << 32) + 1024;
    let archive = gen.generate_zip64_boundary_archive(virtual_4gb_plus);

    assert_eq!(archive.kind, ArchiveContainerKind::Zip);
    let eocd = find_eocd(&archive.data).expect("Zip64 EOCD must be recognized");
    assert!(eocd.is_zip64, "Archive must be parsed as Zip64");
    assert_eq!(eocd.total_entries, 1);

    let (entry, _) = parse_cdfh_entry(&archive.data, eocd.cd_offset as usize).expect("CDFH parse");
    assert_eq!(
        entry.uncompressed_size, virtual_4gb_plus,
        "Zip64 uncompressed size must cross 4GB boundary"
    );
}

#[test]
fn test_degenerate_dynamic_huffman_zip() {
    let mut gen = ArchiveCorpusGenerator::new(0x5555_AAAA_3333_CCCC);
    let archive = gen.generate_degenerate_huffman_zip(16);

    assert_eq!(archive.kind, ArchiveContainerKind::Zip);
    let eocd = find_eocd(&archive.data).expect("EOCD must be valid");
    assert_eq!(eocd.total_entries, 1);
    let (entry, _) = parse_cdfh_entry(&archive.data, eocd.cd_offset as usize).expect("CDFH parse");
    assert_eq!(entry.compression_method, 8); // Deflate
}

#[test]
fn test_sevenz_bcj2_4stream_synthesis() {
    let mut gen = ArchiveCorpusGenerator::new(0x7777_7777_7777_7777);
    let dummy_payload = vec![0x90; 256];
    let archive = gen.generate_bcj2_4stream_7z(&dummy_payload);

    assert_eq!(archive.kind, ArchiveContainerKind::SevenZ);
    assert!(archive.data.len() > 32);

    // Verify 7z Signature Header
    let sig = SevenZSignatureHeader::parse(&archive.data).expect("7z Signature Header must be valid");
    assert_eq!(sig.major_version, 0);
    assert_eq!(sig.minor_version, 4);

    let header_start = (32 + sig.next_header_offset) as usize;
    let header_end = header_start + (sig.next_header_size as usize);
    let header_slice = &archive.data[header_start..header_end];

    let mut header_info = SevenZHeaderInfo::default();
    parse_7z_header_stream(header_slice, &mut header_info).expect("7z Header must parse");

    assert_eq!(header_info.folders.len(), 1);
    let folder = &header_info.folders[0];
    assert_eq!(folder.coders.len(), 4, "BCJ2 folder must contain 4 coders");
    assert_eq!(folder.coders[0].num_in_streams, 4);
    assert_eq!(folder.coders[0].num_out_streams, 1);
}

#[test]
fn test_sevenz_empty_streams_and_extreme_repeats() {
    let mut gen = ArchiveCorpusGenerator::new(0x1122_3344_5566_7788);
    let config = SevenZSynthesisConfig {
        num_files: 4,
        variants: vec![
            SevenZExtremeVariant::EmptyStreamMarkers,
            SevenZExtremeVariant::ExtremeRepeatOffsets,
        ],
        primary_method: 0,
        payload_size: 512,
        num_unpack_streams: 1,
    };

    let archive = gen.generate_extreme_7z(&config);
    let sig = SevenZSignatureHeader::parse(&archive.data).expect("7z Sig");
    let header_start = (32 + sig.next_header_offset) as usize;
    let header_end = header_start + (sig.next_header_size as usize);

    let mut header_info = SevenZHeaderInfo::default();
    parse_7z_header_stream(&archive.data[header_start..header_end], &mut header_info)
        .expect("7z Header parse");

    assert_eq!(header_info.files.len(), 4);
    assert!(header_info.files.iter().any(|f| f.is_empty_stream), "Empty stream flags must be present");
}

#[test]
fn test_corpus_matrix_generation_diversity() {
    let mut gen = ArchiveCorpusGenerator::new(0xA1B2_C3D4_E5F6_0718);
    let matrix = gen.generate_corpus_matrix(12);

    assert_eq!(matrix.len(), 12);
    let mut zip_count = 0;
    let mut sevenz_count = 0;

    for item in &matrix {
        assert!(!item.data.is_empty());
        match item.kind {
            ArchiveContainerKind::Zip => zip_count += 1,
            ArchiveContainerKind::SevenZ => sevenz_count += 1,
        }
    }

    assert_eq!(zip_count, 6);
    assert_eq!(sevenz_count, 6);
}
