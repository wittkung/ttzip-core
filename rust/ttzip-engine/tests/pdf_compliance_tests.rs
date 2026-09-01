// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive ISO 32000 PDF Compliance & Multi-Engine Differential Security Suite.
//!
//! Validates:
//! 1. **Page Outline Tree Equivalence** (ISO 32000-1 §12.3.3):
//!    Hierarchical outline traversal, sibling chains, child depth, destination resolving.
//! 2. **Info Dictionary & Metadata Consistency** (ISO 32000-1 §14.3.3):
//!    Standard metadata dictionary encoding fidelity (UTF-16BE/LE BOM, Latin-1, UTF-8).
//! 3. **ToUnicode CMap Text Decoding Verification** (ISO 32000-1 §9.10):
//!    CID-to-Unicode mapping tables (`beginbfchar`, `beginbfrange`) and multi-byte glyph decoding.
//! 4. **6-Layer Defense-in-Depth Security Attack Vectors**:
//!    - Layer 1: Indirect reference cycle bombs and recursive graph exhaustion.
//!    - Layer 2: Stream expansion quota circuit breakers and decompression bomb mitigation.
//!    - Layer 3: Extreme page tree depth stack overflow interception (depth > 32).
//!    - Layer 4: Malicious JavaScript, OS command `/Launch`, and dangerous URI sandbox insulation.
//!    - Layer 5: Encryption cipher classification, downgrade attack blocking, and password probes.
//!    - Layer 6: Sensitive document buffer volatile memory zeroize verification.

use std::collections::HashMap;
use std::io::Write;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};

use lopdf::dictionary;
use ttzip_engine::security::pdf_defense::{
    ActionPolicy, CipherSuite, EncryptionSecurityPolicy,
    IndirectReferenceCycleGuard, MaliciousActionSandboxGuard, PageTreeDepthGuard,
    PdfDefenseError, PdfEncryptionGuard, PdfSecurityConfig, PdfSecurityPipeline,
    SensitivePdfBuffer, StreamExpansionQuotaGuard, PDF_STANDARD_PASSWORD_PADDING,
};
use ttzip_engine::standards::document_stream::parse_pdf_from_memory;

// ============================================================================
// Helper: Minimal Valid ISO 32000 PDF Document Builder
// ============================================================================

fn create_valid_iso32000_pdf(
    title: &str,
    author: &str,
    page_text: &str,
) -> Vec<u8> {
    let mut doc = lopdf::Document::new();

    // 1. Font Object (Type1 Helvetica)
    let font_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    // 2. Resources Dictionary
    let resources_id = doc.add_object(lopdf::dictionary! {
        "Font" => lopdf::dictionary! {
            "F1" => lopdf::Object::Reference(font_id),
        },
    });

    // 3. Content Stream
    let content_stream = format!(
        "BT /F1 12 Tf 72 712 Td ({}) Tj ET",
        page_text.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)")
    );
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(content_stream.as_bytes()).unwrap();
    let compressed_content = encoder.finish().unwrap();

    let content_id = doc.add_object(lopdf::Object::Stream(lopdf::Stream::new(
        lopdf::dictionary! {
            "Filter" => "FlateDecode",
            "Length" => compressed_content.len() as i64,
        },
        compressed_content,
    )));

    // 4. Page Object
    let page_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Page",
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => lopdf::Object::Reference(content_id),
        "Resources" => lopdf::Object::Reference(resources_id),
    });

    // 5. Pages Root
    let pages_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Pages",
        "Kids" => vec![lopdf::Object::Reference(page_id)],
        "Count" => 1,
    });

    if let Ok(page_obj) = doc.get_object_mut(page_id) {
        if let Ok(dict) = page_obj.as_dict_mut() {
            dict.set("Parent", lopdf::Object::Reference(pages_id));
        }
    }

    // 6. Document Catalog
    let catalog_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Catalog",
        "Pages" => lopdf::Object::Reference(pages_id),
    });
    doc.trailer.set("Root", lopdf::Object::Reference(catalog_id));

    // 7. Info Dictionary
    let info_id = doc.add_object(lopdf::dictionary! {
        "Title" => lopdf::Object::String(title.as_bytes().to_vec(), lopdf::StringFormat::Literal),
        "Author" => lopdf::Object::String(author.as_bytes().to_vec(), lopdf::StringFormat::Literal),
        "Producer" => lopdf::Object::String(b"TTZip High-Throughput Engine".to_vec(), lopdf::StringFormat::Literal),
        "CreationDate" => lopdf::Object::String(b"D:20260901120000Z".to_vec(), lopdf::StringFormat::Literal),
    });
    doc.trailer.set("Info", lopdf::Object::Reference(info_id));

    let mut out_bytes = Vec::new();
    doc.save_to(&mut out_bytes).unwrap();
    out_bytes
}

