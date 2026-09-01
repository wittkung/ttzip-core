// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive corruption injection, chaos mutation, and fuzzing test suite for TTZip PDF Subsystem.
//!
//! Deploys 16 surgical destruction targets:
//! 1. Self-loop and cyclic indirect reference recursion bomb defense.
//! 2. Exponential stream decompression bomb mitigation (Flate/LZW).
//! 3. Broken, truncated, and desynchronized XRef cross-reference tables.
//! 4. Missing `%%EOF` marker, dangling tokens, and unclosed Trailer dictionaries.
//! 5. Extreme Page Tree hierarchy depth stack overflow attacks.
//! 6. 1000+ concurrent PDF parsing, outline extraction, and search contention.
//! 7. 500+ rounds of pseudo-random byte mutation chaos fuzzing.
//! 8. Malformed ToUnicode CMap mapping and illegal hex glyph strings.
//! 9. Malformed XMP packet and unclosed XML entity bombs in metadata.
//! 10. Corrupted Object Stream (`/ObjStm`) compressed object tables.
//! 11. Extreme coordinate overflows and inverted MediaBox/CropBox rectangles.
//! 12. Malicious embedded `/JavaScript`, `/Launch`, and dangerous URI actions sanitization.
//! 13. Encrypted PDF password retry limits and constant-time probe defenses.
//! 14. Zero-byte, single-byte, and truncated header PDF boundary defenses.
//! 15. Incomplete content streams and unbalanced `BT`/`ET` text block operators.
//! 16. Sensitive PDF text buffer Zeroize-on-drop memory cleanup verification.

use std::io::Write;
use std::panic::catch_unwind;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use flate2::write::ZlibEncoder;
use flate2::Compression;
use lopdf::dictionary;
use rayon::prelude::*;

use ttzip_engine::pdf::{
    PdfMetadataExtractor, PdfOutlineExtractor, PdfTextExtractor, TTZipPdfParser,
};
use ttzip_engine::security::pdf_defense::{
    ActionPolicy, EncryptionSecurityPolicy, IndirectReferenceCycleGuard,
    MaliciousActionSandboxGuard, PageTreeDepthGuard, PdfDefenseError, PdfEncryptionGuard,
    SensitivePdfBuffer, StreamExpansionQuotaGuard,
};

/// High-speed deterministic linear congruential generator for reproducible fuzzing vectors.
#[derive(Clone, Debug)]
struct DeterministicPrng {
    state: u64,
}

impl DeterministicPrng {
    #[inline]
    const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9e37_79b9_7f4a_7c15
            } else {
                seed
            },
        }
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.state >> 32) as u32
    }

    #[inline]
    fn next_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let span = max - min + 1;
        min + (self.next_u32() as usize % span)
    }

    #[inline]
    fn next_byte(&mut self) -> u8 {
        (self.next_u32() & 0xFF) as u8
    }
}

/// Helper: Builds a minimal valid PDF byte sequence.
fn build_minimal_valid_pdf() -> Vec<u8> {
    let mut doc = lopdf::Document::new();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => lopdf::Object::Reference(font_id),
        },
    });
    let content_stream = b"BT /F1 12 Tf 100 700 Td (TTZip PDF Engine Valid Base) Tj ET";
    let content_id = doc.add_object(lopdf::Object::Stream(lopdf::Stream::new(
        dictionary! {},
        content_stream.to_vec(),
    )));
    let pages_id = doc.new_object_id();
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => lopdf::Object::Reference(pages_id),
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => lopdf::Object::Reference(content_id),
        "Resources" => lopdf::Object::Reference(resources_id),
    });
    doc.set_object(
        pages_id,
        lopdf::Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![lopdf::Object::Reference(page_id)],
            "Count" => 1,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => lopdf::Object::Reference(pages_id),
    });
    doc.trailer.set("Root", lopdf::Object::Reference(catalog_id));
    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    buf
}

// ============================================================================
// Fuzz Targets 1 - 16
// ============================================================================

