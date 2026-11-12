// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! High-performance streaming DOCX text extraction and Dublin Core metadata parsing via quick-xml SAX.

use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;

use super::{find_and_extract_zip_entry, DocumentStreamError};
use crate::ebook::container::strip_utf8_bom;
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

    let doc_xml_bytes = find_and_extract_zip_entry(&zip, "word/document.xml")
        .ok_or_else(|| DocumentStreamError::EntryNotFound("word/document.xml".to_string()))?;

    let (full_text, paragraphs) = parse_docx_xml_content(&doc_xml_bytes)?;

    let mut props = if let Some(core_bytes) = find_and_extract_zip_entry(&zip, "docProps/core.xml") {
        parse_docx_core_properties(&core_bytes).unwrap_or_default()
    } else {
        DocxCoreProperties::default()
    };

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
    let mut reader = XmlReader::from_reader(strip_utf8_bom(xml_bytes));
    reader.config_mut().trim_text(false);

    let mut buf = Vec::with_capacity(1024);
    let approx_paras = (xml_bytes.len() / 64).max(128);
    let mut paragraphs: Vec<String> = Vec::with_capacity(approx_paras);
    let mut current_paragraph = String::with_capacity(512);
    let mut in_text = false;

    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| DocumentStreamError::XmlError(e.to_string()))?
        {
            Event::Start(ref e) => on_docx_start_tag(e.local_name().as_ref(), &mut current_paragraph, &mut in_text),
            Event::Empty(ref e) => on_docx_empty_tag(e.local_name().as_ref(), &mut current_paragraph),
            Event::Text(ref e) if in_text => append_docx_text_run(e, &mut current_paragraph),
            Event::End(ref e) => on_docx_end_tag(
                e.local_name().as_ref(),
                &mut current_paragraph,
                &mut paragraphs,
                &mut in_text,
            ),
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let full_text = paragraphs.join("\n\n");
    Ok((full_text, paragraphs))
}

fn on_docx_start_tag(tag: &[u8], current_paragraph: &mut String, in_text: &mut bool) {
    match tag {
        b"p" => current_paragraph.clear(),
        b"t" => *in_text = true,
        _ => {}
    }
}

fn on_docx_empty_tag(tag: &[u8], current_paragraph: &mut String) {
    match tag {
        b"tab" => current_paragraph.push('\t'),
        b"br" | b"cr" => current_paragraph.push('\n'),
        _ => {}
    }
}

fn append_docx_text_run(e: &quick_xml::events::BytesText<'_>, current_paragraph: &mut String) {
    let raw: &[u8] = e.as_ref();
    if !raw.contains(&b'&') {
        if let Ok(s) = std::str::from_utf8(raw) {
            current_paragraph.push_str(s);
        }
    } else if let Ok(txt) = e.unescape() {
        current_paragraph.push_str(&txt);
    }
}

fn on_docx_end_tag(
    tag: &[u8],
    current_paragraph: &mut String,
    paragraphs: &mut Vec<String>,
    in_text: &mut bool,
) {
    match tag {
        b"p" => {
            if !current_paragraph.trim().is_empty() {
                paragraphs.push(current_paragraph.clone());
            }
            current_paragraph.clear();
        }
        b"t" => *in_text = false,
        _ => {}
    }
}

/// Streams Dublin Core and OpenXML core properties (`docProps/core.xml`) via `quick-xml` SAX.
pub fn parse_core_properties_entries(
    xml_bytes: &[u8],
    mut on_field: impl FnMut(&str, String),
) -> Result<(), DocumentStreamError> {
    let mut reader = XmlReader::from_reader(strip_utf8_bom(xml_bytes));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::with_capacity(512);
    let mut current_tag = String::new();
    let mut current_text = String::new();

    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| DocumentStreamError::XmlError(e.to_string()))?
        {
            Event::Start(ref e) => {
                current_tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                current_text.clear();
            }
            Event::Text(ref e) => {
                if let Ok(txt) = e.unescape() {
                    current_text.push_str(&txt);
                }
            }
            Event::CData(ref e) => {
                if let Ok(txt) = std::str::from_utf8(e.as_ref()) {
                    current_text.push_str(txt);
                }
            }
            Event::End(ref e) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if tag == current_tag {
                    let trimmed = current_text.trim().to_string();
                    if !trimmed.is_empty() {
                        on_field(&tag, trimmed);
                    }
                    current_tag.clear();
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

/// Computes `(word_count, character_count, paragraph_count)` from `word/document.xml` via `quick-xml` SAX.
pub fn compute_docx_xml_stats(xml_bytes: &[u8]) -> (u32, u32, u32) {
    let mut reader = XmlReader::from_reader(strip_utf8_bom(xml_bytes));
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
            Ok(Event::Text(ref e)) if in_text => {
                if let Ok(txt) = e.unescape() {
                    char_count += txt.chars().count() as u32;
                    word_count += txt.split_whitespace().count() as u32;
                }
            }
            Ok(Event::End(ref e)) if e.local_name().as_ref() == b"t" => in_text = false,
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    (word_count, char_count, paragraph_count)
}

/// Parses Dublin Core metadata from `docProps/core.xml`.
fn parse_docx_core_properties(xml_bytes: &[u8]) -> Result<DocxCoreProperties, DocumentStreamError> {
    let mut props = DocxCoreProperties::default();
    parse_core_properties_entries(xml_bytes, |tag, val| match tag {
        "title" => props.title = Some(val),
        "creator" => props.creator = Some(val),
        "description" => props.description = Some(val),
        "lastModifiedBy" => props.last_modified_by = Some(val),
        "created" => props.created = Some(val),
        "modified" => props.modified = Some(val),
        "revision" => props.revision = Some(val),
        _ => {}
    })?;
    Ok(props)
}

fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}
