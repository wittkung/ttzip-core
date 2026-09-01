// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Office Open XML (DOCX, XLSX, PPTX) streaming metadata and outline extractor.
//!
//! Provides zero-allocation stream tokenization for Dublin Core properties, extended application
//! statistics, DOCX headings and paragraph text, XLSX workbook sheets and shared string tables,
//! and PPTX slide text box hierarchies.

use quick_xml::events::Event;

use super::parser::TTZipXmlParser;
use super::XmlError;

/// Standard Dublin Core metadata extracted from `docProps/core.xml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OfficeCoreProperties {
    pub title: Option<String>,
    pub creator: Option<String>,
    pub subject: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
    pub last_modified_by: Option<String>,
    pub revision: Option<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub category: Option<String>,
    pub content_status: Option<String>,
}

/// Extended application properties and statistics extracted from `docProps/app.xml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OfficeAppProperties {
    pub application: Option<String>,
    pub app_version: Option<String>,
    pub total_time_mins: Option<u32>,
    pub pages: Option<u32>,
    pub words: Option<u32>,
    pub characters: Option<u32>,
    pub characters_with_spaces: Option<u32>,
    pub lines: Option<u32>,
    pub paragraphs: Option<u32>,
    pub slides: Option<u32>,
    pub notes: Option<u32>,
    pub hidden_slides: Option<u32>,
    pub company: Option<String>,
}

/// A structured heading or outline entry in a DOCX document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocxOutlineItem {
    pub level: u32,
    pub style: String,
    pub text: String,
}

/// Extracted outline structure and body text from DOCX `word/document.xml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocxOutline {
    pub headings: Vec<DocxOutlineItem>,
    pub paragraphs: Vec<String>,
    pub full_text: String,
    pub word_count: usize,
    pub paragraph_count: usize,
}

/// Information about a single sheet in an XLSX workbook.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XlsxSheetInfo {
    pub name: String,
    pub sheet_id: u32,
    pub state: Option<String>,
    pub r_id: String,
}

/// Metadata extracted from `xl/workbook.xml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XlsxWorkbookMeta {
    pub sheets: Vec<XlsxSheetInfo>,
    pub date_1904: bool,
}

/// Extracted outline and text from a single PPTX slide (`ppt/slides/slideN.xml`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PptxSlideOutline {
    pub slide_number: u32,
    pub title: Option<String>,
    pub text_boxes: Vec<String>,
    pub full_text: String,
}

/// Extractor for Office Open XML file formats (DOCX / XLSX / PPTX).
pub struct OfficeXmlExtractor;

