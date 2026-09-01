// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Core PDF parsing engine utilizing `lopdf` for zero-disk-landing inspection.

use std::collections::{HashMap, HashSet};

use crate::uniffi_api::types::TTZipError;
use super::types::{UniFFIPdfMetadata, UniFFIPdfOutlineNode, UniFFIPdfPageText, UniFFIPdfSearchResult};

// ============================================================================
// Metadata Extraction
// ============================================================================

/// Parses PDF document metadata properties, encryption status, and page counts from raw bytes.
pub fn parse_pdf_metadata_from_slice(bytes: &[u8]) -> Result<UniFFIPdfMetadata, TTZipError> {
    let doc = lopdf::Document::load_mem(bytes).map_err(|e| TTZipError::IoError {
        message: format!("PDF Parse Error: {e}"),
    })?;

    let format_version = format!("PDF-{}", doc.version);
    let pages = doc.get_pages();
    let page_count = pages.len() as u32;
    let is_encrypted = doc.is_encrypted();
    let file_size_bytes = bytes.len() as u64;

    let mut title = None;
    let mut author = None;
    let mut subject = None;
    let mut keywords = None;
    let mut creator = None;
    let mut producer = None;
    let mut creation_date = None;
    let mut modification_date = None;
    let mut custom_properties = HashMap::new();

    // Parse Info dictionary
    if let Ok(info_obj) = doc.trailer.get(b"Info") {
        if let Some(info_dict) = resolve_dictionary(&doc, info_obj) {
            for (key_bytes, val_obj) in info_dict.iter() {
                let key_name = String::from_utf8_lossy(key_bytes).to_string();
                let decoded_val = decode_pdf_object_string(val_obj);

                match key_name.as_str() {
                    "Title" => title = decoded_val,
                    "Author" => author = decoded_val,
                    "Subject" => subject = decoded_val,
                    "Keywords" => keywords = decoded_val,
                    "Creator" => creator = decoded_val,
                    "Producer" => producer = decoded_val,
                    "CreationDate" => creation_date = decoded_val,
                    "ModDate" => modification_date = decoded_val,
                    _ => {
                        if let Some(val) = decoded_val {
                            custom_properties.insert(key_name, val);
                        }
                    }
                }
            }
        }
    }

    // Check for outline bookmarks in document Catalog
    let has_outline = check_has_outline(&doc);

    Ok(UniFFIPdfMetadata {
        format_version,
        page_count,
        title,
        author,
        subject,
        keywords,
        creator,
        producer,
        creation_date,
        modification_date,
        is_encrypted,
        file_size_bytes,
        has_outline,
        custom_properties,
    })
}

// ============================================================================
// Outline Tree Extraction
// ============================================================================

/// Parses hierarchical outline bookmarks tree from PDF document bytes.
pub fn parse_pdf_outline_from_slice(bytes: &[u8]) -> Result<Vec<UniFFIPdfOutlineNode>, TTZipError> {
    let doc = lopdf::Document::load_mem(bytes).map_err(|e| TTZipError::IoError {
        message: format!("PDF Parse Error: {e}"),
    })?;

    let pages = doc.get_pages();
    let mut page_map = HashMap::with_capacity(pages.len());
    for (page_num, page_id) in pages.iter() {
        page_map.insert(*page_id, *page_num);
    }

    let root_obj = match doc.trailer.get(b"Root") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()),
    };

    let catalog_dict = match resolve_dictionary(&doc, root_obj) {
        Some(dict) => dict,
        None => return Ok(Vec::new()),
    };

    let outlines_obj = match catalog_dict.get(b"Outlines") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()),
    };

    let outlines_dict = match resolve_dictionary(&doc, outlines_obj) {
        Some(dict) => dict,
        None => return Ok(Vec::new()),
    };

    let first_id = match outlines_dict.get(b"First") {
        Ok(lopdf::Object::Reference(id)) => *id,
        _ => return Ok(Vec::new()),
    };

    let mut visited = HashSet::new();
    let nodes = parse_outline_siblings(&doc, first_id, &page_map, &mut visited, 0);
    Ok(nodes)
}

