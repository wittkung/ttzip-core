// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Office Open XML (DOCX, XLSX, PPTX) metadata and outline extraction.

use std::io::Cursor;

use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;

use crate::standards::document_stream::docx::{
    compute_docx_xml_stats, parse_core_properties_entries,
};
use crate::uniffi_api::types::TTZipError;
use crate::zip::reader::ZipArchive;
use super::types::{UniFFIDocumentMetadata, UniFFIOfficeOutline};
use super::find_and_extract_entry;

fn detect_office_format_name(zip: &ZipArchive) -> &'static str {
    if zip.entries().iter().any(|e| e.rel_path.starts_with("word/")) {
        "DOCX"
    } else if zip.entries().iter().any(|e| e.rel_path.starts_with("xl/")) {
        "XLSX"
    } else if zip.entries().iter().any(|e| e.rel_path.starts_with("ppt/")) {
        "PPTX"
    } else {
        "Office Open XML"
    }
}

fn collect_sorted_pptx_slide_paths(zip: &ZipArchive) -> Vec<String> {
    let mut slide_entries: Vec<String> = zip
        .entries()
        .iter()
        .filter(|e| e.rel_path.starts_with("ppt/slides/slide") && e.rel_path.ends_with(".xml"))
        .map(|e| e.rel_path.clone())
        .collect();
    slide_entries.sort();
    slide_entries
}

fn populate_pptx_slide_metadata(zip: &ZipArchive, meta: &mut UniFFIDocumentMetadata) {
    let slide_entries = collect_sorted_pptx_slide_paths(zip);
    if meta.slide_count == 0 {
        meta.slide_count = slide_entries.len() as u32;
    }
    if meta.slide_titles.is_empty() {
        for slide_path in &slide_entries {
            if let Some(slide_bytes) = find_and_extract_entry(zip, slide_path) {
                if let Some(title) = extract_pptx_slide_title(&slide_bytes) {
                    meta.slide_titles.push(title);
                }
            }
        }
    }
}

fn populate_docx_stats_fallback(zip: &ZipArchive, meta: &mut UniFFIDocumentMetadata) {
    if let Some(doc_bytes) = find_and_extract_entry(zip, "word/document.xml") {
        let (w_cnt, c_cnt, p_cnt) = compute_docx_text_stats(&doc_bytes);
        meta.word_count = w_cnt;
        meta.character_count = c_cnt;
        if meta.page_count == 0 {
            meta.page_count = (p_cnt / 5).max(1);
        }
    }
}

/// Parses Office Open XML document metadata directly from byte slice.
pub fn parse_office_metadata_from_slice(bytes: &[u8]) -> Result<UniFFIDocumentMetadata, TTZipError> {
    let zip = ZipArchive::open_slice(bytes).map_err(|e| TTZipError::CorruptHeader {
        details: format!("Failed to open Office archive: {e:?}"),
        offset: 0,
    })?;

    let mut meta = UniFFIDocumentMetadata {
        format_name: detect_office_format_name(&zip).to_string(),
        ..Default::default()
    };

    if let Some(core_bytes) = find_and_extract_entry(&zip, "docProps/core.xml") {
        parse_core_properties_xml(&core_bytes, &mut meta);
    }
    if let Some(app_bytes) = find_and_extract_entry(&zip, "docProps/app.xml") {
        parse_app_properties_xml(&app_bytes, &mut meta);
    }

    if meta.format_name == "XLSX" && meta.sheet_names.is_empty() {
        if let Some(wb_bytes) = find_and_extract_entry(&zip, "xl/workbook.xml") {
            parse_xlsx_workbook_sheets(&wb_bytes, &mut meta);
        }
    }
    meta.sheet_count = meta.sheet_names.len() as u32;

    if meta.format_name == "PPTX" {
        populate_pptx_slide_metadata(&zip, &mut meta);
    } else if meta.format_name == "DOCX" && meta.word_count == 0 {
        populate_docx_stats_fallback(&zip, &mut meta);
    }

    Ok(meta)
}