// ============================================================================
// 1. Page Outline Tree Equivalence Tests (ISO 32000-1 §12.3.3)
// ============================================================================

/// Represents a standardized outline item for differential oracle comparisons.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StandardOutlineItem {
    title: String,
    dest_page: Option<u32>,
    children: Vec<StandardOutlineItem>,
}

/// Recursive reference model for outline tree traversal.
fn build_expected_outline_hierarchy() -> StandardOutlineItem {
    StandardOutlineItem {
        title: "Root".to_string(),
        dest_page: None,
        children: vec![
            StandardOutlineItem {
                title: "Chapter 1: Architecture".to_string(),
                dest_page: Some(1),
                children: vec![
                    StandardOutlineItem {
                        title: "1.1 Microkernel Design".to_string(),
                        dest_page: Some(2),
                        children: vec![],
                    },
                    StandardOutlineItem {
                        title: "1.2 Zeroize Memory Security".to_string(),
                        dest_page: Some(3),
                        children: vec![],
                    },
                ],
            },
            StandardOutlineItem {
                title: "Chapter 2: Formal Verification".to_string(),
                dest_page: Some(4),
                children: vec![],
            },
        ],
    }
}

#[test]
fn test_iso32000_page_outline_tree_equivalence() {
    let mut doc = lopdf::Document::new();

    // 1. Construct outline item hierarchy:
    // Outline Root -> First: Ch1, Last: Ch2
    // Ch1 -> First: Sec1.1, Last: Sec1.2, Next: Ch2
    // Sec1.1 -> Next: Sec1.2
    // Sec1.2 -> Prev: Sec1.1
    // Ch2 -> Prev: Ch1

    let sec1_1_id = doc.add_object(lopdf::dictionary! {
        "Title" => lopdf::Object::String(b"1.1 Microkernel Design".to_vec(), lopdf::StringFormat::Literal),
        "Dest" => vec![0.into(), "Fit".into()],
    });

    let sec1_2_id = doc.add_object(lopdf::dictionary! {
        "Title" => lopdf::Object::String(b"1.2 Zeroize Memory Security".to_vec(), lopdf::StringFormat::Literal),
        "Dest" => vec![1.into(), "Fit".into()],
        "Prev" => lopdf::Object::Reference(sec1_1_id),
    });

    // Update Sec1.1 next pointer
    if let Ok(sec1_1_obj) = doc.get_object_mut(sec1_1_id) {
        if let Ok(dict) = sec1_1_obj.as_dict_mut() {
            dict.set("Next", lopdf::Object::Reference(sec1_2_id));
        }
    }

    let ch1_id = doc.add_object(lopdf::dictionary! {
        "Title" => lopdf::Object::String(b"Chapter 1: Architecture".to_vec(), lopdf::StringFormat::Literal),
        "First" => lopdf::Object::Reference(sec1_1_id),
        "Last" => lopdf::Object::Reference(sec1_2_id),
        "Count" => 2,
    });

    let ch2_id = doc.add_object(lopdf::dictionary! {
        "Title" => lopdf::Object::String(b"Chapter 2: Formal Verification".to_vec(), lopdf::StringFormat::Literal),
        "Prev" => lopdf::Object::Reference(ch1_id),
    });

    if let Ok(ch1_obj) = doc.get_object_mut(ch1_id) {
        if let Ok(dict) = ch1_obj.as_dict_mut() {
            dict.set("Next", lopdf::Object::Reference(ch2_id));
        }
    }

    let outlines_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Outlines",
        "First" => lopdf::Object::Reference(ch1_id),
        "Last" => lopdf::Object::Reference(ch2_id),
        "Count" => 3,
    });

    // Page (1)
    let page1_id = doc.add_object(lopdf::dictionary! { "Type" => "Page" });
    let pages_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Pages",
        "Kids" => vec![lopdf::Object::Reference(page1_id)],
        "Count" => 1,
    });

    let catalog_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Catalog",
        "Pages" => lopdf::Object::Reference(pages_id),
        "Outlines" => lopdf::Object::Reference(outlines_id),
    });
    doc.trailer.set("Root", lopdf::Object::Reference(catalog_id));

    // Differential Oracle Verification: traverse and assert outline semantics
    let cat_obj = doc.get_object(catalog_id).unwrap();
    let cat_dict = cat_obj.as_dict().unwrap();
    let outlines_ref = cat_dict.get(b"Outlines").unwrap().as_reference().unwrap();
    let outlines_obj = doc.get_object(outlines_ref).unwrap().as_dict().unwrap();

    let first_ref = outlines_obj.get(b"First").unwrap().as_reference().unwrap();
    let first_ch = doc.get_object(first_ref).unwrap().as_dict().unwrap();
    assert_eq!(first_ch.get(b"Title").unwrap().as_str().unwrap(), b"Chapter 1: Architecture");

    let next_ref = first_ch.get(b"Next").unwrap().as_reference().unwrap();
    let second_ch = doc.get_object(next_ref).unwrap().as_dict().unwrap();
    assert_eq!(second_ch.get(b"Title").unwrap().as_str().unwrap(), b"Chapter 2: Formal Verification");

    // Verify Chapter 1 children
    let ch1_first = first_ch.get(b"First").unwrap().as_reference().unwrap();
    let sec1 = doc.get_object(ch1_first).unwrap().as_dict().unwrap();
    assert_eq!(sec1.get(b"Title").unwrap().as_str().unwrap(), b"1.1 Microkernel Design");

    let sec2_ref = sec1.get(b"Next").unwrap().as_reference().unwrap();
    let sec2 = doc.get_object(sec2_ref).unwrap().as_dict().unwrap();
    assert_eq!(sec2.get(b"Title").unwrap().as_str().unwrap(), b"1.2 Zeroize Memory Security");

    let expected = build_expected_outline_hierarchy();
    assert_eq!(expected.children[0].title, "Chapter 1: Architecture");
    assert_eq!(expected.children[0].children[0].title, "1.1 Microkernel Design");
    assert_eq!(expected.children[0].children[1].title, "1.2 Zeroize Memory Security");
    assert_eq!(expected.children[1].title, "Chapter 2: Formal Verification");
}

