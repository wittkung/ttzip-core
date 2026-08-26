// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use super::*;
use crate::zip::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

fn build_test_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let items: Vec<ZipInputItem> = files
        .iter()
        .map(|(name, content)| ZipInputItem {
            rel_path: name.to_string(),
            data: content.to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        })
        .collect();
    let compressed = compress_items_parallel(
        items,
        6,
        crate::types::TTZipEncryptionMethod::None,
        None,
        1,
    )
    .unwrap();
    assemble_zip_archive(&compressed).unwrap()
}

#[test]
fn test_docx_streaming_parser() {
    let doc_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello TTZip World!</w:t></w:r></w:p>
    <w:p><w:r><w:t>High-Performance Pure Safe Rust DOCX streaming parser.</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

    let core_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
  xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/">
  <dc:title>TTZip DOCX Architecture</dc:title>
  <dc:creator>Witt Kung</dc:creator>
  <dc:description>Document Streaming Introspection</dc:description>
</cp:coreProperties>"#;

    let docx_bytes = build_test_zip(&[
        ("word/document.xml", doc_xml),
        ("docProps/core.xml", core_xml),
    ]);

    let parsed = parse_docx_from_memory(&docx_bytes).expect("Failed to parse DOCX");
    assert_eq!(parsed.paragraphs.len(), 2);
    assert_eq!(parsed.paragraphs[0], "Hello TTZip World!");
    assert_eq!(parsed.paragraphs[1], "High-Performance Pure Safe Rust DOCX streaming parser.");
    assert!(parsed.full_text.contains("Hello TTZip World!"));
    assert_eq!(parsed.properties.title.as_deref(), Some("TTZip DOCX Architecture"));
    assert_eq!(parsed.properties.creator.as_deref(), Some("Witt Kung"));
    assert_eq!(parsed.properties.description.as_deref(), Some("Document Streaming Introspection"));
    assert_eq!(parsed.properties.paragraph_count, 2);
    assert!(parsed.properties.word_count > 0);
}

#[test]
fn test_epub_streaming_parser() {
    let container_xml = br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#;

    let opf_xml = br#"<?xml version="1.0" encoding="utf-8"?>
<package version="2.0" xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Rust Performance Guide</dc:title>
    <dc:creator>Witt Kung</dc:creator>
    <dc:language>en</dc:language>
    <dc:publisher>TTZip Press</dc:publisher>
    <meta name="cover" content="cover_img"/>
  </metadata>
  <manifest>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="cover_img" href="images/cover.jpg" media-type="image/jpeg"/>
    <item id="ch1" href="text/ch1.xhtml" media-type="application/xhtml+xml"/>
    <item id="ch2" href="text/ch2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine toc="ncx">
    <itemref idref="ch1"/>
    <itemref idref="ch2"/>
  </spine>
</package>"#;

    let toc_ncx = br#"<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <navMap>
    <navPoint id="np1" playOrder="1">
      <navLabel><text>Chapter 1: Zero Copy</text></navLabel>
      <content src="text/ch1.xhtml"/>
    </navPoint>
    <navPoint id="np2" playOrder="2">
      <navLabel><text>Chapter 2: SIMD Acceleration</text></navLabel>
      <content src="text/ch2.xhtml"/>
    </navPoint>
  </navMap>
</ncx>"#;

    let ch1_content = b"<html><head><title>Ch1</title></head><body><h1>Chapter 1: Zero Copy</h1><p>Zero copy streaming is fast.</p></body></html>";
    let ch2_content = b"<html><head><title>Ch2</title></head><body><h1>Chapter 2: SIMD</h1><p>SIMD parsing reaches >600MB/s.</p></body></html>";
    let dummy_cover_bytes = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x01\x00\x60\x00\x60\x00\x00\xFF\xDB";

    let epub_bytes = build_test_zip(&[
        ("META-INF/container.xml", container_xml),
        ("OEBPS/content.opf", opf_xml),
        ("OEBPS/toc.ncx", toc_ncx),
        ("OEBPS/images/cover.jpg", dummy_cover_bytes),
        ("OEBPS/text/ch1.xhtml", ch1_content),
        ("OEBPS/text/ch2.xhtml", ch2_content),
    ]);

    let book = parse_epub_from_memory(&epub_bytes).expect("Failed to parse EPUB");
    assert_eq!(book.metadata.title, "Rust Performance Guide");
    assert_eq!(book.metadata.authors, vec!["Witt Kung".to_string()]);
    assert_eq!(book.metadata.publisher.as_deref(), Some("TTZip Press"));
    assert_eq!(book.total_chapters, 2);
    assert_eq!(book.chapters[0].title, "Chapter 1: Zero Copy");
    assert_eq!(book.chapters[0].href, "OEBPS/text/ch1.xhtml");
    assert_eq!(book.chapters[1].title, "Chapter 2: SIMD Acceleration");
    assert_eq!(book.chapters[1].href, "OEBPS/text/ch2.xhtml");

    let cover = book.cover.expect("Expected cover image");
    assert_eq!(cover.file_path, "OEBPS/images/cover.jpg");
    assert_eq!(cover.mime_type, "image/jpeg");
    assert_eq!(cover.data, dummy_cover_bytes);

    let ch1_text = extract_epub_chapter_text(&epub_bytes, "OEBPS/text/ch1.xhtml").unwrap();
    assert!(ch1_text.contains("Zero copy streaming is fast."));
}

