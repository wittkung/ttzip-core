// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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

fn apply_plist_bundle_key(dict: &mut UniFFIPlistDictionary, key: &str, val: &str) {
    match key {
        "CFBundleIdentifier" => dict.bundle_identifier = Some(val.to_string()),
        "CFBundleName" if dict.bundle_name.is_none() => dict.bundle_name = Some(val.to_string()),
        "CFBundleDisplayName" => dict.bundle_name = Some(val.to_string()),
        "CFBundleVersion" => dict.bundle_version = Some(val.to_string()),
        "CFBundleShortVersionString" => dict.bundle_short_version = Some(val.to_string()),
        "LSMinimumSystemVersion" => dict.minimum_os_version = Some(val.to_string()),
        "CFBundleExecutable" => dict.executable_name = Some(val.to_string()),
        _ => {}
    }
}

struct PlistParseState {
    current_key: Option<String>,
    in_key: bool,
    in_val: bool,
    val_tag: String,
    current_text: String,
    dict_depth: usize,
}

impl PlistParseState {
    fn on_start(&mut self, tag: String) {
        if tag == "dict" {
            self.dict_depth += 1;
        } else if tag == "key" && self.dict_depth == 1 {
            self.in_key = true;
            self.current_text.clear();
        } else if self.dict_depth == 1 && self.current_key.is_some() {
            self.in_val = true;
            self.val_tag = tag;
            self.current_text.clear();
        }
    }

    fn on_empty(&mut self, dict: &mut UniFFIPlistDictionary, tag: &str) {
        if self.dict_depth == 1 {
            if let Some(key) = self.current_key.take() {
                let value = match tag {
                    "true" => "true".to_string(),
                    "false" => "false".to_string(),
                    _ => String::new(),
                };
                dict.entries.insert(key, value);
            }
        }
    }

    fn on_end(&mut self, dict: &mut UniFFIPlistDictionary, tag: &str) {
        if tag == "dict" {
            self.dict_depth = self.dict_depth.saturating_sub(1);
        } else if tag == "key" && self.in_key {
            self.in_key = false;
            self.current_key = Some(self.current_text.trim().to_string());
            self.current_text.clear();
        } else if self.in_val && tag == self.val_tag {
            self.in_val = false;
            if let Some(key) = self.current_key.take() {
                let val = self.current_text.trim().to_string();
                apply_plist_bundle_key(dict, &key, &val);
                dict.entries.insert(key, val);
            }
            self.current_text.clear();
        }
    }
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
    let mut state = PlistParseState {
        current_key: None,
        in_key: false,
        in_val: false,
        val_tag: String::new(),
        current_text: String::new(),
        dict_depth: 0,
    };

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                state.on_start(tag);
            }
            Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                state.on_empty(&mut dict, &tag);
            }
            Ok(Event::Text(ref e)) if state.in_key || state.in_val => {
                if let Ok(txt) = e.unescape() {
                    state.current_text.push_str(&txt);
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                state.on_end(&mut dict, &tag);
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
