// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 1: Manifest Item Count & Quota Guard.
//!
//! Intercepts malformed OPF manifest bombs, hash flooding DoS, oversized XML files,
//! and entity expansion attacks (Billion Laughs) in EPUB containers.

use std::collections::HashMap;
use std::io::BufRead;
use quick_xml::events::Event;
use quick_xml::reader::Reader;

use super::{
    EbookDefenseError, MAX_HREF_LENGTH, MAX_ITEM_ID_LENGTH, MAX_MANIFEST_ITEMS, MAX_OPF_FILE_SIZE,
};

/// Represents a validated manifest item in an EPUB container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestItem {
    /// Unique identifier for the manifest item.
    pub id: String,
    /// Relative or virtual path to the resource.
    pub href: String,
    /// MIME media type of the resource (e.g. `application/xhtml+xml`).
    pub media_type: String,
    /// Optional EPUB 3 properties attribute (e.g. `nav`, `cover-image`, `scripted`).
    pub properties: Option<String>,
}

/// Guard protecting against manifest item flooding, attribute overflow, and DTD entity injection.
#[derive(Debug, Default, Clone)]
pub struct ManifestItemCountGuard {
    items: HashMap<String, ManifestItem>,
}

impl ManifestItemCountGuard {
    /// Creates a new, empty manifest item count guard.
    pub fn new() -> Self {
        Self {
            items: HashMap::with_capacity(128),
        }
    }

    /// Parses an OPF XML stream in a bounded, streaming pull-parser mode without building a DOM tree.
    pub fn parse_opf_stream<R: BufRead>(
        &mut self,
        mut reader: R,
        stream_len: u64,
    ) -> Result<&HashMap<String, ManifestItem>, EbookDefenseError> {
        if stream_len > MAX_OPF_FILE_SIZE {
            return Err(EbookDefenseError::OpfFileTooLarge {
                size: stream_len,
                limit: MAX_OPF_FILE_SIZE,
            });
        }

        let mut xml_reader = Reader::from_reader(&mut reader);
        xml_reader.config_mut().trim_text(true);
        xml_reader.config_mut().check_end_names = true;

        let mut buf = Vec::with_capacity(1024);
        let mut in_manifest = false;

        loop {
            match xml_reader.read_event_into(&mut buf) {
                Ok(Event::DocType(_)) => {
                    // Strictly block external DTD / entity declarations (Billion Laughs defense)
                    return Err(EbookDefenseError::DtdEntitiesForbidden);
                }
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"manifest" => {
                    in_manifest = true;
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"manifest" => {
                    break;
                }
                Ok(Event::Empty(ref e)) if in_manifest && e.name().as_ref() == b"item" => {
                    self.process_item_element(e)?;
                }
                Ok(Event::Start(ref e)) if in_manifest && e.name().as_ref() == b"item" => {
                    self.process_item_element(e)?;
                }
                Ok(Event::Eof) => break,
                Err(err) => return Err(EbookDefenseError::MalformedXml(err.to_string())),
                _ => {}
            }
            buf.clear();
        }

        Ok(&self.items)
    }

    /// Internal helper to validate and register a manifest `<item>` element.
    fn process_item_element(
        &mut self,
        element: &quick_xml::events::BytesStart<'_>,
    ) -> Result<(), EbookDefenseError> {
        if self.items.len() >= MAX_MANIFEST_ITEMS {
            return Err(EbookDefenseError::ManifestItemCountExceeded {
                count: self.items.len() + 1,
                limit: MAX_MANIFEST_ITEMS,
            });
        }

        let mut id = None;
        let mut href = None;
        let mut media_type = None;
        let mut properties = None;

        for attr in element.attributes().flatten() {
            match attr.key.as_ref() {
                b"id" => {
                    if attr.value.len() > MAX_ITEM_ID_LENGTH {
                        return Err(EbookDefenseError::AttributeLengthExceeded {
                            attr: "id",
                            len: attr.value.len(),
                            limit: MAX_ITEM_ID_LENGTH,
                        });
                    }
                    id = Some(String::from_utf8_lossy(&attr.value).to_string());
                }
                b"href" => {
                    if attr.value.len() > MAX_HREF_LENGTH {
                        return Err(EbookDefenseError::AttributeLengthExceeded {
                            attr: "href",
                            len: attr.value.len(),
                            limit: MAX_HREF_LENGTH,
                        });
                    }
                    href = Some(String::from_utf8_lossy(&attr.value).to_string());
                }
                b"media-type" => {
                    media_type = Some(String::from_utf8_lossy(&attr.value).to_string());
                }
                b"properties" => {
                    properties = Some(String::from_utf8_lossy(&attr.value).to_string());
                }
                _ => {}
            }
        }

        if let (Some(id_str), Some(href_str), Some(media_type_str)) = (id, href, media_type) {
            if self.items.contains_key(&id_str) {
                return Err(EbookDefenseError::DuplicateItemId(id_str));
            }
            self.items.insert(
                id_str.clone(),
                ManifestItem {
                    id: id_str,
                    href: href_str,
                    media_type: media_type_str,
                    properties,
                },
            );
        }

        Ok(())
    }

    /// Validates an external item count against configured limits.
    #[inline]
    pub fn validate_item_count(&self, count: usize) -> Result<(), EbookDefenseError> {
        if count > MAX_MANIFEST_ITEMS {
            Err(EbookDefenseError::ManifestItemCountExceeded {
                count,
                limit: MAX_MANIFEST_ITEMS,
            })
        } else {
            Ok(())
        }
    }

    /// Validates an href attribute string length.
    #[inline]
    pub fn validate_href(&self, href: &str) -> Result<(), EbookDefenseError> {
        if href.len() > MAX_HREF_LENGTH {
            Err(EbookDefenseError::AttributeLengthExceeded {
                attr: "href",
                len: href.len(),
                limit: MAX_HREF_LENGTH,
            })
        } else {
            Ok(())
        }
    }

    /// Validates an item ID attribute string length.
    #[inline]
    pub fn validate_id(&self, id: &str) -> Result<(), EbookDefenseError> {
        if id.len() > MAX_ITEM_ID_LENGTH {
            Err(EbookDefenseError::AttributeLengthExceeded {
                attr: "id",
                len: id.len(),
                limit: MAX_ITEM_ID_LENGTH,
            })
        } else {
            Ok(())
        }
    }

    /// Returns a reference to the parsed and verified manifest items map.
    #[inline]
    pub fn items(&self) -> &HashMap<String, ManifestItem> {
        &self.items
    }

    /// Returns the number of registered manifest items.
    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns whether the manifest items map is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Clears all registered manifest items.
    #[inline]
    pub fn clear(&mut self) {
        self.items.clear();
    }
}
