// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unit tests for e-book 6-Layer Defense-in-Depth guards.

use std::io::Cursor;

use crate::security::ebook_defense::*;

#[test]
fn test_manifest_guard_valid_opf() {
    let opf_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <manifest>
    <item id="cover" href="cover.xhtml" media-type="application/xhtml+xml"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="img1" href="images/fig1.png" media-type="image/png"/>
  </manifest>
</package>"#;

    let mut guard = ManifestItemCountGuard::new();
    let items = guard
        .parse_opf_stream(Cursor::new(opf_xml), opf_xml.len() as u64)
        .expect("Valid OPF parsing failed");

    assert_eq!(items.len(), 3);
    assert_eq!(items.get("cover").unwrap().href, "cover.xhtml");
    assert_eq!(items.get("c1").unwrap().properties.as_deref(), Some("nav"));
}

#[test]
fn test_manifest_guard_blocks_dtd_entities() {
    let malicious_opf = r#"<?xml version="1.0"?>
<!DOCTYPE package [
  <!ENTITY lol "lol">
  <!ENTITY lol2 "&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;">
]>
<package><manifest><item id="i1" href="c.xhtml" media-type="text/html"/></manifest></package>"#;

    let mut guard = ManifestItemCountGuard::new();
    let result = guard.parse_opf_stream(Cursor::new(malicious_opf), malicious_opf.len() as u64);

    assert!(matches!(result, Err(EbookDefenseError::DtdEntitiesForbidden)));
}

#[test]
fn test_manifest_guard_opf_size_limit() {
    let mut guard = ManifestItemCountGuard::new();
    let fake_stream = Cursor::new(b"<package/>");
    let result = guard.parse_opf_stream(fake_stream, MAX_OPF_FILE_SIZE + 1);

    assert!(matches!(result, Err(EbookDefenseError::OpfFileTooLarge { .. })));
}

#[test]
fn test_manifest_guard_attribute_limits() {
    let long_href = "a".repeat(MAX_HREF_LENGTH + 1);
    let opf = format!(
        r#"<package><manifest><item id="i1" href="{}" media-type="text/html"/></manifest></package>"#,
        long_href
    );

    let mut guard = ManifestItemCountGuard::new();
    let result = guard.parse_opf_stream(Cursor::new(opf.as_bytes()), opf.len() as u64);

    assert!(matches!(result, Err(EbookDefenseError::AttributeLengthExceeded { attr: "href", .. })));
}

#[test]
fn test_toc_depth_guard_invariants() {
    let mut guard = TocRecursionDepthGuard::new();

    // 1. Normal insertions
    let root = guard
        .push_node("n1".into(), "Chapter 1".into(), "c1.xhtml".into(), 1, None)
        .expect("Root node push failed");
    let sub = guard
        .push_node("n2".into(), "Section 1.1".into(), "c1.xhtml#s1".into(), 2, Some(root))
        .expect("Sub node push failed");

    assert_eq!(guard.len(), 2);
    assert_eq!(guard.entries()[root].children_indices, vec![sub]);

    // 2. Nesting depth limit exceeded (> 16)
    let res_depth = guard.push_node("n_deep".into(), "Deep".into(), "d.xhtml".into(), 17, Some(sub));
    assert!(matches!(res_depth, Err(EbookDefenseError::TocNestingDepthExceeded { depth: 17, .. })));

    // 3. Empty label rejected
    let res_empty = guard.push_node("n3".into(), "   ".into(), "c3.xhtml".into(), 1, None);
    assert!(matches!(res_empty, Err(EbookDefenseError::EmptyNavLabel)));

    // 4. Cycle detection
    guard.enter_branch("branch_A").expect("Enter branch failed");
    let cycle_res = guard.enter_branch("branch_A");
    assert!(matches!(cycle_res, Err(EbookDefenseError::TocCyclicReferenceDetected { .. })));
    guard.leave_branch("branch_A");
}

#[test]
fn test_palmdoc_decompress_and_compress_roundtrip() {
    let text = b"The quick brown fox jumps over the lazy dog. The quick brown fox jumps again!";
    let compressed = PalmDocDecompressGuard::compress_record(text)
        .expect("Compression failed");
    let decompressed = PalmDocDecompressGuard::decompress_record(&compressed)
        .expect("Decompression failed");

    assert_eq!(decompressed, text);
}

#[test]
fn test_palmdoc_backreference_underflow_prevention() {
    // Construct a corrupt 2-byte pair with distance = 10 when output is only 2 bytes
    let corrupt_stream = [
        b'A', b'B', // 2 bytes literal
        0x80 | 0x01, 0x48, // Distance = 41, len = 3
    ];

    let result = PalmDocDecompressGuard::decompress_record(&corrupt_stream);
    assert!(matches!(result, Err(EbookDefenseError::IllegalBackreferenceDistance { .. })));
}