fn parse_outline_siblings(
    doc: &lopdf::Document,
    start_id: lopdf::ObjectId,
    page_map: &HashMap<lopdf::ObjectId, u32>,
    visited: &mut HashSet<lopdf::ObjectId>,
    depth: u32,
) -> Vec<UniFFIPdfOutlineNode> {
    if depth > 32 {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut curr_id = start_id;

    while !visited.contains(&curr_id) {
        visited.insert(curr_id);

        let item_obj = match doc.get_object(curr_id) {
            Ok(obj) => obj,
            Err(_) => break,
        };

        let item_dict = match resolve_dictionary(doc, item_obj) {
            Some(dict) => dict,
            None => break,
        };

        let title = item_dict
            .get(b"Title")
            .ok()
            .and_then(decode_pdf_object_string)
            .unwrap_or_else(|| "Untitled".to_string());

        let count = item_dict
            .get(b"Count")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0);
        let is_expanded = count >= 0;

        let (page_number, dest) = resolve_outline_destination(doc, item_dict, page_map);

        let mut children = Vec::new();
        if let Ok(lopdf::Object::Reference(child_id)) = item_dict.get(b"First") {
            children = parse_outline_siblings(doc, *child_id, page_map, visited, depth + 1);
        }

        result.push(UniFFIPdfOutlineNode {
            title,
            page_number,
            dest,
            is_expanded,
            children,
        });

        match item_dict.get(b"Next") {
            Ok(lopdf::Object::Reference(next_id)) => {
                curr_id = *next_id;
            }
            _ => break,
        }
    }

    result
}

fn resolve_outline_destination(
    doc: &lopdf::Document,
    dict: &lopdf::Dictionary,
    page_map: &HashMap<lopdf::ObjectId, u32>,
) -> (u32, Option<String>) {
    // 1. Direct /Dest
    if let Ok(dest_obj) = dict.get(b"Dest") {
        if let Some((page, dest_str)) = resolve_destination_object(doc, dest_obj, page_map) {
            return (page, dest_str);
        }
    }

    // 2. Action /A
    if let Ok(action_obj) = dict.get(b"A") {
        if let Some(action_dict) = resolve_dictionary(doc, action_obj) {
            if let Ok(lopdf::Object::Name(s_bytes)) = action_dict.get(b"S") {
                if s_bytes == b"GoTo" {
                    if let Ok(d_obj) = action_dict.get(b"D") {
                        if let Some((page, dest_str)) = resolve_destination_object(doc, d_obj, page_map) {
                            return (page, dest_str);
                        }
                    }
                } else if s_bytes == b"URI" {
                    if let Ok(uri_obj) = action_dict.get(b"URI") {
                        let uri_str = decode_pdf_object_string(uri_obj);
                        return (1, uri_str);
                    }
                }
            }
        }
    }

    (1, None)
}

fn resolve_destination_object(
    doc: &lopdf::Document,
    obj: &lopdf::Object,
    page_map: &HashMap<lopdf::ObjectId, u32>,
) -> Option<(u32, Option<String>)> {
    match obj {
        lopdf::Object::Array(arr) => {
            if let Some(first) = arr.first() {
                match first {
                    lopdf::Object::Reference(id) => {
                        let page = page_map.get(id).copied().unwrap_or(1);
                        Some((page, None))
                    }
                    lopdf::Object::Integer(num) => Some(((*num).max(1) as u32, None)),
                    _ => Some((1, None)),
                }
            } else {
                Some((1, None))
            }
        }
        lopdf::Object::Reference(id) => {
            if let Ok(resolved) = doc.get_object(*id) {
                resolve_destination_object(doc, resolved, page_map)
            } else {
                let page = page_map.get(id).copied().unwrap_or(1);
                Some((page, None))
            }
        }
        lopdf::Object::String(..) | lopdf::Object::Name(..) => {
            let dest_name = decode_pdf_object_string(obj);
            Some((1, dest_name))
        }
        lopdf::Object::Integer(num) => Some(((*num).max(1) as u32, None)),
        _ => None,
    }
}

// ============================================================================
// Page Text Extraction
// ============================================================================

/// Extracts plain text and character/word metrics for a specific 1-based page.
pub fn extract_pdf_page_text_from_slice(bytes: &[u8], page_number: u32) -> Result<UniFFIPdfPageText, TTZipError> {
    let doc = lopdf::Document::load_mem(bytes).map_err(|e| TTZipError::IoError {
        message: format!("PDF Parse Error: {e}"),
    })?;

    let total_pages = doc.get_pages().len() as u32;
    if page_number == 0 || page_number > total_pages {
        return Err(TTZipError::IoError {
            message: format!("Page number {page_number} is out of bounds (1..={total_pages})"),
        });
    }

    let text = if doc.is_encrypted() {
        String::new()
    } else {
        doc.extract_text(&[page_number]).unwrap_or_default()
    };

    let character_count = text.chars().count() as u32;
    let word_count = text.split_whitespace().count() as u32;

    Ok(UniFFIPdfPageText {
        page_number,
        text,
        character_count,
        word_count,
    })
}

