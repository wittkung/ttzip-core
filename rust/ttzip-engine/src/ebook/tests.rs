// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit test suite for TTZip E-book Microkernel.

use crate::crypto::crc32::crc32_fast;
use crate::ebook::mobi::{decompress_palmdoc_record, EbookMobiDecoder};
use crate::ebook::navigation::{EbookNavigationExtractor, SpineItem};
use crate::ebook::parser::{EbookFormat, TTZipEbookParser};
use crate::ebook::resource::{clean_container_path, normalize_path, strip_fragment};

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
        zip_data.extend_from_slice(&lfh_offset.to_le_bytes()); // lfh offset
        zip_data.extend_from_slice(name_bytes);
    }

    let cd_size = (zip_data.len() as u32) - cd_offset;

    // End of central directory record (22 bytes)
    zip_data.extend_from_slice(&0x06054b50u32.to_le_bytes()); // magic
    zip_data.extend_from_slice(&0u16.to_le_bytes()); // disk number
    zip_data.extend_from_slice(&0u16.to_le_bytes()); // cd disk
    zip_data.extend_from_slice(&(cd_entries.len() as u16).to_le_bytes()); // entries this disk
    zip_data.extend_from_slice(&(cd_entries.len() as u16).to_le_bytes()); // total entries
    zip_data.extend_from_slice(&cd_size.to_le_bytes()); // cd size
    zip_data.extend_from_slice(&cd_offset.to_le_bytes()); // cd offset
    zip_data.extend_from_slice(&0u16.to_le_bytes()); // comment len

    zip_data
}

