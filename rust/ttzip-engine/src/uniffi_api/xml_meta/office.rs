// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Office Open XML (DOCX, XLSX, PPTX) metadata and outline extraction.

use std::io::Cursor;

use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;

use crate::uniffi_api::types::TTZipError;
use crate::zip::reader::ZipArchive;
use super::types::{UniFFIDocumentMetadata, UniFFIOfficeOutline};
use super::find_and_extract_entry;

/// Parses Office Open XML document metadata directly from byte slice.
pub fn parse_office_metadata_from_slice(bytes: &[u8]) -> Result<UniFFIDocumentMetadata, TTZipError> {
    let zip = ZipArchive::open_slice(bytes).map_err(|e| TTZipError::CorruptHeader {
        details: format!("Failed to open Office archive: {e:?}"),
        offset: 0,
    })?;

    let mut meta = UniFFIDocumentMetadata::default();

    // 1. Detect Office format flavor
    let has_word = zip.entries().iter().any(|e| e.rel_path.starts_with("word/"));
    let has_xl = zip.entries().iter().any(|e| e.rel_path.starts_with("xl/"));
    let has_ppt = zip.entries().iter().any(|e| e.rel_path.starts_with("ppt/"));

    if has_word {
        meta.format_name = "DOCX".to_string();
    } else if has_xl {
        meta.format_name = "XLSX".to_string();
    } else if has_ppt {
        meta.format_name = "PPTX".to_string();
    } else {
        meta.format_name = "Office Open XML".to_string();
    }

    // 2. Parse docProps/core.xml (Dublin Core properties)
    if let Some(core_bytes) = find_and_extract_entry(&zip, "docProps/core.xml") {
        parse_core_properties_xml(&core_bytes, &mut meta);
    }

    // 3. Parse docProps/app.xml (Extended application properties)
    if let Some(app_bytes) = find_and_extract_entry(&zip, "docProps/app.xml") {
        parse_app_properties_xml(&app_bytes, &mut meta);
    }

    // 4. XLSX workbook sheet enumeration fallback
    if meta.format_name == "XLSX" && meta.sheet_names.is_empty() {
        if let Some(wb_bytes) = find_and_extract_entry(&zip, "xl/workbook.xml") {
            parse_xlsx_workbook_sheets(&wb_bytes, &mut meta);
        }
    }
    meta.sheet_count = meta.sheet_names.len() as u32;

    // 5. PPTX slide enumeration fallback
    if meta.format_name == "PPTX" {
        let mut slide_entries: Vec<String> = zip
            .entries()
            .iter()
            .filter(|e| e.rel_path.starts_with("ppt/slides/slide") && e.rel_path.ends_with(".xml"))
            .map(|e| e.rel_path.clone())
            .collect();
        slide_entries.sort();

        if meta.slide_count == 0 {
            meta.slide_count = slide_entries.len() as u32;
        }

        if meta.slide_titles.is_empty() {
            for slide_path in &slide_entries {
                if let Some(slide_bytes) = find_and_extract_entry(&zip, slide_path) {
                    if let Some(title) = extract_pptx_slide_title(&slide_bytes) {
                        meta.slide_titles.push(title);
                    }
                }
            }
        }
    }

    // 6. DOCX statistics fallback
    if meta.format_name == "DOCX" && meta.word_count == 0 {
        if let Some(doc_bytes) = find_and_extract_entry(&zip, "word/document.xml") {
            let (w_cnt, c_cnt, p_cnt) = compute_docx_text_stats(&doc_bytes);
            meta.word_count = w_cnt;
            meta.character_count = c_cnt;
            meta.page_count = if meta.page_count > 0 { meta.page_count } else { (p_cnt / 5).max(1) };
        }
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
        let mut slide_entries: Vec<String> = zip
            .entries()
            .iter()
            .filter(|e| e.rel_path.starts_with("ppt/slides/slide") && e.rel_path.ends_with(".xml"))
            .map(|e| e.rel_path.clone())
            .collect();
        slide_entries.sort();

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

pub(crate) fn parse_core_properties_xml(xml_bytes: &[u8], meta: &mut UniFFIDocumentMetadata) {
    let mut reader = XmlReader::from_reader(Cursor::new(xml_bytes));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::with_capacity(512);
    let mut current_tag = String::new();
    let mut current_text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                current_tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                current_text.clear();
            }
            Ok(Event::Text(ref e)) => {
                if let Ok(txt) = e.unescape() {
                    current_text.push_str(&txt);
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if tag == current_tag {
                    let val = current_text.trim().to_string();
                    if !val.is_empty() {
                        match tag.as_str() {
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
                    current_tag.clear();
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
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
                    let val = current_text.trim().to_string();
                    if !val.is_empty() {
                        if in_titles_of_parts && (tag == "lpstr" || tag == "string") {
                            if meta.format_name == "XLSX" {
                                meta.sheet_names.push(val);
                            } else if meta.format_name == "PPTX" {
                                meta.slide_titles.push(val);
                            }
                        } else {
                            match tag.as_str() {
                                "Application" => meta.application = Some(val),
                                "Pages" => {
                                    if let Ok(n) = val.parse::<u32>() {
                                        meta.page_count = n;
                                    }
                                }
                                "Words" => {
                                    if let Ok(n) = val.parse::<u32>() {
                                        meta.word_count = n;
                                    }
                                }
                                "Characters" => {
                                    if let Ok(n) = val.parse::<u32>() {
                                        meta.character_count = n;
                                    }
                                }
                                "Slides" => {
                                    if let Ok(n) = val.parse::<u32>() {
                                        meta.slide_count = n;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
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

pub(crate) fn extract_pptx_slide_title(xml_bytes: &[u8]) -> Option<String> {
    let mut reader = XmlReader::from_reader(Cursor::new(xml_bytes));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::with_capacity(512);
    let mut in_text = false;
    let mut title_buf = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.local_name().as_ref() == b"t" {
                    in_text = true;
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_text {
                    if let Ok(txt) = e.unescape() {
                        let t = txt.trim();
                        if !t.is_empty() {
                            if !title_buf.is_empty() {
                                title_buf.push(' ');
                            }
                            title_buf.push_str(t);
                            if title_buf.len() >= 60 {
                                return Some(title_buf);
                            }
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().as_ref() == b"t" {
                    in_text = false;
                    if !title_buf.is_empty() {
                        return Some(title_buf);
                    }
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    if !title_buf.is_empty() {
        Some(title_buf)
    } else {
        None
    }
}

pub(crate) fn compute_docx_text_stats(xml_bytes: &[u8]) -> (u32, u32, u32) {
    let mut reader = XmlReader::from_reader(Cursor::new(xml_bytes));
    reader.config_mut().trim_text(false);

    let mut buf = Vec::with_capacity(512);
    let mut in_text = false;
    let mut word_count: u32 = 0;
    let mut char_count: u32 = 0;
    let mut paragraph_count: u32 = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.local_name().as_ref() {
                b"p" => paragraph_count += 1,
                b"t" => in_text = true,
                _ => {}
            },
            Ok(Event::Text(ref e)) => {
                if in_text {
                    if let Ok(txt) = e.unescape() {
                        char_count += txt.chars().count() as u32;
                        word_count += txt.split_whitespace().count() as u32;
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().as_ref() == b"t" {
                    in_text = false;
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    (word_count, char_count, paragraph_count)
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
            Ok(Event::Start(ref e)) => match e.local_name().as_ref() {
                b"p" => {
                    current_paragraph.clear();
                    is_heading = false;
                }
                b"t" => in_text = true,
                b"pStyle" => {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"val" || attr.key.as_ref() == b"w:val" {
                            let val_str = String::from_utf8_lossy(&attr.value).to_lowercase();
                            if val_str.contains("heading") || val_str.contains("title") {
                                is_heading = true;
                            }
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Empty(ref e)) => {
                if e.local_name().as_ref() == b"pStyle" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"val" || attr.key.as_ref() == b"w:val" {
                            let val_str = String::from_utf8_lossy(&attr.value).to_lowercase();
                            if val_str.contains("heading") || val_str.contains("title") {
                                is_heading = true;
                            }
                        }
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_text {
                    if let Ok(txt) = e.unescape() {
                        current_paragraph.push_str(&txt);
                    }
                }
            }
            Ok(Event::End(ref e)) => match e.local_name().as_ref() {
                b"t" => in_text = false,
                b"p" => {
                    let trimmed = current_paragraph.trim();
                    if !trimmed.is_empty() {
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
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    (headings, full_preview)
}
