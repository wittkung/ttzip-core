// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! DOCX Structured Document Parser, Heading Hierarchy Analyzer,
//! Table Matrix Extractor, and Native Markdown Conversion Engine.

use std::io::Cursor;

use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;

use super::types::{
    UniFFIDocxDocument, UniFFIDocxParagraph, UniFFIDocxTable, UniFFIDocxTableRow, UniFFIOfficeError,
};
use crate::uniffi_api::xml_meta::office::parse_core_properties_xml;
use crate::uniffi_api::xml_meta::types::UniFFIDocumentMetadata;
use crate::zip::reader::ZipArchive;

/// Parses a DOCX document archive extracting paragraphs, tables, outline metrics, and Markdown.
pub fn parse_docx_archive(bytes: &[u8]) -> Result<UniFFIDocxDocument, UniFFIOfficeError> {
    let zip = ZipArchive::open_slice(bytes)
        .map_err(|e| UniFFIOfficeError::corrupted(format!("Failed to open DOCX archive: {e:?}")))?;

    // 1. Extract optional title from docProps/core.xml
    let mut title = None;
    if let Some(core_bytes) = find_zip_entry(&zip, "docProps/core.xml") {
        let mut meta = UniFFIDocumentMetadata::default();
        parse_core_properties_xml(&core_bytes, &mut meta);
        title = meta.title;
    }

    // 2. Extract word/document.xml
    let doc_bytes = find_zip_entry(&zip, "word/document.xml").ok_or_else(|| {
        UniFFIOfficeError::entry_not_found("Required entry 'word/document.xml' not found")
    })?;

    // 3. Parse paragraphs, tables, and render markdown
    let (paragraphs, tables, markdown) = parse_document_xml(&doc_bytes, title.as_deref())?;

    // 4. Compute document metrics
    let (total_words, total_chars) = compute_docx_metrics(&paragraphs, &tables);

    // If title was not in core.xml, check if a Title paragraph exists
    let resolved_title = title.or_else(|| {
        paragraphs
            .iter()
            .find(|p| p.heading_level == Some(0) || p.style.eq_ignore_ascii_case("Title"))
            .map(|p| p.text.clone())
    });

    Ok(UniFFIDocxDocument {
        title: resolved_title,
        paragraphs,
        tables,
        total_words,
        total_characters: total_chars,
        markdown_content: markdown,
    })
}

// ============================================================================
// Internal Parsing and Markdown Construction
// ============================================================================

fn find_zip_entry(zip: &ZipArchive<'_>, target_path: &str) -> Option<Vec<u8>> {
    let target_norm = target_path.to_lowercase().replace('\\', "/");
    for (idx, entry) in zip.entries().iter().enumerate() {
        if entry.rel_path.to_lowercase().replace('\\', "/") == target_norm {
            return zip.extract_entry_bytes(idx, None).ok();
        }
    }
    None
}

enum DocxBodyItem {
    Paragraph(UniFFIDocxParagraph),
    Table(UniFFIDocxTable),
}

