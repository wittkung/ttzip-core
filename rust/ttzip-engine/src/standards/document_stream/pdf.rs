// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! PDF document dictionary and text streaming parser via lopdf.

use super::DocumentStreamError;

/// Metadata and extracted content of a PDF document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PdfDocumentInfo {
    pub format_version: String,
    pub page_count: u32,
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub creation_date: Option<String>,
    pub modification_date: Option<String>,
    pub is_encrypted: bool,
    pub extracted_text: Option<String>,
    pub extracted_page_count: u32,
}

/// Parses a PDF document directly from memory, extracting metadata dictionary and page text.
pub fn parse_pdf_from_memory(
    pdf_bytes: &[u8],
    max_pages_text: Option<u32>,
) -> Result<PdfDocumentInfo, DocumentStreamError> {
    let doc = lopdf::Document::load_mem(pdf_bytes)
        .map_err(|e| DocumentStreamError::PdfError(e.to_string()))?;

    let format_version = format!("PDF-{}", doc.version);
    let pages = doc.get_pages();
    let page_count = pages.len() as u32;
    let is_encrypted = doc.is_encrypted();

    let mut title = None;
    let mut author = None;
    let mut subject = None;
    let mut keywords = None;
    let mut creator = None;
    let mut producer = None;
    let mut creation_date = None;
    let mut modification_date = None;

    // Parse Info dictionary
    if let Ok(info_obj) = doc.trailer.get(b"Info") {
        let info_dict = match info_obj {
            lopdf::Object::Dictionary(dict) => Some(dict),
            lopdf::Object::Reference(id) => {
                if let Ok(lopdf::Object::Dictionary(dict)) = doc.get_object(*id) {
                    Some(dict)
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(dict) = info_dict {
            if let Ok(obj) = dict.get(b"Title") {
                title = decode_pdf_object_string(obj);
            }
            if let Ok(obj) = dict.get(b"Author") {
                author = decode_pdf_object_string(obj);
            }
            if let Ok(obj) = dict.get(b"Subject") {
                subject = decode_pdf_object_string(obj);
            }
            if let Ok(obj) = dict.get(b"Keywords") {
                keywords = decode_pdf_object_string(obj);
            }
            if let Ok(obj) = dict.get(b"Creator") {
                creator = decode_pdf_object_string(obj);
            }
            if let Ok(obj) = dict.get(b"Producer") {
                producer = decode_pdf_object_string(obj);
            }
            if let Ok(obj) = dict.get(b"CreationDate") {
                creation_date = decode_pdf_object_string(obj);
            }
            if let Ok(obj) = dict.get(b"ModDate") {
                modification_date = decode_pdf_object_string(obj);
            }
        }
    }

    // Extract text from the first N pages if requested
    let mut extracted_text = None;
    let mut extracted_page_count = 0;

    let pages_to_extract = match max_pages_text {
        Some(n) => n.min(page_count),
        None => page_count.min(10), // default to first 10 pages for preview
    };

    if pages_to_extract > 0 && !is_encrypted {
        let page_nums: Vec<u32> = (1..=pages_to_extract).collect();
        if let Ok(text) = doc.extract_text(&page_nums) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                extracted_text = Some(trimmed.to_string());
                extracted_page_count = pages_to_extract;
            }
        }
    }

    Ok(PdfDocumentInfo {
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
        extracted_text,
        extracted_page_count,
    })
}

/// Decodes string values from a PDF dictionary object (UTF-16BE/LE, UTF-8, or PDFDocEncoding).
fn decode_pdf_object_string(obj: &lopdf::Object) -> Option<String> {
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

/// Decodes PDF raw byte string handling BOM headers.
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
    // Try UTF-8
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    // Fallback: Latin-1 / PDFDocEncoding
    bytes.iter().map(|&b| b as char).collect()
}