/// Helper to build a synthetic MOBI / AZW3 file buffer.
fn create_test_mobi_bytes(
    title: &str,
    author: &str,
    publisher: &str,
    asin: &str,
    text_content: &[u8],
    is_kf8: bool,
    has_cover: bool,
) -> Vec<u8> {
    let mut file_buf = Vec::new();

    // 1. Build Record 0
    let mut rec0 = Vec::new();

    // PalmDOC header (16 bytes)
    rec0.extend_from_slice(&1u16.to_be_bytes()); // compression = 1 (none)
    rec0.extend_from_slice(&0u16.to_be_bytes()); // unused
    rec0.extend_from_slice(&(text_content.len() as u32).to_be_bytes()); // text_length
    rec0.extend_from_slice(&1u16.to_be_bytes()); // record_count = 1
    rec0.extend_from_slice(&4096u16.to_be_bytes()); // record_size = 4096
    rec0.extend_from_slice(&0u32.to_be_bytes()); // current_position

    // MOBI header (starts at offset 16)
    let mobi_header_len = 232u32;
    let file_version = if is_kf8 { 8u32 } else { 6u32 };

    let mut mobi_hdr = vec![0u8; mobi_header_len as usize];
    mobi_hdr[0..4].copy_from_slice(b"MOBI");
    mobi_hdr[4..8].copy_from_slice(&mobi_header_len.to_be_bytes());
    mobi_hdr[8..12].copy_from_slice(&2u32.to_be_bytes()); // mobi_type = 2 (book)
    mobi_hdr[12..16].copy_from_slice(&65001u32.to_be_bytes()); // text_encoding = UTF-8
    mobi_hdr[16..20].copy_from_slice(&12345u32.to_be_bytes()); // unique_id
    mobi_hdr[20..24].copy_from_slice(&file_version.to_be_bytes()); // file_version

    // full_name offset & len
    let title_bytes = title.as_bytes();
    // full_name will be placed after EXTH
    let first_image_index = 2u32; // rec 0 is header, rec 1 is text, rec 2 is cover image
    mobi_hdr[92..96].copy_from_slice(&first_image_index.to_be_bytes()); // first_image_index at mobi_hdr[92..96] (offset 108 in rec0)
    mobi_hdr[112..116].copy_from_slice(&0x40u32.to_be_bytes()); // exth_flags at mobi_hdr[112..116] (offset 128 in rec0)

    rec0.extend_from_slice(&mobi_hdr);

    // EXTH header
    let mut exth_records = Vec::new();
    // tag 100: author
    exth_records.push((100u32, author.as_bytes().to_vec()));
    // tag 101: publisher
    exth_records.push((101u32, publisher.as_bytes().to_vec()));
    // tag 113: asin
    exth_records.push((113u32, asin.as_bytes().to_vec()));
    // tag 503: updated title
    exth_records.push((503u32, title.as_bytes().to_vec()));
    if is_kf8 {
        exth_records.push((121u32, vec![0, 0, 0, 0])); // KF8 boundary offset
    }
    if has_cover {
        exth_records.push((201u32, 0u32.to_be_bytes().to_vec())); // cover offset = 0
    }

    let mut exth_body = Vec::new();
    for (tag, data) in &exth_records {
        let rec_len = (8 + data.len()) as u32;
        exth_body.extend_from_slice(&tag.to_be_bytes());
        exth_body.extend_from_slice(&rec_len.to_be_bytes());
        exth_body.extend_from_slice(data);
    }

    // Pad exth_body to 4 bytes boundary
    while exth_body.len() % 4 != 0 {
        exth_body.push(0);
    }

    let total_exth_len = (12 + exth_body.len()) as u32;
    let mut exth_hdr = Vec::new();
    exth_hdr.extend_from_slice(b"EXTH");
    exth_hdr.extend_from_slice(&total_exth_len.to_be_bytes());
    exth_hdr.extend_from_slice(&(exth_records.len() as u32).to_be_bytes());
    exth_hdr.extend_from_slice(&exth_body);

    rec0.extend_from_slice(&exth_hdr);

    // Append full name at the end of Rec 0
    let title_offset = rec0.len() as u32;
    rec0.extend_from_slice(title_bytes);

    // Update full_name_offset (offset 84 of rec0) and full_name_length (offset 88 of rec0)
    rec0[84..88].copy_from_slice(&title_offset.to_be_bytes());
    rec0[88..92].copy_from_slice(&(title_bytes.len() as u32).to_be_bytes());

    // Record 1: Text content
    let rec1 = text_content.to_vec();

    // Record 2: Cover image (if present)
    let rec2 = if has_cover {
        vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46] // JPEG magic
    } else {
        Vec::new()
    };

    // Calculate PDB layout
    let num_records = if has_cover { 3u16 } else { 2u16 };
    let header_size = 78 + (num_records as usize) * 8 + 2; // +2 padding

    let rec0_offset = header_size as u32;
    let rec1_offset = rec0_offset + (rec0.len() as u32);
    let rec2_offset = rec1_offset + (rec1.len() as u32);

    // PDB Header (78 bytes)
    let mut pdb_name = [0u8; 32];
    let name_slice = title.as_bytes();
    let copy_len = name_slice.len().min(31);
    pdb_name[..copy_len].copy_from_slice(&name_slice[..copy_len]);

    file_buf.extend_from_slice(&pdb_name);
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
    file_buf.push(0); // attributes
    file_buf.extend_from_slice(&[0, 0, 0]); // unique ID

    // Record 1 entry
    file_buf.extend_from_slice(&rec1_offset.to_be_bytes());
    file_buf.push(0);
    file_buf.extend_from_slice(&[0, 0, 1]);

    if has_cover {
        // Record 2 entry
        file_buf.extend_from_slice(&rec2_offset.to_be_bytes());
        file_buf.push(0);
        file_buf.extend_from_slice(&[0, 0, 2]);
    }

    file_buf.extend_from_slice(&[0, 0]); // padding

    file_buf.extend_from_slice(&rec0);
    file_buf.extend_from_slice(&rec1);
    if has_cover {
        file_buf.extend_from_slice(&rec2);
    }

    file_buf
}