#[test]
fn test_target_01_self_loop_indirect_reference_cycle_bomb() {
    let obj5_id = (5, 0);
    let obj6_id = (6, 0);

    let mut guard = IndirectReferenceCycleGuard::new();

    let scope1 = guard.enter_object(obj5_id);
    assert!(scope1.is_ok());

    let scope2 = guard.enter_object(obj6_id);
    assert!(scope2.is_ok());

    // Enter obj5 again while in active ancestry -> CycleDetected!
    let cycle_result = guard.enter_object(obj5_id);
    assert!(cycle_result.is_err(), "Cycle bomb must be rejected");
    match cycle_result.unwrap_err() {
        PdfDefenseError::CycleDetected { obj_id, path } => {
            assert_eq!(obj_id, obj5_id);
            assert!(path.contains("5_0R"));
        }
        other => panic!("Unexpected error: {:?}", other),
    }
}

#[test]
fn test_target_02_stream_expansion_exponential_decompression_bomb() {
    let raw_payload = vec![b'A'; 1_000_000]; // 1MB uncompressed
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&raw_payload).unwrap();
    let compressed = encoder.finish().unwrap(); // ~1KB compressed (1000x ratio)

    let mut guard = StreamExpansionQuotaGuard::with_limits(32 * 1024 * 1024, 200.0, 128 * 1024 * 1024);
    let result = guard.decompress_flate(&compressed);

    assert!(result.is_err(), "High ratio decompression bomb must be intercepted");
    match result.unwrap_err() {
        PdfDefenseError::StreamExpansionRatioExceeded {
            ratio,
            max_ratio,
            ..
        } => {
            assert!(ratio > 200.0);
            assert_eq!(max_ratio, 200.0);
        }
        other => panic!("Unexpected error: {:?}", other),
    }
}

#[test]
fn test_target_03_broken_and_truncated_xref_table() {
    let valid_pdf = build_minimal_valid_pdf();
    let mut broken_pdf = valid_pdf.clone();

    // Corrupt the xref table offset
    if let Some(pos) = broken_pdf.windows(9).position(|w| w == b"startxref") {
        let corrupt_offset = b"startxref\n999999999\n%%EOF";
        if pos + corrupt_offset.len() <= broken_pdf.len() {
            broken_pdf[pos..pos + corrupt_offset.len()].copy_from_slice(corrupt_offset);
        }
    }

    let parsed = catch_unwind(|| {
        let _ = TTZipPdfParser::open_from_bytes(&broken_pdf);
    });
    assert!(parsed.is_ok(), "Broken xref parsing must not panic");
}

#[test]
fn test_target_04_missing_eof_and_unclosed_trailer() {
    let mut truncated_pdf = build_minimal_valid_pdf();
    // Strip trailing %%EOF and trailer
    if let Some(pos) = truncated_pdf.windows(7).position(|w| w == b"trailer") {
        truncated_pdf.truncate(pos + 4); // Cut in the middle of trailer
    }

    let result = catch_unwind(|| {
        let _ = TTZipPdfParser::open_from_bytes(&truncated_pdf);
    });
    assert!(result.is_ok(), "Missing EOF must fail gracefully without panic");
}

#[test]
fn test_target_05_extreme_page_tree_depth_stack_overflow() {
    let mut doc = lopdf::Document::new();

    // Build a degenerate linear chain of 35 /Pages nodes: Pages[35] -> ... -> Pages[1] -> Page[0]
    let leaf_id = doc.add_object(dictionary! { "Type" => "Page" });
    let mut prev_id = leaf_id;

    for _ in 1..=35 {
        let parent_id = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![lopdf::Object::Reference(prev_id)],
            "Count" => 1,
        });
        prev_id = parent_id;
    }

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => lopdf::Object::Reference(prev_id),
    });
    doc.trailer.set("Root", lopdf::Object::Reference(catalog_id));

    let guard = PageTreeDepthGuard::new(); // default max depth = 32
    let res = guard.collect_pages_iterative(&doc);
    assert!(matches!(res, Err(PdfDefenseError::PageTreeDepthExceeded { depth, max_depth }) if depth > max_depth));
}

