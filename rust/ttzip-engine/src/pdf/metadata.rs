// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust PDF Info Dictionary and XMP XML Metadata Extractor.
//!
//! Streams and parses Trailer `/Info` dictionaries and Catalog `/Metadata` XMP XML streams,
//! extracting Dublin Core (`dc:`), XMP Basic (`xmp:`), Adobe PDF (`pdf:`), and PDF/A schemas
//! with normalized ISO 8601 date parsing.

use std::collections::BTreeMap;

use lopdf::Object;
use roxmltree::Document as XmlDocument;

use super::parser::TTZipPdfParser;
use super::PdfError;

/// Structured metadata extracted from an embedded XMP (Extensible Metadata Platform) XML packet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XmpMetadata {
    /// Dublin Core title (`dc:title`).
    pub dc_title: Option<String>,
    /// Dublin Core authors/creators list (`dc:creator`).
    pub dc_creators: Vec<String>,
    /// Dublin Core description or abstract (`dc:description`).
    pub dc_description: Option<String>,
    /// Dublin Core subject keywords (`dc:subject`).
    pub dc_subjects: Vec<String>,
    /// Dublin Core copyright/rights statement (`dc:rights`).
    pub dc_rights: Option<String>,
    /// XMP creation date (`xmp:CreateDate`).
    pub xmp_create_date: Option<String>,
    /// XMP modification date (`xmp:ModifyDate`).
    pub xmp_modify_date: Option<String>,
    /// Software used to create the document (`xmp:CreatorTool`).
    pub xmp_creator_tool: Option<String>,
    /// Conversion engine name (`pdf:Producer`).
    pub pdf_producer: Option<String>,
    /// PDF keywords list (`pdf:Keywords`).
    pub pdf_keywords: Option<String>,
    /// PDF version string specified in XMP (`pdf:PDFVersion`).
    pub pdf_version: Option<String>,
    /// PDF/A specification part number (`pdfaid:part`).
    pub pdfa_part: Option<u32>,
    /// PDF/A conformance level e.g. "A", "B", "U" (`pdfaid:conformance`).
    pub pdfa_conformance: Option<String>,
    /// Raw unparsed XMP XML string if preserved.
    pub raw_xmp_xml: Option<String>,
}

/// Unified document metadata combining `/Info` dictionary entries and embedded XMP data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PdfMetadata {
    /// Document title (preferring XMP title when available, falling back to /Info Title).
    pub title: Option<String>,
    /// Author / primary creator.
    pub author: Option<String>,
    /// Subject or document description.
    pub subject: Option<String>,
    /// Aggregated keywords list.
    pub keywords: Vec<String>,
    /// Application that created the original source document.
    pub creator: Option<String>,
    /// PDF engine or producer software.
    pub producer: Option<String>,
    /// Normalized creation timestamp (ISO 8601).
    pub creation_date: Option<String>,
    /// Normalized modification timestamp (ISO 8601).
    pub modification_date: Option<String>,
    /// PDF specification version (e.g. "1.7").
    pub pdf_version: String,
    /// Whether document security encryption is active.
    pub is_encrypted: bool,
    /// Total page count.
    pub page_count: u32,
    /// Non-standard or custom key-value pairs from the `/Info` dictionary.
    pub custom_info: BTreeMap<String, String>,
    /// Complete parsed XMP metadata packet if present.
    pub xmp: Option<XmpMetadata>,
}

/// Pure Safe Rust PDF metadata extractor.
pub struct PdfMetadataExtractor;

