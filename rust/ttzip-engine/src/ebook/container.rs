// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Canonical EPUB container rootfile resolution, archive entry lookup, and XHTML text helpers.

use quick_xml::events::Event;

use crate::ebook::navigation::local_name;
use crate::ebook::resource::clean_container_path;
use crate::ebook::{EbookError, EbookResult};
use crate::zip::ZipArchive;

/// Strips an optional UTF-8 byte order mark (`0xEF, 0xBB, 0xBF`) from the beginning of a slice.
#[inline]
pub fn strip_utf8_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)
}

/// Checks if a ZIP archive conforms to the EPUB container specification.
pub fn is_epub_container(zip: &ZipArchive<'_>) -> bool {
    let mut has_mimetype = false;
    let mut has_container = false;

    for entry in zip.entries() {
        let clean = clean_container_path(&entry.rel_path);
        if clean == "mimetype" {
            has_mimetype = true;
        } else if clean == "META-INF/container.xml" {
            has_container = true;
        }
    }

    has_container || has_mimetype
}

/// Parses the OPF package `full-path` attribute from `META-INF/container.xml`.
pub fn parse_container_rootfile(xml: &[u8]) -> EbookResult<String> {
    let mut reader = quick_xml::Reader::from_reader(strip_utf8_bom(xml));
    let mut buf = Vec::with_capacity(512);

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) | Event::Empty(ref e) => {
                if local_name(e.name().into_inner()).eq_ignore_ascii_case(b"rootfile") {
                    for attr in e.attributes().flatten() {
                        if local_name(attr.key.into_inner()).eq_ignore_ascii_case(b"full-path")
                            && let Ok(v) = attr.unescape_value()
                        {
                            let trimmed = v.trim();
                            if !trimmed.is_empty() {
                                return Ok(trimmed.to_string());
                            }
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Err(EbookError::NotFound(
        "Missing <rootfile full-path=...> in META-INF/container.xml".to_string(),
    ))
}

/// Finds and extracts raw decompressed entry bytes from an in-memory ZIP archive.
pub fn find_and_extract_zip_entry(zip: &ZipArchive<'_>, target_path: &str) -> Option<Vec<u8>> {
    let clean_target = clean_container_path(target_path);
    for (idx, entry) in zip.entries().iter().enumerate() {
        if clean_container_path(&entry.rel_path) == clean_target {
            return zip.extract_entry_bytes(idx, None).ok();
        }
    }

    let lower_target = clean_target.to_lowercase();
    for (idx, entry) in zip.entries().iter().enumerate() {
        if clean_container_path(&entry.rel_path).to_lowercase() == lower_target {
            return zip.extract_entry_bytes(idx, None).ok();
        }
    }
    None
}

/// Resolves the OPF package path inside an EPUB archive via `META-INF/container.xml` or `.opf` fallback.
pub fn resolve_epub_opf_path(zip: &ZipArchive<'_>) -> Option<String> {
    if let Some(bytes) = find_and_extract_zip_entry(zip, "META-INF/container.xml")
        && let Ok(path) = parse_container_rootfile(&bytes)
    {
        return Some(path);
    }

    zip.entries()
        .iter()
        .find(|e| e.rel_path.to_lowercase().ends_with(".opf"))
        .map(|e| e.rel_path.clone())
}

/// Returns the parent directory prefix of an OPF container path (or `""` if at root).
#[inline]
pub fn opf_parent_dir(opf_path: &str) -> &str {
    match opf_path.rfind('/') {
        Some(pos) => &opf_path[..pos],
        None => "",
    }
}

/// Strips XML/HTML markup tags and decodes common character entities.
pub fn strip_html_tags(s: &str) -> String {
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

/// Extracts inner text from the first matching element tag (`tag_name`, case-insensitive).
pub fn extract_inner_element_text(xml: &str, tag_name: &str) -> Option<String> {
    let lower = xml.to_lowercase();
    let tag_lower = tag_name.to_lowercase();
    let open = format!("<{tag_lower}");
    let close = format!("</{tag_lower}>");

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
        search_pos = next_idx;
    }
    None
}

/// Extracts the primary heading (`<h1>`, `<h2>`, or `<title>`) from XHTML/HTML content.
pub fn extract_html_heading(html: &str) -> Option<String> {
    for tag in ["h1", "h2", "title"] {
        if let Some(heading) = extract_inner_element_text(html, tag) {
            return Some(heading);
        }
    }
    None
}