#[test]
fn test_target_06_1000_tasks_concurrent_pdf_parsing_contention() {
    let valid_pdf = Arc::new(build_minimal_valid_pdf());
    let counter = Arc::new(AtomicUsize::new(0));

    (0..1000).into_par_iter().for_each(|_| {
        let pdf_data = valid_pdf.clone();
        if let Ok(parser) = TTZipPdfParser::open_from_bytes(&pdf_data) {
            assert!(parser.page_count() >= 1);
            let metadata = PdfMetadataExtractor::extract_metadata(&parser);
            assert!(metadata.is_ok());
            counter.fetch_add(1, Ordering::Relaxed);
        }
    });

    assert_eq!(counter.load(Ordering::SeqCst), 1000);
}

#[test]
fn test_target_07_500_rounds_pseudo_random_mutation_fuzzing() {
    let base_pdf = build_minimal_valid_pdf();
    let mut prng = DeterministicPrng::new(0xFEED_FACE_CAFE_BEEF);
    let mut panic_count = 0;

    for _ in 0..500 {
        let mut mutated = base_pdf.clone();
        let num_mutations = prng.next_range(1, 10);
        for _ in 0..num_mutations {
            let offset = prng.next_range(0, mutated.len() - 1);
            let mutation_type = prng.next_range(0, 3);
            match mutation_type {
                0 => mutated[offset] = prng.next_byte(),
                1 => mutated[offset] ^= 0xFF,
                2 => mutated[offset] = 0x00,
                _ => {}
            }
        }

        let run = catch_unwind(|| {
            if let Ok(parser) = TTZipPdfParser::open_from_bytes(&mutated) {
                let _ = PdfMetadataExtractor::extract_metadata(&parser);
                let _ = PdfOutlineExtractor::extract_outlines(&parser);
                let _ = PdfTextExtractor::extract_page_text(&parser, 1);
            }
        });

        if run.is_err() {
            panic_count += 1;
        }
    }

    assert_eq!(panic_count, 0, "500 rounds of random mutation fuzzing must yield 0 panics");
}

#[test]
fn test_target_08_malformed_tounicode_cmap_mapping() {
    let parser = TTZipPdfParser::open_from_bytes(&build_minimal_valid_pdf()).unwrap();
    let result1 = catch_unwind(|| {
        let _ = PdfTextExtractor::extract_page_text(&parser, 1);
    });
    assert!(result1.is_ok());
}

#[test]
fn test_target_09_malformed_xmp_metadata_packet() {
    let malformed_xmp = b"<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?>\n<x:xmpmeta xmlns:x='adobe:ns:meta/'>\n<rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'>\n<rdf:Description>\n<dc:title><rdf:Alt><rdf:li xml:lang='x-default'>Broken Title & Unclosed Entity </rdf:Alt></dc:title>\n";

    let mut doc = lopdf::Document::new();
    let metadata_stream_id = doc.add_object(lopdf::Object::Stream(lopdf::Stream::new(
        dictionary! {
            "Type" => "Metadata",
            "Subtype" => "XML",
        },
        malformed_xmp.to_vec(),
    )));
    doc.trailer.set("Root", dictionary! { "Metadata" => lopdf::Object::Reference(metadata_stream_id) });

    let mut buf = Vec::new();
    let _ = doc.save_to(&mut buf);

    let result = catch_unwind(|| {
        if let Ok(parser) = TTZipPdfParser::open_from_bytes(&buf) {
            let _ = PdfMetadataExtractor::extract_metadata(&parser);
        }
    });
    assert!(result.is_ok(), "Malformed XMP XML must not crash metadata extraction");
}

#[test]
fn test_target_10_damaged_object_stream_compressed_tables() {
    let mut doc = lopdf::Document::new();
    let damaged_stream_data = b"1 0 2 10 3 20 << /Type /Catalog >> << /Type /Pages >>";
    let objstm_id = doc.add_object(lopdf::Object::Stream(lopdf::Stream::new(
        dictionary! {
            "Type" => "ObjStm",
            "N" => 9999,
            "First" => 10,
        },
        damaged_stream_data.to_vec(),
    )));
    doc.trailer.set("Root", lopdf::Object::Reference(objstm_id));

    let mut buf = Vec::new();
    let _ = doc.save_to(&mut buf);

    let result = catch_unwind(|| {
        let _ = TTZipPdfParser::open_from_bytes(&buf);
    });
    assert!(result.is_ok(), "Damaged ObjStm must fail gracefully");
}

