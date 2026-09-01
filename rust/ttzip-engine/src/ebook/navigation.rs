// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! E-book Navigation Extractor for EPUB 2 NCX and EPUB 3 Navigation Documents.
//!
//! Provides unified Table of Contents (TOC) hierarchy tree construction and linear reading
//! spine item resolution.

use quick_xml::events::Event;
use crate::ebook::resource::normalize_path;
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
        let mut reader = quick_xml::Reader::from_reader(ncx_xml);
        reader.config_mut().trim_text(true);

        let mut root_nodes: Vec<EbookTocNode> = Vec::new();
        let mut stack: Vec<EbookTocNode> = Vec::new();
        let mut buf = Vec::with_capacity(512);

        let mut in_nav_label = false;
        let mut in_text = false;
        let mut label_text = String::with_capacity(64);

        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let local = local_name(e.name().into_inner());
                    match local {
                        b"navPoint" => {
                            let mut node_id = String::new();
                            for attr in e.attributes().flatten() {
                                if local_name(attr.key.into_inner()) == b"id" {
                                    if let Ok(v) = attr.unescape_value() {
                                        node_id = v.to_string();
                                    }
                                }
                            }
                            stack.push(EbookTocNode {
                                id: node_id,
                                title: String::new(),
                                href: String::new(),
                                target_index: None,
                                children: Vec::new(),
                            });
                        }
                        b"navLabel" => {
                            in_nav_label = true;
                            label_text.clear();
                        }
                        b"text" if in_nav_label => {
                            in_text = true;
                        }
                        _ => {}
                    }
                }
                Event::Empty(ref e) => {
                    let local = local_name(e.name().into_inner());
                    if local == b"content" {
                        let mut src = String::new();
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.into_inner()) == b"src" {
                                if let Ok(v) = attr.unescape_value() {
                                    src = v.to_string();
                                }
                            }
                        }
                        if let Some(curr) = stack.last_mut() {
                            let normalized = normalize_path(base_dir, &src);
                            curr.href = normalized.clone();
                            curr.target_index = resolve_spine_index(&normalized, spine_items);
                        }
                    }
                }
                Event::Text(ref e) if in_text => {
                    if let Ok(s) = e.unescape() {
                        label_text.push_str(&s);
                    }
                }
                Event::CData(ref e) if in_text => {
                    if let Ok(s) = std::str::from_utf8(e.as_ref()) {
                        label_text.push_str(s);
                    }
                }
                Event::End(ref e) => {
                    let local = local_name(e.name().into_inner());
                    match local {
                        b"text" => in_text = false,
                        b"navLabel" => {
                            in_nav_label = false;
                            if let Some(curr) = stack.last_mut() {
                                curr.title = label_text.trim().to_string();
                            }
                        }
                        b"navPoint" => {
                            if let Some(node) = stack.pop() {
                                if let Some(parent) = stack.last_mut() {
                                    parent.children.push(node);
                                } else {
                                    root_nodes.push(node);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(root_nodes)
    }

    /// Parses an EPUB 3 Navigation Document (`nav.xhtml`) into a structured TOC hierarchy.
    pub fn parse_nav_xhtml(
        nav_xhtml: &[u8],
        base_dir: &str,
        spine_items: &[SpineItem],
    ) -> EbookResult<Vec<EbookTocNode>> {
        let mut reader = quick_xml::Reader::from_reader(nav_xhtml);
        reader.config_mut().trim_text(true);

        let mut root_nodes: Vec<EbookTocNode> = Vec::new();
        let mut stack: Vec<EbookTocNode> = Vec::new();
        let mut buf = Vec::with_capacity(512);

        let mut in_toc_nav = false;
        let mut in_a = false;
        let mut current_a_href = String::new();
        let mut current_a_id = String::new();
        let mut text_buf = String::with_capacity(64);

        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let local = local_name(e.name().into_inner());
                    match local {
                        b"nav" => {
                            let mut is_toc = false;
                            for attr in e.attributes().flatten() {
                                let k = local_name(attr.key.into_inner());
                                if let Ok(v) = attr.unescape_value() {
                                    if (k == b"type" && v.split_whitespace().any(|p| p == "toc"))
                                        || (k == b"role" && v == "doc-toc")
                                        || (k == b"id" && v == "toc")
                                    {
                                        is_toc = true;
                                    }
                                }
                            }
                            if is_toc || !in_toc_nav {
                                in_toc_nav = true;
                            }
                        }
                        b"li" if in_toc_nav => {
                            let mut node_id = String::new();
                            for attr in e.attributes().flatten() {
                                if local_name(attr.key.into_inner()) == b"id" {
                                    if let Ok(v) = attr.unescape_value() {
                                        node_id = v.to_string();
                                    }
                                }
                            }
                            stack.push(EbookTocNode {
                                id: node_id,
                                title: String::new(),
                                href: String::new(),
                                target_index: None,
                                children: Vec::new(),
                            });
                        }
                        b"a" if in_toc_nav => {
                            in_a = true;
                            text_buf.clear();
                            current_a_href.clear();
                            current_a_id.clear();
                            for attr in e.attributes().flatten() {
                                let k = local_name(attr.key.into_inner());
                                if let Ok(v) = attr.unescape_value() {
                                    if k == b"href" {
                                        current_a_href = v.to_string();
                                    } else if k == b"id" {
                                        current_a_id = v.to_string();
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Event::Text(ref e) if in_toc_nav && in_a => {
                    if let Ok(s) = e.unescape() {
                        if !text_buf.is_empty() {
                            text_buf.push(' ');
                        }
                        text_buf.push_str(&s);
                    }
                }
                Event::CData(ref e) if in_toc_nav && in_a => {
                    if let Ok(s) = std::str::from_utf8(e.as_ref()) {
                        if !text_buf.is_empty() {
                            text_buf.push(' ');
                        }
                        text_buf.push_str(s);
                    }
                }
                Event::End(ref e) => {
                    let local = local_name(e.name().into_inner());
                    match local {
                        b"a" if in_a => {
                            in_a = false;
                            if let Some(curr) = stack.last_mut() {
                                if curr.title.is_empty() {
                                    curr.title = text_buf.split_whitespace().collect::<Vec<_>>().join(" ");
                                }
                                if curr.href.is_empty() && !current_a_href.is_empty() {
                                    let normalized = normalize_path(base_dir, &current_a_href);
                                    curr.target_index = resolve_spine_index(&normalized, spine_items);
                                    curr.href = normalized;
                                }
                                if curr.id.is_empty() && !current_a_id.is_empty() {
                                    curr.id = current_a_id.clone();
                                }
                            }
                        }
                        b"li" if in_toc_nav => {
                            if let Some(node) = stack.pop() {
                                if let Some(parent) = stack.last_mut() {
                                    parent.children.push(node);
                                } else {
                                    root_nodes.push(node);
                                }
                            }
                        }
                        b"nav" if in_toc_nav => {
                            in_toc_nav = false;
                        }
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(root_nodes)
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
