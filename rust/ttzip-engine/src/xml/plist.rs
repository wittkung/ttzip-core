// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Apple XML Property List (Plist) streaming parser and App Bundle `Info.plist` metadata extractor.
//!
//! Conforms to `Apple//DTD PLIST 1.0//EN` specification with strong AST representations,
//! arbitrary nested array/dict hierarchies, base64 payload decoding, and macOS App metadata extraction.

use std::collections::BTreeMap;
use quick_xml::events::Event;

use super::parser::TTZipXmlParser;
use super::XmlError;

/// Strongly-typed Algebraic Data Type representing any valid Apple Property List value.
#[derive(Debug, Clone, PartialEq)]
pub enum PlistValue {
    /// UTF-8 text string.
    String(String),
    /// 64-bit signed integer.
    Integer(i64),
    /// 64-bit IEEE-754 floating point number.
    Real(f64),
    /// Boolean flag (`<true/>` or `<false/>`).
    Boolean(bool),
    /// ISO 8601 UTC date string.
    Date(String),
    /// Binary data decoded from base64 representation.
    Data(Vec<u8>),
    /// Ordered sequence of Plist values.
    Array(Vec<PlistValue>),
    /// Key-value dictionary sorted lexicographically by key.
    Dictionary(BTreeMap<String, PlistValue>),
}

impl PlistValue {
    /// Borrows value as string slice if variant is `PlistValue::String`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Returns integer if variant is `PlistValue::Integer`.
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Returns floating point number if variant is `PlistValue::Real`.
    pub fn as_real(&self) -> Option<f64> {
        match self {
            Self::Real(f) => Some(*f),
            _ => None,
        }
    }

    /// Returns boolean if variant is `PlistValue::Boolean`.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Borrows slice of values if variant is `PlistValue::Array`.
    pub fn as_array(&self) -> Option<&[PlistValue]> {
        match self {
            Self::Array(arr) => Some(arr.as_slice()),
            _ => None,
        }
    }

    /// Borrows dictionary map if variant is `PlistValue::Dictionary`.
    pub fn as_dict(&self) -> Option<&BTreeMap<String, PlistValue>> {
        match self {
            Self::Dictionary(d) => Some(d),
            _ => None,
        }
    }

    /// Looks up a value by string key in a dictionary variant.
    pub fn get(&self, key: &str) -> Option<&PlistValue> {
        self.as_dict().and_then(|d| d.get(key))
    }

    /// Convenience lookup for string values in a dictionary variant.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_str())
    }

    /// Convenience lookup for boolean values in a dictionary variant.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }

    /// Convenience lookup for integer values in a dictionary variant.
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_integer())
    }
}

/// Extracted metadata from macOS / iOS App Bundle `Info.plist`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InfoPlistMeta {
    pub bundle_identifier: Option<String>,
    pub bundle_name: Option<String>,
    pub bundle_display_name: Option<String>,
    pub bundle_short_version_string: Option<String>,
    pub bundle_version: Option<String>,
    pub bundle_executable: Option<String>,
    pub bundle_package_type: Option<String>,
    pub minimum_system_version: Option<String>,
    pub human_readable_copyright: Option<String>,
    pub ui_element: Option<bool>,
    pub principal_class: Option<String>,
    pub raw_dict: BTreeMap<String, PlistValue>,
}

/// Apple XML Property List streaming parser.
pub struct ApplePlistParser;

