// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Records, Enums, and Errors for Ebook Metadata and Introspection.

use std::collections::HashMap;

/// Supported ebook and digital publication container formats.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, uniffi::Enum)]
pub enum UniFFIEbookFormat {
    #[default]
    Unknown,
    Epub,
    Cbz,
    Fb2,
    Mobi,
    Azw3,
    Pdf,
}

/// Strongly-typed ebook operation error enum mapped directly to Swift `throws UniFFIEbookError`.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum UniFFIEbookError {
    /// Failure due to malformed or corrupted archive structure.
    #[error("Corrupted ebook archive: {message}")]
    CorruptedFormat { message: String },

    /// Requested chapter, image, stylesheet, or metadata entry was not found.
    #[error("Ebook entry or resource not found: {href}")]
    EntryNotFound { href: String },

    /// Failure during XML / OPF / NCX document parsing.
    #[error("XML parsing error in ebook container: {message}")]
    XmlParseError { message: String },

    /// Format is not supported or recognized.
    #[error("Unsupported ebook format: {format}")]
    UnsupportedFormat { format: String },

    /// File system or stream I/O failure.
    #[error("I/O error during ebook operation: {message}")]
    IoError { message: String },

    /// Operation was cancelled by caller.
    #[error("Ebook operation cancelled")]
    Cancelled,
}

impl UniFFIEbookError {
    pub fn corrupted(msg: impl std::fmt::Display) -> Self {
        Self::CorruptedFormat {
            message: msg.to_string(),
        }
    }

    pub fn not_found(href: impl std::fmt::Display) -> Self {
        Self::EntryNotFound {
            href: href.to_string(),
        }
    }

    pub fn xml_err(msg: impl std::fmt::Display) -> Self {
        Self::XmlParseError {
            message: msg.to_string(),
        }
    }

    pub fn io_err(msg: impl std::fmt::Display) -> Self {
        Self::IoError {
            message: msg.to_string(),
        }
    }
}

/// Comprehensive publication metadata descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIEbookMetadata {
    /// Primary title of the publication.
    pub title: String,
    /// List of authors, creators, or editors.
    pub authors: Vec<String>,
    /// Publishing house or distributor.
    pub publisher: Option<String>,
    /// Primary language code (e.g. "en", "zh-CN", "ja").
    pub language: Option<String>,
    /// Canonical book identifier (e.g. ISBN, UUID, DOI).
    pub identifier: Option<String>,
    /// Synopsis or description text.
    pub description: Option<String>,
    /// Original publication date string.
    pub publication_date: Option<String>,
    /// Modification or package revision date string.
    pub modified_date: Option<String>,
    /// Legal copyright and licensing statement.
    pub rights: Option<String>,
    /// Detected ebook container format.
    pub format: UniFFIEbookFormat,
    /// Total count of linear reading chapters in spine.
    pub total_chapters: u32,
    /// Total count of manifest resources.
    pub total_resources: u32,
    /// Total size of the archive in bytes.
    pub file_size_bytes: u64,
    /// Whether an embedded cover image exists.
    pub has_cover: bool,
    /// Archive relative path to the cover image if present.
    pub cover_path: Option<String>,
    /// Additional unstructured metadata tag pairs.
    pub extra_metadata: HashMap<String, String>,
}

/// Hierarchical Table of Contents (TOC) bookmark node.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIEbookTocNode {
    /// Unique identifier of the TOC node.
    pub id: String,
    /// Human-readable section or chapter title.
    pub title: String,
    /// Normalized relative target resource path (with optional `#anchor`).
    pub href: String,
    /// 1-based sequential playback or reading order.
    pub play_order: u32,
    /// Nesting depth level (0 for top-level root sections).
    pub level: u32,
    /// Nested child sections.
    pub children: Vec<UniFFIEbookTocNode>,
}

/// Sequential item in the publication's reading spine.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIEbookSpineItem {
    /// Manifest ID reference.
    pub id: String,
    /// Normalized relative resource path.
    pub href: String,
    /// MIME type of the chapter resource (e.g. "application/xhtml+xml").
    pub media_type: String,
    /// 1-based sequential spine reading order.
    pub play_order: u32,
    /// Whether this chapter is part of the primary reading flow (linear="yes").
    pub is_linear: bool,
}

/// Extracted XHTML / HTML chapter content descriptor with metrics.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIEbookChapter {
    /// Manifest ID or chapter identifier.
    pub id: String,
    /// Resolved chapter title from TOC, heading, or spine index.
    pub title: String,
    /// Normalized relative resource path.
    pub href: String,
    /// MIME type of the document.
    pub media_type: String,
    /// 1-based sequential spine order.
    pub play_order: u32,
    /// Raw XHTML / HTML markup or text string.
    pub content_string: String,
    /// Total character count of stripped textual content.
    pub character_count: u32,
    /// Total word count of stripped textual content.
    pub word_count: u32,
}

/// Generic embedded asset resource (cover image, stylesheet, font).
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIEbookResource {
    /// Normalized relative path within archive.
    pub href: String,
    /// MIME type (e.g. "image/jpeg", "image/png", "text/css", "font/woff2").
    pub media_type: String,
    /// Raw binary data bytes of the resource.
    pub data: Vec<u8>,
    /// Byte length of the uncompressed data.
    pub size_bytes: u64,
}
