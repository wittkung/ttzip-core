// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Pure Safe Rust PDF document dictionary and header parser without lopdf.

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
    if pdf_bytes.len() < 8 {
        return Err(DocumentStreamError::PdfError("PDF buffer too small".into()));
    }

    // 1. Format version
    let header_prefix = &pdf_bytes[..pdf_bytes.len().min(1024)];
    let format_version = if let Some(pos) = find_subsequence(header_prefix, b"%PDF-") {
        let ver_start = pos + 5;
        let mut ver_end = ver_start;
        while ver_end < header_prefix.len()
            && (header_prefix[ver_end].is_ascii_digit() || header_prefix[ver_end] == b'.')
        {
            ver_end += 1;
        }
        let ver_str = String::from_utf8_lossy(&header_prefix[ver_start..ver_end]);
        format!("PDF-{ver_str}")
    } else {
        return Err(DocumentStreamError::PdfError("Missing %PDF header".into()));
    };

    // 2. Encryption detection
    let is_encrypted = find_subsequence(pdf_bytes, b"/Encrypt").is_some();

    // 3. Page count
    let page_count = extract_page_count(pdf_bytes);

    // 4. Metadata dictionary extraction
    let title = extract_pdf_field(pdf_bytes, b"/Title");
    let author = extract_pdf_field(pdf_bytes, b"/Author");
    let subject = extract_pdf_field(pdf_bytes, b"/Subject");
    let keywords = extract_pdf_field(pdf_bytes, b"/Keywords");
    let creator = extract_pdf_field(pdf_bytes, b"/Creator");
    let producer = extract_pdf_field(pdf_bytes, b"/Producer");
    let creation_date = extract_pdf_field(pdf_bytes, b"/CreationDate");
    let modification_date = extract_pdf_field(pdf_bytes, b"/ModDate");

    // 5. Extracted text
    let mut extracted_text = None;
    let mut extracted_page_count = 0;
    let pages_to_extract = max_pages_text.unwrap_or_else(|| page_count.min(10));
    if pages_to_extract > 0 && !is_encrypted {
        if let Some(text) = extract_stream_text(pdf_bytes) {
            extracted_text = Some(text);
            extracted_page_count = pages_to_extract.min(page_count.max(1));
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

#[inline]
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn extract_page_count(data: &[u8]) -> u32 {
    if let Some(count) = parse_count_field(data) {
        return count;
    }
    count_page_type_nodes(data).max(1)
}

fn parse_count_field(data: &[u8]) -> Option<u32> {
    let pos = find_subsequence(data, b"/Count")?;
    let mut idx = pos + 6;
    while idx < data.len() && data[idx].is_ascii_whitespace() {
        idx += 1;
    }
    let start = idx;
    while idx < data.len() && data[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx <= start {
        return None;
    }
    let count = std::str::from_utf8(&data[start..idx]).ok()?.parse::<u32>().ok()?;
    (count > 0).then_some(count)
}

fn count_page_type_nodes(data: &[u8]) -> u32 {
    let mut count = 0u32;
    let mut idx = 0;
    while let Some(pos) = find_subsequence(&data[idx..], b"/Type") {
        let abs_pos = idx + pos;
        let mut check_idx = abs_pos + 5;
        while check_idx < data.len() && data[check_idx].is_ascii_whitespace() {
            check_idx += 1;
        }
        if check_idx + 5 <= data.len() && &data[check_idx..check_idx + 5] == b"/Page" {
            let next_byte = data.get(check_idx + 5).copied().unwrap_or(b' ');
            if next_byte != b's' && next_byte != b'S' {
                count += 1;
            }
        }
        idx = abs_pos + 5;
    }
    count
}

fn extract_pdf_field(data: &[u8], key: &[u8]) -> Option<String> {
    let pos = find_subsequence(data, key)?;
    let mut idx = pos + key.len();
    while idx < data.len() && data[idx].is_ascii_whitespace() {
        idx += 1;
    }
    let first = *data.get(idx)?;

    if first == b'(' {
        parse_literal_pdf_string(data, idx + 1)
    } else if first == b'<' && data.get(idx + 1).copied() != Some(b'<') {
        parse_hex_pdf_string(data, idx + 1)
    } else {
        None
    }
}

fn parse_literal_pdf_string(data: &[u8], mut idx: usize) -> Option<String> {
    let mut depth = 1usize;
    let mut raw_bytes = Vec::new();
    while idx < data.len() && depth > 0 {
        let b = data[idx];
        if b == b'\\' && idx + 1 < data.len() {
            idx += 1;
            raw_bytes.push(unescape_pdf_byte(data[idx]));
        } else if b == b'(' {
            depth += 1;
            raw_bytes.push(b);
        } else if b == b')' {
            depth -= 1;
            if depth > 0 {
                raw_bytes.push(b);
            }
        } else {
            raw_bytes.push(b);
        }
        idx += 1;
    }
    non_empty_pdf_string(&raw_bytes)
}

#[inline]
fn unescape_pdf_byte(escaped: u8) -> u8 {
    match escaped {
        b'n' => b'\n',
        b'r' => b'\r',
        b't' => b'\t',
        b'b' => 0x08,
        b'f' => 0x0C,
        _ => escaped,
    }
}

fn parse_hex_pdf_string(data: &[u8], mut idx: usize) -> Option<String> {
    let mut hex_str = String::new();
    while idx < data.len() && data[idx] != b'>' {
        if !data[idx].is_ascii_whitespace() {
            hex_str.push(data[idx] as char);
        }
        idx += 1;
    }
    if !hex_str.len().is_multiple_of(2) {
        hex_str.push('0');
    }
    let bytes = hex_decode_str(&hex_str).ok()?;
    non_empty_pdf_string(&bytes)
}

fn non_empty_pdf_string(bytes: &[u8]) -> Option<String> {
    let s = decode_pdf_string_bytes(bytes);
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn hex_decode_str(hex: &str) -> Result<Vec<u8>, ()> {
    if !hex.len().is_multiple_of(2) {
        return Err(());
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    let bytes = hex.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = hex_val(bytes[i]).ok_or(())?;
        let lo = hex_val(bytes[i + 1]).ok_or(())?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

#[inline]
fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn extract_stream_text(data: &[u8]) -> Option<String> {
    let mut collected = String::new();
    let mut idx = 0;

    while let Some(pos) = find_subsequence(&data[idx..], b"Tj") {
        let tj_pos = idx + pos;
        if let Some(text) = extract_tj_string_before(data, idx, tj_pos) {
            if !collected.is_empty() {
                collected.push(' ');
            }
            collected.push_str(&text);
        }
        idx = tj_pos + 2;
    }

    let trimmed = collected.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn extract_tj_string_before(data: &[u8], lower_bound: usize, tj_pos: usize) -> Option<String> {
    let mut close_paren = tj_pos;
    while close_paren > lower_bound && data[close_paren] != b')' {
        close_paren -= 1;
    }
    if data[close_paren] != b')' {
        return None;
    }

    let mut open_paren = close_paren;
    let mut depth = 1usize;
    while open_paren > lower_bound && depth > 0 {
        open_paren -= 1;
        if data[open_paren] == b'(' && (open_paren == 0 || data[open_paren - 1] != b'\\') {
            depth -= 1;
        }
    }

    if depth == 0 && close_paren > open_paren {
        let text = decode_pdf_string_bytes(&data[open_paren + 1..close_paren]);
        (!text.is_empty()).then_some(text)
    } else {
        None
    }
}

fn decode_pdf_string_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let (u16_pairs, _) = bytes[2..].as_chunks::<2>();
        let u16s: Vec<u16> = u16_pairs
            .iter()
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&u16s);
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let (u16_pairs, _) = bytes[2..].as_chunks::<2>();
        let u16s: Vec<u16> = u16_pairs
            .iter()
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&u16s);
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    bytes.iter().map(|&b| b as char).collect()
}