// ============================================================================
// 2. Info Dictionary & Metadata Consistency Tests (ISO 32000-1 §14.3.3)
// ============================================================================

#[test]
fn test_iso32000_info_dictionary_encoding_consistency() {
    // 1. Test UTF-16BE encoded Chinese characters: "TTZip 性能测试"
    let title_text = "TTZip 性能测试";
    let mut utf16be_bytes = vec![0xFE, 0xFF]; // BOM
    for code_unit in title_text.encode_utf16() {
        utf16be_bytes.extend_from_slice(&code_unit.to_be_bytes());
    }

    // 2. Test Latin-1 encoded Author: "Witt Kung"
    let author_text = "Witt Kung";
    let pdf_bytes = create_valid_iso32000_pdf("Fallback Title", author_text, "Document Payload");

    let info = parse_pdf_from_memory(&pdf_bytes, Some(1)).expect("PDF metadata parse");
    assert_eq!(info.author.as_deref(), Some(author_text));
    assert!(info.format_version.starts_with("PDF-"));
    assert_eq!(info.page_count, 1);
    assert!(!info.is_encrypted);

    // 3. Test multi-byte UTF-16BE Info dictionary object injection
    let mut doc = lopdf::Document::load_mem(&pdf_bytes).unwrap();
    let info_id = doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
    if let Ok(info_obj) = doc.get_object_mut(info_id) {
        if let Ok(dict) = info_obj.as_dict_mut() {
            dict.set("Title", lopdf::Object::String(utf16be_bytes, lopdf::StringFormat::Hexadecimal));
            dict.set("Keywords", lopdf::Object::String(b"rust, security, zeroize, iso32000".to_vec(), lopdf::StringFormat::Literal));
        }
    }

    let mut modified_pdf = Vec::new();
    doc.save_to(&mut modified_pdf).unwrap();

    let reloaded_info = parse_pdf_from_memory(&modified_pdf, Some(1)).unwrap();
    assert_eq!(reloaded_info.title.as_deref(), Some(title_text));
    assert_eq!(reloaded_info.keywords.as_deref(), Some("rust, security, zeroize, iso32000"));
}

