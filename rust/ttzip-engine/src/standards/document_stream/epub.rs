// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! EPUB in-memory container resolution, metadata, spine, TOC, and cover image extraction.

use std::collections::HashMap;
use std::path::Path;

use super::DocumentStreamError;
use crate::ebook::container::{
    find_and_extract_zip_entry, opf_parent_dir, resolve_epub_opf_path, strip_html_tags,
};
use crate::ebook::ncx::collect_toc_title_map;
use crate::ebook::opf::{parse_opf_xml, ParsedOpfPackage};
use crate::zip::reader::ZipArchive;

/// Metadata of an EPUB publication.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpubMetadata {
    pub title: String,
    pub authors: Vec<String>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub identifier: Option<String>,
    pub description: Option<String>,
    pub publication_date: Option<String>,
    pub modified_date: Option<String>,
    pub rights: Option<String>,
}

/// A chapter in an EPUB book.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpubChapter {
    pub id: String,
    pub title: String,
    pub href: String,
    pub media_type: String,
    pub play_order: u32,
}

/// In-memory cover image extracted from an EPUB publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpubCover {
    pub file_path: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// Full in-memory EPUB parse result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpubBook {
    pub metadata: EpubMetadata,
    pub chapters: Vec<EpubChapter>,
    pub cover: Option<EpubCover>,
    pub total_chapters: u32,
    pub manifest_items_count: u32,
}

/// Parses an EPUB archive completely in-memory without disk extraction.
pub fn parse_epub_from_memory(epub_bytes: &[u8]) -> Result<EpubBook, DocumentStreamError> {
    let zip = ZipArchive::open_slice(epub_bytes)
        .map_err(|e| DocumentStreamError::ZipError(format!("{e:?}")))?;

    let opf_path = resolve_epub_opf_path(&zip)
        .ok_or_else(|| DocumentStreamError::EntryNotFound("content.opf".to_string()))?;
    let opf_dir = opf_parent_dir(&opf_path);

    let opf_bytes = find_and_extract_zip_entry(&zip, &opf_path)
        .ok_or_else(|| DocumentStreamError::EntryNotFound(opf_path.clone()))?;
    let opf = parse_opf_xml(&opf_bytes, opf_dir)
        .map_err(|e| DocumentStreamError::XmlError(e.to_string()))?;

    let toc_titles = collect_toc_title_map(
        &zip,
        opf.ncx_href.as_deref(),
        opf.nav_href.as_deref(),
        opf_dir,
    );

    let mut chapters = build_spine_chapters(&opf, &toc_titles);
    if chapters.is_empty() {
        chapters = fallback_scan_chapters(&zip);
    }

    let cover = extract_cover_image(&zip, &opf);
    let metadata = into_epub_metadata(opf.metadata, opf.modified_date);
    let total_chapters = chapters.len() as u32;
    let manifest_items_count = opf.manifest.len() as u32;

    Ok(EpubBook {
        metadata,
        chapters,
        cover,
        total_chapters,
        manifest_items_count,
    })
}

fn into_epub_metadata(
    meta: crate::ebook::parser::EbookMetadata,
    modified_date: Option<String>,
) -> EpubMetadata {
    let title = meta
        .title
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Untitled Book".to_string());
    EpubMetadata {
        title,
        authors: meta.authors,
        publisher: meta.publisher,
        language: meta.language,
        identifier: meta.identifiers.into_iter().next(),
        description: meta.description,
        publication_date: meta.publication_date,
        modified_date,
        rights: meta.rights,
    }
}

fn build_spine_chapters(
    opf: &ParsedOpfPackage,
    toc: &HashMap<String, String>,
) -> Vec<EpubChapter> {
    let mut chapters = Vec::new();
    for (order, (idref, _)) in opf.spine_raw.iter().enumerate() {
        if let Some(item) = opf.manifest.get(idref) {
            let href = item.href.clone();
            let fname = Path::new(&href)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&href);
            let title = toc
                .get(&href)
                .or_else(|| toc.get(fname))
                .cloned()
                .unwrap_or_else(|| format!("Chapter {}", order + 1));
            chapters.push(EpubChapter {
                id: idref.clone(),
                title,
                href,
                media_type: item.media_type.clone(),
                play_order: (order + 1) as u32,
            });
        }
    }
    chapters
}

fn extract_cover_image(zip: &ZipArchive<'_>, opf: &ParsedOpfPackage) -> Option<EpubCover> {
    let cover_href = opf.metadata.cover_image_href.as_ref()?;
    let data = find_and_extract_zip_entry(zip, cover_href)?;
    let mime_type = opf
        .manifest
        .values()
        .find(|item| &item.href == cover_href)
        .map(|item| item.media_type.clone())
        .unwrap_or_else(|| "image/jpeg".to_string());
    Some(EpubCover {
        file_path: cover_href.clone(),
        mime_type,
        data,
    })
}

fn fallback_scan_chapters(zip: &ZipArchive<'_>) -> Vec<EpubChapter> {
    let mut entries: Vec<String> = zip
        .entries()
        .iter()
        .filter(|e| {
            let l = e.rel_path.to_lowercase();
            l.ends_with(".xhtml") || l.ends_with(".html") || l.ends_with(".htm")
        })
        .map(|e| e.rel_path.clone())
        .collect();
    entries.sort();
    entries
        .into_iter()
        .enumerate()
        .map(|(idx, href)| {
            let stem = Path::new(&href)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Chapter")
                .to_string();
            EpubChapter {
                id: format!("item_{idx}"),
                title: format!("Chapter {} · {}", idx + 1, stem),
                href,
                media_type: "application/xhtml+xml".to_string(),
                play_order: (idx + 1) as u32,
            }
        })
        .collect()
}

/// Extracts plain text of a specific chapter from an in-memory EPUB archive.
pub fn extract_epub_chapter_text(
    epub_bytes: &[u8],
    chapter_href: &str,
) -> Result<String, DocumentStreamError> {
    let zip = ZipArchive::open_slice(epub_bytes)
        .map_err(|e| DocumentStreamError::ZipError(format!("{e:?}")))?;
    let clean = chapter_href.split('#').next().unwrap_or(chapter_href);
    let bytes = find_and_extract_zip_entry(&zip, clean)
        .ok_or_else(|| DocumentStreamError::EntryNotFound(clean.to_string()))?;
    let html = std::str::from_utf8(&bytes)
        .map_err(|e| DocumentStreamError::DecodeError(e.to_string()))?;
    Ok(strip_html_markup(html))
}

fn strip_html_markup(html: &str) -> String {
    strip_html_tags(html)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}
