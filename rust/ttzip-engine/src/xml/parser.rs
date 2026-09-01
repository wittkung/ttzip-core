// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-copy streaming XML pull parser based on `quick-xml`.
//!
//! Provides event-driven tokenization, zero-allocation QName prefix/local-name extraction,
//! adaptive buffer pooling to eliminate allocation churn, and bounded traversal helpers.

use std::borrow::Cow;
use std::io::Cursor;
use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::name::QName;
use quick_xml::Reader as QuickXmlReader;

use super::XmlError;

/// Pre-allocated reusable buffer pool to minimize heap allocation during streaming XML parsing.
#[derive(Debug, Clone)]
pub struct AdaptiveBufferPool {
    buffer: Vec<u8>,
    max_capacity: usize,
}

impl Default for AdaptiveBufferPool {
    fn default() -> Self {
        Self::new(1024, 64 * 1024)
    }
}

impl AdaptiveBufferPool {
    /// Creates a new buffer pool with an initial capacity and a maximum retention capacity.
    #[inline]
    pub fn new(initial_capacity: usize, max_capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(initial_capacity),
            max_capacity,
        }
    }

    /// Acquires mutable reference to the internal buffer, clearing existing bytes.
    /// If the buffer grew beyond `max_capacity`, it is shrunk to prevent memory retention spikes.
    #[inline]
    pub fn get_buf(&mut self) -> &mut Vec<u8> {
        if self.buffer.capacity() > self.max_capacity {
            self.buffer = Vec::with_capacity(self.max_capacity);
        } else {
            self.buffer.clear();
        }
        &mut self.buffer
    }
}

/// Zero-copy event-driven XML pull parser wrapping `quick_xml::Reader`.
pub struct TTZipXmlParser<'a> {
    reader: QuickXmlReader<Cursor<&'a [u8]>>,
    pool: AdaptiveBufferPool,
    depth: usize,
}

impl<'a> TTZipXmlParser<'a> {
    /// Constructs a new `TTZipXmlParser` from an in-memory byte slice.
    pub fn from_slice(bytes: &'a [u8]) -> Self {
        let mut reader = QuickXmlReader::from_reader(Cursor::new(bytes));
        reader.config_mut().trim_text(true);
        reader.config_mut().expand_empty_elements = false;
        Self {
            reader,
            pool: AdaptiveBufferPool::default(),
            depth: 0,
        }
    }

    /// Configures whether text contents should have leading/trailing whitespace trimmed.
    pub fn set_trim_text(&mut self, trim: bool) {
        self.reader.config_mut().trim_text(trim);
    }

    /// Configures whether empty elements (`<tag/>`) are automatically expanded into Start and End events.
    pub fn set_expand_empty_elements(&mut self, expand: bool) {
        self.reader.config_mut().expand_empty_elements = expand;
    }

    /// Current nesting depth of XML elements.
    #[inline]
    pub fn current_depth(&self) -> usize {
        self.depth
    }

