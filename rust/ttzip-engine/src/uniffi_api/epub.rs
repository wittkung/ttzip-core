// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! EPUB and Office Compound Document Streaming Introspection & Metadata Parser.
//!
//! Provides zero-disk-footprint streaming metadata extraction directly from memory-mapped
//! ZIP archives, extracting container definitions, OPF manifests, spines, and NCX/NAV tables of contents.

use std::path::Path;

#[derive(uniffi::Record, Clone, Debug)]
pub struct UniFFIEpubChapter {
    pub title: String,
    pub href: String,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct UniFFIEpubBook {
    pub title: String,
    pub chapters: Vec<UniFFIEpubChapter>,
    pub cover_path: Option<String>,
}

/// Parses EPUB book metadata and chapters with zero temporary disk extraction.
#[uniffi::export]
pub fn parse_epub_metadata(epub_path: String) -> Option<UniFFIEpubBook> {
    let book = crate::ebook::preview::fallback_book(Path::new(&epub_path))?;
    Some(UniFFIEpubBook {
        title: book.title,
        chapters: book.chapters.into_iter().map(|chapter| UniFFIEpubChapter {
            title: chapter.title,
            href: chapter.href,
        }).collect(),
        cover_path: book.cover_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

    #[test]
    fn test_parse_epub_metadata_consolidated_opf_and_ncx() {
        let container_xml = br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;
        let opf_xml = br#"<?xml version="1.0" encoding="utf-8"?><package version="2.0" xmlns="http://www.idpf.org/2007/opf"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Unified Book</dc:title><meta name="cover" content="cov"/></metadata><manifest><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/><item id="cov" href="img/cover.jpg" media-type="image/jpeg"/><item id="c1" href="ch1.xhtml" media-type="application/xhtml+xml"/></manifest><spine toc="ncx"><itemref idref="c1"/></spine></package>"#;
        let ncx_xml = br#"<?xml version="1.0" encoding="utf-8"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><navMap><navPoint id="n1"><navLabel><text>Intro</text></navLabel><content src="ch1.xhtml"/></navPoint></navMap></ncx>"#;
        let ch1_html = b"<html><body><h1>Heading</h1><p>Body</p></body></html>";

        let items: Vec<ZipInputItem> = [
            ("META-INF/container.xml", &container_xml[..]),
            ("OEBPS/content.opf", &opf_xml[..]),
            ("OEBPS/toc.ncx", &ncx_xml[..]),
            ("OEBPS/ch1.xhtml", &ch1_html[..]),
        ]
        .into_iter()
        .map(|(p, d)| ZipInputItem {
            rel_path: p.to_string(),
            data: d.to_vec(),
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
        .expect("compress");
        let archive_bytes = assemble_zip_archive(&compressed).expect("assemble");

        let dir = tempfile::tempdir().expect("tempdir");
        let epub_path = dir.path().join("unified.epub");
        std::fs::write(&epub_path, &archive_bytes).expect("write epub");

        let parsed =
            parse_epub_metadata(epub_path.to_string_lossy().into_owned()).expect("parse epub");
        assert_eq!(parsed.title, "Unified Book");
        assert_eq!(parsed.cover_path.as_deref(), Some("OEBPS/img/cover.jpg"));
        assert_eq!(parsed.chapters.len(), 1);
        assert_eq!(parsed.chapters[0].title, "Chapter 1 · Intro");
        assert_eq!(parsed.chapters[0].href, "OEBPS/ch1.xhtml");
    }
}
