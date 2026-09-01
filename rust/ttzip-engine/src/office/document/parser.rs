// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust Word DOCX document parser and structured Markdown extractor.
//!
//! Provides zero-copy archive unpacking, structured paragraph and table extraction,
//! run styling inspection (bold, italic, font size), and Markdown formatting.

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::office::types::{OfficeError, OfficeResult};
use crate::zip::ZipArchive;

/// Paragraph text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocxAlignment {
    Left,
    Center,
    Right,
    Justify,
}

/// A contiguous run of text sharing identical character formatting.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocxRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub font_size: Option<f32>,
    pub color: Option<String>,
}

/// A paragraph in a Word DOCX document.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocxParagraph {
    pub text: String,
    pub runs: Vec<DocxRun>,
    pub style: Option<String>,
    pub heading_level: Option<u32>,
    pub alignment: Option<DocxAlignment>,
}

/// A single cell inside a Word document table.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocxTableCell {
    pub text: String,
    pub paragraphs: Vec<DocxParagraph>,
    pub grid_span: u32,
}

/// A row inside a Word document table.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocxTableRow {
    pub cells: Vec<DocxTableCell>,
}

/// A structured table in a Word DOCX document.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocxTable {
    pub rows: Vec<DocxTableRow>,
}

/// Top-level document flow items in order of appearance.
#[derive(Debug, Clone, PartialEq)]
pub enum DocxBodyItem {
    Paragraph(DocxParagraph),
    Table(DocxTable),
}

/// High-throughput Word DOCX document parser.
pub struct TTZipDocxParser {
    body_items: Vec<DocxBodyItem>,
    paragraphs: Vec<DocxParagraph>,
    tables: Vec<DocxTable>,
}

impl TTZipDocxParser {
    /// Parses a Word DOCX document from an in-memory byte slice.
    pub fn open_from_bytes(data: &[u8]) -> OfficeResult<Self> {
        if data.len() < 4 {
            return Err(OfficeError::UnsupportedFormat("Data payload too short".to_string()));
        }

        let zip = ZipArchive::open_slice(data).map_err(OfficeError::Zip)?;
        let doc_entry_idx = zip
            .entries()
            .iter()
            .position(|e| e.rel_path == "word/document.xml")
            .ok_or_else(|| OfficeError::Corrupt("Missing word/document.xml in DOCX package".to_string()))?;

        let doc_bytes = zip.extract_entry_bytes(doc_entry_idx, None).map_err(OfficeError::Zip)?;
        let body_items = parse_docx_document_xml(&doc_bytes)?;

        let mut paragraphs = Vec::new();
        let mut tables = Vec::new();

        for item in &body_items {
            match item {
                DocxBodyItem::Paragraph(p) => paragraphs.push(p.clone()),
                DocxBodyItem::Table(t) => tables.push(t.clone()),
            }
        }

        Ok(Self {
            body_items,
            paragraphs,
            tables,
        })
    }

    /// Returns all top-level paragraphs extracted from the document.
    pub fn paragraphs(&self) -> &[DocxParagraph] {
        &self.paragraphs
    }

    /// Returns all tables extracted from the document.
    pub fn tables(&self) -> &[DocxTable] {
        &self.tables
    }

    /// Returns all body flow items in sequential document order.
    pub fn body_items(&self) -> &[DocxBodyItem] {
        &self.body_items
    }

    /// Converts the entire document content into plain text with paragraph line breaks.
    pub fn to_plain_text(&self) -> String {
        let mut out = String::new();
        for item in &self.body_items {
            match item {
                DocxBodyItem::Paragraph(p) => {
                    if !p.text.is_empty() {
                        out.push_str(&p.text);
                        out.push_str("\n\n");
                    }
                }
                DocxBodyItem::Table(t) => {
                    for row in &t.rows {
                        let cell_texts: Vec<&str> = row.cells.iter().map(|c| c.text.as_str()).collect();
                        out.push_str(&cell_texts.join("\t"));
                        out.push('\n');
                    }
                    out.push('\n');
                }
            }
        }
        out.trim_end().to_string()
    }

    /// Converts the document into clean, GitHub-Flavored Markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();