#[test]
fn test_palmdoc_lz77_decompression_all_branches() {
    // 1. Literal 0x00
    let input1 = [0x00];
    let res1 = decompress_palmdoc_record(&input1, 4096).expect("LZ77 dec");
    assert_eq!(res1, vec![0x00]);

    // 2. Sequence of literals (0x01..0x08)
    let input2 = [0x04, b'W', b'i', b't', b't'];
    let res2 = decompress_palmdoc_record(&input2, 4096).expect("LZ77 dec");
    assert_eq!(res2, b"Witt");

    // 3. Single literals (0x09..0x7F)
    let input3 = [b'A', b'B', b'C'];
    let res3 = decompress_palmdoc_record(&input3, 4096).expect("LZ77 dec");
    assert_eq!(res3, b"ABC");

    // 4. Space + ASCII char (0xC0..0xFF)
    // 0xC0 | 'K' (0x4B) = 0xCB -> output " K"
    let input4 = [0xCB];
    let res4 = decompress_palmdoc_record(&input4, 4096).expect("LZ77 dec");
    assert_eq!(res4, b" K");

    // 5. Sliding window back reference:
    // First push "ABCD", then reference distance = 4, length = 4
    // distance = 4: (distance >> 3) = 0 -> b0 = 0x80 | 0 = 0x80
    // (distance & 7) << 5 = 4 << 5 = 0x80. length = 4 -> length - 3 = 1 -> b1 = 0x80 | 1 = 0x81
    let input5 = [b'A', b'B', b'C', b'D', 0x80, 0x81];
    let res5 = decompress_palmdoc_record(&input5, 4096).expect("LZ77 dec");
    assert_eq!(res5, b"ABCDABCD");

    // 6. Sliding window with overlapping run-length repetition (e.g. "A" followed by distance = 1, length = 5)
    // distance = 1: b0 = 0x80 | 0 = 0x80
    // b1 = (1 << 5) | (5 - 3) = 0x20 | 2 = 0x22
    let input6 = [b'A', 0x80, 0x22];
    let res6 = decompress_palmdoc_record(&input6, 4096).expect("LZ77 dec");
    assert_eq!(res6, b"AAAAAA");
}

#[test]
fn test_palmdoc_lz77_error_handling() {
    // Distance greater than buffer size
    let bad_distance = [b'A', 0x81, 0x00]; // distance >> 1
    assert!(decompress_palmdoc_record(&bad_distance, 4096).is_err());

    // Unexpected EOF in literal sequence
    let bad_seq = [0x05, b'A', b'B'];
    assert!(decompress_palmdoc_record(&bad_seq, 4096).is_err());
}

#[test]
fn test_mobi_parser_and_metadata() {
    let text = b"Welcome to TTZip pure safe Rust e-book microkernel engine.";
    let mobi_bytes = create_test_mobi_bytes(
        "TTZip Architecture Guide",
        "Witt Kung",
        "DeepMind Press",
        "B001234567",
        text,
        false,
        true,
    );

    let decoder = EbookMobiDecoder::parse(&mobi_bytes).expect("Failed to parse MOBI");
    assert!(!decoder.is_azw3());
    assert_eq!(decoder.title().as_deref(), Some("TTZip Architecture Guide"));
    assert_eq!(decoder.authors(), vec!["Witt Kung".to_string()]);
    assert_eq!(decoder.publisher().as_deref(), Some("DeepMind Press"));
    assert_eq!(decoder.asin_or_isbn().as_deref(), Some("B001234567"));
    assert!(decoder.extract_cover_image().is_some());

    let full_text = decoder.extract_full_text().expect("extract full text");
    assert!(full_text.contains("TTZip pure safe Rust"));

    // Also test through TTZipEbookParser facade
    let parser = TTZipEbookParser::open_from_bytes(&mobi_bytes).expect("open_from_bytes");
    assert_eq!(parser.format(), EbookFormat::Mobi);
    assert_eq!(parser.metadata().title.as_deref(), Some("TTZip Architecture Guide"));
    assert_eq!(parser.metadata().authors, vec!["Witt Kung".to_string()]);
    assert!(parser.extract_cover().expect("cover").is_some());
}

#[test]
fn test_azw3_kf8_detection() {
    let text = b"<html><body><h1>KF8 Format Chapter</h1></body></html>";
    let azw3_bytes = create_test_mobi_bytes(
        "Modern KF8 Handbook",
        "Witt Kung",
        "TTZip Publishing",
        "B00KF8AZW3",
        text,
        true,
        false,
    );

    let parser = TTZipEbookParser::open_from_bytes(&azw3_bytes).expect("open azw3");
    assert_eq!(parser.format(), EbookFormat::Azw3);
    assert_eq!(parser.metadata().title.as_deref(), Some("Modern KF8 Handbook"));
}

