// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! E-book Navigation Extractor for EPUB 2 NCX and EPUB 3 Navigation Documents.
//!
//! Provides unified Table of Contents (TOC) hierarchy tree construction and linear reading
//! spine item resolution.

use crate::ebook::EbookResult;

/// An item in the sequential reading order (spine) of an e-book.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpineItem {
    /// Identifier referenced in the OPF package.
    pub idref: String,
    /// Normalized container relative path to the resource file.
    pub href: String,
    /// Whether this item is part of the primary linear reading flow.
    pub linear: bool,
    /// MIME media type of the resource (e.g., `application/xhtml+xml`).
    pub media_type: String,
}

/// A node in the hierarchical Table of Contents (TOC) tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EbookTocNode {
    /// Unique identifier of the navigation node.
    pub id: String,
    /// Display title or chapter headline.
    pub title: String,
    /// Target resource path or URI fragment (e.g., `chapter1.xhtml#section2`).
    pub href: String,
    /// Zero-based target index in the `SpineItem` reading queue, if resolved.
    pub target_index: Option<usize>,
    /// Child navigation sub-nodes.
    pub children: Vec<EbookTocNode>,
}

/// Extractor for navigation documents, NCX trees, and spine mappings.
pub struct EbookNavigationExtractor;

impl EbookNavigationExtractor {
    /// Parses an EPUB 2 NCX XML (`toc.ncx`) into a structured TOC hierarchy.
    pub fn parse_ncx(
        ncx_xml: &[u8],
        base_dir: &str,
        spine_items: &[SpineItem],
    ) -> EbookResult<Vec<EbookTocNode>> {
        crate::ebook::ncx::parse_ncx(ncx_xml, base_dir, spine_items)
    }

    /// Parses an EPUB 3 Navigation Document (`nav.xhtml`) into a structured TOC hierarchy.
    pub fn parse_nav_xhtml(
        nav_xhtml: &[u8],
        base_dir: &str,
        spine_items: &[SpineItem],
    ) -> EbookResult<Vec<EbookTocNode>> {
        crate::ebook::ncx::parse_nav_xhtml(nav_xhtml, base_dir, spine_items)
    }
}

/// Resolves the index of a TOC href inside the spine item collection.
pub fn resolve_spine_index(href: &str, spine_items: &[SpineItem]) -> Option<usize> {
    let clean_href = match href.split_once('#') {
        Some((path, _)) => path,
        None => href,
    };

    spine_items
        .iter()
        .position(|item| item.href == clean_href || item.href == href)
}

/// Extracts local XML tag or attribute name stripping namespace prefixes.
#[inline]
pub(crate) fn local_name(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&b| b == b':') {
        Some(pos) => &name[pos + 1..],
        None => name,
    }
}
