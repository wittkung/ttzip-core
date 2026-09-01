// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Records and Service Interfaces for Document Metadata.

use std::collections::HashMap;
use std::sync::Arc;

use crate::uniffi_api::types::TTZipError;
use super::office::{parse_office_metadata_from_slice, parse_office_outline_from_slice};
use super::epub::parse_epub_metadata_from_slice;
use super::plist::{uniffi_parse_plist_from_bytes, uniffi_parse_plist_xml};
use super::{uniffi_extract_epub_metadata, uniffi_extract_office_metadata, uniffi_extract_office_outline};

/// Universal document metadata record exposing Dublin Core and Office attributes.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIDocumentMetadata {
    /// Document format classification (e.g. "DOCX", "XLSX", "PPTX", "EPUB", "PropertyList").
    pub format_name: String,
    /// Document title or book name.
    pub title: Option<String>,
    /// Primary author, creator, or composer.
    pub author: Option<String>,
    /// Subject matter or topic category.
    pub subject: Option<String>,
    /// Summary description, abstract, or comments.
    pub description: Option<String>,
    /// Comma-separated keyword tags.
    pub keywords: Option<String>,
    /// Creation timestamp string (ISO 8601 or W3CDTF).
    pub created_date: Option<String>,
    /// Last modification timestamp string.
    pub modified_date: Option<String>,
    /// Name of the last user who edited the document.
    pub last_modified_by: Option<String>,
    /// Authoring application or software generator.
    pub application: Option<String>,
    /// Total page count for paginated documents (0 if not applicable).
    pub page_count: u32,
    /// Estimated or recorded word count.
    pub word_count: u32,
    /// Total character count.
    pub character_count: u32,
    /// Total slide count for presentation documents.
    pub slide_count: u32,
    /// Total worksheet count for spreadsheet documents.
    pub sheet_count: u32,
    /// List of worksheet names in spreadsheet workbooks.
    pub sheet_names: Vec<String>,
    /// Extracted slide titles or section header labels.
    pub slide_titles: Vec<String>,
    /// Arbitrary key-value custom properties map.
    pub custom_properties: HashMap<String, String>,
}

/// Structural outline of an Office compound document.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIOfficeOutline {
    /// High-level document category (e.g. "Word Processing", "Spreadsheet", "Presentation").
    pub document_type: String,
    /// Ordered list of section headings and paragraph titles (for DOCX).
    pub headings: Vec<String>,
    /// Ordered list of sheet names (for XLSX).
    pub sheets: Vec<String>,
    /// Ordered list of slide titles and notes headers (for PPTX).
    pub slides: Vec<String>,
    /// Total aggregate count of structural sections across all types.
    pub total_sections: u32,
    /// Leading plain text preview excerpt of the document content.
    pub summary_preview: String,
}

/// Dublin Core metadata record for EPUB digital publications.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIEpubMetadata {
    /// Title of the publication.
    pub title: String,
    /// List of contributing authors and creators.
    pub authors: Vec<String>,
    /// Publishing house or entity.
    pub publisher: Option<String>,
    /// Primary language code (e.g. "en", "zh-CN").
    pub language: Option<String>,
    /// ISBN, DOI, or unique package identifier.
    pub identifier: Option<String>,
    /// Book synopsis or description.
    pub description: Option<String>,
    /// Initial publication date string.
    pub publication_date: Option<String>,
    /// Last modified timestamp string.
    pub modified_date: Option<String>,
    /// Copyright and intellectual property statement.
    pub rights: Option<String>,
}

/// Parsed Apple Property List (XML plist) key-value dictionary.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIPlistDictionary {
    /// Bundle identifier (`CFBundleIdentifier`) if present.
    pub bundle_identifier: Option<String>,
    /// Application display or bundle name (`CFBundleName` / `CFBundleDisplayName`).
    pub bundle_name: Option<String>,
    /// Build version number string (`CFBundleVersion`).
    pub bundle_version: Option<String>,
    /// Marketing release version string (`CFBundleShortVersionString`).
    pub bundle_short_version: Option<String>,
    /// Minimum macOS deployment target version (`LSMinimumSystemVersion`).
    pub minimum_os_version: Option<String>,
    /// Main binary executable name (`CFBundleExecutable`).
    pub executable_name: Option<String>,
    /// All top-level key-value string pairs in the dictionary.
    pub entries: HashMap<String, String>,
    /// Complete raw XML text for inspection.
    pub raw_xml: String,
}

/// Stateful UniFFI service managing XML and Office/EPUB/Plist document metadata extraction.
#[derive(uniffi::Object, Default)]
pub struct UniFFIXmlMetaService {}

#[uniffi::export]
impl UniFFIXmlMetaService {
    /// Constructs a new XML metadata service instance.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    /// Extracts Office document metadata from a filesystem path.
    pub fn extract_office_metadata(&self, file_path: String) -> Result<UniFFIDocumentMetadata, TTZipError> {
        uniffi_extract_office_metadata(file_path)
    }

    /// Extracts Office document metadata directly from an in-memory byte buffer.
    pub fn extract_office_metadata_from_bytes(&self, bytes: Vec<u8>) -> Result<UniFFIDocumentMetadata, TTZipError> {
        parse_office_metadata_from_slice(&bytes)
    }

    /// Extracts structural outline from an Office document file path.
    pub fn extract_office_outline(&self, file_path: String) -> Result<UniFFIOfficeOutline, TTZipError> {
        uniffi_extract_office_outline(file_path)
    }

    /// Extracts structural outline from an in-memory Office document byte buffer.
    pub fn extract_office_outline_from_bytes(&self, bytes: Vec<u8>) -> Result<UniFFIOfficeOutline, TTZipError> {
        parse_office_outline_from_slice(&bytes)
    }

    /// Extracts EPUB publication metadata from a filesystem path.
    pub fn extract_epub_metadata(&self, file_path: String) -> Result<UniFFIEpubMetadata, TTZipError> {
        uniffi_extract_epub_metadata(file_path)
    }

    /// Extracts EPUB publication metadata directly from an in-memory byte buffer.
    pub fn extract_epub_metadata_from_bytes(&self, bytes: Vec<u8>) -> Result<UniFFIEpubMetadata, TTZipError> {
        parse_epub_metadata_from_slice(&bytes)
    }

    /// Deserializes an Apple XML Property List string into a structured dictionary.
    pub fn parse_plist_xml(&self, xml_content: String) -> Result<UniFFIPlistDictionary, TTZipError> {
        uniffi_parse_plist_xml(xml_content)
    }

    /// Deserializes an Apple XML Property List byte buffer into a structured dictionary.
    pub fn parse_plist_from_bytes(&self, bytes: Vec<u8>) -> Result<UniFFIPlistDictionary, TTZipError> {
        uniffi_parse_plist_from_bytes(bytes)
    }
}
