// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! EPUB in-memory container resolution, metadata, spine, TOC, and cover image extraction via roxmltree.

use std::collections::HashMap;
use std::path::Path;

use super::{find_and_extract_zip_entry, resolve_relative_path, DocumentStreamError};
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

/// Manifest item extracted from OPF.
#[derive(Debug, Clone)]
struct ManifestItem {
    href: String,
    media_type: String,
    _properties: String,
}


/// Parses an EPUB archive completely in-memory without disk extraction.
pub fn parse_epub_from_memory(epub_bytes: &[u8]) -> Result<EpubBook, DocumentStreamError> {
    let zip = ZipArchive::open_slice(epub_bytes)
        .map_err(|e| DocumentStreamError::ZipError(format!("{e:?}")))?;

    // 1. Resolve OPF package path from META-INF/container.xml
    let opf_path = resolve_epub_opf_path(&zip)?;
    let opf_dir = Path::new(&opf_path).parent().and_then(|p| p.to_str()).unwrap_or("");

    // 2. Read and parse OPF XML
    let opf_bytes = find_and_extract_zip_entry(&zip, &opf_path)
        .ok_or_else(|| DocumentStreamError::EntryNotFound(opf_path.clone()))?;
    let opf_str = std::str::from_utf8(&opf_bytes)
        .map_err(|e| DocumentStreamError::DecodeError(e.to_string()))?;
    let opf_doc = roxmltree::Document::parse(opf_str)
        .map_err(|e| DocumentStreamError::XmlError(e.to_string()))?;

    // 3. Parse Metadata & Manifest
    let (metadata, mut cover_id) = parse_metadata(&opf_doc);
    let (manifest, ncx_href, nav_href, manifest_cover) = parse_manifest(&opf_doc);
    if cover_id.is_none() {
        cover_id = manifest_cover;
    }

    // 4. Parse Table of Contents mapping
    let mut toc_titles: HashMap<String, String> = HashMap::new();
    for rel in [ncx_href, nav_href].into_iter().flatten() {
        let full = resolve_relative_path(opf_dir, &rel);
        if let Some(bytes) = find_and_extract_zip_entry(&zip, &full) {
            parse_toc_content(&bytes, &mut toc_titles);
        }
    }

    // 5. Parse Spine & Build Chapters
    let mut chapters = parse_spine(&opf_doc, &manifest, opf_dir, &toc_titles);
    if chapters.is_empty() {
        chapters = fallback_scan_chapters(&zip);
    }

    // 6. Extract Cover Image Bytes into memory
    let cover = cover_id
        .and_then(|cid| manifest.get(&cid))
        .and_then(|item| {
            let path = resolve_relative_path(opf_dir, &item.href);
            let data = find_and_extract_zip_entry(&zip, &path)?;
            Some(EpubCover { file_path: path, mime_type: item.media_type.clone(), data })
        });

    let (total_chapters, manifest_items_count) = (chapters.len() as u32, manifest.len() as u32);
    Ok(EpubBook { metadata, chapters, cover, total_chapters, manifest_items_count })
}

fn parse_metadata(doc: &roxmltree::Document) -> (EpubMetadata, Option<String>) {
    let mut meta = EpubMetadata::default();
    let mut cover_id = None;
    if let Some(node) = doc.descendants().find(|n| n.tag_name().name() == "metadata") {
        for c in node.children().filter(|n| n.is_element()) {
            let tag = c.tag_name().name();
            let text = c.text().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            match tag {
                "title" => if let Some(t) = text { if meta.title.is_empty() { meta.title = t; } },
                "creator" => if let Some(a) = text { meta.authors.push(a); },
                "publisher" => meta.publisher = text,
                "language" => meta.language = text,
                "identifier" => if meta.identifier.is_none() { meta.identifier = text; },
                "description" => meta.description = text,
                "date" => meta.publication_date = text,
                "rights" => meta.rights = text,
                "meta" => {
                    if c.attribute("name") == Some("cover") {
                        cover_id = c.attribute("content").map(ToString::to_string);
                    }
                    if c.attribute("property") == Some("dcterms:modified") {
                        meta.modified_date = text;
                    }
                }
                _ => {}
            }
        }
    }
    if meta.title.is_empty() { meta.title = "Untitled Book".to_string(); }
    (meta, cover_id)
}

fn parse_manifest(doc: &roxmltree::Document) -> (HashMap<String, ManifestItem>, Option<String>, Option<String>, Option<String>) {
    let mut manifest = HashMap::new();
    let (mut ncx, mut nav, mut cover) = (None, None, None);
    if let Some(node) = doc.descendants().find(|n| n.tag_name().name() == "manifest") {
        for item in node.children().filter(|n| n.tag_name().name() == "item") {
            if let (Some(id), Some(href)) = (item.attribute("id"), item.attribute("href")) {
                let media_type = item.attribute("media-type").unwrap_or("").to_string();
                let properties = item.attribute("properties").unwrap_or("").to_string();
                if media_type == "application/x-dtbncx+xml" || href.to_lowercase().ends_with(".ncx") {
                    ncx = Some(href.to_string());
                }
                if properties.contains("nav") { nav = Some(href.to_string()); }
                if properties.contains("cover-image") || id == "cover" || id == "cover-image" {
                    cover = Some(id.to_string());
                }
                manifest.insert(id.to_string(), ManifestItem { href: href.to_string(), media_type, _properties: properties });

            }
        }
    }
    (manifest, ncx, nav, cover)
}

