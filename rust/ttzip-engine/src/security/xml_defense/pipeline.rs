// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unified XML security pipeline orchestrating all 6 defense layers.

use std::io::BufRead;

use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;

use super::guards::{
    AttributeAndCDataFuseGuard, EntityExpansionQuotaGuard, MaxDepthGuard, XxeExternalEntityGuard,
};
use super::XmlDefenseError;

/// Unified 6-layer defense orchestrator for secure streaming XML parsing.
#[derive(Debug, Clone)]
pub struct XmlSecurityPipeline {
    xxe_guard: XxeExternalEntityGuard,
    expansion_guard: EntityExpansionQuotaGuard,
    depth_guard: MaxDepthGuard,
    fuse_guard: AttributeAndCDataFuseGuard,
}

impl Default for XmlSecurityPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlSecurityPipeline {
    /// Creates a new unified security pipeline with default safety configurations.
    #[must_use]
    pub fn new() -> Self {
        Self {
            xxe_guard: XxeExternalEntityGuard,
            expansion_guard: EntityExpansionQuotaGuard::new(),
            depth_guard: MaxDepthGuard::new(),
            fuse_guard: AttributeAndCDataFuseGuard::new(),
        }
    }

    /// Validates raw XML bytes against all 6 security layers.
    pub fn validate_xml_bytes(&mut self, xml_bytes: &[u8]) -> Result<(), XmlDefenseError> {
        self.xxe_guard.scan(xml_bytes)?;
        self.expansion_guard.record_input_bytes(xml_bytes.len());
        self.parse_securely(xml_bytes, |_| Ok(()))
    }

    /// Securely parses a buffered reader stream, passing validated events to a consumer callback.
    pub fn parse_securely<R: BufRead, F>(
        &mut self,
        reader_source: R,
        mut callback: F,
    ) -> Result<(), XmlDefenseError>
    where
        F: FnMut(&Event<'_>) -> Result<(), XmlDefenseError>,
    {
        let mut reader = XmlReader::from_reader(reader_source);
        reader.config_mut().trim_text(false);

        let mut buf = Vec::with_capacity(512);
        loop {
            let event = reader.read_event_into(&mut buf).map_err(|e| {
                XmlDefenseError::MalformedXml {
                    reason: format!("{e:?}"),
                    offset: reader.buffer_position() as usize,
                }
            })?;

            match &event {
                Event::Start(e) => {
                    let tag_name = String::from_utf8_lossy(e.local_name().into_inner()).into_owned();
                    self.depth_guard.push_element(&tag_name)?;

                    let mut attr_count = 0;
                    for attr in e.attributes().flatten() {
                        attr_count += 1;
                        self.fuse_guard.inspect_attribute(attr.key.as_ref(), &attr.value)?;
                    }
                    self.fuse_guard.inspect_attribute_count(attr_count)?;
                }
                Event::End(e) => {
                    let tag_name = String::from_utf8_lossy(e.local_name().into_inner());
                    self.depth_guard.pop_element(&tag_name)?;
                }
                Event::Empty(e) => {
                    let mut attr_count = 0;
                    for attr in e.attributes().flatten() {
                        attr_count += 1;
                        self.fuse_guard.inspect_attribute(attr.key.as_ref(), &attr.value)?;
                    }
                    self.fuse_guard.inspect_attribute_count(attr_count)?;
                }
                Event::CData(e) => {
                    self.fuse_guard.inspect_cdata(e.as_ref())?;
                }
                Event::DocType(e) => {
                    let text = String::from_utf8_lossy(e.as_ref());
                    XxeExternalEntityGuard::sanitize_doctype(&text)?;
                }
                Event::PI(e) => {
                    XxeExternalEntityGuard::inspect_event(&Event::PI(e.clone()))?;
                }
                Event::Eof => {
                    if self.depth_guard.current_depth() > 0 {
                        return Err(XmlDefenseError::UnexpectedEof {
                            unclosed_tags: self.depth_guard.tag_stack().to_vec(),
                        });
                    }
                    break;
                }
                _ => {}
            }

            callback(&event)?;
            buf.clear();
        }

        Ok(())
    }

    /// Accesses the inner XXE guard.
    #[must_use]
    pub fn xxe_guard(&self) -> &XxeExternalEntityGuard {
        &self.xxe_guard
    }

    /// Accesses the inner depth guard.
    #[must_use]
    pub fn depth_guard(&self) -> &MaxDepthGuard {
        &self.depth_guard
    }

    /// Accesses the inner expansion quota guard.
    #[must_use]
    pub fn expansion_guard(&self) -> &EntityExpansionQuotaGuard {
        &self.expansion_guard
    }

    /// Accesses the inner fuse guard.
    #[must_use]
    pub fn fuse_guard(&self) -> &AttributeAndCDataFuseGuard {
        &self.fuse_guard
    }
}