/// Extracts plain text from all pages in a document up to `max_pages`.
pub fn extract_all_pages_text_from_slice(
    bytes: &[u8],
    max_pages: Option<u32>,
) -> Result<Vec<UniFFIPdfPageText>, TTZipError> {
    let doc = lopdf::Document::load_mem(bytes).map_err(|e| TTZipError::IoError {
        message: format!("PDF Parse Error: {e}"),
    })?;

    let total_pages = doc.get_pages().len() as u32;
    let count = match max_pages {
        Some(max) => max.min(total_pages),
        None => total_pages,
    };

    let mut results = Vec::with_capacity(count as usize);
    let is_encrypted = doc.is_encrypted();

    for p in 1..=count {
        let text = if is_encrypted {
            String::new()
        } else {
            doc.extract_text(&[p]).unwrap_or_default()
        };

        let character_count = text.chars().count() as u32;
        let word_count = text.split_whitespace().count() as u32;

        results.push(UniFFIPdfPageText {
            page_number: p,
            text,
            character_count,
            word_count,
        });
    }

    Ok(results)
}

// ============================================================================
// Full-Text Keyword Search
// ============================================================================

/// Searches for substring occurrences across all pages of a PDF document.
pub fn search_pdf_text_from_slice(
    bytes: &[u8],
    query: &str,
    max_results: u32,
    case_sensitive: bool,
) -> Result<Vec<UniFFIPdfSearchResult>, TTZipError> {
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let doc = lopdf::Document::load_mem(bytes).map_err(|e| TTZipError::IoError {
        message: format!("PDF Parse Error: {e}"),
    })?;

    if doc.is_encrypted() {
        return Ok(Vec::new());
    }

    let pages = doc.get_pages();
    let limit = if max_results == 0 { usize::MAX } else { max_results as usize };
    let mut results = Vec::new();

    let query_chars_len = query.chars().count() as u32;
    let query_lower = if !case_sensitive { query.to_lowercase() } else { String::new() };

    for &page_num in pages.keys() {
        if let Ok(text) = doc.extract_text(&[page_num]) {
            if text.is_empty() {
                continue;
            }

            if case_sensitive {
                for (byte_idx, _) in text.match_indices(query) {
                    let char_offset = text[..byte_idx].chars().count() as u32;
                    let match_text = build_snippet(&text, char_offset as usize, query_chars_len as usize);
                    results.push(UniFFIPdfSearchResult {
                        page_number: page_num,
                        match_text,
                        char_offset,
                        match_length: query_chars_len,
                    });
                    if results.len() >= limit {
                        return Ok(results);
                    }
                }
            } else {
                let text_lower = text.to_lowercase();
                for (byte_idx, _) in text_lower.match_indices(&query_lower) {
                    let char_offset = text_lower[..byte_idx].chars().count() as u32;
                    let match_text = build_snippet(&text, char_offset as usize, query_chars_len as usize);
                    results.push(UniFFIPdfSearchResult {
                        page_number: page_num,
                        match_text,
                        char_offset,
                        match_length: query_chars_len,
                    });
                    if results.len() >= limit {
                        return Ok(results);
                    }
                }
            }
        }
    }

    Ok(results)
}

fn build_snippet(text: &str, char_offset: usize, match_len: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();
    let context_len = 35;
    let start = char_offset.saturating_sub(context_len);
    let end = (char_offset + match_len + context_len).min(total);

    let mut snippet = String::new();
    if start > 0 {
        snippet.push_str("... ");
    }
    for &c in &chars[start..end] {
        if c == '\n' || c == '\r' || c == '\t' {
            snippet.push(' ');
        } else {
            snippet.push(c);
        }
    }
    if end < total {
        snippet.push_str(" ...");
    }
    snippet.trim().to_string()
}

// ============================================================================
// Internal PDF Decoding Helpers
// ============================================================================

fn resolve_dictionary<'a>(doc: &'a lopdf::Document, obj: &'a lopdf::Object) -> Option<&'a lopdf::Dictionary> {
    match obj {
        lopdf::Object::Dictionary(dict) => Some(dict),
        lopdf::Object::Reference(id) => {
            if let Ok(lopdf::Object::Dictionary(dict)) = doc.get_object(*id) {
                Some(dict)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn check_has_outline(doc: &lopdf::Document) -> bool {
    if let Ok(root_obj) = doc.trailer.get(b"Root") {
        if let Some(catalog) = resolve_dictionary(doc, root_obj) {
            if let Ok(outlines_obj) = catalog.get(b"Outlines") {
                if let Some(outlines_dict) = resolve_dictionary(doc, outlines_obj) {
                    return outlines_dict.get(b"First").is_ok();
                }
            }
        }
    }
    false
}

pub(crate) fn decode_pdf_object_string(obj: &lopdf::Object) -> Option<String> {
    match obj {
        lopdf::Object::String(bytes, _) => {
            let s = decode_pdf_string_bytes(bytes);
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        lopdf::Object::Name(bytes) => {
            let s = String::from_utf8_lossy(bytes).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        _ => None,
    }
}

fn decode_pdf_string_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    // Check UTF-16BE BOM
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&u16s);
    }
    // Check UTF-16LE BOM
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&u16s);
    }
    // Try standard UTF-8
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    // Fallback: Latin-1 / PDFDocEncoding
    bytes.iter().map(|&b| b as char).collect()
}
