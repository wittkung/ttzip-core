// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust PDF Syntax Tree Deconstruction, Outline Extraction, and Text Engine.
//!
//! Provides zero-unsafe PDF parsing, outline hierarchy traversal, CMap-aware
//! UTF-8 text extraction, keyword search with snippet highlighting, and Info/XMP metadata extraction.

pub mod metadata;
pub mod outline;
pub mod parser;
pub mod text;

#[cfg(test)]
mod tests;

pub use metadata::{PdfMetadata, PdfMetadataExtractor, XmpMetadata};
pub use outline::{PdfDestination, PdfFlatOutlineItem, PdfOutlineExtractor, PdfOutlineNode};
pub use parser::{PdfPageInfo, TTZipPdfParser};
pub use text::{
    PdfHighlightSpan, PdfPageText, PdfSearchResult, PdfTextExtractor, PdfTextSearchOptions,
    ToUnicodeCMap,
};

use thiserror::Error;

/// Comprehensive error types encountered during PDF parsing, decoding, and extraction.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PdfError {
    /// PDF syntax or structure is malformed.
    #[error("Invalid PDF structure: {0}")]
    InvalidStructure(String),

    /// Requested indirect object was not found in the XRef table.
    #[error("PDF object ({0}, {1}) not found")]
    ObjectNotFound(u32, u16),

    /// Object type did not match expected dictionary/stream/array/string type.
    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch {
        expected: &'static str,
        found: &'static str,
    },

    /// Document is encrypted with an unsupported or locked security handler.
    #[error("Encrypted PDF document: password required or unsupported cipher")]
    EncryptedDocument,

    /// Failed to decompress or decode stream content (e.g. FlateDecode, ASCIIHexDecode).
    #[error("Stream decode error: {0}")]
    StreamDecodeError(String),

    /// XML/XMP metadata stream could not be parsed.
    #[error("XMP metadata parsing error: {0}")]
    XmlParseError(String),

    /// Page index was out of bounds.
    #[error("Page index {0} out of bounds (total pages: {1})")]
    PageOutOfBounds(u32, u32),

    /// Underlying lopdf library error.
    #[error("Lopdf engine error: {0}")]
    LopdfError(String),

    /// Standard I/O error during file or reader access.
    #[error("I/O error: {0}")]
    IoError(String),
}

impl From<lopdf::Error> for PdfError {
    fn from(err: lopdf::Error) -> Self {
        PdfError::LopdfError(err.to_string())
    }
}

impl From<std::io::Error> for PdfError {
    fn from(err: std::io::Error) -> Self {
        PdfError::IoError(err.to_string())
    }
}
