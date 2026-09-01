// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Records and Service Interfaces for PDF Document Introspection.

use std::collections::HashMap;
use std::sync::Arc;

use crate::uniffi_api::types::TTZipError;
use super::parser::{
    extract_all_pages_text_from_slice, extract_pdf_page_text_from_slice,
    parse_pdf_metadata_from_slice, parse_pdf_outline_from_slice, search_pdf_text_from_slice,
};
use super::{
    uniffi_extract_pdf_metadata, uniffi_extract_pdf_outline, uniffi_extract_pdf_page_text,
    uniffi_search_pdf_text,
};

/// Strongly-typed metadata record extracted from a PDF document.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIPdfMetadata {
    /// Format specification version string (e.g. "PDF-1.7", "PDF-2.0").
    pub format_version: String,
    /// Total number of pages in the document.
    pub page_count: u32,
    /// Document title string from Info dictionary or XMP metadata.
    pub title: Option<String>,
    /// Author or primary creator of the document.
    pub author: Option<String>,
    /// Subject matter or description.
    pub subject: Option<String>,
    /// Semicolon or comma-separated keyword tags.
    pub keywords: Option<String>,
    /// Authoring application or tool (e.g. "Adobe InDesign", "LaTeX").
    pub creator: Option<String>,
    /// PDF producer or conversion library (e.g. "Quartz PDFContext", "lopdf").
    pub producer: Option<String>,
    /// Document creation timestamp string (PDF ASN.1 date or ISO 8601).
    pub creation_date: Option<String>,
    /// Document modification timestamp string.
    pub modification_date: Option<String>,
    /// Whether the document requires a password or encryption key to open.
    pub is_encrypted: bool,
    /// Size of the raw PDF file in bytes.
    pub file_size_bytes: u64,
    /// Whether the document contains a hierarchical bookmark or outline tree.
    pub has_outline: bool,
    /// Additional custom key-value pairs parsed from the Info dictionary.
    pub custom_properties: HashMap<String, String>,
}

/// Hierarchical bookmark or outline node in a PDF document outline tree.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIPdfOutlineNode {
    /// Human-readable title label of the outline item.
    pub title: String,
    /// 1-based target page number (1 if not directly linked or unresolved).
    pub page_number: u32,
    /// Optional destination string or action URI (e.g. named target or external link).
    pub dest: Option<String>,
    /// Whether this outline node is initially in an expanded state.
    pub is_expanded: bool,
    /// Nested child outline items under this section heading.
    pub children: Vec<UniFFIPdfOutlineNode>,
}

/// Extracted text content and metric properties for a specific PDF page.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIPdfPageText {
    /// 1-based page number.
    pub page_number: u32,
    /// Extracted plain text content of the page.
    pub text: String,
    /// Total character count of the extracted page text.
    pub character_count: u32,
    /// Total word count of the extracted page text.
    pub word_count: u32,
}

/// Result entry from a full-text search across PDF document content streams.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIPdfSearchResult {
    /// 1-based page number where the match occurred.
    pub page_number: u32,
    /// Contextual snippet containing the matching query and surrounding text.
    pub match_text: String,
    /// 0-based character offset of the match start within the page text.
    pub char_offset: u32,
    /// Length of the matched substring in characters.
    pub match_length: u32,
}

/// Stateful UniFFI service managing PDF document metadata, outline trees, and full-text search.
#[derive(uniffi::Object, Default)]
pub struct UniFFIPdfService {}

#[uniffi::export]
impl UniFFIPdfService {
    /// Constructs a new thread-safe PDF service instance.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    /// Extracts PDF metadata properties from a local filesystem path.
    pub fn extract_metadata(&self, file_path: String) -> Result<UniFFIPdfMetadata, TTZipError> {
        uniffi_extract_pdf_metadata(file_path)
    }

    /// Extracts PDF metadata properties directly from an in-memory byte buffer.
    pub fn extract_metadata_from_bytes(&self, data: Vec<u8>) -> Result<UniFFIPdfMetadata, TTZipError> {
        parse_pdf_metadata_from_slice(&data)
    }

    /// Extracts the complete hierarchical outline bookmark tree from a local filesystem path.
    pub fn extract_outline(&self, file_path: String) -> Result<Vec<UniFFIPdfOutlineNode>, TTZipError> {
        uniffi_extract_pdf_outline(file_path)
    }

    /// Extracts the complete hierarchical outline bookmark tree from an in-memory byte buffer.
    pub fn extract_outline_from_bytes(&self, data: Vec<u8>) -> Result<Vec<UniFFIPdfOutlineNode>, TTZipError> {
        parse_pdf_outline_from_slice(&data)
    }

    /// Extracts plain text from a specific 1-based page number from a local filesystem path.
    pub fn extract_page_text(&self, file_path: String, page_number: u32) -> Result<UniFFIPdfPageText, TTZipError> {
        uniffi_extract_pdf_page_text(file_path, page_number)
    }

    /// Extracts plain text from a specific 1-based page number from an in-memory byte buffer.
    pub fn extract_page_text_from_bytes(&self, data: Vec<u8>, page_number: u32) -> Result<UniFFIPdfPageText, TTZipError> {
        extract_pdf_page_text_from_slice(&data, page_number)
    }

    /// Extracts plain text from all pages (or up to `max_pages`) from a local filesystem path.
    pub fn extract_all_pages_text(&self, file_path: String, max_pages: Option<u32>) -> Result<Vec<UniFFIPdfPageText>, TTZipError> {
        let bytes = super::read_file_bytes(&file_path)?;
        extract_all_pages_text_from_slice(&bytes, max_pages)
    }

    /// Extracts plain text from all pages (or up to `max_pages`) from an in-memory byte buffer.
    pub fn extract_all_pages_text_from_bytes(&self, data: Vec<u8>, max_pages: Option<u32>) -> Result<Vec<UniFFIPdfPageText>, TTZipError> {
        extract_all_pages_text_from_slice(&data, max_pages)
    }

    /// Performs full-text keyword search across all pages of a PDF document from a local filesystem path.
    pub fn search_text(
        &self,
        file_path: String,
        query: String,
        max_results: u32,
        case_sensitive: bool,
    ) -> Result<Vec<UniFFIPdfSearchResult>, TTZipError> {
        uniffi_search_pdf_text(file_path, query, max_results, case_sensitive)
    }

    /// Performs full-text keyword search across all pages of a PDF document from an in-memory byte buffer.
    pub fn search_text_from_bytes(
        &self,
        data: Vec<u8>,
        query: String,
        max_results: u32,
        case_sensitive: bool,
    ) -> Result<Vec<UniFFIPdfSearchResult>, TTZipError> {
        search_pdf_text_from_slice(&data, &query, max_results, case_sensitive)
    }
}