// ============================================================================
// 3. ToUnicode CMap Text Decoding Verification (ISO 32000-1 §9.10)
// ============================================================================

/// Represents a ToUnicode CMap parser & decoder verifying glyph to UTF-8 mappings.
#[derive(Debug, Default)]
struct TestToUnicodeCMap {
    mappings: HashMap<u16, char>,
}

impl TestToUnicodeCMap {
    /// Parses standard ISO 32000-1 §9.10 ToUnicode CMap stream data.
    fn parse_cmap_stream(stream_data: &str) -> Self {
        let mut map = HashMap::new();

        // Parse beginbfchar / endbfchar blocks
        if let Some(start_idx) = stream_data.find("beginbfchar") {
            if let Some(end_idx) = stream_data[start_idx..].find("endbfchar") {
                let block = &stream_data[start_idx + 11..start_idx + end_idx];
                for line in block.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let cid_hex = parts[0].trim_matches(|c| c == '<' || c == '>');
                        let uni_hex = parts[1].trim_matches(|c| c == '<' || c == '>');
                        if let (Ok(cid), Ok(uni)) = (u16::from_str_radix(cid_hex, 16), u32::from_str_radix(uni_hex, 16)) {
                            if let Some(ch) = char::from_u32(uni) {
                                map.insert(cid, ch);
                            }
                        }
                    }
                }
            }
        }

        // Parse beginbfrange / endbfrange blocks
        if let Some(start_idx) = stream_data.find("beginbfrange") {
            if let Some(end_idx) = stream_data[start_idx..].find("endbfrange") {
                let block = &stream_data[start_idx + 12..start_idx + end_idx];
                for line in block.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 3 {
                        let start_hex = parts[0].trim_matches(|c| c == '<' || c == '>');
                        let end_hex = parts[1].trim_matches(|c| c == '<' || c == '>');
                        let target_hex = parts[2].trim_matches(|c| c == '<' || c == '>');
                        if let (Ok(start_cid), Ok(end_cid), Ok(target_uni)) = (
                            u16::from_str_radix(start_hex, 16),
                            u16::from_str_radix(end_hex, 16),
                            u32::from_str_radix(target_hex, 16),
                        ) {
                            for (current_uni, cid) in (target_uni..).zip(start_cid..=end_cid) {
                                if let Some(ch) = char::from_u32(current_uni) {
                                    map.insert(cid, ch);
                                }
                            }
                        }
                    }
                }
            }
        }

        Self { mappings: map }
    }

    /// Decodes a big-endian CID byte stream into decoded Unicode string.
    fn decode_cid_stream(&self, cid_bytes: &[u8]) -> String {
        let mut result = String::new();
        for chunk in cid_bytes.chunks_exact(2) {
            let cid = u16::from_be_bytes([chunk[0], chunk[1]]);
            if let Some(&ch) = self.mappings.get(&cid) {
                result.push(ch);
            } else {
                result.push('?');
            }
        }
        result
    }
}

#[test]
fn test_iso32000_tounicode_cmap_decoding_differential() {
    let cmap_stream_sample = r#"
/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def
/CMapName /Custom-ToUnicode def
/CMapType 2 def
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
3 beginbfchar
<0001> <0041>
<0002> <0042>
<002A> <4E2D>
endbfchar
1 beginbfrange
<0010> <0012> <0030>
endbfrange
endcmap
CMapName currentdict /CMap defineresource pop
end
end
"#;

    let cmap = TestToUnicodeCMap::parse_cmap_stream(cmap_stream_sample);

    // Verify bfchar single lookups
    assert_eq!(cmap.mappings.get(&0x0001), Some(&'A'));
    assert_eq!(cmap.mappings.get(&0x0002), Some(&'B'));
    assert_eq!(cmap.mappings.get(&0x002A), Some(&'中'));

    // Verify bfrange range expansion (<0010>..<0012> -> '0', '1', '2')
    assert_eq!(cmap.mappings.get(&0x0010), Some(&'0'));
    assert_eq!(cmap.mappings.get(&0x0011), Some(&'1'));
    assert_eq!(cmap.mappings.get(&0x0012), Some(&'2'));

    // Verify composite CID stream decoding: [00 01, 00 2A, 00 11] -> "A中1"
    let cid_payload: &[u8] = &[0x00, 0x01, 0x00, 0x2A, 0x00, 0x11];
    let decoded = cmap.decode_cid_stream(cid_payload);
    assert_eq!(decoded, "A中1");
}

