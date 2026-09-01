// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for zero-copy XML parsing, Office Open XML outlines,
//! EPUB structure extraction, and Apple Property List strong AST evaluation.

use quick_xml::events::Event;

use super::epub::*;
use super::office::*;
use super::parser::*;
use super::plist::*;

// ============================================================================
// 1. TTZipXmlParser & AdaptiveBufferPool Tests
// ============================================================================

#[test]
fn test_xml_parser_basic_tokens_and_depth() {
    let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <root id="123" xmlns:tt="http://ttzip.io">
        <tt:child active="true">Hello &amp; World</tt:child>
        <empty_tag attr="val"/>
        <![CDATA[raw cdata content <unescaped>]]>
    </root>"#;

    let mut parser = TTZipXmlParser::from_slice(xml);
    let mut events = Vec::new();
    let mut buf = Vec::with_capacity(128);

    loop {
        match parser.read_event_into(&mut buf).expect("Valid XML parsing") {
            Event::Decl(_) => events.push("Decl"),
            Event::Start(ref e) => {
                let local = TTZipXmlParser::local_name(e.name());
                if local == b"root" {
                    assert_eq!(parser.current_depth(), 1);
                    assert_eq!(
                        TTZipXmlParser::get_attribute(e, b"id").as_deref(),
                        Some("123")
                    );
                } else if local == b"child" {
                    assert_eq!(parser.current_depth(), 2);
                    let (prefix, name) = TTZipXmlParser::split_qname(e.name().into_inner());
                    assert_eq!(prefix, Some(b"tt".as_slice()));
                    assert_eq!(name, b"child");
                }
                events.push("Start");
            }
            Event::Text(ref t) => {
                let text = t.unescape().expect("Unescape");
                if text == "Hello & World" {
                    events.push("Text");
                }
            }
            Event::Empty(ref e) => {
                let local = TTZipXmlParser::local_name(e.name());
                assert_eq!(local, b"empty_tag");
                assert_eq!(
                    TTZipXmlParser::get_attribute(e, b"attr").as_deref(),
                    Some("val")
                );
                events.push("Empty");
            }
            Event::CData(ref c) => {
                let s = std::str::from_utf8(c.as_ref()).expect("UTF-8 CDATA");
                assert_eq!(s, "raw cdata content <unescaped>");
                events.push("CData");
            }
            Event::End(ref e) => {
                let local = TTZipXmlParser::local_name(e.name());
                if local == b"root" {
                    assert_eq!(parser.current_depth(), 0);
                }
                events.push("End");
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    assert!(events.contains(&"Decl"));
    assert!(events.contains(&"Start"));
    assert!(events.contains(&"Text"));
    assert!(events.contains(&"Empty"));
    assert!(events.contains(&"CData"));
}

#[test]
fn test_adaptive_buffer_pool_resizing() {
    let mut pool = AdaptiveBufferPool::new(64, 256);
    {
        let buf = pool.get_buf();
        buf.extend_from_slice(&[0u8; 100]);
        assert_eq!(buf.len(), 100);
    }
    {
        let buf = pool.get_buf();
        assert_eq!(buf.len(), 0);
        // Exceed max_capacity
        buf.extend_from_slice(&[1u8; 500]);
        assert_eq!(buf.capacity(), 500);
    }
    {
        // Should shrink back to max_capacity
        let buf = pool.get_buf();
        assert_eq!(buf.len(), 0);
        assert!(buf.capacity() <= 256);
    }
}

#[test]
fn test_extract_single_element_text_helper() {
    let xml = br#"<document><metadata><title>TTZip Whitepaper</title></metadata></document>"#;
    let title = extract_single_element_text(xml, b"title");
    assert_eq!(title.as_deref(), Some("TTZip Whitepaper"));

    let missing = extract_single_element_text(xml, b"author");
    assert_eq!(missing, None);
}

// ============================================================================
// 2. OfficeXmlExtractor Tests (DOCX, XLSX, PPTX)
// ============================================================================

#[test]
fn test_office_core_properties_parsing() {
    let core_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                       xmlns:dc="http://purl.org/dc/elements/1.1/"
                       xmlns:dcterms="http://purl.org/dc/terms/">
        <dc:title>High Performance Compression</dc:title>
        <dc:creator>Witt Kung</dc:creator>
        <dc:subject>Systems Engineering</dc:subject>
        <dc:description>Architectural blueprint for TTZip</dc:description>
        <cp:keywords>rust, uniffi, microkernel, compression</cp:keywords>
        <cp:lastModifiedBy>Witt Kung</cp:lastModifiedBy>
        <cp:revision>42</cp:revision>
        <dcterms:created>2026-09-01T08:00:00Z</dcterms:created>
        <dcterms:modified>2026-09-01T12:00:00Z</dcterms:modified>
        <cp:category>Architecture Docs</cp:category>
        <cp:contentStatus>Draft</cp:contentStatus>
    </cp:coreProperties>"#;

    let props = OfficeXmlExtractor::parse_core_properties(core_xml).expect("Core props parse");
    assert_eq!(props.title.as_deref(), Some("High Performance Compression"));
    assert_eq!(props.creator.as_deref(), Some("Witt Kung"));
    assert_eq!(props.subject.as_deref(), Some("Systems Engineering"));
    assert_eq!(
        props.description.as_deref(),
        Some("Architectural blueprint for TTZip")
    );
    assert_eq!(
        props.keywords.as_deref(),
        Some("rust, uniffi, microkernel, compression")
    );
    assert_eq!(props.last_modified_by.as_deref(), Some("Witt Kung"));
    assert_eq!(props.revision.as_deref(), Some("42"));
    assert_eq!(props.created.as_deref(), Some("2026-09-01T08:00:00Z"));
    assert_eq!(props.modified.as_deref(), Some("2026-09-01T12:00:00Z"));
    assert_eq!(props.category.as_deref(), Some("Architecture Docs"));
    assert_eq!(props.content_status.as_deref(), Some("Draft"));
}

#[test]
fn test_office_app_properties_parsing() {
    let app_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
        <Application>Microsoft Macintosh Word</Application>
        <AppVersion>16.0000</AppVersion>
        <TotalTime>180</TotalTime>
        <Pages>12</Pages>
        <Words>4520</Words>
        <Characters>28910</Characters>
        <CharactersWithSpaces>33430</CharactersWithSpaces>
        <Lines>320</Lines>
        <Paragraphs>98</Paragraphs>
        <Slides>0</Slides>
        <Notes>0</Notes>
        <HiddenSlides>0</HiddenSlides>
        <Company>TTZip Core Team</Company>
    </Properties>"#;

    let app = OfficeXmlExtractor::parse_app_properties(app_xml).expect("App props parse");
    assert_eq!(app.application.as_deref(), Some("Microsoft Macintosh Word"));
    assert_eq!(app.app_version.as_deref(), Some("16.0000"));
    assert_eq!(app.total_time_mins, Some(180));
    assert_eq!(app.pages, Some(12));
    assert_eq!(app.words, Some(4520));
    assert_eq!(app.characters, Some(28910));
    assert_eq!(app.characters_with_spaces, Some(33430));
    assert_eq!(app.lines, Some(320));
    assert_eq!(app.paragraphs, Some(98));
    assert_eq!(app.company.as_deref(), Some("TTZip Core Team"));
}

#[test]
fn test_docx_document_outline_and_paragraphs() {
    let doc_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
        <w:body>
            <w:p>
                <w:pPr><w:pStyle w:val="Title"/></w:pPr>
                <w:r><w:t>TTZip Engine Specification</w:t></w:r>
            </w:p>
            <w:p>
                <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
                <w:r><w:t>1. Architecture Overview</w:t></w:r>
            </w:p>
            <w:p>
                <w:r><w:t>The TTZip engine utilizes zero-copy parsing techniques.</w:t></w:r>
            </w:p>
            <w:p>
                <w:pPr><w:outlineLvl w:val="1"/></w:pPr>
                <w:r><w:t>1.1 Memory Insulation</w:t></w:r>
            </w:p>
            <w:p>
                <w:r><w:t>All XML processing is bound to bounded memory buffers.</w:t></w:r>
            </w:p>
        </w:body>
    </w:document>"#;

    let outline = OfficeXmlExtractor::parse_docx_document(doc_xml).expect("Docx parse");
    assert_eq!(outline.paragraph_count, 5);
    assert_eq!(outline.headings.len(), 3);

    assert_eq!(outline.headings[0].level, 0); // Title
    assert_eq!(outline.headings[0].text, "TTZip Engine Specification");

    assert_eq!(outline.headings[1].level, 1); // Heading1
    assert_eq!(outline.headings[1].text, "1. Architecture Overview");

    assert_eq!(outline.headings[2].level, 2); // outlineLvl 1 -> level 2
    assert_eq!(outline.headings[2].text, "1.1 Memory Insulation");

    assert!(outline.full_text.contains("TTZip Engine Specification"));
    assert!(outline.full_text.contains("bounded memory buffers"));
    assert!(outline.word_count >= 15);
}

#[test]
fn test_xlsx_workbook_and_shared_strings() {
    let wb_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
              xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
        <workbookPr date1904="1"/>
        <sheets>
            <sheet name="Summary" sheetId="1" state="visible" r:id="rId1"/>
            <sheet name="Q1 Financials" sheetId="2" state="visible" r:id="rId2"/>
            <sheet name="Internal Raw Data" sheetId="3" state="hidden" r:id="rId3"/>
        </sheets>
    </workbook>"#;

    let wb = OfficeXmlExtractor::parse_xlsx_workbook(wb_xml).expect("Xlsx workbook parse");
    assert!(wb.date_1904);
    assert_eq!(wb.sheets.len(), 3);
    assert_eq!(wb.sheets[0].name, "Summary");
    assert_eq!(wb.sheets[0].sheet_id, 1);
    assert_eq!(wb.sheets[0].r_id, "rId1");
    assert_eq!(wb.sheets[2].name, "Internal Raw Data");
    assert_eq!(wb.sheets[2].state.as_deref(), Some("hidden"));

    let sst_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3" uniqueCount="3">
        <si><t>Revenue</t></si>
        <si><t>Operating Margin</t></si>
        <si><t>Net Income</t></si>
    </sst>"#;

    let strings = OfficeXmlExtractor::parse_xlsx_shared_strings(sst_xml, Some(2))
        .expect("Shared strings parse");
    assert_eq!(strings.len(), 2);
    assert_eq!(strings[0], "Revenue");
    assert_eq!(strings[1], "Operating Margin");
}

#[test]
fn test_pptx_slide_text_and_title() {
    let slide_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
           xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
        <p:cSld>
            <p:spTree>
                <p:sp>
                    <p:nvSpPr>
                        <p:nvPr>
                            <p:ph type="ctrTitle"/>
                        </p:nvPr>
                    </p:nvSpPr>
                    <p:txBody>
                        <a:p><a:r><a:t>Executive Summary</a:t></a:r></a:p>
                    </p:txBody>
                </p:sp>
                <p:sp>
                    <p:txBody>
                        <a:p><a:r><a:t>Point 1: Streaming throughput exceeds 1 GB/s.</a:t></a:r></a:p>
                        <a:p><a:r><a:t>Point 2: Memory bounded to 64MB.</a:t></a:r></a:p>
                    </p:txBody>
                </p:sp>
            </p:spTree>
        </p:cSld>
    </p:sld>"#;

    let slide = OfficeXmlExtractor::parse_pptx_slide(slide_xml, 1).expect("PPTX slide parse");
    assert_eq!(slide.slide_number, 1);
    assert_eq!(slide.title.as_deref(), Some("Executive Summary"));
    assert_eq!(slide.text_boxes.len(), 2);
    assert!(slide.full_text.contains("Executive Summary"));
    assert!(slide.full_text.contains("Streaming throughput exceeds 1 GB/s"));
}

// ============================================================================
// 3. EpubMetadataExtractor Tests (Container, OPF, NCX, Nav XHTML)
// ============================================================================

#[test]
fn test_epub_container_xml_resolution() {
    let container_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
        <rootfiles>
            <rootfile full-path="EPUB/package.opf" media-type="application/oebps-package+xml"/>
        </rootfiles>
    </container>"#;

    let opf_path =
        EpubMetadataExtractor::parse_container_xml(container_xml).expect("Container parse");
    assert_eq!(opf_path, "EPUB/package.opf");
}

#[test]
fn test_epub_opf_metadata_and_manifest() {
    let opf_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <package xmlns="http://www.idpf.org/2007/opf" unique-identifier="pub-id" version="3.0">
        <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/">
            <dc:title>The Art of Systems Programming</dc:title>
            <dc:creator>Witt Kung</dc:creator>
            <dc:contributor>DeepMind Team</dc:contributor>
            <dc:publisher>TTZip Press</dc:publisher>
            <dc:language>en-US</dc:language>
            <dc:identifier id="pub-id">urn:isbn:9781234567890</dc:identifier>
            <dc:description>A masterclass in pure safe Rust and UniFFI engineering.</dc:description>
            <dc:date>2026-09-01</dc:date>
            <dc:rights>Copyright (c) 2026 Witt Kung</dc:rights>
            <dc:subject>Computer Science</dc:subject>
            <dc:subject>Systems Programming</dc:subject>
            <meta name="cover" content="cover-image"/>
            <meta property="dcterms:modified">2026-09-01T12:00:00Z</meta>
        </metadata>
        <manifest>
            <item id="cover-image" href="images/cover.jpg" media-type="image/jpeg" properties="cover-image"/>
            <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
            <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
            <item id="ch1" href="text/ch1.xhtml" media-type="application/xhtml+xml"/>
            <item id="ch2" href="text/ch2.xhtml" media-type="application/xhtml+xml"/>
        </manifest>
        <spine toc="ncx">
            <itemref idref="ch1" linear="yes"/>
            <itemref idref="ch2" linear="yes"/>
        </spine>
    </package>"#;

    let pkg = EpubMetadataExtractor::parse_opf(opf_xml).expect("OPF parse");
    assert_eq!(
        pkg.metadata.title.as_deref(),
        Some("The Art of Systems Programming")
    );
    assert_eq!(pkg.metadata.creators, vec!["Witt Kung"]);
    assert_eq!(pkg.metadata.contributors, vec!["DeepMind Team"]);
    assert_eq!(pkg.metadata.publisher.as_deref(), Some("TTZip Press"));
    assert_eq!(pkg.metadata.language.as_deref(), Some("en-US"));
    assert_eq!(
        pkg.metadata.identifier.as_deref(),
        Some("urn:isbn:9781234567890")
    );
    assert_eq!(pkg.metadata.subjects.len(), 2);
    assert_eq!(
        pkg.metadata.modified_date.as_deref(),
        Some("2026-09-01T12:00:00Z")
    );

    assert_eq!(pkg.manifest.len(), 5);
    assert_eq!(pkg.spine.len(), 2);
    assert_eq!(pkg.cover_image_href.as_deref(), Some("images/cover.jpg"));
    assert_eq!(pkg.toc_ncx_href.as_deref(), Some("toc.ncx"));
    assert_eq!(pkg.nav_xhtml_href.as_deref(), Some("nav.xhtml"));
}

#[test]
fn test_epub_ncx_hierarchical_toc() {
    let ncx_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
        <navMap>
            <navPoint id="np-1" playOrder="1">
                <navLabel><text>Part I: Foundations</text></navLabel>
                <content src="text/part1.xhtml"/>
                <navPoint id="np-2" playOrder="2">
                    <navLabel><text>Chapter 1: Zero-Copy Tokenization</text></navLabel>
                    <content src="text/ch1.xhtml"/>
                </navPoint>
                <navPoint id="np-3" playOrder="3">
                    <navLabel><text>Chapter 2: Memory Insulation</text></navLabel>
                    <content src="text/ch2.xhtml"/>
                </navPoint>
            </navPoint>
            <navPoint id="np-4" playOrder="4">
                <navLabel><text>Part II: Advanced Architecture</text></navLabel>
                <content src="text/part2.xhtml"/>
            </navPoint>
        </navMap>
    </ncx>"#;

    let toc = EpubMetadataExtractor::parse_ncx_toc(ncx_xml).expect("NCX TOC parse");
    assert_eq!(toc.nodes.len(), 2);
    assert_eq!(toc.nodes[0].title, "Part I: Foundations");
    assert_eq!(toc.nodes[0].play_order, 1);
    assert_eq!(toc.nodes[0].href, "text/part1.xhtml");
    assert_eq!(toc.nodes[0].children.len(), 2);

    assert_eq!(
        toc.nodes[0].children[0].title,
        "Chapter 1: Zero-Copy Tokenization"
    );
    assert_eq!(toc.nodes[0].children[0].play_order, 2);
    assert_eq!(
        toc.nodes[0].children[1].title,
        "Chapter 2: Memory Insulation"
    );
    assert_eq!(toc.nodes[1].title, "Part II: Advanced Architecture");
}

#[test]
fn test_epub_nav_xhtml_hierarchical_toc() {
    let nav_xhtml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
        <body>
            <nav epub:type="toc" id="toc">
                <h1>Table of Contents</h1>
                <ol>
                    <li>
                        <a href="text/ch1.xhtml">Chapter 1: Introduction</a>
                        <ol>
                            <li><a href="text/ch1.xhtml#sec1">1.1 Motivation</a></li>
                            <li><a href="text/ch1.xhtml#sec2">1.2 Design Goals</a></li>
                        </ol>
                    </li>
                    <li><a href="text/ch2.xhtml">Chapter 2: Performance Evaluation</a></li>
                </ol>
            </nav>
        </body>
    </html>"#;

    let toc = EpubMetadataExtractor::parse_nav_xhtml(nav_xhtml).expect("Nav XHTML TOC parse");
    assert_eq!(toc.nodes.len(), 2);
    assert_eq!(toc.nodes[0].title, "Chapter 1: Introduction");
    assert_eq!(toc.nodes[0].href, "text/ch1.xhtml");
    assert_eq!(toc.nodes[0].children.len(), 2);
    assert_eq!(toc.nodes[0].children[0].title, "1.1 Motivation");
    assert_eq!(toc.nodes[0].children[0].href, "text/ch1.xhtml#sec1");
    assert_eq!(
        toc.nodes[1].title,
        "Chapter 2: Performance Evaluation"
    );
}

// ============================================================================
// 4. ApplePlistParser Tests (AST & Info.plist)
// ============================================================================

#[test]
fn test_apple_plist_ast_types_and_nested_dict() {
    let plist_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0">
    <dict>
        <key>Name</key>
        <string>TTZip Pro</string>
        <key>BuildNumber</key>
        <integer>1042</integer>
        <key>HexValue</key>
        <integer>0x2A</integer>
        <key>PerformanceRatio</key>
        <real>9.85</real>
        <key>IsSandboxed</key>
        <true/>
        <key>IsTrial</key>
        <false/>
        <key>ReleaseDate</key>
        <date>2026-09-01T12:00:00Z</date>
        <key>PayloadData</key>
        <data>VFRaaXAgUm9ja3Mh</data>
        <key>Tags</key>
        <array>
            <string>archiver</string>
            <string>compression</string>
            <integer>2026</integer>
        </array>
        <key>NestedConfig</key>
        <dict>
            <key>MaxThreads</key>
            <integer>8</integer>
            <key>EnableSimd</key>
            <true/>
        </dict>
    </dict>
    </plist>"#;

    let root = ApplePlistParser::parse_xml_plist(plist_xml).expect("Plist AST parse");
    let dict = root.as_dict().expect("Dictionary root");

    assert_eq!(root.get_str("Name"), Some("TTZip Pro"));
    assert_eq!(root.get_i64("BuildNumber"), Some(1042));
    assert_eq!(root.get_i64("HexValue"), Some(42));
    assert_eq!(root.get("PerformanceRatio").and_then(|v| v.as_real()), Some(9.85));
    assert_eq!(root.get_bool("IsSandboxed"), Some(true));
    assert_eq!(root.get_bool("IsTrial"), Some(false));

    match dict.get("ReleaseDate") {
        Some(PlistValue::Date(d)) => assert_eq!(d, "2026-09-01T12:00:00Z"),
        _ => panic!("Expected Date variant"),
    }

    match dict.get("PayloadData") {
        Some(PlistValue::Data(d)) => assert_eq!(d, b"TTZip Rocks!"),
        _ => panic!("Expected Data variant"),
    }

    let tags = root.get("Tags").and_then(|v| v.as_array()).expect("Array");
    assert_eq!(tags.len(), 3);
    assert_eq!(tags[0].as_str(), Some("archiver"));
    assert_eq!(tags[1].as_str(), Some("compression"));
    assert_eq!(tags[2].as_integer(), Some(2026));

    let nested = root.get("NestedConfig").expect("NestedConfig");
    assert_eq!(nested.get_i64("MaxThreads"), Some(8));
    assert_eq!(nested.get_bool("EnableSimd"), Some(true));
}

#[test]
fn test_apple_info_plist_metadata_extraction() {
    let info_plist = r#"<?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0">
    <dict>
        <key>CFBundleIdentifier</key>
        <string>com.wittkung.ttzip</string>
        <key>CFBundleName</key>
        <string>TTZip</string>
        <key>CFBundleDisplayName</key>
        <string>TTZip Archiver</string>
        <key>CFBundleShortVersionString</key>
        <string>2.4.0</string>
        <key>CFBundleVersion</key>
        <string>2400</string>
        <key>CFBundleExecutable</key>
        <string>ttzip_exec</string>
        <key>CFBundlePackageType</key>
        <string>APPL</string>
        <key>LSMinimumSystemVersion</key>
        <string>14.0</string>
        <key>NSHumanReadableCopyright</key>
        <string>Copyright © 2026 Witt Kung. All rights reserved.</string>
        <key>LSUIElement</key>
        <false/>
        <key>NSPrincipalClass</key>
        <string>PrincipalAppClass</string>
    </dict>
    </plist>"#.as_bytes();

    let meta = ApplePlistParser::parse_info_plist(info_plist).expect("Info.plist parse");
    assert_eq!(meta.bundle_identifier.as_deref(), Some("com.wittkung.ttzip"));
    assert_eq!(meta.bundle_name.as_deref(), Some("TTZip"));
    assert_eq!(meta.bundle_display_name.as_deref(), Some("TTZip Archiver"));
    assert_eq!(meta.bundle_short_version_string.as_deref(), Some("2.4.0"));
    assert_eq!(meta.bundle_version.as_deref(), Some("2400"));
    assert_eq!(meta.bundle_executable.as_deref(), Some("ttzip_exec"));
    assert_eq!(meta.bundle_package_type.as_deref(), Some("APPL"));
    assert_eq!(meta.minimum_system_version.as_deref(), Some("14.0"));
    assert_eq!(
        meta.human_readable_copyright.as_deref(),
        Some("Copyright © 2026 Witt Kung. All rights reserved.")
    );
    assert_eq!(meta.ui_element, Some(false));
    assert_eq!(meta.principal_class.as_deref(), Some("PrincipalAppClass"));
}

#[test]
fn test_malformed_xml_and_plist_handling() {
    let unclosed_xml = b"<root><unclosed>";
    let mut parser = TTZipXmlParser::from_slice(unclosed_xml);
    let mut buf = Vec::new();
    let mut reached_end = false;
    while let Ok(event) = parser.read_event_into(&mut buf) {
        if matches!(event, Event::Eof) {
            reached_end = true;
            break;
        }
        buf.clear();
    }
    assert!(reached_end);

    let non_plist = b"Not XML at all";
    let res = ApplePlistParser::parse_xml_plist(non_plist);
    assert!(res.is_err());
}