#[test]
fn test_palmdoc_record_buffer_overflow_prevention() {
    // Construct a stream that attempts to produce > 4096 bytes
    let mut overflow_stream = Vec::new();
    for _ in 0..513 {
        // Literal 8 bytes: count=8 followed by 8 'X' chars
        overflow_stream.push(8u8);
        overflow_stream.extend_from_slice(b"XXXXXXXX");
    } // 513 * 8 = 4104 bytes > 4096

    let result = PalmDocDecompressGuard::decompress_record(&overflow_stream);
    assert!(matches!(result, Err(EbookDefenseError::RecordBufferOverflow { .. })));
}

#[test]
fn test_mobi_exth_arithmetic_overflow_prevention() {
    // EXTH header with malicious length
    let mut exth_data = Vec::new();
    exth_data.extend_from_slice(b"EXTH");
    exth_data.extend_from_slice(&100u32.to_be_bytes()); // Header len: 100
    exth_data.extend_from_slice(&1u32.to_be_bytes()); // Count: 1
    // Record with oversized len
    exth_data.extend_from_slice(&101u32.to_be_bytes()); // Type
    exth_data.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes()); // Malicious Length

    let result = PalmDocDecompressGuard::parse_mobi_exth_records(&exth_data);
    assert!(matches!(
        result,
        Err(EbookDefenseError::ExthIntegerOverflow) | Err(EbookDefenseError::ExthRecordOutOfBounds { .. })
    ));
}

#[test]
fn test_sandbox_xhtml_stripping() {
    let malicious_xhtml = r#"<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <head>
    <script src="http://evil.com/xss.js">alert('hacked');</script>
  </head>
  <body onload="doStealCookie()" onclick="handleClick()">
    <h1>Chapter 1</h1>
    <iframe src="file:///etc/passwd"></iframe>
    <a href="javascript:alert(1)">Click Me</a>
    <p>Safe content paragraph.</p>
  </body>
</html>"#;

    let (sanitized, report) = EbookSandboxGuard::sanitize_xhtml_content(malicious_xhtml);

    assert!(!sanitized.contains("<script"));
    assert!(!sanitized.contains("evil.com"));
    assert!(!sanitized.contains("<iframe"));
    assert!(!sanitized.contains("onload="));
    assert!(!sanitized.contains("javascript:"));
    assert!(sanitized.contains("<h1>Chapter 1</h1>"));
    assert!(sanitized.contains("Safe content paragraph."));

    assert_eq!(report.stripped_tags_count, 2); // <script>, <iframe>
    assert!(report.neutralized_events_count >= 2); // onload, onclick
    assert_eq!(report.neutralized_protocols_count, 1); // javascript:
}

#[test]
fn test_memory_budget_guard_lifecycle() {
    let guard = EbookMemoryBudgetGuard::new(1024, 512);

    // 1. Valid allocation
    {
        let permit = guard.allocate(500).expect("Allocation failed");
        assert_eq!(guard.current_bytes(), 500);
        assert_eq!(guard.remaining_bytes(), 524);
        assert_eq!(permit.size(), 500);
    }
    // Permit dropped: usage back to 0
    assert_eq!(guard.current_bytes(), 0);

    // 2. Over-budget allocation
    let result = guard.allocate(2000);
    assert!(matches!(result, Err(EbookDefenseError::MemoryBudgetExceeded { .. })));

    // 3. Chapter viewport validation
    assert!(guard.validate_chapter_size(600).is_err());
    assert!(guard.validate_chapter_size(300).is_ok());
}

#[test]
fn test_sensitive_ebook_buffer_zeroize() {
    let mut buffer = SensitiveEbookBuffer::from_slice(b"SuperSecretBookTextContent");
    assert_eq!(buffer.as_slice(), b"SuperSecretBookTextContent");
    assert_eq!(buffer.len(), 26);

    // Mutate buffer
    buffer.as_mut_slice()[0] = b's';
    assert_eq!(buffer.as_str().unwrap(), "superSecretBookTextContent");

    // Manually zeroize
    buffer.clear();
    assert_eq!(buffer.len(), 0);
}

#[test]
fn test_ebook_security_pipeline_orchestration() {
    let mut pipeline = EbookSecurityPipeline::default();

    let opf = r#"<package><manifest><item id="i1" href="ch1.xhtml" media-type="application/xhtml+xml"/></manifest></package>"#;
    let rep = pipeline
        .inspect_opf_manifest(Cursor::new(opf.as_bytes()), opf.len() as u64)
        .expect("Pipeline OPF inspection failed");
    assert_eq!(rep.items_count, 1);

    let (clean_html, report) = pipeline
        .sanitize_chapter_content(r#"<p onclick="attack()">Text</p>"#)
        .expect("Sanitize chapter failed");
    assert!(clean_html.contains("<p data-disabled-event="));
    assert_eq!(report.sanitization_report.neutralized_events_count, 1);
}