        for item in &self.body_items {
            match item {
                DocxBodyItem::Paragraph(p) => {
                    if p.text.trim().is_empty() {
                        continue;
                    }

                    if let Some(level) = p.heading_level {
                        let hashes = "#".repeat(level.clamp(1, 6) as usize);
                        out.push_str(&format!("{} {}\n\n", hashes, p.text.trim()));
                        continue;
                    }

                    // Format styled paragraph runs
                    let mut p_md = String::new();
                    for run in &p.runs {
                        if run.text.is_empty() {
                            continue;
                        }
                        let mut text = run.text.clone();
                        if run.bold && run.italic {
                            text = format!("***{}***", text.trim());
                        } else if run.bold {
                            text = format!("**{}**", text.trim());
                        } else if run.italic {
                            text = format!("*{}*", text.trim());
                        } else if run.strike {
                            text = format!("~~{}~~", text.trim());
                        }
                        p_md.push_str(&text);
                    }

                    if !p_md.is_empty() {
                        out.push_str(&p_md);
                        out.push_str("\n\n");
                    }
                }
                DocxBodyItem::Table(t) => {
                    if t.rows.is_empty() {
                        continue;
                    }

                    // Build markdown table
                    for (i, row) in t.rows.iter().enumerate() {
                        let cells: Vec<String> = row
                            .cells
                            .iter()
                            .map(|c| c.text.trim().replace('|', "\\|").replace('\n', " "))
                            .collect();

                        out.push_str(&format!("| {} |\n", cells.join(" | ")));

                        // Header separator after row 0
                        if i == 0 {
                            let seps: Vec<&str> = row.cells.iter().map(|_| "---").collect();
                            out.push_str(&format!("| {} |\n", seps.join(" | ")));
                        }
                    }
                    out.push('\n');
                }
            }
        }

        out.trim_end().to_string()
    }
}

#[inline]
fn local_name(name: &[u8]) -> &[u8] {
    if let Some(pos) = name.iter().rposition(|&b| b == b':') {
        &name[pos + 1..]
    } else {
        name
    }
}

