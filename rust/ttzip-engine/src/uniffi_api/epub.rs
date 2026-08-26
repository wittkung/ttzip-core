// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! EPUB and Office Compound Document Streaming Introspection & Metadata Parser.
//!
//! Provides zero-disk-footprint streaming metadata extraction directly from memory-mapped
//! ZIP archives, extracting container definitions, OPF manifests, spines, and NCX/NAV tables of contents.

use std::collections::HashMap;
use std::fs::File;
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
    let path = Path::new(&epub_path);
    if !path.exists() {
        return None;
    }

    let file = File::open(path).ok()?;
    let mmap = unsafe { memmap2::Mmap::map(&file) }.ok()?;
    let zip = crate::zip::ZipArchive::open_slice(&mmap[..]).ok()?;

    // 1. Resolve OPF package path from META-INF/container.xml or fallback scan
    let opf_path = find_and_extract_entry(&zip, "META-INF/container.xml")
        .and_then(|bytes| extract_attr(&String::from_utf8_lossy(&bytes), "rootfile", "full-path"))
        .or_else(|| {
            zip.entries()
                .iter()
                .find(|e| e.rel_path.to_lowercase().ends_with(".opf"))
                .map(|e| e.rel_path.clone())
        })?;

    let opf_dir = Path::new(&opf_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");

    // 2. Extract and parse OPF content
    let opf_bytes = find_and_extract_entry(&zip, &opf_path)?;
    let opf_str = String::from_utf8_lossy(&opf_bytes);

    let default_title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled Book")
        .to_string();
    let book_title = extract_inner_text(&opf_str, "dc:title")
        .filter(|t| !t.trim().is_empty())
        .unwrap_or(default_title);

    let mut manifest_hrefs: HashMap<String, String> = HashMap::new();
    let mut ncx_href: Option<String> = None;
    let mut nav_href: Option<String> = None;
    let mut cover_item_id: Option<String> = None;

    for tag in find_tags(&opf_str, "item") {
        if let (Some(id), Some(href)) = (extract_attr(&tag, "item", "id"), extract_attr(&tag, "item", "href")) {
            let media_type = extract_attr(&tag, "item", "media-type").unwrap_or_default();
            let props = extract_attr(&tag, "item", "properties").unwrap_or_default();

            if media_type == "application/x-dtbncx+xml" || href.to_lowercase().ends_with(".ncx") {
                ncx_href = Some(href.clone());
            }
            if props.contains("nav") {
                nav_href = Some(href.clone());
            }
            if props.contains("cover-image") || id == "cover" || id == "cover-image" {
                cover_item_id = Some(id.clone());
            }
            manifest_hrefs.insert(id, href);
        }
    }

    for tag in find_tags(&opf_str, "meta") {
        if extract_attr(&tag, "meta", "name").as_deref() == Some("cover") {
            if let Some(content) = extract_attr(&tag, "meta", "content") {
                cover_item_id = Some(content);
            }
        }
    }

    let cover_path = cover_item_id
        .and_then(|cid| manifest_hrefs.get(&cid))
        .map(|href| resolve_path(opf_dir, href));

    let spine_ids: Vec<String> = find_tags(&opf_str, "itemref")
        .into_iter()
        .filter_map(|tag| extract_attr(&tag, "itemref", "idref"))
        .collect();

    // 3. Build Table of Contents mapping (href/filename -> title)
    let mut toc_titles: HashMap<String, String> = HashMap::new();

    if let Some(ncx_rel) = ncx_href {
        let ncx_full = resolve_path(opf_dir, &ncx_rel);
        if let Some(ncx_bytes) = find_and_extract_entry(&zip, &ncx_full) {
            parse_ncx_toc(&String::from_utf8_lossy(&ncx_bytes), &mut toc_titles);
        }
    }

    if let Some(nav_rel) = nav_href {
        let nav_full = resolve_path(opf_dir, &nav_rel);
        if let Some(nav_bytes) = find_and_extract_entry(&zip, &nav_full) {
            parse_nav_toc(&String::from_utf8_lossy(&nav_bytes), &mut toc_titles);
        }
    }

    // 4. Construct ordered chapters
    let mut ordered_chapters: Vec<UniFFIEpubChapter> = Vec::new();
    for idref in &spine_ids {
        if let Some(rel_href) = manifest_hrefs.get(idref) {
            let full_href = resolve_path(opf_dir, rel_href);
            let file_name = Path::new(&full_href)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&full_href);

            let mut chapter_title = toc_titles
                .get(&full_href)
                .or_else(|| toc_titles.get(rel_href))
                .or_else(|| toc_titles.get(file_name))
                .cloned();

            if chapter_title.is_none() {
                if let Some(ch_bytes) = find_and_extract_entry(&zip, &full_href) {
                    let ch_str = String::from_utf8_lossy(&ch_bytes);
                    chapter_title = extract_inner_text(&ch_str, "h1")
                        .or_else(|| extract_inner_text(&ch_str, "h2"))
                        .or_else(|| extract_inner_text(&ch_str, "title"))
                        .filter(|t| t != &book_title && !t.trim().is_empty());
                }
            }

            let base_title = chapter_title.unwrap_or_else(|| {
                Path::new(&full_href)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Chapter")
                    .to_string()
            });

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

    // Fallback if spine was empty: scan HTML/XHTML entries
    if ordered_chapters.is_empty() {
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

        for (idx, entry_path) in html_entries.into_iter().enumerate() {
            let stem = Path::new(&entry_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Chapter")
                .to_string();
            ordered_chapters.push(UniFFIEpubChapter {
                title: format!("Chapter {} · {}", idx + 1, stem),
                href: entry_path,
            });
        }
    }

    if ordered_chapters.is_empty() {
        return None;
    }

    Some(UniFFIEpubBook {
        title: book_title,
        chapters: ordered_chapters,
        cover_path,
    })
}

// MARK: - Internal Helpers

fn find_and_extract_entry(zip: &crate::zip::ZipArchive, target_path: &str) -> Option<Vec<u8>> {
    let norm = target_path.trim_start_matches('/').replace('\\', "/");
    for (idx, entry) in zip.entries().iter().enumerate() {
        let entry_norm = entry.rel_path.trim_start_matches('/').replace('\\', "/");
        if entry_norm == norm {
            return zip.extract_entry_bytes(idx, None).ok();
        }
    }
    let norm_lower = norm.to_lowercase();
    for (idx, entry) in zip.entries().iter().enumerate() {
        let entry_norm = entry.rel_path.trim_start_matches('/').replace('\\', "/").to_lowercase();
        if entry_norm == norm_lower {
            return zip.extract_entry_bytes(idx, None).ok();
        }
    }
    None
}

fn resolve_path(base_dir: &str, rel_path: &str) -> String {
    let clean_rel = rel_path.split('#').next().unwrap_or(rel_path);
    let clean_rel = clean_rel.split('?').next().unwrap_or(clean_rel);
    if base_dir.is_empty() || clean_rel.starts_with('/') {
        return clean_rel.trim_start_matches('/').to_string();
    }
    let combined = format!("{}/{}", base_dir.trim_matches('/'), clean_rel.trim_matches('/'));
    let mut segments = Vec::new();
    for seg in combined.split('/') {
        if seg == "." || seg.is_empty() {
            continue;
        } else if seg == ".." {
            segments.pop();
        } else {
            segments.push(seg);
        }
    }
    segments.join("/")
}

fn find_tags<'a>(xml: &'a str, tag_name: &'a str) -> Vec<&'a str> {
    let mut results = Vec::new();
    let lower = xml.to_lowercase();
    let open = format!("<{}", tag_name.to_lowercase());
    let mut search_pos = 0;

    while let Some(idx) = lower[search_pos..].find(&open) {
        let tag_start = search_pos + idx;
        let next_idx = tag_start + open.len();
        if next_idx < xml.len() {
            let next_b = xml.as_bytes()[next_idx];
            if !matches!(next_b, b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/') {
                search_pos = next_idx;
                continue;
            }
        }
        if let Some(end_offset) = xml[tag_start..].find('>') {
            results.push(&xml[tag_start..tag_start + end_offset + 1]);
            search_pos = tag_start + end_offset + 1;
        } else {
            break;
        }
    }
    results
}

fn extract_attr(tag_str: &str, _tag_name: &str, attr_name: &str) -> Option<String> {
    let lower = tag_str.to_lowercase();
    for quote in ["\"", "'"] {
        let pat = format!("{}={}", attr_name.to_lowercase(), quote);
        if let Some(pos) = lower.find(&pat) {
            let start = pos + pat.len();
            if let Some(end) = tag_str[start..].find(quote) {
                return Some(tag_str[start..start + end].to_string());
            }
        }
    }
    None
}

fn extract_inner_text(xml: &str, tag_name: &str) -> Option<String> {
    let lower = xml.to_lowercase();
    let open = format!("<{}", tag_name.to_lowercase());
    let close = format!("</{}>", tag_name.to_lowercase());

    let mut search_pos = 0;
    while let Some(idx) = lower[search_pos..].find(&open) {
        let tag_start = search_pos + idx;
        let next_idx = tag_start + open.len();
        if next_idx < xml.len() {
            let next_b = xml.as_bytes()[next_idx];
            if !matches!(next_b, b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/') {
                search_pos = next_idx;
                continue;
            }
        }
        if let Some(gt_offset) = xml[tag_start..].find('>') {
            let content_start = tag_start + gt_offset + 1;
            if let Some(close_offset) = lower[content_start..].find(&close) {
                let inner = &xml[content_start..content_start + close_offset];
                let trimmed = strip_html_tags(inner).trim().to_string();
                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
            }
        }
        search_pos = tag_start + open.len();
    }
    None
}

fn strip_html_tags(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            res.push(c);
        }
    }
    res.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn parse_ncx_toc(ncx_xml: &str, toc_titles: &mut HashMap<String, String>) {
    let lower = ncx_xml.to_lowercase();
    let mut search_pos = 0;
    while let Some(np_idx) = lower[search_pos..].find("<navpoint") {
        let np_start = search_pos + np_idx;
        let np_end = lower[np_start..]
            .find("</navpoint>")
            .map(|e| np_start + e + "</navpoint>".len())
            .unwrap_or(ncx_xml.len());

        let block = &ncx_xml[np_start..np_end];
        let text = extract_inner_text(block, "text");
        let src = extract_attr(block, "content", "src");

        if let (Some(t), Some(s)) = (text, src) {
            let clean_src = s.split('#').next().unwrap_or(&s).to_string();
            if !t.is_empty() && !clean_src.is_empty() {
                let file_name = Path::new(&clean_src)
                    .file_name()
                    .and_then(|os| os.to_str())
                    .unwrap_or(&clean_src)
                    .to_string();
                toc_titles.insert(clean_src, t.clone());
                toc_titles.insert(file_name, t);
            }
        }
        search_pos = np_start + "<navpoint".len();
    }
}

fn parse_nav_toc(nav_xml: &str, toc_titles: &mut HashMap<String, String>) {
    let lower = nav_xml.to_lowercase();
    let mut search_pos = 0;
    while let Some(idx) = lower[search_pos..].find("<a") {
        let a_start = search_pos + idx;
        let a_end = lower[a_start..]
            .find("</a>")
            .map(|e| a_start + e + "</a>".len())
            .unwrap_or(nav_xml.len());
        let a_block = &nav_xml[a_start..a_end];

        if let Some(href) = extract_attr(a_block, "a", "href") {
            let clean_href = href.split('#').next().unwrap_or(&href).to_string();
            if let Some(text) = extract_inner_text(a_block, "a") {
                if !text.is_empty() && !clean_href.is_empty() {
                    let file_name = Path::new(&clean_href)
                        .file_name()
                        .and_then(|os| os.to_str())
                        .unwrap_or(&clean_href)
                        .to_string();
                    toc_titles.insert(clean_href, text.clone());
                    toc_titles.insert(file_name, text);
                }
            }
        }
        search_pos = a_start + "<a".len();
    }
}