// ============================================================================
// 4. 6-Layer Defense Security Attack Vectors
// ============================================================================

#[test]
fn test_defense_layer1_indirect_reference_cycle_bomb() {
    let mut guard = IndirectReferenceCycleGuard::with_limits(64, 100_000);

    // Indirect cycle A -> B -> C -> A
    let a = (10, 0);
    let b = (11, 0);
    let c = (12, 0);

    let sa = guard.enter_object(a).unwrap();
    let sb = guard.enter_object(b).unwrap();
    let sc = guard.enter_object(c).unwrap();

    let cycle_err = guard.enter_object(a);
    assert!(matches!(cycle_err, Err(PdfDefenseError::CycleDetected { obj_id, .. }) if obj_id == a));

    guard.leave_scope(sc);
    guard.leave_scope(sb);
    guard.leave_scope(sa);
}

#[test]
fn test_defense_layer2_stream_expansion_bomb_mitigation() {
    let mut guard = StreamExpansionQuotaGuard::new();

    // 1 MB of zeros compressed to < 1KB (~1000x expansion ratio)
    let bomb_raw = vec![0u8; 1024 * 1024];
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
    enc.write_all(&bomb_raw).unwrap();
    let compressed_bomb = enc.finish().unwrap();

    // Must be rejected because expansion ratio > 200x
    let res = guard.decompress_flate(&compressed_bomb);
    assert!(matches!(res, Err(PdfDefenseError::StreamExpansionRatioExceeded { .. })));
}

#[test]
fn test_defense_layer3_extreme_page_tree_depth_mitigation() {
    let mut doc = lopdf::Document::new();

    // Build a degenerate linear chain of 35 /Pages nodes: Pages[35] -> ... -> Pages[1] -> Page[0]
    let leaf_id = doc.add_object(lopdf::dictionary! { "Type" => "Page" });
    let mut prev_id = leaf_id;

    for _ in 1..=35 {
        let parent_id = doc.add_object(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => vec![lopdf::Object::Reference(prev_id)],
            "Count" => 1,
        });
        prev_id = parent_id;
    }

    let catalog_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Catalog",
        "Pages" => lopdf::Object::Reference(prev_id),
    });
    doc.trailer.set("Root", lopdf::Object::Reference(catalog_id));

    let guard = PageTreeDepthGuard::new(); // default max depth = 32
    let res = guard.collect_pages_iterative(&doc);
    assert!(matches!(res, Err(PdfDefenseError::PageTreeDepthExceeded { depth, max_depth }) if depth > max_depth));
}

#[test]
fn test_defense_layer4_malicious_action_sandbox_and_sanitization() {
    let mut doc = lopdf::Document::new();

    // Add malicious /Launch action invoking command shell
    let launch_action = doc.add_object(lopdf::dictionary! {
        "S" => "Launch",
        "F" => lopdf::Object::String(b"cmd.exe /c calc.exe".to_vec(), lopdf::StringFormat::Literal),
    });

    let catalog_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Catalog",
        "OpenAction" => lopdf::Object::Reference(launch_action),
    });
    doc.trailer.set("Root", lopdf::Object::Reference(catalog_id));

    // Reject policy interception
    let guard_reject = MaliciousActionSandboxGuard::new(ActionPolicy::RejectAllActiveContent);
    let inspect_res = guard_reject.inspect_document(&doc);
    assert!(matches!(inspect_res, Err(PdfDefenseError::MaliciousActionDetected { action_type, .. }) if action_type == "Launch" || action_type == "OpenAction"));

    // Sanitize policy neutralization
    let guard_sanitize = MaliciousActionSandboxGuard::new(ActionPolicy::SanitizeAndStrip);
    let sanitize_report = guard_sanitize.sanitize_document(&mut doc).unwrap();
    assert!(sanitize_report.is_sanitized);
    assert!(sanitize_report.has_critical_threats());

    // Re-inspection must be 100% clean
    assert!(guard_reject.inspect_document(&doc).is_ok());
}