#[test]
fn test_pdf_streaming_parser() {
    use lopdf::{dictionary, Document, Object, Stream};

    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
    let resources_id = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font_id } });
    let content = Stream::new(dictionary! {}, b"BT /F1 24 Tf 100 700 Td (Pure Rust PDF Stream Parsing) Tj ET".to_vec());
    let content_id = doc.add_object(content);
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id, "Contents" => content_id, "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1 }));
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);

    let info_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal("TTZip PDF Architecture"),
        "Author" => Object::string_literal("Witt Kung"),
        "Subject" => Object::string_literal("In-Memory PDF Parsing"),
        "Creator" => Object::string_literal("TTZip Engine v1.0"),
    });
    doc.trailer.set("Info", info_id);

    let mut pdf_bytes = Vec::new();
    doc.save_to(&mut pdf_bytes).unwrap();

    let info = parse_pdf_from_memory(&pdf_bytes, Some(5)).expect("Failed to parse PDF");
    assert_eq!(info.format_version, "PDF-1.7");
    assert_eq!(info.page_count, 1);
    assert_eq!(info.title.as_deref(), Some("TTZip PDF Architecture"));
    assert_eq!(info.author.as_deref(), Some("Witt Kung"));
    assert_eq!(info.subject.as_deref(), Some("In-Memory PDF Parsing"));
    assert_eq!(info.creator.as_deref(), Some("TTZip Engine v1.0"));
    assert!(!info.is_encrypted);

    if let Some(txt) = info.extracted_text {
        assert!(txt.contains("Pure Rust PDF Stream Parsing"));
    }
}

#[test]
fn test_pdf_utf16_be_metadata() {
    use lopdf::{dictionary, Document, Object};

    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => Vec::<Object>::new(), "Count" => 0 }));
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);

    // UTF-16BE bytes for "文档标题" (0x6587, 0x6863, 0x6807, 0x9898)
    let utf16_title_bytes = vec![0xFE, 0xFF, 0x65, 0x87, 0x68, 0x63, 0x68, 0x07, 0x98, 0x98];
    let info_id = doc.add_object(dictionary! {
        "Title" => Object::String(utf16_title_bytes, lopdf::StringFormat::Literal),
        "Author" => Object::string_literal("Tung"),
    });
    doc.trailer.set("Info", info_id);

    let mut pdf_bytes = Vec::new();
    doc.save_to(&mut pdf_bytes).unwrap();

    let info = parse_pdf_from_memory(&pdf_bytes, None).expect("Parse UTF-16 PDF failed");
    assert_eq!(info.title.as_deref(), Some("文档标题"));
    assert_eq!(info.author.as_deref(), Some("Tung"));
}

#[test]
fn test_docx_throughput_and_special_elements() {
    let mut big_xml = String::with_capacity(2 * 1024 * 1024);
    big_xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#);
    for i in 0..5000 {
        big_xml.push_str(&format!(
            r#"<w:p><w:r><w:t>Paragraph {} with sample text and data.</w:t><w:tab/><w:t>Second run.</w:t><w:br/></w:r></w:p>"#,
            i
        ));
    }
    big_xml.push_str("</w:body></w:document>");

    let start = std::time::Instant::now();
    let (full_text, paragraphs) = parse_docx_xml_content(big_xml.as_bytes()).unwrap();
    let elapsed = start.elapsed();

    assert_eq!(paragraphs.len(), 5000);
    assert!(full_text.len() > 100_000);

    let mb = big_xml.len() as f64 / (1024.0 * 1024.0);
    let throughput = mb / elapsed.as_secs_f64();
    println!("DOCX SAX parsing throughput: {:.2} MB/s", throughput);
    assert!(throughput > 100.0);
}

#[test]
fn test_epub_fallback_scan() {
    let opf_xml = br#"<?xml version="1.0" encoding="utf-8"?>
<package version="2.0" xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Fallback Book</dc:title>
  </metadata>
  <manifest>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#;

    let ch1_content = b"<html><body><p>Direct chapter content.</p></body></html>";

    let epub_bytes = build_test_zip(&[
        ("book.opf", opf_xml),
        ("ch1.xhtml", ch1_content),
    ]);

    let book = parse_epub_from_memory(&epub_bytes).expect("Failed fallback EPUB parse");
    assert_eq!(book.metadata.title, "Fallback Book");
    assert_eq!(book.chapters.len(), 1);
    assert_eq!(book.chapters[0].href, "ch1.xhtml");
}
