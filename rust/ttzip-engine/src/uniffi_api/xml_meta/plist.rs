// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Apple Property List (XML Plist) deserialization for UniFFI bindings.

use std::io::Cursor;

use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;

use crate::uniffi_api::types::TTZipError;
use super::types::UniFFIPlistDictionary;

/// Deserializes an Apple XML Property List string into a structured dictionary.
#[uniffi::export]
pub fn uniffi_parse_plist_xml(xml_content: String) -> Result<UniFFIPlistDictionary, TTZipError> {
    parse_plist_xml_str(&xml_content)
}

/// Deserializes an Apple XML Property List from raw bytes.
#[uniffi::export]
pub fn uniffi_parse_plist_from_bytes(bytes: Vec<u8>) -> Result<UniFFIPlistDictionary, TTZipError> {
    let xml_str = std::str::from_utf8(&bytes).map_err(|e| TTZipError::IoError {
        message: format!("Invalid UTF-8 in Property List: {e}"),
    })?;
    parse_plist_xml_str(xml_str)
}

/// Parses Apple XML Property List string into structured dictionary.
pub fn parse_plist_xml_str(xml_str: &str) -> Result<UniFFIPlistDictionary, TTZipError> {
    let mut reader = XmlReader::from_reader(Cursor::new(xml_str.as_bytes()));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::with_capacity(1024);
    let mut dict = UniFFIPlistDictionary {
        raw_xml: xml_str.to_string(),
        ..Default::default()
    };

    let mut current_key: Option<String> = None;
    let mut in_key = false;
    let mut in_val = false;
    let mut val_tag = String::new();
    let mut current_text = String::new();
    let mut dict_depth: usize = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if tag == "dict" {
                    dict_depth += 1;
                } else if tag == "key" && dict_depth == 1 {
                    in_key = true;
                    current_text.clear();
                } else if dict_depth == 1 && current_key.is_some() {
                    in_val = true;
                    val_tag = tag;
                    current_text.clear();
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if dict_depth == 1 {
                    if let Some(key) = current_key.take() {
                        let value = match tag.as_str() {
                            "true" => "true".to_string(),
                            "false" => "false".to_string(),
                            _ => String::new(),
                        };
                        dict.entries.insert(key, value);
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_key || in_val {
                    if let Ok(txt) = e.unescape() {
                        current_text.push_str(&txt);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if tag == "dict" {
                    dict_depth = dict_depth.saturating_sub(1);
                } else if tag == "key" && in_key {
                    in_key = false;
                    current_key = Some(current_text.trim().to_string());
                    current_text.clear();
                } else if in_val && tag == val_tag {
                    in_val = false;
                    if let Some(key) = current_key.take() {
                        let val = current_text.trim().to_string();
                        match key.as_str() {
                            "CFBundleIdentifier" => dict.bundle_identifier = Some(val.clone()),
                            "CFBundleName" => {
                                if dict.bundle_name.is_none() {
                                    dict.bundle_name = Some(val.clone());
                                }
                            }
                            "CFBundleDisplayName" => dict.bundle_name = Some(val.clone()),
                            "CFBundleVersion" => dict.bundle_version = Some(val.clone()),
                            "CFBundleShortVersionString" => dict.bundle_short_version = Some(val.clone()),
                            "LSMinimumSystemVersion" => dict.minimum_os_version = Some(val.clone()),
                            "CFBundleExecutable" => dict.executable_name = Some(val.clone()),
                            _ => {}
                        }
                        dict.entries.insert(key, val);
                    }
                    current_text.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(TTZipError::IoError {
                    message: format!("XML Parse Error in Property List: {e}"),
                })
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(dict)
}
