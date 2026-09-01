// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Export Layer for Ebook Metadata, Hierarchical TOC,
//! Spine Layout, Chapter Content Extraction, and Embedded Resources.

pub(crate) mod cbz;
pub(crate) mod epub;
pub(crate) mod helpers;
pub(crate) mod service;
pub mod types;

pub use service::{
    uniffi_extract_ebook_chapter, uniffi_extract_ebook_cover, uniffi_extract_ebook_metadata,
    uniffi_extract_ebook_resource, uniffi_extract_ebook_toc, uniffi_get_ebook_spine,
    uniffi_probe_ebook_bytes, UniFFIEbookService,
};
pub use types::{
    UniFFIEbookChapter, UniFFIEbookError, UniFFIEbookFormat, UniFFIEbookMetadata,
    UniFFIEbookResource, UniFFIEbookSpineItem, UniFFIEbookTocNode,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

    fn make_test_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
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
        .expect("zip compress");
        assemble_zip_archive(&compressed).expect("zip assemble")
    }

    fn make_synthetic_epub() -> Vec<u8> {
        let container_xml = br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="EPUB/package.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;

        let package_opf = br#"<?xml version="1.0" encoding="utf-8"?>
<package version="3.0" xmlns="http://www.idpf.org/2007/opf" unique-identifier="pub-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="pub-id">urn:uuid:12345-67890-abcdef</dc:identifier>
    <dc:title>TTZip High-Performance Systems Guide</dc:title>
    <dc:creator>Witt Kung</dc:creator>
    <dc:creator>Co-Author Name</dc:creator>
    <dc:publisher>TTZip Architecture Press</dc:publisher>
    <dc:language>en</dc:language>
    <dc:description>Comprehensive guide to zero-disk streaming archiving.</dc:description>
    <dc:date>2026-09-01</dc:date>
    <dc:rights>BSD-3-Clause OR Apache-2.0</dc:rights>
    <meta name="cover" content="cover-image-id"/>
    <meta property="dcterms:modified">2026-09-01T12:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="cover-image-id" href="images/cover.jpg" media-type="image/jpeg" properties="cover-image"/>
    <item id="style" href="styles/main.css" media-type="text/css"/>
    <item id="ch1" href="text/ch1.xhtml" media-type="application/xhtml+xml"/>
    <item id="ch2" href="text/ch2.xhtml" media-type="application/xhtml+xml"/>
    <item id="ch3" href="text/ch3.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine toc="ncx">
    <itemref idref="ch1" linear="yes"/>
    <itemref idref="ch2" linear="yes"/>
    <itemref idref="ch3" linear="no"/>
  </spine>
</package>"#;

        let toc_ncx = br#"<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <navMap>
    <navPoint id="np-1" playOrder="1">
      <navLabel><text>1. Microkernel Architecture</text></navLabel>
      <content src="text/ch1.xhtml"/>
      <navPoint id="np-1-1" playOrder="2">
        <navLabel><text>1.1 Memory Bounds</text></navLabel>
        <content src="text/ch1.xhtml#bounds"/>
      </navPoint>
    </navPoint>
    <navPoint id="np-2" playOrder="3">
      <navLabel><text>2. Streaming Pipelines</text></navLabel>
      <content src="text/ch2.xhtml"/>
    </navPoint>
  </navMap>
</ncx>"#;

        let nav_xhtml = br#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>TOC</title></head>
<body>
  <nav epub:type="toc">
    <ol>
      <li><a href="text/ch1.xhtml">1. Microkernel Architecture</a></li>
      <li><a href="text/ch2.xhtml">2. Streaming Pipelines</a></li>
    </ol>
  </nav>
</body>
</html>"#;

        let ch1_html = b"<!DOCTYPE html><html><head><title>Ch1</title></head><body><h1>1. Microkernel Architecture</h1><p id=\"bounds\">Memory bounds are strictly enforced to 64MB.</p></body></html>";
        let ch2_html = b"<!DOCTYPE html><html><head><title>Ch2</title></head><body><h1>2. Streaming Pipelines</h1><p>Throughput exceeds 600 MB/s.</p></body></html>";
        let ch3_html = b"<!DOCTYPE html><html><head><title>Ch3 Appendix</title></head><body><h1>3. Non-Linear Appendix</h1><p>Extra technical notes.</p></body></html>";
        let style_css = b"body { font-family: -apple-system, sans-serif; margin: 2rem; color: #1a1a1a; }";
        let dummy_cover_jpg = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x01\x00\x60\x00\x60\x00\x00\xFF\xDB\x00\x43\x00";

        make_test_zip(&[
            ("META-INF/container.xml", container_xml),
            ("EPUB/package.opf", package_opf),
            ("EPUB/toc.ncx", toc_ncx),
            ("EPUB/nav.xhtml", nav_xhtml),
            ("EPUB/images/cover.jpg", dummy_cover_jpg),
            ("EPUB/styles/main.css", style_css),
            ("EPUB/text/ch1.xhtml", ch1_html),
            ("EPUB/text/ch2.xhtml", ch2_html),
            ("EPUB/text/ch3.xhtml", ch3_html),
        ])
    }

    fn make_synthetic_cbz() -> Vec<u8> {
        let p1 = b"\xFF\xD8\xFF\xE0\x00\x10JFIF_PAGE_1";
        let p2 = b"\xFF\xD8\xFF\xE0\x00\x10JFIF_PAGE_2";
        let p3 = b"\xFF\xD8\xFF\xE0\x00\x10JFIF_PAGE_3";

        make_test_zip(&[
            ("001_page.jpg", p1),
            ("002_page.jpg", p2),
            ("003_page.jpg", p3),
        ])
    }

    #[test]
    fn test_ebook_probe_and_format_identification() {
        let epub_bytes = make_synthetic_epub();
        let fmt = uniffi_probe_ebook_bytes(epub_bytes, Some("book.epub".to_string())).expect("probe epub");
        assert_eq!(fmt, UniFFIEbookFormat::Epub);

        let cbz_bytes = make_synthetic_cbz();
        let fmt_cbz = uniffi_probe_ebook_bytes(cbz_bytes, Some("comic.cbz".to_string())).expect("probe cbz");
        assert_eq!(fmt_cbz, UniFFIEbookFormat::Cbz);

        let pdf_data = b"%PDF-1.7\n1 0 obj\n<<>>\nendobj\n%%EOF".to_vec();
        let fmt_pdf = uniffi_probe_ebook_bytes(pdf_data, Some("doc.pdf".to_string())).expect("probe pdf");
        assert_eq!(fmt_pdf, UniFFIEbookFormat::Pdf);
    }

    #[test]
    fn test_epub_metadata_extraction() {
        let epub_bytes = make_synthetic_epub();
        let meta = uniffi_extract_ebook_metadata(epub_bytes, Some("guide.epub".to_string())).expect("extract metadata");

        assert_eq!(meta.title, "TTZip High-Performance Systems Guide");
        assert_eq!(meta.authors, vec!["Witt Kung".to_string(), "Co-Author Name".to_string()]);
        assert_eq!(meta.publisher.as_deref(), Some("TTZip Architecture Press"));
        assert_eq!(meta.language.as_deref(), Some("en"));
        assert_eq!(meta.identifier.as_deref(), Some("urn:uuid:12345-67890-abcdef"));
        assert_eq!(meta.publication_date.as_deref(), Some("2026-09-01"));
        assert_eq!(meta.modified_date.as_deref(), Some("2026-09-01T12:00:00Z"));
        assert_eq!(meta.rights.as_deref(), Some("BSD-3-Clause OR Apache-2.0"));
        assert_eq!(meta.format, UniFFIEbookFormat::Epub);
        assert_eq!(meta.total_chapters, 3);
        assert!(meta.total_resources >= 7);
        assert!(meta.has_cover);
        assert_eq!(meta.cover_path.as_deref(), Some("EPUB/images/cover.jpg"));
    }

    #[test]
    fn test_epub_hierarchical_toc_tree() {
        let epub_bytes = make_synthetic_epub();
        let toc = uniffi_extract_ebook_toc(epub_bytes, None).expect("extract toc");

        assert_eq!(toc.len(), 2);
        assert_eq!(toc[0].title, "1. Microkernel Architecture");
        assert_eq!(toc[0].href, "EPUB/text/ch1.xhtml");
        assert_eq!(toc[0].level, 0);
        assert_eq!(toc[0].children.len(), 1);

        // Child node
        let child = &toc[0].children[0];
        assert_eq!(child.title, "1.1 Memory Bounds");
        assert_eq!(child.href, "EPUB/text/ch1.xhtml#bounds");
        assert_eq!(child.level, 1);
        assert_eq!(child.play_order, 2);

        // Second root section
        assert_eq!(toc[1].title, "2. Streaming Pipelines");
        assert_eq!(toc[1].href, "EPUB/text/ch2.xhtml");
        assert_eq!(toc[1].play_order, 3);
        assert!(toc[1].children.is_empty());
    }

    #[test]
    fn test_epub_spine_and_chapters() {
        let epub_bytes = make_synthetic_epub();

        // 1. Spine
        let spine = uniffi_get_ebook_spine(epub_bytes.clone(), None).expect("get spine");
        assert_eq!(spine.len(), 3);
        assert_eq!(spine[0].id, "ch1");
        assert_eq!(spine[0].href, "EPUB/text/ch1.xhtml");
        assert!(spine[0].is_linear);
        assert_eq!(spine[1].id, "ch2");
        assert!(spine[1].is_linear);
        assert_eq!(spine[2].id, "ch3");
        assert!(!spine[2].is_linear);

        // 2. Chapter Extraction
        let ch1 = uniffi_extract_ebook_chapter(epub_bytes.clone(), "EPUB/text/ch1.xhtml".to_string(), None)
            .expect("extract chapter 1");
        assert_eq!(ch1.title, "1. Microkernel Architecture");
        assert!(ch1.content_string.contains("Memory bounds are strictly enforced"));
        assert!(ch1.character_count > 0);
        assert!(ch1.word_count > 0);

        let ch2 = uniffi_extract_ebook_chapter(epub_bytes, "EPUB/text/ch2.xhtml".to_string(), None)
            .expect("extract chapter 2");
        assert_eq!(ch2.title, "2. Streaming Pipelines");
        assert!(ch2.content_string.contains("600 MB/s"));
    }

    #[test]
    fn test_epub_resource_and_cover() {
        let epub_bytes = make_synthetic_epub();

        // 1. Cover
        let cover = uniffi_extract_ebook_cover(epub_bytes.clone(), None).expect("extract cover").expect("cover exists");
        assert_eq!(cover.href, "EPUB/images/cover.jpg");
        assert_eq!(cover.media_type, "image/jpeg");
        assert!(!cover.data.is_empty());

        // 2. Stylesheet resource
        let css = uniffi_extract_ebook_resource(epub_bytes, "EPUB/styles/main.css".to_string(), None)
            .expect("extract css");
        assert_eq!(css.media_type, "text/css");
        let css_str = String::from_utf8(css.data).unwrap();
        assert!(css_str.contains("font-family"));
    }

    #[test]
    fn test_cbz_comic_book_pipeline() {
        let cbz_bytes = make_synthetic_cbz();

        let meta = uniffi_extract_ebook_metadata(cbz_bytes.clone(), Some("Superman.cbz".to_string()))
            .expect("cbz metadata");
        assert_eq!(meta.title, "Superman");
        assert_eq!(meta.format, UniFFIEbookFormat::Cbz);
        assert_eq!(meta.total_chapters, 3);
        assert!(meta.has_cover);

        let toc = uniffi_extract_ebook_toc(cbz_bytes.clone(), Some("Superman.cbz".to_string()))
            .expect("cbz toc");
        assert_eq!(toc.len(), 3);
        assert_eq!(toc[0].title, "Page 1");
        assert_eq!(toc[0].href, "001_page.jpg");

        let spine = uniffi_get_ebook_spine(cbz_bytes.clone(), Some("Superman.cbz".to_string()))
            .expect("cbz spine");
        assert_eq!(spine.len(), 3);
        assert_eq!(spine[0].href, "001_page.jpg");

        let cover = uniffi_extract_ebook_cover(cbz_bytes.clone(), Some("Superman.cbz".to_string()))
            .expect("cbz cover")
            .expect("has cover");
        assert_eq!(cover.href, "001_page.jpg");

        let ch = uniffi_extract_ebook_chapter(cbz_bytes, "001_page.jpg".to_string(), Some("Superman.cbz".to_string()))
            .expect("cbz chapter");
        assert!(ch.content_string.contains("001_page.jpg"));
    }

    #[test]
    fn test_uniffi_ebook_service_object_lifecycle() {
        let service = UniFFIEbookService::new();
        let epub_bytes = make_synthetic_epub();

        let fmt = service.probe_bytes(epub_bytes.clone(), None).expect("probe");
        assert_eq!(fmt, UniFFIEbookFormat::Epub);

        let meta = service.extract_metadata(epub_bytes.clone(), None).expect("meta");
        assert_eq!(meta.title, "TTZip High-Performance Systems Guide");

        let toc = service.extract_toc(epub_bytes.clone(), None).expect("toc");
        assert_eq!(toc.len(), 2);

        let spine = service.get_spine(epub_bytes.clone(), None).expect("spine");
        assert_eq!(spine.len(), 3);

        let ch = service.extract_chapter(epub_bytes.clone(), "EPUB/text/ch1.xhtml".to_string(), None).expect("chapter");
        assert_eq!(ch.title, "1. Microkernel Architecture");

        let cover = service.extract_cover(epub_bytes, None).expect("cover").expect("cover exists");
        assert_eq!(cover.media_type, "image/jpeg");
    }

    #[test]
    fn test_ebook_error_resilience() {
        // Corrupted empty byte slice
        let empty_data = Vec::new();
        assert!(uniffi_probe_ebook_bytes(empty_data.clone(), None).is_err());
        assert!(uniffi_extract_ebook_metadata(empty_data, None).is_err());

        // Non-existent resource in valid EPUB
        let epub_bytes = make_synthetic_epub();
        let err = uniffi_extract_ebook_resource(epub_bytes, "non/existent.png".to_string(), None);
        assert!(err.is_err());
    }
}
