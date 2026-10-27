// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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
    let pdf_bytes = b"%PDF-1.7\n\
1 0 obj\n\
<< /Type /Catalog /Pages 2 0 R >>\n\
endobj\n\
2 0 obj\n\
<< /Type /Pages /Kids [3 0 R] /Count 1 >>\n\
endobj\n\
3 0 obj\n\
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R >>\n\
endobj\n\
4 0 obj\n\
<< /Length 64 >>\n\
stream\n\
BT /F1 24 Tf 100 700 Td (Pure Rust PDF Stream Parsing) Tj ET\n\
endstream\n\
endobj\n\
5 0 obj\n\
<< /Title (TTZip PDF Architecture) /Author (Witt Kung) /Subject (In-Memory PDF Parsing) /Creator (TTZip Engine v1.0) >>\n\
endobj\n\
trailer\n\
<< /Root 1 0 R /Info 5 0 R >>\n\
startxref\n\
450\n\
%%EOF\n";

    let info = parse_pdf_from_memory(pdf_bytes, Some(5)).expect("Failed to parse PDF");
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
    let mut pdf_bytes = Vec::new();
    pdf_bytes.extend_from_slice(b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Count 1 >>\nendobj\n3 0 obj\n<< /Title (");
    pdf_bytes.extend_from_slice(&[0xFE, 0xFF, 0x65, 0x87, 0x68, 0x63, 0x68, 0x07, 0x98, 0x98]);
    pdf_bytes.extend_from_slice(b") /Author (Tung) >>\nendobj\ntrailer\n<< /Root 1 0 R /Info 3 0 R >>\n%%EOF\n");

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
    let min_throughput = if cfg!(debug_assertions) { 15.0 } else { 50.0 };
    assert!(throughput > min_throughput, "DOCX SAX throughput too low: {:.2} MB/s (min: {:.2} MB/s)", throughput, min_throughput);
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
