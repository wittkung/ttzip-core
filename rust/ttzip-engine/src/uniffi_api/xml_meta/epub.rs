// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! EPUB Dublin Core publication metadata extraction for UniFFI bindings.

use std::io::Cursor;

use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;

use crate::uniffi_api::types::TTZipError;
use crate::zip::reader::ZipArchive;
use super::types::UniFFIEpubMetadata;
use super::find_and_extract_entry;

/// Parses EPUB Dublin Core publication metadata from byte slice.
pub fn parse_epub_metadata_from_slice(bytes: &[u8]) -> Result<UniFFIEpubMetadata, TTZipError> {
    let zip = ZipArchive::open_slice(bytes).map_err(|e| TTZipError::CorruptHeader {
        details: format!("Failed to open EPUB archive: {e:?}"),
        offset: 0,
    })?;

    // 1. Resolve OPF package path from META-INF/container.xml
    let opf_path = find_and_extract_entry(&zip, "META-INF/container.xml")
        .and_then(|c_bytes| extract_attribute_from_xml(&c_bytes, "rootfile", "full-path"))
        .or_else(|| {
            zip.entries()
                .iter()
                .find(|e| e.rel_path.to_lowercase().ends_with(".opf"))
                .map(|e| e.rel_path.clone())
        })
        .ok_or_else(|| TTZipError::FileNotFound {
            path: "META-INF/container.xml or *.opf".to_string(),
        })?;

    // 2. Read OPF file content
    let opf_bytes = find_and_extract_entry(&zip, &opf_path).ok_or_else(|| TTZipError::FileNotFound {
        path: opf_path.clone(),
    })?;

    // 3. SAX parse Dublin Core elements
    let mut meta = UniFFIEpubMetadata::default();
    parse_opf_metadata_xml(&opf_bytes, &mut meta);

    if meta.title.is_empty() {
        meta.title = "Untitled Book".to_string();
    }

    Ok(meta)
}

pub(crate) fn parse_opf_metadata_xml(xml_bytes: &[u8], meta: &mut UniFFIEpubMetadata) {
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
                            "title" => meta.title = val,
                            "creator" => meta.authors.push(val),
                            "publisher" => meta.publisher = Some(val),
                            "language" => meta.language = Some(val),
                            "identifier" => meta.identifier = Some(val),
                            "description" => meta.description = Some(val),
                            "date" => meta.publication_date = Some(val),
                            "modified" => meta.modified_date = Some(val),
                            "rights" => meta.rights = Some(val),
                            _ => {}
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

pub(crate) fn extract_attribute_from_xml(xml_bytes: &[u8], tag_name: &str, attr_name: &str) -> Option<String> {
    let mut reader = XmlReader::from_reader(Cursor::new(xml_bytes));
    let mut buf = Vec::with_capacity(512);

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                if local.as_ref() == tag_name.as_bytes() {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == attr_name.as_bytes() {
                            if let Ok(val) = String::from_utf8(attr.value.to_vec()) {
                                return Some(val);
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
    None
}