fn parse_docx_document_xml(xml_bytes: &[u8]) -> OfficeResult<Vec<DocxBodyItem>> {
    let mut reader = Reader::from_reader(xml_bytes);
    reader.config_mut().trim_text(false);

    let mut body_items = Vec::new();
    let mut in_table = false;
    let mut in_table_cell = false;

    let mut current_table = DocxTable::default();
    let mut current_row = DocxTableRow::default();
    let mut current_cell = DocxTableCell::default();

    let mut current_p = DocxParagraph::default();
    let mut current_run = DocxRun::default();
    let mut in_p_pr = false;
    let mut in_r = false;
    let mut in_r_pr = false;
    let mut in_t = false;
    let mut in_tc_pr = false;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = local_name(e.name().into_inner());
                match local {
                    b"tbl" => {
                        in_table = true;
                        current_table = DocxTable::default();
                    }
                    b"tr" => {
                        current_row = DocxTableRow::default();
                    }
                    b"tc" => {
                        in_table_cell = true;
                        current_cell = DocxTableCell {
                            grid_span: 1,
                            ..Default::default()
                        };
                    }
                    b"tcPr" => {
                        in_tc_pr = true;
                    }
                    b"p" => {
                        current_p = DocxParagraph::default();
                    }
                    b"pPr" => {
                        in_p_pr = true;
                    }
                    b"pStyle" if in_p_pr => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.into_inner()) == b"val" {
                                if let Ok(v) = std::str::from_utf8(&attr.value) {
                                    current_p.style = Some(v.to_string());
                                    if let Some(lvl) = parse_heading_level(v) {
                                        current_p.heading_level = Some(lvl);
                                    }
                                }
                            }
                        }
                    }
                    b"jc" if in_p_pr => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.into_inner()) == b"val" {
                                if let Ok(v) = std::str::from_utf8(&attr.value) {
                                    current_p.alignment = match v.to_ascii_lowercase().as_str() {
                                        "center" => Some(DocxAlignment::Center),
                                        "right" => Some(DocxAlignment::Right),
                                        "both" | "justify" => Some(DocxAlignment::Justify),
                                        _ => Some(DocxAlignment::Left),
                                    };
                                }
                            }
                        }
                    }
                    b"r" => {
                        in_r = true;
                        current_run = DocxRun::default();
                    }
                    b"rPr" => {
                        in_r_pr = true;
                    }
                    b"b" if in_r_pr => {
                        current_run.bold = true;
                    }
                    b"i" if in_r_pr => {
                        current_run.italic = true;
                    }
                    b"u" if in_r_pr => {
                        current_run.underline = true;
                    }
                    b"strike" if in_r_pr => {
                        current_run.strike = true;
                    }
                    b"sz" if in_r_pr => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.into_inner()) == b"val" {
                                if let Ok(v) = std::str::from_utf8(&attr.value) {
                                    if let Ok(val) = v.parse::<f32>() {
                                        current_run.font_size = Some(val / 2.0); // half-points to pt
                                    }
                                }
                            }
                        }
                    }
                    b"color" if in_r_pr => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.into_inner()) == b"val" {
                                if let Ok(v) = std::str::from_utf8(&attr.value) {
                                    current_run.color = Some(v.to_string());
                                }
                            }
                        }
                    }
                    b"t" if in_r => {
                        in_t = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = local_name(e.name().into_inner());
                match local {
                    b"pStyle" if in_p_pr => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.into_inner()) == b"val" {
                                if let Ok(v) = std::str::from_utf8(&attr.value) {
                                    current_p.style = Some(v.to_string());
                                    if let Some(lvl) = parse_heading_level(v) {
                                        current_p.heading_level = Some(lvl);
                                    }
                                }
                            }
                        }
                    }
                    b"jc" if in_p_pr => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.into_inner()) == b"val" {
                                if let Ok(v) = std::str::from_utf8(&attr.value) {
                                    current_p.alignment = match v.to_ascii_lowercase().as_str() {
                                        "center" => Some(DocxAlignment::Center),
                                        "right" => Some(DocxAlignment::Right),
                                        "both" | "justify" => Some(DocxAlignment::Justify),
                                        _ => Some(DocxAlignment::Left),
                                    };
                                }
                            }
                        }
                    }
                    b"gridSpan" if in_tc_pr => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.into_inner()) == b"val" {
                                if let Ok(v) = std::str::from_utf8(&attr.value) {
                                    if let Ok(val) = v.parse::<u32>() {
                                        current_cell.grid_span = val;
                                    }
                                }
                            }
                        }
                    }
                    b"b" if in_r_pr => {
                        current_run.bold = true;
                    }
                    b"i" if in_r_pr => {
                        current_run.italic = true;
                    }
                    b"u" if in_r_pr => {
                        current_run.underline = true;
                    }
                    b"strike" if in_r_pr => {
                        current_run.strike = true;
                    }
                    b"sz" if in_r_pr => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.into_inner()) == b"val" {
                                if let Ok(v) = std::str::from_utf8(&attr.value) {
                                    if let Ok(val) = v.parse::<f32>() {
                                        current_run.font_size = Some(val / 2.0);
                                    }
                                }
                            }
                        }
                    }
                    b"color" if in_r_pr => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.into_inner()) == b"val" {
                                if let Ok(v) = std::str::from_utf8(&attr.value) {
                                    current_run.color = Some(v.to_string());
                                }
                            }
                        }
                    }
                    b"tab" if in_r => {
                        current_run.text.push('\t');
                        current_p.text.push('\t');
                    }
                    b"br" if in_r => {
                        current_run.text.push('\n');
                        current_p.text.push('\n');
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_t {
                    if let Ok(t) = e.unescape() {
                        current_run.text.push_str(&t);
                        current_p.text.push_str(&t);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = local_name(e.name().into_inner());
                match local {
                    b"t" => in_t = false,
                    b"rPr" => in_r_pr = false,
                    b"r" => {
                        in_r = false;
                        if !current_run.text.is_empty() {
                            current_p.runs.push(current_run.clone());
                        }
                        current_run = DocxRun::default();
                    }
                    b"pPr" => in_p_pr = false,
                    b"tcPr" => in_tc_pr = false,
                    b"p" => {
                        if in_table_cell {
                            if !current_p.text.is_empty() {
                                if !current_cell.text.is_empty() {
                                    current_cell.text.push('\n');
                                }
                                current_cell.text.push_str(&current_p.text);
                            }
                            current_cell.paragraphs.push(current_p.clone());
                        } else if !in_table {
                            body_items.push(DocxBodyItem::Paragraph(current_p.clone()));
                        }
                        current_p = DocxParagraph::default();
                    }
                    b"tc" => {
                        in_table_cell = false;
                        current_row.cells.push(current_cell.clone());
                        current_cell = DocxTableCell::default();
                    }
                    b"tr" => {
                        current_table.rows.push(current_row.clone());
                        current_row = DocxTableRow::default();
                    }
                    b"tbl" => {
                        in_table = false;
                        body_items.push(DocxBodyItem::Table(current_table.clone()));
                        current_table = DocxTable::default();
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(OfficeError::Xml(e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(body_items)
}

fn parse_heading_level(style_str: &str) -> Option<u32> {
    let lower = style_str.to_ascii_lowercase();
    if lower.starts_with("heading") || lower.starts_with("title") {
        if lower.starts_with("title") {
            return Some(1);
        }
        let digits: String = lower.chars().filter(|c| c.is_ascii_digit()).collect();
        digits.parse::<u32>().ok().map(|lvl| lvl.clamp(1, 6))
    } else {
        None
    }
}