#[test]
fn test_epub2_parsing_and_ncx_toc() {
    let container_xml = br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;

    let content_opf = br#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Mastering Microkernels</dc:title>
    <dc:creator>Witt Kung</dc:creator>
    <dc:publisher>TTZip Labs</dc:publisher>
    <dc:language>en</dc:language>
    <dc:identifier id="BookId">urn:uuid:12345-67890</dc:identifier>
    <meta name="cover" content="cover-img"/>
  </metadata>
  <manifest>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="cover-img" href="images/cover.jpg" media-type="image/jpeg"/>
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
    <navPoint id="navPoint-1" playOrder="1">
      <navLabel><text>Chapter 1: Foundations</text></navLabel>
      <content src="text/ch1.xhtml"/>
      <navPoint id="navPoint-1-1" playOrder="2">
        <navLabel><text>Section 1.1: Memory Bounds</text></navLabel>
        <content src="text/ch1.xhtml#sec1"/>
      </navPoint>
    </navPoint>
    <navPoint id="navPoint-2" playOrder="3">
      <navLabel><text>Chapter 2: Concurrency</text></navLabel>
      <content src="text/ch2.xhtml"/>
    </navPoint>
  </navMap>
</ncx>"#;

    let ch1_xhtml = br#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Chapter 1</title></head>
<body>
  <h1>Chapter 1: Foundations</h1>
  <p>Microkernels must enforce strict memory and concurrency isolation.</p>
  <h2 id="sec1">Section 1.1: Memory Bounds</h2>
  <p>Zeroize sensitive registers and bounds-check all allocations.</p>
</body>
</html>"#;

    let ch2_xhtml = br#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Chapter 2</title></head>
<body>
  <h1>Chapter 2: Concurrency</h1>
  <p>Pure message passing eliminates shared mutable state.</p>
</body>
</html>"#;

    let cover_jpg = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46];

    let zip_bytes = create_test_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", container_xml),
        ("OEBPS/content.opf", content_opf),
        ("OEBPS/toc.ncx", toc_ncx),
        ("OEBPS/text/ch1.xhtml", ch1_xhtml),
        ("OEBPS/text/ch2.xhtml", ch2_xhtml),
        ("OEBPS/images/cover.jpg", &cover_jpg),
    ]);

    let parser = TTZipEbookParser::open_from_bytes(&zip_bytes).expect("open EPUB 2");
    assert_eq!(parser.format(), EbookFormat::Epub2);
    assert_eq!(parser.metadata().title.as_deref(), Some("Mastering Microkernels"));
    assert_eq!(parser.metadata().authors, vec!["Witt Kung".to_string()]);
    assert_eq!(parser.metadata().language.as_deref(), Some("en"));
    assert_eq!(parser.metadata().cover_image_href.as_deref(), Some("OEBPS/images/cover.jpg"));

    // Check Spine
    assert_eq!(parser.spine().len(), 2);
    assert_eq!(parser.spine()[0].idref, "ch1");
    assert_eq!(parser.spine()[0].href, "OEBPS/text/ch1.xhtml");
    assert_eq!(parser.spine()[1].idref, "ch2");
    assert_eq!(parser.spine()[1].href, "OEBPS/text/ch2.xhtml");

    // Check TOC
    assert_eq!(parser.toc().len(), 2);
    assert_eq!(parser.toc()[0].title, "Chapter 1: Foundations");
    assert_eq!(parser.toc()[0].target_index, Some(0));
    assert_eq!(parser.toc()[0].children.len(), 1);
    assert_eq!(parser.toc()[0].children[0].title, "Section 1.1: Memory Bounds");
    assert_eq!(parser.toc()[0].children[0].target_index, Some(0));
    assert_eq!(parser.toc()[1].title, "Chapter 2: Concurrency");
    assert_eq!(parser.toc()[1].target_index, Some(1));

    // Check Chapter and Resource Extraction
    let ch1_text = parser.extract_chapter_text(0).expect("extract ch1");
    assert!(ch1_text.contains("Microkernels must enforce strict memory"));

    let cover_res = parser.extract_cover().expect("cover extraction").expect("cover exists");
    assert_eq!(cover_res.path, "OEBPS/images/cover.jpg");
    assert_eq!(cover_res.data, cover_jpg);

    let full_text = parser.extract_full_text().expect("extract full text");
    assert!(full_text.contains("Chapter 1: Foundations"));
    assert!(full_text.contains("Chapter 2: Concurrency"));
}

