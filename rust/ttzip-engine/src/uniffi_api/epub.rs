// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! EPUB and Office Compound Document Streaming Introspection & Metadata Parser.
//!
//! Provides zero-disk-footprint streaming metadata extraction directly from memory-mapped
//! ZIP archives, extracting container definitions, OPF manifests, spines, and NCX/NAV tables of contents.

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use crate::ebook::container::{
    extract_html_heading, find_and_extract_zip_entry, opf_parent_dir, resolve_epub_opf_path,
};
use crate::ebook::ncx::collect_toc_title_map;
use crate::ebook::opf::{parse_opf_xml, ParsedOpfPackage};

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

fn resolve_chapter_title(
    zip: &crate::zip::ZipArchive<'_>,
    full_href: &str,
    toc_titles: &HashMap<String, String>,
    book_title: &str,
) -> String {
    let file_name = Path::new(full_href)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(full_href);

    let from_toc = toc_titles
        .get(full_href)
        .or_else(|| toc_titles.get(file_name))
        .cloned();

    let chapter_title = from_toc.or_else(|| {
        let ch_bytes = find_and_extract_zip_entry(zip, full_href)?;
        let ch_str = String::from_utf8_lossy(&ch_bytes);
        extract_html_heading(&ch_str).filter(|t| t != book_title && !t.trim().is_empty())
    });

    chapter_title.unwrap_or_else(|| {
        Path::new(full_href)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Chapter")
            .to_string()
    })
}

fn build_ordered_chapters(
    zip: &crate::zip::ZipArchive<'_>,
    opf: &ParsedOpfPackage,
    toc_titles: &HashMap<String, String>,
    book_title: &str,
) -> Vec<UniFFIEpubChapter> {
    let mut ordered_chapters = Vec::new();
    for (idref, _) in &opf.spine_raw {
        if let Some(item) = opf.manifest.get(idref) {
            let full_href = item.href.clone();
            let base_title = resolve_chapter_title(zip, &full_href, toc_titles, book_title);
            let idx = ordered_chapters.len();
            let is_prefixed = base_title.starts_with('第')
                || base_title.contains('章')
                || base_title.to_lowercase().contains("chapter");
            let display_title = if is_prefixed {
                base_title
            } else {
                format!("Chapter {} · {}", idx + 1, base_title)
            };
            ordered_chapters.push(UniFFIEpubChapter {
                title: display_title,
                href: full_href,
            });
        }
    }
    ordered_chapters
}

fn scan_fallback_html_chapters(zip: &crate::zip::ZipArchive<'_>) -> Vec<UniFFIEpubChapter> {
    let mut html_entries: Vec<String> = zip
        .entries()
        .iter()
        .filter(|e| {
            let l = e.rel_path.to_lowercase();
            l.ends_with(".xhtml") || l.ends_with(".html") || l.ends_with(".htm")
        })
        .map(|e| e.rel_path.clone())
        .collect();
    html_entries.sort();

    html_entries
        .into_iter()
        .enumerate()
        .map(|(idx, entry_path)| {
            let stem = Path::new(&entry_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Chapter");
            UniFFIEpubChapter {
                title: format!("Chapter {} · {stem}", idx + 1),
                href: entry_path,
            }
        })
        .collect()
}

/// Parses EPUB book metadata and chapters with zero temporary disk extraction.
#[uniffi::export]
pub fn parse_epub_metadata(epub_path: String) -> Option<UniFFIEpubBook> {
    let path = Path::new(&epub_path);
    if !path.exists() {
        return None;
    }

    let file = File::open(path).ok()?;
    let mmap = unsafe { memmap2::Mmap::map(&file) }.ok()?;
    let zip = crate::zip::ZipArchive::open_slice(&mmap[..]).ok()?;

    let opf_path = resolve_epub_opf_path(&zip)?;
    let opf_dir = opf_parent_dir(&opf_path);
    let opf_bytes = find_and_extract_zip_entry(&zip, &opf_path)?;
    let opf = parse_opf_xml(&opf_bytes, opf_dir).ok()?;

    let default_title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled Book")
        .to_string();
    let book_title = opf
        .metadata
        .title
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or(default_title);

    let toc_titles = collect_toc_title_map(
        &zip,
        opf.ncx_href.as_deref(),
        opf.nav_href.as_deref(),
        opf_dir,
    );

    let mut ordered_chapters = build_ordered_chapters(&zip, &opf, &toc_titles, &book_title);
    if ordered_chapters.is_empty() {
        ordered_chapters = scan_fallback_html_chapters(&zip);
    }
    if ordered_chapters.is_empty() {
        return None;
    }

    Some(UniFFIEpubBook {
        title: book_title,
        chapters: ordered_chapters,
        cover_path: opf.metadata.cover_image_href,
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