impl OfficeXmlExtractor {
    /// Parses Dublin Core metadata from `docProps/core.xml`.
    pub fn parse_core_properties(xml_bytes: &[u8]) -> Result<OfficeCoreProperties, XmlError> {
        let mut parser = TTZipXmlParser::from_slice(xml_bytes);
        let mut props = OfficeCoreProperties::default();
        let mut buf = Vec::with_capacity(512);

        loop {
            match parser.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"title" => props.title = Some(parser.read_element_text(b"title")?),
                        b"creator" => props.creator = Some(parser.read_element_text(b"creator")?),
                        b"subject" => props.subject = Some(parser.read_element_text(b"subject")?),
                        b"description" => {
                            props.description = Some(parser.read_element_text(b"description")?);
                        }
                        b"keywords" => props.keywords = Some(parser.read_element_text(b"keywords")?),
                        b"lastModifiedBy" => {
                            props.last_modified_by =
                                Some(parser.read_element_text(b"lastModifiedBy")?);
                        }
                        b"revision" => props.revision = Some(parser.read_element_text(b"revision")?),
                        b"created" => props.created = Some(parser.read_element_text(b"created")?),
                        b"modified" => props.modified = Some(parser.read_element_text(b"modified")?),
                        b"category" => props.category = Some(parser.read_element_text(b"category")?),
                        b"contentStatus" => {
                            props.content_status =
                                Some(parser.read_element_text(b"contentStatus")?);
                        }
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(props)
    }

    /// Parses application statistics and extended properties from `docProps/app.xml`.
    pub fn parse_app_properties(xml_bytes: &[u8]) -> Result<OfficeAppProperties, XmlError> {
        let mut parser = TTZipXmlParser::from_slice(xml_bytes);
        let mut props = OfficeAppProperties::default();
        let mut buf = Vec::with_capacity(512);

        loop {
            match parser.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"Application" => {
                            props.application = Some(parser.read_element_text(b"Application")?);
                        }
                        b"AppVersion" => {
                            props.app_version = Some(parser.read_element_text(b"AppVersion")?);
                        }
                        b"TotalTime" => {
                            let text = parser.read_element_text(b"TotalTime")?;
                            props.total_time_mins = text.trim().parse().ok();
                        }
                        b"Pages" => {
                            let text = parser.read_element_text(b"Pages")?;
                            props.pages = text.trim().parse().ok();
                        }
                        b"Words" => {
                            let text = parser.read_element_text(b"Words")?;
                            props.words = text.trim().parse().ok();
                        }
                        b"Characters" => {
                            let text = parser.read_element_text(b"Characters")?;
                            props.characters = text.trim().parse().ok();
                        }
                        b"CharactersWithSpaces" => {
                            let text = parser.read_element_text(b"CharactersWithSpaces")?;
                            props.characters_with_spaces = text.trim().parse().ok();
                        }
                        b"Lines" => {
                            let text = parser.read_element_text(b"Lines")?;
                            props.lines = text.trim().parse().ok();
                        }
                        b"Paragraphs" => {
                            let text = parser.read_element_text(b"Paragraphs")?;
                            props.paragraphs = text.trim().parse().ok();
                        }
                        b"Slides" => {
                            let text = parser.read_element_text(b"Slides")?;
                            props.slides = text.trim().parse().ok();
                        }
                        b"Notes" => {
                            let text = parser.read_element_text(b"Notes")?;
                            props.notes = text.trim().parse().ok();
                        }
                        b"HiddenSlides" => {
                            let text = parser.read_element_text(b"HiddenSlides")?;
                            props.hidden_slides = text.trim().parse().ok();
                        }
                        b"Company" => {
                            props.company = Some(parser.read_element_text(b"Company")?);
                        }
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(props)
    }

    /// Parses DOCX `word/document.xml` extracting paragraph contents and hierarchical headings.
    pub fn parse_docx_document(xml_bytes: &[u8]) -> Result<DocxOutline, XmlError> {
        let mut parser = TTZipXmlParser::from_slice(xml_bytes);
        parser.set_trim_text(false);
        let mut outline = DocxOutline::default();

        let mut buf = Vec::with_capacity(1024);
        let mut current_paragraph = String::with_capacity(256);
        let mut in_paragraph = false;
        let mut current_style = String::new();
        let mut current_outline_lvl: Option<u32> = None;
        let mut in_text = false;

        loop {
            match parser.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"p" => {
                            in_paragraph = true;
                            current_paragraph.clear();
                            current_style.clear();
                            current_outline_lvl = None;
                        }
                        b"t" => {
                            in_text = true;
                        }
                        _ => {}
                    }
                }
                Event::Empty(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"pStyle" => {
                            if let Some(val) = TTZipXmlParser::get_attribute(e, b"val") {
                                current_style = val.to_string();
                            }
                        }
                        b"outlineLvl" => {
                            if let Some(val) = TTZipXmlParser::get_attribute(e, b"val") {
                                current_outline_lvl = val.trim().parse().ok();
                            }
                        }
                        b"tab" if in_paragraph => {
                            current_paragraph.push('\t');
                        }
                        b"br" | b"cr" if in_paragraph => {
                            current_paragraph.push('\n');
                        }
                        _ => {}
                    }
                }
                Event::Text(ref e) if in_text => {
                    let text = TTZipXmlParser::decode_text(e)?;
                    current_paragraph.push_str(&text);
                }
                Event::End(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"t" => in_text = false,
                        b"p" => {
                            in_paragraph = false;
                            let trimmed = current_paragraph.trim();
                            if !trimmed.is_empty() {
                                let p_str = current_paragraph.clone();
                                outline.paragraphs.push(p_str.clone());

                                // Check if this paragraph represents a heading or outline item
                                if let Some(heading) = Self::detect_docx_heading(
                                    &current_style,
                                    current_outline_lvl,
                                    trimmed,
                                ) {
                                    outline.headings.push(heading);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        // Build full text and compute stats
        let mut full_text = String::new();
        for (i, p) in outline.paragraphs.iter().enumerate() {
            if i > 0 {
                full_text.push('\n');
            }
            full_text.push_str(p);
        }
        outline.word_count = count_words(&full_text);
        outline.paragraph_count = outline.paragraphs.len();
        outline.full_text = full_text;

        Ok(outline)
    }

    /// Helper to detect heading level from DOCX style name and outline level attribute.
    fn detect_docx_heading(
        style: &str,
        outline_lvl: Option<u32>,
        text: &str,
    ) -> Option<DocxOutlineItem> {
        let style_lower = style.to_lowercase();
        let level = if let Some(lvl) = outline_lvl {
            lvl.saturating_add(1)
        } else if style_lower == "title" {
            0
        } else if style_lower == "subtitle" {
            1
        } else if style_lower.starts_with("heading") || style_lower.starts_with("heading ") {
            let num_part = style_lower.trim_start_matches("heading").trim();
            num_part.parse::<u32>().unwrap_or(1)
        } else if style_lower.starts_with("toc") {
            let num_part = style_lower.trim_start_matches("toc").trim();
            num_part.parse::<u32>().unwrap_or(1)
        } else {
            return None;
        };

        Some(DocxOutlineItem {
            level,
            style: style.to_string(),
            text: text.to_string(),
        })
    }

    /// Parses XLSX `xl/workbook.xml` extracting list of worksheets.
    pub fn parse_xlsx_workbook(xml_bytes: &[u8]) -> Result<XlsxWorkbookMeta, XmlError> {
        let mut parser = TTZipXmlParser::from_slice(xml_bytes);
        let mut meta = XlsxWorkbookMeta::default();
        let mut buf = Vec::with_capacity(512);

        loop {
            match parser.read_event_into(&mut buf)? {
                Event::Start(ref e) | Event::Empty(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"sheet" => {
                            let name = TTZipXmlParser::get_attribute(e, b"name")
                                .map(|s| s.into_owned())
                                .unwrap_or_default();
                            let sheet_id = TTZipXmlParser::get_attribute(e, b"sheetId")
                                .and_then(|s| s.trim().parse::<u32>().ok())
                                .unwrap_or(0);
                            let state = TTZipXmlParser::get_attribute(e, b"state")
                                .map(|s| s.into_owned());
                            let r_id = TTZipXmlParser::get_attribute(e, b"r:id")
                                .or_else(|| TTZipXmlParser::get_attribute(e, b"id"))
                                .map(|s| s.into_owned())
                                .unwrap_or_default();

                            if !name.is_empty() {
                                meta.sheets.push(XlsxSheetInfo {
                                    name,
                                    sheet_id,
                                    state,
                                    r_id,
                                });
                            }
                        }
                        b"workbookPr" => {
                            if let Some(val) = TTZipXmlParser::get_attribute(e, b"date1904") {
                                meta.date_1904 = val == "1" || val == "true";
                            }
                        }
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(meta)
    }

    /// Parses XLSX `xl/sharedStrings.xml` extracting shared string pool with an optional item limit.
    pub fn parse_xlsx_shared_strings(
        xml_bytes: &[u8],
        limit: Option<usize>,
    ) -> Result<Vec<String>, XmlError> {
        let mut parser = TTZipXmlParser::from_slice(xml_bytes);
        parser.set_trim_text(false);

        let mut strings = Vec::new();
        let max_items = limit.unwrap_or(usize::MAX);
        let mut buf = Vec::with_capacity(512);

        let mut in_si = false;
        let mut in_t = false;
        let mut current_str = String::with_capacity(64);

        loop {
            if strings.len() >= max_items {
                break;
            }
            match parser.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"si" => {
                            in_si = true;
                            current_str.clear();
                        }
                        b"t" if in_si => {
                            in_t = true;
                        }
                        _ => {}
                    }
                }
                Event::Text(ref e) if in_t => {
                    let text = TTZipXmlParser::decode_text(e)?;
                    current_str.push_str(&text);
                }
                Event::End(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"t" => in_t = false,
                        b"si" => {
                            in_si = false;
                            strings.push(current_str.clone());
                        }
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(strings)
    }

    /// Parses PPTX slide (`ppt/slides/slideN.xml`) extracting slide title and text box strings.
    pub fn parse_pptx_slide(
        xml_bytes: &[u8],
        slide_number: u32,
    ) -> Result<PptxSlideOutline, XmlError> {
        let mut parser = TTZipXmlParser::from_slice(xml_bytes);
        parser.set_trim_text(false);

        let mut outline = PptxSlideOutline {
            slide_number,
            title: None,
            text_boxes: Vec::new(),
            full_text: String::new(),
        };

        let mut buf = Vec::with_capacity(512);
        let mut in_sp = false;
        let mut is_title_shape = false;
        let mut in_t = false;
        let mut current_box_text = String::with_capacity(128);

        loop {
            match parser.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"sp" => {
                            in_sp = true;
                            is_title_shape = false;
                            current_box_text.clear();
                        }
                        b"t" if in_sp => {
                            in_t = true;
                        }
                        _ => {}
                    }
                }
                Event::Empty(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    if local == b"ph" && in_sp {
                        if let Some(ph_type) = TTZipXmlParser::get_attribute(e, b"type") {
                            if ph_type == "title" || ph_type == "ctrTitle" {
                                is_title_shape = true;
                            }
                        }
                    }
                }
                Event::Text(ref e) if in_t => {
                    let text = TTZipXmlParser::decode_text(e)?;
                    current_box_text.push_str(&text);
                }
                Event::End(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"t" => in_t = false,
                        b"p" if in_sp => {
                            if !current_box_text.ends_with('\n') && !current_box_text.is_empty() {
                                current_box_text.push('\n');
                            }
                        }
                        b"sp" => {
                            in_sp = false;
                            let trimmed = current_box_text.trim();
                            if !trimmed.is_empty() {
                                if is_title_shape && outline.title.is_none() {
                                    outline.title = Some(trimmed.to_string());
                                }
                                outline.text_boxes.push(trimmed.to_string());
                            }
                        }
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        // Aggregate full slide text
        let mut full_text = String::new();
        for (i, tb) in outline.text_boxes.iter().enumerate() {
            if i > 0 {
                full_text.push('\n');
            }
            full_text.push_str(tb);
        }
        outline.full_text = full_text;

        Ok(outline)
    }
}

/// Helper function to count words in a Unicode string.
fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}