/// Parses structural outline and preview text from an Office document byte slice.
pub fn parse_office_outline_from_slice(bytes: &[u8]) -> Result<UniFFIOfficeOutline, TTZipError> {
    let zip = ZipArchive::open_slice(bytes).map_err(|e| TTZipError::CorruptHeader {
        details: format!("Failed to open Office archive: {e:?}"),
        offset: 0,
    })?;

    let has_word = zip.entries().iter().any(|e| e.rel_path.starts_with("word/"));
    let has_xl = zip.entries().iter().any(|e| e.rel_path.starts_with("xl/"));
    let has_ppt = zip.entries().iter().any(|e| e.rel_path.starts_with("ppt/"));

    let mut outline = UniFFIOfficeOutline::default();

    if has_word {
        outline.document_type = "Word Processing".to_string();
        if let Some(doc_bytes) = find_and_extract_entry(&zip, "word/document.xml") {
            let (headings, preview) = extract_docx_outline_and_preview(&doc_bytes);
            outline.headings = headings;
            outline.summary_preview = preview;
        }
        outline.total_sections = outline.headings.len() as u32;
    } else if has_xl {
        outline.document_type = "Spreadsheet".to_string();
        if let Some(wb_bytes) = find_and_extract_entry(&zip, "xl/workbook.xml") {
            let mut meta = UniFFIDocumentMetadata::default();
            parse_xlsx_workbook_sheets(&wb_bytes, &mut meta);
            outline.sheets = meta.sheet_names;
        }
        if outline.sheets.is_empty() {
            if let Some(app_bytes) = find_and_extract_entry(&zip, "docProps/app.xml") {
                let mut meta = UniFFIDocumentMetadata::default();
                parse_app_properties_xml(&app_bytes, &mut meta);
                outline.sheets = meta.sheet_names;
            }
        }
        outline.total_sections = outline.sheets.len() as u32;
        outline.summary_preview = format!("Workbook containing {} worksheet(s)", outline.sheets.len());
    } else if has_ppt {
        outline.document_type = "Presentation".to_string();
        let slide_entries = collect_sorted_pptx_slide_paths(&zip);
        for (idx, slide_path) in slide_entries.iter().enumerate() {
            if let Some(slide_bytes) = find_and_extract_entry(&zip, slide_path) {
                let title = extract_pptx_slide_title(&slide_bytes)
                    .unwrap_or_else(|| format!("Slide {}", idx + 1));
                outline.slides.push(title);
            }
        }
        outline.total_sections = outline.slides.len() as u32;
        outline.summary_preview = format!("Presentation deck with {} slide(s)", outline.slides.len());
    } else {
        outline.document_type = "Office Document".to_string();
        outline.summary_preview = "Generic compound Office container".to_string();
    }

    Ok(outline)
}

fn apply_core_property_field(meta: &mut UniFFIDocumentMetadata, tag: &str, val: String) {
    if val.is_empty() {
        return;
    }
    match tag {
        "title" => meta.title = Some(val),
        "creator" => meta.author = Some(val),
        "subject" => meta.subject = Some(val),
        "description" => meta.description = Some(val),
        "keywords" => meta.keywords = Some(val),
        "lastModifiedBy" => meta.last_modified_by = Some(val),
        "created" => meta.created_date = Some(val),
        "modified" => meta.modified_date = Some(val),
        other => {
            meta.custom_properties.insert(other.to_string(), val);
        }
    }
}

pub(crate) fn parse_core_properties_xml(xml_bytes: &[u8], meta: &mut UniFFIDocumentMetadata) {
    let _ = parse_core_properties_entries(xml_bytes, |tag, val| {
        apply_core_property_field(meta, tag, val);
    });
}

fn apply_app_property_field(
    meta: &mut UniFFIDocumentMetadata,
    tag: &str,
    val: String,
    in_titles_of_parts: bool,
) {
    if val.is_empty() {
        return;
    }
    if in_titles_of_parts && (tag == "lpstr" || tag == "string") {
        if meta.format_name == "XLSX" {
            meta.sheet_names.push(val);
        } else if meta.format_name == "PPTX" {
            meta.slide_titles.push(val);
        }
        return;
    }
    match tag {
        "Application" => meta.application = Some(val),
        "Pages" => meta.page_count = val.parse::<u32>().unwrap_or(meta.page_count),
        "Words" => meta.word_count = val.parse::<u32>().unwrap_or(meta.word_count),
        "Characters" => meta.character_count = val.parse::<u32>().unwrap_or(meta.character_count),
        "Slides" => meta.slide_count = val.parse::<u32>().unwrap_or(meta.slide_count),
        _ => {}
    }
}