impl PdfMetadataExtractor {
    /// Extracts unified metadata from both Trailer `/Info` and Catalog `/Metadata` (XMP).
    pub fn extract_metadata(parser: &TTZipPdfParser) -> Result<PdfMetadata, PdfError> {
        let mut meta = PdfMetadata {
            pdf_version: format!("PDF-{}", parser.version()),
            is_encrypted: parser.is_encrypted(),
            page_count: parser.page_count(),
            ..Default::default()
        };

        // 1. Parse /Info dictionary
        Self::populate_from_info_dict(parser, &mut meta);

        // 2. Parse XMP /Metadata stream
        if let Ok(Some(xmp)) = Self::extract_xmp_metadata(parser) {
            // Apply XMP higher-precedence overrides or fill missing fields
            if meta.title.is_none() && xmp.dc_title.is_some() {
                meta.title = xmp.dc_title.clone();
            }
            if meta.author.is_none() && !xmp.dc_creators.is_empty() {
                meta.author = Some(xmp.dc_creators.join(", "));
            }
            if meta.subject.is_none() && xmp.dc_description.is_some() {
                meta.subject = xmp.dc_description.clone();
            }
            if meta.creator.is_none() && xmp.xmp_creator_tool.is_some() {
                meta.creator = xmp.xmp_creator_tool.clone();
            }
            if meta.producer.is_none() && xmp.pdf_producer.is_some() {
                meta.producer = xmp.pdf_producer.clone();
            }
            if meta.creation_date.is_none() && xmp.xmp_create_date.is_some() {
                meta.creation_date = xmp.xmp_create_date.clone();
            }
            if meta.modification_date.is_none() && xmp.xmp_modify_date.is_some() {
                meta.modification_date = xmp.xmp_modify_date.clone();
            }

            for subj in &xmp.dc_subjects {
                if !meta.keywords.contains(subj) {
                    meta.keywords.push(subj.clone());
                }
            }

            meta.xmp = Some(xmp);
        }

        Ok(meta)
    }