    /// Reads the next XML event into the provided buffer, tracking element depth.
    pub fn read_event_into<'b>(&mut self, buf: &'b mut Vec<u8>) -> Result<Event<'b>, XmlError> {
        let event = self.reader.read_event_into(buf)?;
        match &event {
            Event::Start(_) => {
                self.depth = self.depth.saturating_add(1);
            }
            Event::End(_) => {
                self.depth = self.depth.saturating_sub(1);
            }
            _ => {}
        }
        Ok(event)
    }

    /// Reads the next XML event using the internal adaptive buffer pool.
    pub fn next_event(&mut self) -> Result<Event<'_>, XmlError> {
        let buf = self.pool.get_buf();
        let event = self.reader.read_event_into(buf)?;
        match &event {
            Event::Start(_) => {
                self.depth = self.depth.saturating_add(1);
            }
            Event::End(_) => {
                self.depth = self.depth.saturating_sub(1);
            }
            _ => {}
        }
        Ok(event)
    }

    /// Splits a qualified XML tag name into `(Option<prefix>, local_name)` without allocating.
    #[inline]
    pub fn split_qname(name: &[u8]) -> (Option<&[u8]>, &[u8]) {
        if let Some(pos) = name.iter().position(|&b| b == b':') {
            (Some(&name[..pos]), &name[pos + 1..])
        } else {
            (None, name)
        }
    }

    /// Extracts local name slice from `QName` ignoring prefix.
    #[inline]
    pub fn local_name<'b>(qname: QName<'b>) -> &'b [u8] {
        Self::split_qname(qname.into_inner()).1
    }

    /// Reads text content inside the current element until the matching End element is reached.
    pub fn read_element_text(&mut self, target_tag: &[u8]) -> Result<String, XmlError> {
        let target_local = Self::split_qname(target_tag).1;
        let mut text = String::new();
        let mut buf = Vec::with_capacity(256);
        let mut inner_depth = 0usize;

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let local = Self::local_name(e.name());
                    if local == target_local {
                        inner_depth += 1;
                    }
                }
                Event::End(ref e) => {
                    let local = Self::local_name(e.name());
                    if local == target_local {
                        if inner_depth == 0 {
                            self.depth = self.depth.saturating_sub(1);
                            break;
                        }
                        inner_depth -= 1;
                    }
                }
                Event::Text(ref e) => {
                    let decoded = e.unescape()?;
                    text.push_str(&decoded);
                }
                Event::CData(ref e) => {
                    let s = std::str::from_utf8(e.as_ref())?;
                    text.push_str(s);
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(text)
    }

    /// Skips the entire subtree of the currently opened element until its matching end tag.
    pub fn skip_subtree(&mut self, target_tag: &[u8]) -> Result<(), XmlError> {
        let target_local = Self::split_qname(target_tag).1;
        let mut depth = 1usize;
        let mut buf = Vec::with_capacity(128);

        while depth > 0 {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    if Self::local_name(e.name()) == target_local {
                        depth += 1;
                    }
                }
                Event::End(ref e) => {
                    if Self::local_name(e.name()) == target_local {
                        depth -= 1;
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        self.depth = self.depth.saturating_sub(1);
        Ok(())
    }

    /// Searches for an attribute value on a `BytesStart` element by local or qualified key.
    pub fn get_attribute<'b>(e: &'b BytesStart<'_>, attr_name: &[u8]) -> Option<Cow<'b, str>> {
        let attr_local = Self::split_qname(attr_name).1;
        for attr in e.attributes().flatten() {
            let key_local = Self::split_qname(attr.key.as_ref()).1;
            if key_local == attr_local || attr.key.as_ref() == attr_name {
                if let Ok(val) = attr.unescape_value() {
                    return Some(val);
                }
            }
        }
        None
    }

    /// Decodes a `BytesText` event to an unescaped `String`.
    pub fn decode_text(e: &BytesText<'_>) -> Result<String, XmlError> {
        let unescaped = e.unescape()?;
        Ok(unescaped.into_owned())
    }

    /// Checks if a `BytesStart` or `BytesEnd` matches a given tag name (local or full).
    #[inline]
    pub fn matches_tag(name: &[u8], expected: &[u8]) -> bool {
        let name_local = Self::split_qname(name).1;
        let exp_local = Self::split_qname(expected).1;
        name_local == exp_local || name == expected
    }
}

/// Convenience helper to extract plain text between opening and closing tags from raw XML bytes.
pub fn extract_single_element_text(xml: &[u8], tag_name: &[u8]) -> Option<String> {
    let mut parser = TTZipXmlParser::from_slice(xml);
    let mut buf = Vec::with_capacity(512);
    let target_local = TTZipXmlParser::split_qname(tag_name).1;

    loop {
        match parser.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if TTZipXmlParser::local_name(e.name()) == target_local {
                    return parser.read_element_text(tag_name).ok();
                }
            }
            Ok(Event::Empty(_)) => {}
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}