impl ApplePlistParser {
    /// Parses an Apple XML Plist byte slice into a strong AST `PlistValue`.
    pub fn parse_xml_plist(xml_bytes: &[u8]) -> Result<PlistValue, XmlError> {
        let mut parser = TTZipXmlParser::from_slice(xml_bytes);
        let mut buf = Vec::with_capacity(512);

        // Find top-level <plist> element or first container
        loop {
            match parser.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    if local == b"plist" {
                        let value = Self::parse_next_value(&mut parser)?;
                        return value.ok_or_else(|| {
                            XmlError::Malformed("Empty <plist> container element".to_string())
                        });
                    } else if local == b"dict" || local == b"array" {
                        return Self::parse_container_value(&mut parser, local);
                    }
                }
                Event::Eof => {
                    return Err(XmlError::MissingRoot(
                        "Missing root <plist> element in XML stream".to_string(),
                    ));
                }
                _ => {}
            }
            buf.clear();
        }
    }

    /// Parses macOS App Bundle `Info.plist` byte slice into structured `InfoPlistMeta`.
    pub fn parse_info_plist(xml_bytes: &[u8]) -> Result<InfoPlistMeta, XmlError> {
        let root = Self::parse_xml_plist(xml_bytes)?;
        let dict = match root {
            PlistValue::Dictionary(d) => d,
            _ => {
                return Err(XmlError::InvalidPlist(
                    "Expected top-level Dictionary in Info.plist".to_string(),
                ))
            }
        };

        let mut meta = InfoPlistMeta {
            bundle_identifier: dict
                .get("CFBundleIdentifier")
                .and_then(|v| v.as_str())
                .map(String::from),
            bundle_name: dict
                .get("CFBundleName")
                .and_then(|v| v.as_str())
                .map(String::from),
            bundle_display_name: dict
                .get("CFBundleDisplayName")
                .and_then(|v| v.as_str())
                .map(String::from),
            bundle_short_version_string: dict
                .get("CFBundleShortVersionString")
                .and_then(|v| v.as_str())
                .map(String::from),
            bundle_version: dict
                .get("CFBundleVersion")
                .and_then(|v| v.as_str())
                .map(String::from),
            bundle_executable: dict
                .get("CFBundleExecutable")
                .and_then(|v| v.as_str())
                .map(String::from),
            bundle_package_type: dict
                .get("CFBundlePackageType")
                .and_then(|v| v.as_str())
                .map(String::from),
            minimum_system_version: dict
                .get("LSMinimumSystemVersion")
                .or_else(|| dict.get("MinimumOSVersion"))
                .and_then(|v| v.as_str())
                .map(String::from),
            human_readable_copyright: dict
                .get("NSHumanReadableCopyright")
                .and_then(|v| v.as_str())
                .map(String::from),
            ui_element: dict
                .get("LSUIElement")
                .and_then(|v| v.as_bool())
                .or_else(|| {
                    dict.get("LSUIElement")
                        .and_then(|v| v.as_str())
                        .map(|s| s == "1" || s == "true")
                }),
            principal_class: dict
                .get("NSPrincipalClass")
                .and_then(|v| v.as_str())
                .map(String::from),
            raw_dict: dict,
        };

        if meta.bundle_name.is_none() {
            meta.bundle_name = meta.bundle_display_name.clone();
        }

        Ok(meta)
    }

    /// Recursively parses the next Plist value from the XML token stream.
    fn parse_next_value(parser: &mut TTZipXmlParser<'_>) -> Result<Option<PlistValue>, XmlError> {
        let mut buf = Vec::with_capacity(256);

        loop {
            match parser.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"string" => {
                            let text = parser.read_element_text(b"string")?;
                            return Ok(Some(PlistValue::String(text)));
                        }
                        b"integer" => {
                            let text = parser.read_element_text(b"integer")?;
                            let val = parse_plist_integer(&text)?;
                            return Ok(Some(PlistValue::Integer(val)));
                        }
                        b"real" => {
                            let text = parser.read_element_text(b"real")?;
                            let val = text.trim().parse::<f64>().unwrap_or(0.0);
                            return Ok(Some(PlistValue::Real(val)));
                        }
                        b"true" => {
                            let _ = parser.read_element_text(b"true")?;
                            return Ok(Some(PlistValue::Boolean(true)));
                        }
                        b"false" => {
                            let _ = parser.read_element_text(b"false")?;
                            return Ok(Some(PlistValue::Boolean(false)));
                        }
                        b"date" => {
                            let text = parser.read_element_text(b"date")?;
                            return Ok(Some(PlistValue::Date(text.trim().to_string())));
                        }
                        b"data" => {
                            let text = parser.read_element_text(b"data")?;
                            let decoded = decode_base64_data(&text);
                            return Ok(Some(PlistValue::Data(decoded)));
                        }
                        b"dict" => {
                            let dict = Self::parse_dict(parser)?;
                            return Ok(Some(PlistValue::Dictionary(dict)));
                        }
                        b"array" => {
                            let arr = Self::parse_array(parser)?;
                            return Ok(Some(PlistValue::Array(arr)));
                        }
                        _ => {}
                    }
                }
                Event::Empty(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"true" => return Ok(Some(PlistValue::Boolean(true))),
                        b"false" => return Ok(Some(PlistValue::Boolean(false))),
                        b"string" => return Ok(Some(PlistValue::String(String::new()))),
                        b"data" => return Ok(Some(PlistValue::Data(Vec::new()))),
                        b"dict" => return Ok(Some(PlistValue::Dictionary(BTreeMap::new()))),
                        b"array" => return Ok(Some(PlistValue::Array(Vec::new()))),
                        _ => {}
                    }
                }
                Event::End(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    if local == b"plist" || local == b"dict" || local == b"array" {
                        return Ok(None);
                    }
                }
                Event::Eof => return Ok(None),
                _ => {}
            }
            buf.clear();
        }
    }

    /// Handles root `<dict>` or `<array>` starting without outer `<plist>`.
    fn parse_container_value(
        parser: &mut TTZipXmlParser<'_>,
        local_name: &[u8],
    ) -> Result<PlistValue, XmlError> {
        if local_name == b"dict" {
            let dict = Self::parse_dict(parser)?;
            Ok(PlistValue::Dictionary(dict))
        } else {
            let arr = Self::parse_array(parser)?;
            Ok(PlistValue::Array(arr))
        }
    }

    /// Parses `<dict>` key-value pairs until closing `</dict>`.
    fn parse_dict(
        parser: &mut TTZipXmlParser<'_>,
    ) -> Result<BTreeMap<String, PlistValue>, XmlError> {
        let mut map = BTreeMap::new();
        let mut buf = Vec::with_capacity(256);
        let mut current_key: Option<String> = None;

        loop {
            match parser.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    if local == b"key" {
                        let key_text = parser.read_element_text(b"key")?;
                        current_key = Some(key_text.trim().to_string());
                    } else if let Some(key) = current_key.take() {
                        let value = match local {
                            b"string" => {
                                let text = parser.read_element_text(b"string")?;
                                PlistValue::String(text)
                            }
                            b"integer" => {
                                let text = parser.read_element_text(b"integer")?;
                                PlistValue::Integer(parse_plist_integer(&text)?)
                            }
                            b"real" => {
                                let text = parser.read_element_text(b"real")?;
                                PlistValue::Real(text.trim().parse().unwrap_or(0.0))
                            }
                            b"true" => {
                                let _ = parser.read_element_text(b"true")?;
                                PlistValue::Boolean(true)
                            }
                            b"false" => {
                                let _ = parser.read_element_text(b"false")?;
                                PlistValue::Boolean(false)
                            }
                            b"date" => {
                                let text = parser.read_element_text(b"date")?;
                                PlistValue::Date(text.trim().to_string())
                            }
                            b"data" => {
                                let text = parser.read_element_text(b"data")?;
                                PlistValue::Data(decode_base64_data(&text))
                            }
                            b"dict" => PlistValue::Dictionary(Self::parse_dict(parser)?),
                            b"array" => PlistValue::Array(Self::parse_array(parser)?),
                            _ => {
                                parser.skip_subtree(local)?;
                                continue;
                            }
                        };
                        map.insert(key, value);
                    } else {
                        parser.skip_subtree(local)?;
                    }
                }
                Event::Empty(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    if let Some(key) = current_key.take() {
                        let value = match local {
                            b"true" => PlistValue::Boolean(true),
                            b"false" => PlistValue::Boolean(false),
                            b"string" => PlistValue::String(String::new()),
                            b"data" => PlistValue::Data(Vec::new()),
                            b"dict" => PlistValue::Dictionary(BTreeMap::new()),
                            b"array" => PlistValue::Array(Vec::new()),
                            _ => continue,
                        };
                        map.insert(key, value);
                    }
                }
                Event::End(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    if local == b"dict" {
                        break;
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(map)
    }

    /// Parses `<array>` elements until closing `</array>`.
    fn parse_array(parser: &mut TTZipXmlParser<'_>) -> Result<Vec<PlistValue>, XmlError> {
        let mut list = Vec::new();
        let mut buf = Vec::with_capacity(256);

        loop {
            match parser.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    let val = match local {
                        b"string" => {
                            let text = parser.read_element_text(b"string")?;
                            PlistValue::String(text)
                        }
                        b"integer" => {
                            let text = parser.read_element_text(b"integer")?;
                            PlistValue::Integer(parse_plist_integer(&text)?)
                        }
                        b"real" => {
                            let text = parser.read_element_text(b"real")?;
                            PlistValue::Real(text.trim().parse().unwrap_or(0.0))
                        }
                        b"true" => {
                            let _ = parser.read_element_text(b"true")?;
                            PlistValue::Boolean(true)
                        }
                        b"false" => {
                            let _ = parser.read_element_text(b"false")?;
                            PlistValue::Boolean(false)
                        }
                        b"date" => {
                            let text = parser.read_element_text(b"date")?;
                            PlistValue::Date(text.trim().to_string())
                        }
                        b"data" => {
                            let text = parser.read_element_text(b"data")?;
                            PlistValue::Data(decode_base64_data(&text))
                        }
                        b"dict" => PlistValue::Dictionary(Self::parse_dict(parser)?),
                        b"array" => PlistValue::Array(Self::parse_array(parser)?),
                        _ => {
                            parser.skip_subtree(local)?;
                            continue;
                        }
                    };
                    list.push(val);
                }
                Event::Empty(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    let val = match local {
                        b"true" => PlistValue::Boolean(true),
                        b"false" => PlistValue::Boolean(false),
                        b"string" => PlistValue::String(String::new()),
                        b"data" => PlistValue::Data(Vec::new()),
                        b"dict" => PlistValue::Dictionary(BTreeMap::new()),
                        b"array" => PlistValue::Array(Vec::new()),
                        _ => continue,
                    };
                    list.push(val);
                }
                Event::End(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    if local == b"array" {
                        break;
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(list)
    }
}

/// Parses integer representation in decimal or hex (`0x...`).
fn parse_plist_integer(text: &str) -> Result<i64, XmlError> {
    let trimmed = text.trim();
    if let Some(hex_str) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        i64::from_str_radix(hex_str, 16)
            .map_err(|_| XmlError::InvalidPlist(format!("Invalid hex integer: {trimmed}")))
    } else {
        trimmed
            .parse::<i64>()
            .map_err(|_| XmlError::InvalidPlist(format!("Invalid integer: {trimmed}")))
    }
}

/// Simple RFC 4648 Base64 decoder ignoring internal whitespace.
fn decode_base64_data(text: &str) -> Vec<u8> {
    const TABLE: [i8; 256] = {
        let mut t = [-1i8; 256];
        let mut i = 0u8;
        while i < 26 {
            t[(b'A' + i) as usize] = i as i8;
            t[(b'a' + i) as usize] = (i + 26) as i8;
            i += 1;
        }
        let mut d = 0u8;
        while d < 10 {
            t[(b'0' + d) as usize] = (d + 52) as i8;
            d += 1;
        }
        t[b'+' as usize] = 62;
        t[b'/' as usize] = 63;
        t
    };

    let mut out = Vec::with_capacity((text.len() * 3) / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;

    for &b in text.as_bytes() {
        if b.is_ascii_whitespace() {
            continue;
        }
        if b == b'=' {
            break;
        }
        let val = TABLE[b as usize];
        if val < 0 {
            continue;
        }
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }

    out
}