fn parse_spine(doc: &roxmltree::Document, manifest: &HashMap<String, ManifestItem>, opf_dir: &str, toc: &HashMap<String, String>) -> Vec<EpubChapter> {
    let mut chapters = Vec::new();
    if let Some(spine_node) = doc.descendants().find(|n| n.tag_name().name() == "spine") {
        for (order, itemref) in spine_node.children().filter(|n| n.tag_name().name() == "itemref").enumerate() {
            if let Some(idref) = itemref.attribute("idref") {
                if let Some(item) = manifest.get(idref) {
                    let href = resolve_relative_path(opf_dir, &item.href);
                    let fname = Path::new(&href).file_name().and_then(|s| s.to_str()).unwrap_or(&href);
                    let title = toc.get(&href).or_else(|| toc.get(&item.href)).or_else(|| toc.get(fname)).cloned().unwrap_or_else(|| format!("Chapter {}", order + 1));
                    chapters.push(EpubChapter {
                        id: idref.to_string(), title, href, media_type: item.media_type.clone(), play_order: (order + 1) as u32,
                    });
                }
            }
        }
    }
    chapters
}

fn fallback_scan_chapters(zip: &ZipArchive) -> Vec<EpubChapter> {
    let mut entries: Vec<String> = zip.entries().iter().filter(|e| {
        let l = e.rel_path.to_lowercase();
        l.ends_with(".xhtml") || l.ends_with(".html") || l.ends_with(".htm")
    }).map(|e| e.rel_path.clone()).collect();
    entries.sort();
    entries.into_iter().enumerate().map(|(idx, href)| {
        let stem = Path::new(&href).file_stem().and_then(|s| s.to_str()).unwrap_or("Chapter").to_string();
        EpubChapter {
            id: format!("item_{idx}"),
            title: format!("Chapter {} · {}", idx + 1, stem),
            href,
            media_type: "application/xhtml+xml".to_string(),
            play_order: (idx + 1) as u32,
        }
    }).collect()
}

/// Resolves the full path of the OPF package inside an EPUB archive.
fn resolve_epub_opf_path(zip: &ZipArchive) -> Result<String, DocumentStreamError> {
    if let Some(bytes) = find_and_extract_zip_entry(zip, "META-INF/container.xml") {
        if let Ok(s) = std::str::from_utf8(&bytes) {
            if let Ok(doc) = roxmltree::Document::parse(s) {
                if let Some(rf) = doc.descendants().find(|n| n.tag_name().name() == "rootfile") {
                    if let Some(path) = rf.attribute("full-path") { return Ok(path.to_string()); }
                }
            }
        }
    }
    zip.entries().iter().find(|e| e.rel_path.to_lowercase().ends_with(".opf")).map(|e| e.rel_path.clone())
        .ok_or_else(|| DocumentStreamError::EntryNotFound("content.opf".to_string()))
}

/// Parses NCX/NAV TOC document to build mapping from relative href to chapter title.
fn parse_toc_content(bytes: &[u8], toc_titles: &mut HashMap<String, String>) {
    let Ok(s) = std::str::from_utf8(bytes) else { return };
    let Ok(doc) = roxmltree::Document::parse(s) else { return };

    // Support NCX navPoints
    for np in doc.descendants().filter(|n| n.tag_name().name() == "navPoint") {
        let label = np.descendants().find(|n| n.tag_name().name() == "text").and_then(|n| n.text()).map(|t| t.trim().to_string());
        let src = np.descendants().find(|n| n.tag_name().name() == "content").and_then(|n| n.attribute("src"));
        if let (Some(t), Some(s)) = (label, src) {
            if !t.is_empty() {
                let clean = s.split('#').next().unwrap_or(s);
                let fname = Path::new(clean).file_name().and_then(|os| os.to_str()).unwrap_or(clean);
                toc_titles.insert(clean.to_string(), t.clone());
                toc_titles.insert(fname.to_string(), t);
            }
        }
    }
    // Support EPUB3 HTML5 nav <a> tags
    for a in doc.descendants().filter(|n| n.tag_name().name() == "a") {
        if let Some(href) = a.attribute("href") {
            let text = a.text().map(|s| s.trim().to_string()).unwrap_or_default();
            if !text.is_empty() {
                let clean = href.split('#').next().unwrap_or(href);
                let fname = Path::new(clean).file_name().and_then(|os| os.to_str()).unwrap_or(clean);
                toc_titles.insert(clean.to_string(), text.clone());
                toc_titles.insert(fname.to_string(), text);
            }
        }
    }
}

/// Extracts plain text of a specific chapter from an in-memory EPUB archive.
pub fn extract_epub_chapter_text(epub_bytes: &[u8], chapter_href: &str) -> Result<String, DocumentStreamError> {
    let zip = ZipArchive::open_slice(epub_bytes).map_err(|e| DocumentStreamError::ZipError(format!("{e:?}")))?;
    let clean = chapter_href.split('#').next().unwrap_or(chapter_href);
    let bytes = find_and_extract_zip_entry(&zip, clean).ok_or_else(|| DocumentStreamError::EntryNotFound(clean.to_string()))?;
    let html = std::str::from_utf8(&bytes).map_err(|e| DocumentStreamError::DecodeError(e.to_string()))?;
    Ok(strip_html_markup(html))
}

/// Helper to strip HTML tags and decode entities.
fn strip_html_markup(html: &str) -> String {
    let mut res = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        if c == '<' { in_tag = true; } else if c == '>' { in_tag = false; } else if !in_tag { res.push(c); }
    }
    res.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
        .replace("&quot;", "\"").replace("&apos;", "'").replace("&#39;", "'").replace("&nbsp;", " ")
        .lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n\n")
}
