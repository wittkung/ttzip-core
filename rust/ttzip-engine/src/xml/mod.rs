// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TTZip zero-copy streaming XML tokenization and document metadata extraction engine.
//!
//! Provides ultra-low-allocation event-driven XML parsing, Office Open XML (DOCX/XLSX/PPTX)
//! outline and property extraction, EPUB container and TOC tree traversal, and Apple XML Plist
//! strong AST parsing.

pub mod epub;
pub mod office;
pub mod parser;
pub mod plist;

pub use epub::{
    EpubManifestItem, EpubMetadataExtractor, EpubPackage, EpubPackageMetadata, EpubSpineItem,
    EpubToc, EpubTocNode,
};
pub use office::{
    DocxOutline, DocxOutlineItem, OfficeAppProperties, OfficeCoreProperties, OfficeXmlExtractor,
    PptxSlideOutline, XlsxSheetInfo, XlsxWorkbookMeta,
};
pub use parser::{extract_single_element_text, AdaptiveBufferPool, TTZipXmlParser};
pub use plist::{ApplePlistParser, InfoPlistMeta, PlistValue};

use thiserror::Error;

/// Domain error type representing failures encountered during XML and structured document parsing.
#[derive(Debug, Error)]
pub enum XmlError {
    /// Low-level XML parser tokenization or syntactic failure.
    #[error("XML tokenization error: {0}")]
    QuickXml(#[from] quick_xml::Error),

    /// UTF-8 encoding violation in XML byte payload.
    #[error("UTF-8 decoding failure: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    /// Structural or grammatical violation of document format specifications.
    #[error("Malformed document structure: {0}")]
    Malformed(String),

    /// Expected root XML element was missing or invalid.
    #[error("Missing expected root element: {0}")]
    MissingRoot(String),

    /// Requested child element, property, or attribute was not found.
    #[error("Document entity not found: {0}")]
    NotFound(String),

    /// Invalid or unsupported Apple Plist data type or structure.
    #[error("Invalid Plist specification: {0}")]
    InvalidPlist(String),
}

#[cfg(test)]
mod tests;