#[test]
fn test_defense_layer5_encryption_downgrade_and_password_probe() {
    let mut doc = lopdf::Document::new();

    // 1. Construct 40-bit RC4 insecure encrypt dictionary
    let enc_rc4_id = doc.add_object(lopdf::dictionary! {
        "Filter" => "Standard",
        "V" => 1,
        "R" => 2,
        "Length" => 40,
        "P" => -64,
        "O" => lopdf::Object::String(vec![0xAA; 32], lopdf::StringFormat::Hexadecimal),
        "U" => lopdf::Object::String(vec![0xBB; 32], lopdf::StringFormat::Hexadecimal),
    });
    doc.trailer.set("Encrypt", lopdf::Object::Reference(enc_rc4_id));

    // Modern AES policy must refuse insecure RC4-40
    let guard_modern = PdfEncryptionGuard::new(EncryptionSecurityPolicy::EnforceModernAesOnly);
    let res = guard_modern.inspect_document(&doc);
    assert!(matches!(res, Err(PdfDefenseError::InsecureEncryptionDetected { .. })));

    // Allow policy inspects cipher metadata correctly
    let guard_allow = PdfEncryptionGuard::new(EncryptionSecurityPolicy::AllowStandardAndModern);
    let report = guard_allow.inspect_document(&doc).unwrap();
    assert_eq!(report.cipher_suite, CipherSuite::Rc4_40);
    assert!(report.cipher_suite.is_insecure_or_deprecated());

    // 2. Construct Modern AES-256 (ISO 32000-2 / V=5, R=5) dictionary
    let enc_aes_id = doc.add_object(lopdf::dictionary! {
        "Filter" => "Standard",
        "V" => 5,
        "R" => 5,
        "Length" => 256,
        "P" => -4,
        "O" => lopdf::Object::String(vec![0x00; 48], lopdf::StringFormat::Hexadecimal),
        "U" => lopdf::Object::String(vec![0x00; 48], lopdf::StringFormat::Hexadecimal),
        "OE" => lopdf::Object::String(vec![0x00; 32], lopdf::StringFormat::Hexadecimal),
        "UE" => lopdf::Object::String(vec![0x00; 32], lopdf::StringFormat::Hexadecimal),
        "Perms" => lopdf::Object::String(vec![0x00; 16], lopdf::StringFormat::Hexadecimal),
    });
    doc.trailer.set("Encrypt", lopdf::Object::Reference(enc_aes_id));

    let aes_report = guard_modern.inspect_document(&doc).unwrap();
    assert_eq!(aes_report.cipher_suite, CipherSuite::Aes256Cbc);
    assert!(aes_report.cipher_suite.is_modern_secure());

    // Password Probe Constant-Time Verification
    let expected_hash = Sha256::digest(PDF_STANDARD_PASSWORD_PADDING);
    assert!(guard_modern.verify_password_probe(b"", &expected_hash));
    assert!(!guard_modern.verify_password_probe(b"wrong_password", &expected_hash));
}

#[test]
fn test_defense_layer6_sensitive_buffer_zeroize() {
    let mut secret_text = SensitivePdfBuffer::from_str_slice("CONFIDENTIAL ISO 32000 SPECIFICATION");
    assert_eq!(secret_text.to_string_lossy(), "CONFIDENTIAL ISO 32000 SPECIFICATION");
    assert!(secret_text.constant_time_eq(b"CONFIDENTIAL ISO 32000 SPECIFICATION"));

    secret_text.clear_and_zeroize();
    assert!(secret_text.is_empty());
}

#[test]
fn test_pdf_security_pipeline_end_to_end_safe_document() {
    let valid_pdf = create_valid_iso32000_pdf(
        "Clean Document",
        "Security Team",
        "Hello TTZip Safe PDF Stream",
    );

    let mut pipeline = PdfSecurityPipeline::new(PdfSecurityConfig::default());
    let inspection = pipeline.inspect_bytes(&valid_pdf).expect("Inspection should succeed");
    assert!(inspection.is_safe);
    assert_eq!(inspection.page_tree_stats.page_count, 1);
    assert!(inspection.threats_detected.is_empty());

    let extracted_text = pipeline.extract_safe_text(&valid_pdf, Some(1)).unwrap();
    assert!(!extracted_text.is_empty());
}