    /// Extracts and parses the embedded XMP XML metadata stream from the Catalog `/Metadata` key.
    pub fn extract_xmp_metadata(parser: &TTZipPdfParser) -> Result<Option<XmpMetadata>, PdfError> {
        let catalog = match parser.catalog() {
            Ok(cat) => cat,
            Err(_) => return Ok(None),
        };

        let meta_obj = match catalog.get(b"Metadata") {
            Ok(obj) => match parser.resolve_reference(obj) {
                Ok(deref) => deref,
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        };

        let stream = match meta_obj {
            Object::Stream(s) => s,
            _ => return Ok(None),
        };

        let content_bytes = stream
            .decompressed_content()
            .map_err(|e| PdfError::StreamDecodeError(e.to_string()))?;

        let xml_str = String::from_utf8_lossy(&content_bytes);
        let xmp = Self::parse_xmp_xml(&xml_str)?;
        Ok(Some(xmp))
    }

    /// Parses an XMP XML packet string using `roxmltree`.
    pub fn parse_xmp_xml(xml_content: &str) -> Result<XmpMetadata, PdfError> {
        let doc = XmlDocument::parse(xml_content)
            .map_err(|e| PdfError::XmlParseError(e.to_string()))?;

        let mut xmp = XmpMetadata {
            raw_xmp_xml: Some(xml_content.to_string()),
            ..Default::default()
        };

        for node in doc.descendants() {
            let tag_name = node.tag_name().name();
            match tag_name {
                "title" => {
                    if let Some(val) = Self::extract_rdf_text(node) {
                        xmp.dc_title = Some(val);
                    }
                }
                "creator" => {
                    let creators = Self::extract_rdf_list(node);
                    if !creators.is_empty() {
                        xmp.dc_creators = creators;
                    }
                }
                "description" => {
                    if let Some(val) = Self::extract_rdf_text(node) {
                        xmp.dc_description = Some(val);
                    }
                }
                "subject" => {
                    let subjects = Self::extract_rdf_list(node);
                    if !subjects.is_empty() {
                        xmp.dc_subjects = subjects;
                    }
                }
                "rights" => {
                    if let Some(val) = Self::extract_rdf_text(node) {
                        xmp.dc_rights = Some(val);
                    }
                }
                "CreateDate" => {
                    if let Some(text) = node.text() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            xmp.xmp_create_date = Some(trimmed.to_string());
                        }
                    }
                }
                "ModifyDate" => {
                    if let Some(text) = node.text() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            xmp.xmp_modify_date = Some(trimmed.to_string());
                        }
                    }
                }
                "CreatorTool" => {
                    if let Some(text) = node.text() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            xmp.xmp_creator_tool = Some(trimmed.to_string());
                        }
                    }
                }
                "Producer" => {
                    if let Some(text) = node.text() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            xmp.pdf_producer = Some(trimmed.to_string());
                        }
                    }
                }
                "Keywords" => {
                    if let Some(text) = node.text() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            xmp.pdf_keywords = Some(trimmed.to_string());
                        }
                    }
                }
                "PDFVersion" => {
                    if let Some(text) = node.text() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            xmp.pdf_version = Some(trimmed.to_string());
                        }
                    }
                }
                "part" => {
                    if let Some(text) = node.text() {
                        if let Ok(part_num) = text.trim().parse::<u32>() {
                            xmp.pdfa_part = Some(part_num);
                        }
                    }
                }
                "conformance" => {
                    if let Some(text) = node.text() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            xmp.pdfa_conformance = Some(trimmed.to_string());
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(xmp)
    }

    /// Reads and parses all fields from Trailer `/Info` dictionary.
    fn populate_from_info_dict(parser: &TTZipPdfParser, meta: &mut PdfMetadata) {
        let info_dict = match parser.trailer().get(b"Info") {
            Ok(obj) => match parser.resolve_reference(obj) {
                Ok(Object::Dictionary(dict)) => dict,
                _ => return,
            },
            _ => return,
        };

        for (key_bytes, val_obj) in info_dict.iter() {
            let key_str = String::from_utf8_lossy(key_bytes).to_string();
            let val_str = parser.resolve_string(val_obj);

            if let Some(val) = val_str {
                match key_bytes.as_slice() {
                    b"Title" => meta.title = Some(val),
                    b"Author" => meta.author = Some(val),
                    b"Subject" => meta.subject = Some(val),
                    b"Creator" => meta.creator = Some(val),
                    b"Producer" => meta.producer = Some(val),
                    b"CreationDate" => meta.creation_date = Some(Self::normalize_pdf_date(&val)),
                    b"ModDate" => meta.modification_date = Some(Self::normalize_pdf_date(&val)),
                    b"Keywords" => {
                        let kw_split: Vec<String> = val
                            .split([',', ';', '\n'])
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        meta.keywords = kw_split;
                    }
                    _ => {
                        meta.custom_info.insert(key_str, val);
                    }
                }
            }
        }
    }

    /// Normalizes PDF date formats (`D:YYYYMMDDHHmmSS[+-]HH'mm'`) into standard ISO 8601.
    pub fn normalize_pdf_date(raw: &str) -> String {
        let clean = raw.trim().trim_start_matches("D:").trim();
        if clean.len() < 4 {
            return raw.to_string();
        }

        // Format: YYYYMMDDHHmmSS...
        let year = &clean[0..4.min(clean.len())];
        let month = if clean.len() >= 6 { &clean[4..6] } else { "01" };
        let day = if clean.len() >= 8 { &clean[6..8] } else { "01" };
        let hour = if clean.len() >= 10 { &clean[8..10] } else { "00" };
        let min = if clean.len() >= 12 { &clean[10..12] } else { "00" };
        let sec = if clean.len() >= 14 { &clean[12..14] } else { "00" };

        let mut date_str = format!("{}-{}-{}T{}:{}:{}", year, month, day, hour, min, sec);

        // Parse timezone offset e.g. +08'00', -05'00', or Z
        if clean.len() > 14 {
            let tz_part = &clean[14..];
            if tz_part.starts_with('Z') || tz_part.starts_with('z') {
                date_str.push('Z');
            } else if tz_part.starts_with('+') || tz_part.starts_with('-') {
                let sign = &tz_part[0..1];
                let tz_digits: String = tz_part[1..].chars().filter(|c| c.is_ascii_digit()).collect();
                if tz_digits.len() >= 4 {
                    date_str.push_str(&format!("{}{}:{}", sign, &tz_digits[0..2], &tz_digits[2..4]));
                } else if tz_digits.len() >= 2 {
                    date_str.push_str(&format!("{}{}:00", sign, &tz_digits[0..2]));
                }
            }
        }

        date_str
    }

    fn extract_rdf_text<'a>(node: roxmltree::Node<'a, 'a>) -> Option<String> {
        // Direct text
        if let Some(text) = node.text() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        // Nested <rdf:Alt><rdf:li> or <rdf:Seq><rdf:li>
        for child in node.descendants() {
            if child.tag_name().name() == "li" {
                if let Some(text) = child.text() {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
        None
    }

    fn extract_rdf_list<'a>(node: roxmltree::Node<'a, 'a>) -> Vec<String> {
        let mut list = Vec::new();
        for child in node.descendants() {
            if child.tag_name().name() == "li" {
                if let Some(text) = child.text() {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        list.push(trimmed.to_string());
                    }
                }
            }
        }
        if list.is_empty() {
            if let Some(text) = node.text() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    list.push(trimmed.to_string());
                }
            }
        }
        list
    }
}