fn parse_document_xml(
    xml_bytes: &[u8],
    declared_title: Option<&str>,
) -> Result<(Vec<UniFFIDocxParagraph>, Vec<UniFFIDocxTable>, String), UniFFIOfficeError> {
    let mut reader = XmlReader::from_reader(Cursor::new(xml_bytes));
    reader.config_mut().trim_text(false);

    let mut body_items: Vec<DocxBodyItem> = Vec::new();
    let mut paragraphs = Vec::new();
    let mut tables = Vec::new();

    let mut buf = Vec::with_capacity(512);

    // Parsing states
    let mut in_table = false;
    let mut in_table_row = false;
    let mut in_table_cell = false;

    let mut current_table_rows: Vec<UniFFIDocxTableRow> = Vec::new();
    let mut current_row_cells: Vec<String> = Vec::new();
    let mut current_cell_text = String::new();

    let mut in_paragraph = false;
    let mut current_p_text = String::with_capacity(256);
    let mut current_p_style = String::new();
    let mut current_heading_lvl: Option<u32> = None;
    let mut current_is_list = false;
    let mut current_list_lvl: Option<u32> = None;
    let mut in_text = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"tbl" => {
                        in_table = true;
                        current_table_rows.clear();
                    }
                    b"tr" if in_table => {
                        in_table_row = true;
                        current_row_cells.clear();
                    }
                    b"tc" if in_table_row => {
                        in_table_cell = true;
                        current_cell_text.clear();
                    }
                    b"p" => {
                        in_paragraph = true;
                        if !in_table_cell {
                            current_p_text.clear();
                            current_p_style.clear();
                            current_heading_lvl = None;
                            current_is_list = false;
                            current_list_lvl = None;
                        }
                    }
                    b"t" if in_paragraph => {
                        in_text = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"pStyle" if in_paragraph && !in_table_cell => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"val" || attr.key.as_ref() == b"w:val" {
                                if let Ok(val) = String::from_utf8(attr.value.to_vec()) {
                                    current_p_style = val.clone();
                                    current_heading_lvl = detect_heading_level(&val);
                                    if val.to_lowercase().contains("list") {
                                        current_is_list = true;
                                    }
                                }
                            }
                        }
                    }
                    b"outlineLvl" if in_paragraph && !in_table_cell => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"val" || attr.key.as_ref() == b"w:val" {
                                if let Ok(val) = String::from_utf8(attr.value.to_vec()) {
                                    if let Ok(lvl) = val.parse::<u32>() {
                                        current_heading_lvl = Some(lvl.saturating_add(1));
                                    }
                                }
                            }
                        }
                    }
                    b"numPr" | b"numId" if in_paragraph && !in_table_cell => {
                        current_is_list = true;
                    }
                    b"ilvl" if in_paragraph && !in_table_cell => {
                        current_is_list = true;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"val" || attr.key.as_ref() == b"w:val" {
                                if let Ok(val) = String::from_utf8(attr.value.to_vec()) {
                                    current_list_lvl = val.parse::<u32>().ok();
                                }
                            }
                        }
                    }
                    b"tab" if in_paragraph => {
                        if in_table_cell {
                            current_cell_text.push('\t');
                        } else {
                            current_p_text.push('\t');
                        }
                    }
                    b"br" | b"cr" if in_paragraph => {
                        if in_table_cell {
                            current_cell_text.push(' ');
                        } else {
                            current_p_text.push('\n');
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) if in_text => {
                if let Ok(txt) = e.unescape() {
                    if in_table_cell {
                        current_cell_text.push_str(&txt);
                    } else {
                        current_p_text.push_str(&txt);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"t" => in_text = false,
                    b"p" => {
                        in_paragraph = false;
                        if in_table_cell {
                            if !current_cell_text.is_empty() && !current_cell_text.ends_with(' ') {
                                current_cell_text.push(' ');
                            }
                        } else {
                            let trimmed = current_p_text.trim();
                            if !trimmed.is_empty() {
                                let p = UniFFIDocxParagraph {
                                    style: if current_p_style.is_empty() {
                                        "Normal".to_string()
                                    } else {
                                        current_p_style.clone()
                                    },
                                    text: trimmed.to_string(),
                                    heading_level: current_heading_lvl,
                                    is_list_item: current_is_list,
                                    list_level: current_list_lvl,
                                };
                                paragraphs.push(p.clone());
                                body_items.push(DocxBodyItem::Paragraph(p));
                            }
                        }
                    }
                    b"tc" if in_table_row => {
                        in_table_cell = false;
                        current_row_cells.push(current_cell_text.trim().to_string());
                        current_cell_text.clear();
                    }
                    b"tr" if in_table => {
                        in_table_row = false;
                        if !current_row_cells.is_empty() {
                            current_table_rows.push(UniFFIDocxTableRow {
                                cells: current_row_cells.clone(),
                            });
                        }
                        current_row_cells.clear();
                    }
                    b"tbl" => {
                        in_table = false;
                        if !current_table_rows.is_empty() {
                            let max_cols = current_table_rows
                                .iter()
                                .map(|r| r.cells.len())
                                .max()
                                .unwrap_or(0) as u32;
                            let headers = current_table_rows
                                .first()
                                .map(|r| r.cells.clone())
                                .unwrap_or_default();
                            let t = UniFFIDocxTable {
                                total_rows: current_table_rows.len() as u32,
                                total_cols: max_cols,
                                headers,
                                rows: current_table_rows.clone(),
                            };
                            tables.push(t.clone());
                            body_items.push(DocxBodyItem::Table(t));
                        }
                        current_table_rows.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(UniFFIOfficeError::xml_err(format!("DOCX XML error: {e:?}")));
            }
            _ => {}
        }
        buf.clear();
    }

    // Build Markdown output
    let markdown = render_markdown_document(&body_items, declared_title);

    Ok((paragraphs, tables, markdown))
}

fn detect_heading_level(style: &str) -> Option<u32> {
    let lower = style.to_lowercase();
    if lower == "title" {
        Some(0)
    } else if lower == "subtitle" {
        Some(2)
    } else if lower.starts_with("heading") || lower.starts_with("heading ") {
        let num_str = lower.trim_start_matches("heading").trim();
        num_str.parse::<u32>().ok().or(Some(1))
    } else if lower.starts_with("toc") {
        let num_str = lower.trim_start_matches("toc").trim();
        num_str.parse::<u32>().ok().or(Some(1))
    } else {
        None
    }
}

fn render_markdown_document(items: &[DocxBodyItem], declared_title: Option<&str>) -> String {
    let mut md = String::with_capacity(1024);

    if let Some(t) = declared_title {
        let t_clean = t.trim();
        if !t_clean.is_empty() {
            md.push_str(&format!("# {t_clean}\n\n"));
        }
    }

    for item in items {
        match item {
            DocxBodyItem::Paragraph(p) => {
                if let Some(lvl) = p.heading_level {
                    if lvl == 0 {
                        // Title
                        if declared_title.is_none() {
                            md.push_str(&format!("# {}\n\n", p.text));
                        }
                    } else {
                        let hashes = "#".repeat((lvl as usize).min(6));
                        md.push_str(&format!("{hashes} {}\n\n", p.text));
                    }
                } else if p.is_list_item {
                    let indent = "  ".repeat(p.list_level.unwrap_or(0) as usize);
                    md.push_str(&format!("{indent}- {}\n", p.text));
                } else {
                    md.push_str(&format!("{}\n\n", p.text));
                }
            }
            DocxBodyItem::Table(t) => {
                if !t.rows.is_empty() {
                    let col_count = t.total_cols as usize;
                    if col_count > 0 {
                        // Header row
                        if let Some(first_row) = t.rows.first() {
                            md.push('|');
                            for i in 0..col_count {
                                let cell_val = first_row.cells.get(i).map(|s| s.as_str()).unwrap_or("");
                                md.push_str(&format!(" {} |", cell_val.replace('|', "\\|")));
                            }
                            md.push('\n');

                            // Separator row
                            md.push('|');
                            for _ in 0..col_count {
                                md.push_str(" --- |");
                            }
                            md.push('\n');

                            // Data rows (skip first row if used as header)
                            for row in t.rows.iter().skip(1) {
                                md.push('|');
                                for i in 0..col_count {
                                    let cell_val = row.cells.get(i).map(|s| s.as_str()).unwrap_or("");
                                    md.push_str(&format!(" {} |", cell_val.replace('|', "\\|")));
                                }
                                md.push('\n');
                            }
                            md.push('\n');
                        }
                    }
                }
            }
        }
    }

    md.trim().to_string()
}

fn compute_docx_metrics(
    paragraphs: &[UniFFIDocxParagraph],
    tables: &[UniFFIDocxTable],
) -> (u32, u32) {
    let mut total_words = 0u32;
    let mut total_chars = 0u32;

    for p in paragraphs {
        total_chars = total_chars.saturating_add(p.text.chars().count() as u32);
        total_words = total_words.saturating_add(p.text.split_whitespace().count() as u32);
    }

    for t in tables {
        for row in &t.rows {
            for cell in &row.cells {
                total_chars = total_chars.saturating_add(cell.chars().count() as u32);
                total_words = total_words.saturating_add(cell.split_whitespace().count() as u32);
            }
        }
    }

    (total_words, total_chars)
}
