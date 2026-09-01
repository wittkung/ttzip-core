// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! DOCX streaming text extraction and Dublin Core metadata parsing via quick-xml SAX.

use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;

use super::{find_and_extract_zip_entry, DocumentStreamError};
use crate::zip::reader::ZipArchive;

/// Metadata properties extracted from Word document properties (`docProps/core.xml` & `app.xml`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocxCoreProperties {
    pub title: Option<String>,
    pub creator: Option<String>,
    pub description: Option<String>,
    pub last_modified_by: Option<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub revision: Option<String>,
    pub word_count: u32,
    pub character_count: u32,
    pub paragraph_count: u32,
}

/// Extracted DOCX document structure and content.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocxDocument {
    pub full_text: String,
    pub paragraphs: Vec<String>,
    pub properties: DocxCoreProperties,
}

/// Parses a DOCX file from an in-memory byte slice without disk writes.
pub fn parse_docx_from_memory(docx_bytes: &[u8]) -> Result<DocxDocument, DocumentStreamError> {
    let zip = ZipArchive::open_slice(docx_bytes)
        .map_err(|e| DocumentStreamError::ZipError(format!("{e:?}")))?;

    // 1. Locate and extract word/document.xml
    let doc_xml_bytes = find_and_extract_zip_entry(&zip, "word/document.xml")
        .ok_or_else(|| DocumentStreamError::EntryNotFound("word/document.xml".to_string()))?;

    // 2. Stream parse word/document.xml using quick-xml SAX parser
    let (full_text, paragraphs) = parse_docx_xml_content(&doc_xml_bytes)?;

    // 3. Extract docProps/core.xml if present
    let mut props = if let Some(core_bytes) = find_and_extract_zip_entry(&zip, "docProps/core.xml") {
        parse_docx_core_properties(&core_bytes).unwrap_or_default()
    } else {
        DocxCoreProperties::default()
    };

    // 4. Fill in computed statistics
    props.paragraph_count = paragraphs.len() as u32;
    props.character_count = full_text.chars().count() as u32;
    if props.word_count == 0 {
        props.word_count = count_words(&full_text) as u32;
    }

    Ok(DocxDocument {
        full_text,
        paragraphs,
        properties: props,
    })
}

pub fn parse_docx_xml_content(xml_bytes: &[u8]) -> Result<(String, Vec<String>), DocumentStreamError> {
    let mut reader = XmlReader::from_reader(xml_bytes);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::with_capacity(1024);
    let mut paragraphs: Vec<String> = Vec::with_capacity(128);
    let mut current_paragraph = String::with_capacity(512);
    let mut in_text = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"p" => {
                        current_paragraph.clear();
                    }
                    b"t" => {
                        in_text = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"tab" => {
                        current_paragraph.push('\t');
                    }
                    b"br" | b"cr" => {
                        current_paragraph.push('\n');
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_text {
                    let raw: &[u8] = e.as_ref();
                    if !raw.contains(&b'&') {
                        if let Ok(s) = std::str::from_utf8(raw) {
                            current_paragraph.push_str(s);
                        }
                    } else if let Ok(txt) = e.unescape() {
                        current_paragraph.push_str(&txt);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"p" => {
                        let trimmed = current_paragraph.trim();
                        if !trimmed.is_empty() {
                            paragraphs.push(std::mem::take(&mut current_paragraph));
                        } else {
                            current_paragraph.clear();
                        }
                    }
                    b"t" => {
                        in_text = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DocumentStreamError::XmlError(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    let full_text = paragraphs.join("\n\n");
    Ok((full_text, paragraphs))
}

/// Parses Dublin Core metadata from `docProps/core.xml`.
fn parse_docx_core_properties(xml_bytes: &[u8]) -> Result<DocxCoreProperties, DocumentStreamError> {
    let xml_str = std::str::from_utf8(xml_bytes)
        .map_err(|e| DocumentStreamError::DecodeError(e.to_string()))?;
    let doc = roxmltree::Document::parse(xml_str)
        .map_err(|e| DocumentStreamError::XmlError(e.to_string()))?;

    let mut props = DocxCoreProperties::default();

    for node in doc.descendants() {
        let tag = node.tag_name().name();
        let text = node.text().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        match tag {
            "title" => props.title = text,
            "creator" => props.creator = text,
            "description" => props.description = text,
            "lastModifiedBy" => props.last_modified_by = text,
            "created" => props.created = text,
            "modified" => props.modified = text,
            "revision" => props.revision = text,
            _ => {}
        }
    }

    Ok(props)
}

/// Simple unicode-aware word counter.
fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}