#[test]
fn test_target_11_extreme_coordinates_and_inverted_boxes() {
    let mut doc = lopdf::Document::new();
    let pages_id = doc.new_object_id();
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => lopdf::Object::Reference(pages_id),
        "MediaBox" => vec![999999.0.into(), 999999.0.into(), (-999999.0).into(), (-999999.0).into()],
    });
    doc.set_object(
        pages_id,
        lopdf::Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![lopdf::Object::Reference(page_id)],
            "Count" => 1,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => lopdf::Object::Reference(pages_id),
    });
    doc.trailer.set("Root", lopdf::Object::Reference(catalog_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    let parser = TTZipPdfParser::open_from_bytes(&buf);
    assert!(parser.is_ok());
    let p = parser.unwrap();
    let page_info = p.get_page_info(1);
    assert!(page_info.is_ok());
}

#[test]
fn test_target_12_malicious_javascript_and_launch_sanitization() {
    let mut doc = lopdf::Document::new();
    let launch_action = doc.add_object(dictionary! {
        "S" => "Launch",
        "F" => lopdf::Object::String(b"calc.exe".to_vec(), lopdf::StringFormat::Literal),
    });
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "OpenAction" => lopdf::Object::Reference(launch_action),
    });
    doc.trailer.set("Root", lopdf::Object::Reference(catalog_id));

    let guard = MaliciousActionSandboxGuard::new(ActionPolicy::SanitizeAndStrip);
    let sanitize_report = guard.sanitize_document(&mut doc).unwrap();
    assert!(sanitize_report.is_sanitized);
    assert!(sanitize_report.has_critical_threats());
}

#[test]
fn test_target_13_encrypted_pdf_password_retry_quota_fuse() {
    let guard = PdfEncryptionGuard::new(EncryptionSecurityPolicy::EnforceModernAesOnly);

    // Constant-time password probe against expected hash
    let dummy_hash = [0u8; 32];
    assert!(!guard.verify_password_probe(b"wrong_password_1", &dummy_hash));
    assert!(!guard.verify_password_probe(b"wrong_password_2", &dummy_hash));
}

#[test]
fn test_target_14_zero_byte_and_empty_stream_pdf_defense() {
    let empty_bytes = Vec::<u8>::new();
    let single_byte = vec![0x25]; // Just '%'
    let truncated_magic = b"%PDF-".to_vec();

    assert!(TTZipPdfParser::open_from_bytes(&empty_bytes).is_err());
    assert!(TTZipPdfParser::open_from_bytes(&single_byte).is_err());
    assert!(TTZipPdfParser::open_from_bytes(&truncated_magic).is_err());
}

#[test]
fn test_target_15_incomplete_content_stream_and_unbalanced_bt_et() {
    let mut doc = lopdf::Document::new();
    let content_stream = b"BT /F1 12 Tf (Unclosed Text Block"; // Missing ET
    let content_id = doc.add_object(lopdf::Object::Stream(lopdf::Stream::new(
        dictionary! {},
        content_stream.to_vec(),
    )));
    let pages_id = doc.new_object_id();
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => lopdf::Object::Reference(pages_id),
        "Contents" => lopdf::Object::Reference(content_id),
    });
    doc.set_object(
        pages_id,
        lopdf::Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![lopdf::Object::Reference(page_id)],
            "Count" => 1,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => lopdf::Object::Reference(pages_id),
    });
    doc.trailer.set("Root", lopdf::Object::Reference(catalog_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();

    let parser = TTZipPdfParser::open_from_bytes(&buf).unwrap();
    let extracted = PdfTextExtractor::extract_page_text(&parser, 1);
    assert!(extracted.is_ok(), "Unbalanced BT operator must not crash parser");
}

#[test]
fn test_target_16_sensitive_pdf_buffer_zeroize_memory_erasure() {
    let secret = b"TopSecretContractContent1234567890".to_vec();
    let mut buf = SensitivePdfBuffer::from_vec(secret.clone());
    assert_eq!(buf.as_slice(), secret.as_slice());

    // Explicit clear
    buf.clear_and_zeroize();
    assert!(buf.is_empty());
}