pub(crate) fn parse_app_properties_xml(xml_bytes: &[u8], meta: &mut UniFFIDocumentMetadata) {
    let mut reader = XmlReader::from_reader(Cursor::new(xml_bytes));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::with_capacity(512);
    let mut current_tag = String::new();
    let mut current_text = String::new();
    let mut in_titles_of_parts = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if tag == "TitlesOfParts" {
                    in_titles_of_parts = true;
                }
                current_tag = tag;
                current_text.clear();
            }
            Ok(Event::Text(ref e)) => {
                if let Ok(txt) = e.unescape() {
                    current_text.push_str(&txt);
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if tag == "TitlesOfParts" {
                    in_titles_of_parts = false;
                }
                if tag == current_tag {
                    apply_app_property_field(
                        meta,
                        &tag,
                        current_text.trim().to_string(),
                        in_titles_of_parts,
                    );
                    current_tag.clear();
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
}

pub(crate) fn parse_xlsx_workbook_sheets(xml_bytes: &[u8], meta: &mut UniFFIDocumentMetadata) {
    let mut reader = XmlReader::from_reader(Cursor::new(xml_bytes));
    let mut buf = Vec::with_capacity(512);

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                if local.as_ref() == b"sheet" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"name" {
                            if let Ok(val) = String::from_utf8(attr.value.to_vec()) {
                                if !val.is_empty() && !meta.sheet_names.contains(&val) {
                                    meta.sheet_names.push(val);
                                }
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
}

fn append_pptx_text_run(title_buf: &mut String, txt: &str) -> bool {
    let t = txt.trim();
    if t.is_empty() {
        return false;
    }
    if !title_buf.is_empty() {
        title_buf.push(' ');
    }
    title_buf.push_str(t);
    title_buf.len() >= 60
}

pub(crate) fn extract_pptx_slide_title(xml_bytes: &[u8]) -> Option<String> {
    let mut reader = XmlReader::from_reader(Cursor::new(xml_bytes));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::with_capacity(512);
    let mut in_text = false;
    let mut title_buf = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.local_name().as_ref() == b"t" => in_text = true,
            Ok(Event::Text(ref e)) if in_text => {
                if let Ok(txt) = e.unescape() {
                    if append_pptx_text_run(&mut title_buf, &txt) {
                        return Some(title_buf);
                    }
                }
            }
            Ok(Event::End(ref e)) if e.local_name().as_ref() == b"t" => {
                in_text = false;
                if !title_buf.is_empty() {
                    return Some(title_buf);
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    (!title_buf.is_empty()).then_some(title_buf)
}

pub(crate) fn compute_docx_text_stats(xml_bytes: &[u8]) -> (u32, u32, u32) {
    compute_docx_xml_stats(xml_bytes)
}

fn has_heading_style_attr(e: &quick_xml::events::BytesStart<'_>) -> bool {
    e.attributes().flatten().any(|attr| {
        if attr.key.as_ref() == b"val" || attr.key.as_ref() == b"w:val" {
            let val_str = String::from_utf8_lossy(&attr.value).to_lowercase();
            val_str.contains("heading") || val_str.contains("title")
        } else {
            false
        }
    })
}

fn commit_docx_paragraph(
    headings: &mut Vec<String>,
    full_preview: &mut String,
    paragraph: &str,
    is_heading: bool,
) {
    let trimmed = paragraph.trim();
    if trimmed.is_empty() {
        return;
    }
    if is_heading || (headings.len() < 10 && trimmed.len() < 80 && !trimmed.contains('.')) {
        headings.push(trimmed.to_string());
    }
    if full_preview.len() < 300 {
        if !full_preview.is_empty() {
            full_preview.push(' ');
        }
        full_preview.push_str(trimmed);
    }
}

fn on_docx_start_element(
    e: &quick_xml::events::BytesStart<'_>,
    current_paragraph: &mut String,
    in_text: &mut bool,
    is_heading: &mut bool,
) {
    match e.local_name().as_ref() {
        b"p" => {
            current_paragraph.clear();
            *is_heading = false;
        }
        b"t" => *in_text = true,
        b"pStyle" if has_heading_style_attr(e) => *is_heading = true,
        _ => {}
    }
}

pub(crate) fn extract_docx_outline_and_preview(xml_bytes: &[u8]) -> (Vec<String>, String) {
    let mut reader = XmlReader::from_reader(Cursor::new(xml_bytes));
    reader.config_mut().trim_text(false);

    let mut buf = Vec::with_capacity(512);
    let mut headings: Vec<String> = Vec::new();
    let mut current_paragraph = String::with_capacity(256);
    let mut full_preview = String::with_capacity(512);
    let mut in_text = false;
    let mut is_heading = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                on_docx_start_element(e, &mut current_paragraph, &mut in_text, &mut is_heading)
            }
            Ok(Event::Empty(ref e)) => {
                if e.local_name().as_ref() == b"pStyle" && has_heading_style_attr(e) {
                    is_heading = true;
                }
            }
            Ok(Event::Text(ref e)) if in_text => {
                if let Ok(txt) = e.unescape() {
                    current_paragraph.push_str(&txt);
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.local_name();
                if name.as_ref() == b"t" {
                    in_text = false;
                } else if name.as_ref() == b"p" {
                    commit_docx_paragraph(
                        &mut headings,
                        &mut full_preview,
                        &current_paragraph,
                        is_heading,
                    );
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    (headings, full_preview)
}
