// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use lopdf::{dictionary, Document, Object, Stream, StringFormat};

use crate::pdf::*;

    /// Helper to construct a synthetic in-memory PDF document for test scenarios.
    fn create_synthetic_pdf(
        page_texts: &[&str],
        info: Option<lopdf::Dictionary>,
        outlines: bool,
    ) -> Vec<u8> {
        let mut doc = Document::with_version("1.7");

        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
            "Encoding" => "WinAnsiEncoding",
        });

        let mut page_ids = Vec::new();

        for text in page_texts {
            // Build content stream: BT /F1 12 Tf 50 700 Td (text) Tj ET
            let stream_content = format!("BT\n/F1 12 Tf\n50 700 Td\n({}) Tj\nET\n", text);
            let content_id = doc.add_object(Stream::new(
                dictionary! {},
                stream_content.into_bytes(),
            ));

            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                "Contents" => content_id,
                "Resources" => dictionary! {
                    "Font" => dictionary! {
                        "F1" => font_id,
                    },
                },
            });
            page_ids.push(page_id);
        }

        let kids: Vec<Object> = page_ids.iter().map(|id| Object::Reference(*id)).collect();
        doc.set_object(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => kids,
                "Count" => page_ids.len() as i64,
            }),
        );

        let mut catalog_dict = dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        };

        // Construct Outlines if requested
        if outlines && !page_ids.is_empty() {
            let outlines_id = doc.new_object_id();
            let item1_id = doc.new_object_id();
            let item2_id = doc.new_object_id();
            let subitem1_id = doc.new_object_id();

            // Root outlines
            doc.set_object(
                outlines_id,
                Object::Dictionary(dictionary! {
                    "Type" => "Outlines",
                    "First" => item1_id,
                    "Last" => item2_id,
                    "Count" => 3,
                }),
            );

            // Item 1 (has subitem1)
            doc.set_object(
                item1_id,
                Object::Dictionary(dictionary! {
                    "Title" => Object::String(b"Chapter 1".to_vec(), StringFormat::Literal),
                    "Parent" => outlines_id,
                    "Next" => item2_id,
                    "First" => subitem1_id,
                    "Last" => subitem1_id,
                    "Count" => 1,
                    "Dest" => vec![Object::Reference(page_ids[0]), Object::Name(b"Fit".to_vec())],
                    "F" => 2, // Bold
                }),
            );

            // Sub-item 1 under Item 1
            doc.set_object(
                subitem1_id,
                Object::Dictionary(dictionary! {
                    "Title" => Object::String(b"Section 1.1".to_vec(), StringFormat::Literal),
                    "Parent" => item1_id,
                    "Dest" => vec![Object::Reference(page_ids[0]), Object::Name(b"XYZ".to_vec()), 50.into(), 700.into(), 1.into()],
                }),
            );

            // Item 2
            let target_page_2 = if page_ids.len() > 1 { page_ids[1] } else { page_ids[0] };
            doc.set_object(
                item2_id,
                Object::Dictionary(dictionary! {
                    "Title" => Object::String(b"Chapter 2".to_vec(), StringFormat::Literal),
                    "Parent" => outlines_id,
                    "Prev" => item1_id,
                    "Dest" => vec![Object::Reference(target_page_2), Object::Name(b"Fit".to_vec())],
                    "C" => vec![1.0.into(), 0.0.into(), 0.0.into()], // Red
                }),
            );

            catalog_dict.set("Outlines", outlines_id);
        }

        let catalog_id = doc.add_object(catalog_dict);
        doc.trailer.set("Root", catalog_id);

        if let Some(info_dict) = info {
            let info_id = doc.add_object(info_dict);
            doc.trailer.set("Info", info_id);
        }

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn test_pdf_parser_basic_and_geometry() {
        let pdf_bytes = create_synthetic_pdf(&["Hello TTZip Core Engine!"], None, false);
        let parser = TTZipPdfParser::open_from_bytes(&pdf_bytes).unwrap();

        assert_eq!(parser.version(), "1.7");
        assert!(!parser.is_encrypted());
        assert_eq!(parser.page_count(), 1);

        let page_info = parser.get_page_info(1).unwrap();
        assert_eq!(page_info.page_number, 1);
        assert_eq!(page_info.media_box, Some([0.0, 0.0, 612.0, 792.0]));
        assert_eq!(page_info.rotation, 0);
        assert!(!page_info.has_annotations);
        assert!(page_info.content_stream_size > 0);

        // Out of bounds page lookup error
        assert!(matches!(
            parser.get_page_info(99),
            Err(PdfError::PageOutOfBounds(99, 1))
        ));
    }

    #[test]
    fn test_pdf_string_decoding_variants() {
        // UTF-16BE with BOM
        let utf16be = vec![0xFE, 0xFF, 0x00, 0x54, 0x00, 0x54, 0x00, 0x5A, 0x00, 0x69, 0x00, 0x70];
        assert_eq!(TTZipPdfParser::decode_pdf_string(&utf16be), "TTZip");

        // UTF-16LE with BOM
        let utf16le = vec![0xFF, 0xFE, 0x54, 0x00, 0x54, 0x00, 0x5A, 0x00, 0x69, 0x00, 0x70, 0x00];
        assert_eq!(TTZipPdfParser::decode_pdf_string(&utf16le), "TTZip");

        // Plain UTF-8
        let utf8_bytes = "Architecture 架构".as_bytes();
        assert_eq!(TTZipPdfParser::decode_pdf_string(utf8_bytes), "Architecture 架构");

        // Empty bytes
        assert_eq!(TTZipPdfParser::decode_pdf_string(&[]), "");
    }

    #[test]
    fn test_pdf_outline_hierarchy_and_flattening() {
        let pdf_bytes = create_synthetic_pdf(
            &["Page 1 Content", "Page 2 Content"],
            None,
            true, // Enable outlines
        );
        let parser = TTZipPdfParser::open_from_bytes(&pdf_bytes).unwrap();
        assert_eq!(parser.page_count(), 2);

        let outlines = PdfOutlineExtractor::extract_outlines(&parser).unwrap();
        assert_eq!(outlines.len(), 2);

        // Chapter 1
        assert_eq!(outlines[0].title, "Chapter 1");
        assert_eq!(outlines[0].level, 0);
        assert_eq!(outlines[0].page_number, Some(1));
        assert!(outlines[0].is_bold);
        assert!(!outlines[0].is_italic);
        assert_eq!(outlines[0].children.len(), 1);

        // Section 1.1 (Child of Chapter 1)
        let sub = &outlines[0].children[0];
        assert_eq!(sub.title, "Section 1.1");
        assert_eq!(sub.level, 1);
        assert_eq!(sub.page_number, Some(1));
        assert!(matches!(
            sub.destination,
            PdfDestination::FitCoordinates {
                page: 1,
                x: Some(50.0),
                y: Some(700.0),
                zoom: Some(1.0)
            }
        ));

        // Chapter 2
        assert_eq!(outlines[1].title, "Chapter 2");
        assert_eq!(outlines[1].level, 0);
        assert_eq!(outlines[1].page_number, Some(2));
        assert_eq!(outlines[1].color_rgb, Some([1.0, 0.0, 0.0]));

        // Test Flattening
        let flat = PdfOutlineExtractor::flatten_outlines(&outlines);
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].title, "Chapter 1");
        assert!(flat[0].has_children);
        assert_eq!(flat[1].title, "Section 1.1");
        assert!(!flat[1].has_children);
        assert_eq!(flat[2].title, "Chapter 2");
        assert!(!flat[2].has_children);
    }

    #[test]
    fn test_tounicode_cmap_parsing_and_decoding() {
        let cmap_postscript = r#"
/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def
/CMapName /Custom-ToUnicode def
/CMapType 2 def
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
2 beginbfchar
<0001> <0048>
<0002> <0069>
endbfchar
1 beginbfrange
<0003> <0005> <0041>
endbfrange
endcmap
CMapName currentdict /CMap defineresource pop
end
end
"#;

        let cmap = ToUnicodeCMap::parse_from_bytes(cmap_postscript.as_bytes());

        // Test single bfchar decoding: 0x0001 -> 'H', 0x0002 -> 'i'
        let decoded_hi = cmap.decode_bytes(&[0x00, 0x01, 0x00, 0x02]);
        assert_eq!(decoded_hi, "Hi");

        // Test bfrange decoding: 0x0003 -> 'A', 0x0004 -> 'B', 0x0005 -> 'C'
        let decoded_abc = cmap.decode_bytes(&[0x00, 0x03, 0x00, 0x04, 0x00, 0x05]);
        assert_eq!(decoded_abc, "ABC");
    }

    #[test]
    fn test_pdf_text_extraction_and_search_highlighting() {
        let pdf_bytes = create_synthetic_pdf(
            &[
                "The fast and modern TTZip archiving engine delivers ultra throughput.",
                "Zero-allocation streaming and high precision search indexing.",
            ],
            None,
            false,
        );

        let parser = TTZipPdfParser::open_from_bytes(&pdf_bytes).unwrap();
        assert_eq!(parser.page_count(), 2);

        // Extract page text
        let page1_text = PdfTextExtractor::extract_page_text(&parser, 1).unwrap();
        assert!(page1_text.contains("TTZip archiving engine"));

        let page2_text = PdfTextExtractor::extract_page_text(&parser, 2).unwrap();
        assert!(page2_text.contains("search indexing"));

        // Extract all text
        let all_text = PdfTextExtractor::extract_all_text(&parser).unwrap();
        assert!(all_text.contains("TTZip"));
        assert!(all_text.contains("streaming"));

        // Search text case-insensitive
        let search_opts = PdfTextSearchOptions {
            case_sensitive: false,
            whole_word: false,
            max_results: None,
            context_padding: 15,
        };

        let result = PdfTextExtractor::search_text(&parser, "ttzip", &search_opts).unwrap();
        assert_eq!(result.total_matches, 1);
        assert_eq!(result.matches[0].page_number, 1);
        assert_eq!(result.matches[0].line_number, 1);
        assert!(result.matches[0].context_snippet.contains("[TTZip]"));

        // Search case-sensitive negative match
        let sensitive_opts = PdfTextSearchOptions {
            case_sensitive: true,
            ..Default::default()
        };
        let sensitive_res = PdfTextExtractor::search_text(&parser, "ttzip", &sensitive_opts).unwrap();
        assert_eq!(sensitive_res.total_matches, 0);

        // Whole word search
        let word_opts = PdfTextSearchOptions {
            whole_word: true,
            ..Default::default()
        };
        let word_res = PdfTextExtractor::search_text(&parser, "archiving", &word_opts).unwrap();
        assert_eq!(word_res.total_matches, 1);
    }

    #[test]
    fn test_pdf_metadata_extraction_info_and_xmp() {
        let info_dict = dictionary! {
            "Title" => Object::String(b"TTZip Whitepaper".to_vec(), StringFormat::Literal),
            "Author" => Object::String(b"Witt Kung".to_vec(), StringFormat::Literal),
            "Subject" => Object::String(b"High Performance Compression".to_vec(), StringFormat::Literal),
            "Keywords" => Object::String(b"Rust, Archiving, Zero-Copy, UniFFI".to_vec(), StringFormat::Literal),
            "Creator" => Object::String(b"TTZip Generator 1.0".to_vec(), StringFormat::Literal),
            "Producer" => Object::String(b"lopdf pure safe rust".to_vec(), StringFormat::Literal),
            "CreationDate" => Object::String(b"D:20260901134816+08'00'".to_vec(), StringFormat::Literal),
        };

        let pdf_bytes = create_synthetic_pdf(&["Document Body"], Some(info_dict), false);
        let parser = TTZipPdfParser::open_from_bytes(&pdf_bytes).unwrap();

        let meta = PdfMetadataExtractor::extract_metadata(&parser).unwrap();
        assert_eq!(meta.title.as_deref(), Some("TTZip Whitepaper"));
        assert_eq!(meta.author.as_deref(), Some("Witt Kung"));
        assert_eq!(meta.subject.as_deref(), Some("High Performance Compression"));
        assert_eq!(
            meta.keywords,
            vec!["Rust", "Archiving", "Zero-Copy", "UniFFI"]
        );
        assert_eq!(meta.creator.as_deref(), Some("TTZip Generator 1.0"));
        assert_eq!(meta.producer.as_deref(), Some("lopdf pure safe rust"));
        assert_eq!(
            meta.creation_date.as_deref(),
            Some("2026-09-01T13:48:16+08:00")
        );
        assert_eq!(meta.page_count, 1);
        assert_eq!(meta.pdf_version, "PDF-1.7");
    }

    #[test]
    fn test_xmp_xml_parsing_direct() {
        let xmp_xml = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
        xmlns:dc="http://purl.org/dc/elements/1.1/"
        xmlns:xmp="http://ns.adobe.com/xap/1.0/"
        xmlns:pdf="http://ns.adobe.com/pdf/1.3/"
        xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
      <dc:title>
        <rdf:Alt>
          <rdf:li xml:lang="x-default">Advanced System Architecture</rdf:li>
        </rdf:Alt>
      </dc:title>
      <dc:creator>
        <rdf:Seq>
          <rdf:li>DeepMind Team</rdf:li>
          <rdf:li>Witt Kung</rdf:li>
        </rdf:Seq>
      </dc:creator>
      <dc:description>
        <rdf:Alt>
          <rdf:li xml:lang="x-default">Next-generation microkernel technical report</rdf:li>
        </rdf:Alt>
      </dc:description>
      <dc:subject>
        <rdf:Bag>
          <rdf:li>Microkernel</rdf:li>
          <rdf:li>Safety</rdf:li>
        </rdf:Bag>
      </dc:subject>
      <dc:rights>
        <rdf:Alt>
          <rdf:li xml:lang="x-default">Copyright 2026</rdf:li>
        </rdf:Alt>
      </dc:rights>
      <xmp:CreateDate>2026-09-01T12:00:00Z</xmp:CreateDate>
      <xmp:ModifyDate>2026-09-01T13:00:00Z</xmp:ModifyDate>
      <xmp:CreatorTool>TTZip Engine 2.0</xmp:CreatorTool>
      <pdf:Producer>Pure Safe Rust PDF Pipeline</pdf:Producer>
      <pdf:Keywords>Safe, Fast, Deterministic</pdf:Keywords>
      <pdf:PDFVersion>1.7</pdf:PDFVersion>
      <pdfaid:part>1</pdfaid:part>
      <pdfaid:conformance>A</pdfaid:conformance>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;

        let xmp = PdfMetadataExtractor::parse_xmp_xml(xmp_xml).unwrap();
        assert_eq!(xmp.dc_title.as_deref(), Some("Advanced System Architecture"));
        assert_eq!(xmp.dc_creators, vec!["DeepMind Team", "Witt Kung"]);
        assert_eq!(
            xmp.dc_description.as_deref(),
            Some("Next-generation microkernel technical report")
        );
        assert_eq!(xmp.dc_subjects, vec!["Microkernel", "Safety"]);
        assert_eq!(xmp.dc_rights.as_deref(), Some("Copyright 2026"));
        assert_eq!(xmp.xmp_create_date.as_deref(), Some("2026-09-01T12:00:00Z"));
        assert_eq!(xmp.xmp_modify_date.as_deref(), Some("2026-09-01T13:00:00Z"));
        assert_eq!(xmp.xmp_creator_tool.as_deref(), Some("TTZip Engine 2.0"));
        assert_eq!(xmp.pdf_producer.as_deref(), Some("Pure Safe Rust PDF Pipeline"));
        assert_eq!(xmp.pdf_keywords.as_deref(), Some("Safe, Fast, Deterministic"));
        assert_eq!(xmp.pdf_version.as_deref(), Some("1.7"));
        assert_eq!(xmp.pdfa_part, Some(1));
        assert_eq!(xmp.pdfa_conformance.as_deref(), Some("A"));
    }
