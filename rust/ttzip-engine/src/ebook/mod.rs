// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust e-book container detection, unpacking, table of contents, and chapter extraction engine.
//!
//! Supports EPUB 2, EPUB 3, MOBI, and AZW3 (KF8) formats with high-throughput streaming parsing,
//! robust XML SAX/DOM traversal, PalmDOC LZ77 decompression, and Dublin Core / EXTH metadata extraction.

pub mod mobi;
pub mod navigation;
pub mod parser;
pub mod resource;

#[cfg(test)]
mod tests;

pub use mobi::{decompress_palmdoc_record, EbookMobiDecoder, MobiExthRecord, MobiHeaderInfo, PalmDocHeader};
pub use navigation::{EbookNavigationExtractor, EbookTocNode, SpineItem};
pub use parser::{EbookFormat, EbookMetadata, TTZipEbookParser};
pub use resource::{EbookResource, EbookResourceExtractor};

use thiserror::Error;

/// Result type alias for e-book engine operations.
pub type EbookResult<T> = Result<T, EbookError>;

/// Domain errors encountered during e-book format detection, parsing, or extraction.
#[derive(Debug, Error)]
pub enum EbookError {
    /// Failure from underlying ZIP archive container.
    #[error("ZIP container error: {0:?}")]
    Zip(#[from] crate::types::TTZipStatus),

    /// XML tokenization or parsing error.
    #[error("XML parsing error: {0}")]
    Xml(#[from] quick_xml::Error),

    /// XML tree parsing error.
    #[error("XML tree parsing error: {0}")]
    XmlTree(#[from] roxmltree::Error),

    /// Structural or syntactic failure in MOBI / PalmDOC parsing.
    #[error("MOBI parsing error: {0}")]
    Mobi(String),

    /// PalmDOC LZ77 decompressor corruption or boundary violation.
    #[error("PalmDOC LZ77 decompression error: {0}")]
    PalmDocDecompress(String),

    /// Unsupported or unrecognized e-book container format.
    #[error("Unrecognized or unsupported e-book format: {0}")]
    UnsupportedFormat(String),

    /// Missing mandatory metadata, manifest item, or navigation document.
    #[error("Required e-book entity not found: {0}")]
    NotFound(String),

    /// Malformed or corrupt container payload.
    #[error("Corrupt e-book container structure: {0}")]
    Corrupt(String),

    /// Character encoding or UTF-8 decoding violation.
    #[error("UTF-8 decoding failure: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    /// Standard I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