#[test]
fn test_epub3_parsing_and_nav_xhtml() {
    let container_xml = br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="EPUB/package.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;

    let package_opf = br#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="pub-id" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>EPUB 3 NextGen Specs</dc:title>
    <dc:creator>Witt Kung</dc:creator>
    <dc:language>zh-CN</dc:language>
    <dc:identifier id="pub-id">urn:isbn:9781234567890</dc:identifier>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/>
    <item id="c2" href="c2.xhtml" media-type="application/xhtml+xml"/>
    <item id="cover" href="cover.png" media-type="image/png" properties="cover-image"/>
  </manifest>
  <spine>
    <itemref idref="c1"/>
    <itemref idref="c2"/>
  </spine>
</package>"#;

    let nav_xhtml = br#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Navigation</title></head>
<body>
  <nav epub:type="toc" id="toc">
    <h1>Table of Contents</h1>
    <ol>
      <li><a href="c1.xhtml">Chapter I - The Beginning</a>
        <ol>
          <li><a href="c1.xhtml#part1">Part A</a></li>
        </ol>
      </li>
      <li><a href="c2.xhtml">Chapter II - The Culmination</a></li>
    </ol>
  </nav>
</body>
</html>"#;

    let c1_xhtml = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>Chapter One Text</p></body></html>"#;
    let c2_xhtml = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>Chapter Two Text</p></body></html>"#;
    let cover_png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

    let zip_bytes = create_test_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", container_xml),
        ("EPUB/package.opf", package_opf),
        ("EPUB/nav.xhtml", nav_xhtml),
        ("EPUB/c1.xhtml", c1_xhtml),
        ("EPUB/c2.xhtml", c2_xhtml),
        ("EPUB/cover.png", &cover_png),
    ]);

    let parser = TTZipEbookParser::open_from_bytes(&zip_bytes).expect("open EPUB 3");
    assert_eq!(parser.format(), EbookFormat::Epub3);
    assert_eq!(parser.metadata().title.as_deref(), Some("EPUB 3 NextGen Specs"));
    assert_eq!(parser.metadata().cover_image_href.as_deref(), Some("EPUB/cover.png"));

    // Check TOC
    assert_eq!(parser.toc().len(), 2);
    assert_eq!(parser.toc()[0].title, "Chapter I - The Beginning");
    assert_eq!(parser.toc()[0].target_index, Some(0));
    assert_eq!(parser.toc()[0].children.len(), 1);
    assert_eq!(parser.toc()[0].children[0].title, "Part A");
    assert_eq!(parser.toc()[1].title, "Chapter II - The Culmination");
    assert_eq!(parser.toc()[1].target_index, Some(1));
}

#[test]
fn test_path_normalization_and_stripping() {
    assert_eq!(normalize_path("OEBPS/text", "../images/cover.jpg"), "OEBPS/images/cover.jpg");
    assert_eq!(normalize_path("OEBPS/text", "ch1.xhtml#sec1"), "OEBPS/text/ch1.xhtml#sec1");
    assert_eq!(normalize_path("", "intro.xhtml"), "intro.xhtml");
    assert_eq!(clean_container_path("/OEBPS/./text/../style.css"), "OEBPS/style.css");
    assert_eq!(strip_fragment("text/ch1.xhtml#anchor?query=1"), "text/ch1.xhtml");
}

#[test]
fn test_unsupported_format() {
    let dummy = [1, 2, 3, 4, 5, 6, 7, 8];
    assert!(TTZipEbookParser::open_from_bytes(&dummy).is_err());
}

#[test]
fn test_direct_navigation_and_resource_extractors() {
    let ncx_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/">
  <navMap>
    <navPoint id="p1">
      <navLabel><text>Prologue</text></navLabel>
      <content src="text/prologue.xhtml"/>
    </navPoint>
  </navMap>
</ncx>"#;

    let spine = vec![SpineItem {
        idref: "prologue".to_string(),
        href: "OEBPS/text/prologue.xhtml".to_string(),
        linear: true,
        media_type: "application/xhtml+xml".to_string(),
    }];

    let nodes = EbookNavigationExtractor::parse_ncx(ncx_xml, "OEBPS", &spine).expect("parse ncx");
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].title, "Prologue");
    assert_eq!(nodes[0].href, "OEBPS/text/prologue.xhtml");
    assert_eq!(nodes[0].target_index, Some(0));

    let zip_bytes = create_test_zip(&[("OEBPS/text/prologue.xhtml", b"<html><body>Hello</body></html>")]);
    let zip = crate::zip::ZipArchive::open_slice(&zip_bytes).expect("open zip");
    let text = crate::ebook::resource::EbookResourceExtractor::extract_text(&zip, "OEBPS/text/prologue.xhtml").expect("extract text");
    assert_eq!(text, "<html><body>Hello</body></html>");
}


