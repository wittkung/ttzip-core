// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Canonical EPUB 2 NCX (`toc.ncx`) and EPUB 3 Navigation Document (`nav.xhtml`) TOC parsers.

use std::collections::HashMap;
use std::path::Path;

use quick_xml::events::{BytesEnd, BytesStart, Event};

use crate::ebook::container::{find_and_extract_zip_entry, opf_parent_dir, strip_utf8_bom};
use crate::ebook::navigation::{local_name, resolve_spine_index, EbookTocNode, SpineItem};
use crate::ebook::resource::{normalize_path, strip_fragment};
use crate::ebook::EbookResult;
use crate::zip::ZipArchive;

/// Parses an EPUB 2 NCX XML (`toc.ncx`) into a structured TOC hierarchy.
pub fn parse_ncx(
    ncx_xml: &[u8],
    base_dir: &str,
    spine_items: &[SpineItem],
) -> EbookResult<Vec<EbookTocNode>> {
    let mut reader = quick_xml::Reader::from_reader(strip_utf8_bom(ncx_xml));
    reader.config_mut().trim_text(true);

    let mut state = NcxParserState::default();
    let mut buf = Vec::with_capacity(512);

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) => state.on_start(e, base_dir, spine_items),
            Event::Empty(ref e) => state.on_empty(e, base_dir, spine_items),
            Event::Text(e) if state.in_text => {
                if let Ok(s) = e.unescape() {
                    state.label_text.push_str(&s);
                }
            }
            Event::CData(e) if state.in_text => {
                if let Ok(s) = std::str::from_utf8(e.as_ref()) {
                    state.label_text.push_str(s);
                }
            }
            Event::End(ref e) => state.on_end(e),
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(state.root_nodes)
}

/// Parses an EPUB 3 Navigation Document (`nav.xhtml`) into a structured TOC hierarchy.
pub fn parse_nav_xhtml(
    nav_xhtml: &[u8],
    base_dir: &str,
    spine_items: &[SpineItem],
) -> EbookResult<Vec<EbookTocNode>> {
    let mut reader = quick_xml::Reader::from_reader(strip_utf8_bom(nav_xhtml));
    reader.config_mut().trim_text(true);

    let mut state = NavXhtmlParserState::default();
    let mut buf = Vec::with_capacity(512);

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) => state.on_start(e),
            Event::Text(e) if state.in_toc_nav && state.in_a => {
                if let Ok(s) = e.unescape() {
                    state.append_text(&s);
                }
            }
            Event::CData(e) if state.in_toc_nav && state.in_a => {
                if let Ok(s) = std::str::from_utf8(e.as_ref()) {
                    state.append_text(s);
                }
            }
            Event::End(ref e) => state.on_end(e, base_dir, spine_items),
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(state.root_nodes)
}

/// Collects a flat lookup table mapping normalized hrefs, relative hrefs, and filenames to TOC titles.
pub fn collect_toc_title_map(
    zip: &ZipArchive<'_>,
    ncx_href: Option<&str>,
    nav_href: Option<&str>,
    opf_dir: &str,
) -> HashMap<String, String> {
    let mut toc_titles = HashMap::new();
    for href in [ncx_href, nav_href].into_iter().flatten() {
        if let Some(bytes) = find_and_extract_zip_entry(zip, href) {
            let base_dir = if href.contains('/') {
                opf_parent_dir(href)
            } else {
                opf_dir
            };
            parse_toc_title_entries(&bytes, base_dir, opf_dir, &mut toc_titles);
        }
    }
    toc_titles
}

/// Populates `toc_titles` from an NCX or NAV XML payload.
pub fn parse_toc_title_entries(
    xml: &[u8],
    base_dir: &str,
    opf_dir: &str,
    toc_titles: &mut HashMap<String, String>,
) {
    if let Ok(nodes) = parse_ncx(xml, base_dir, &[])
        && !nodes.is_empty()
    {
        record_toc_nodes(&nodes, opf_dir, toc_titles);
        return;
    }
    if let Ok(nodes) = parse_nav_xhtml(xml, base_dir, &[])
        && !nodes.is_empty()
    {
        record_toc_nodes(&nodes, opf_dir, toc_titles);
        return;
    }
    parse_loose_anchor_titles(xml, base_dir, opf_dir, toc_titles);
}

fn record_toc_nodes(
    nodes: &[EbookTocNode],
    opf_dir: &str,
    toc_titles: &mut HashMap<String, String>,
) {
    for node in nodes {
        insert_toc_title_entry(&node.href, &node.title, opf_dir, toc_titles);
        record_toc_nodes(&node.children, opf_dir, toc_titles);
    }
}

fn insert_toc_title_entry(
    href: &str,
    title: &str,
    opf_dir: &str,
    toc_titles: &mut HashMap<String, String>,
) {
    let trimmed = title.trim();
    let clean_href = strip_fragment(href);
    if trimmed.is_empty() || clean_href.is_empty() {
        return;
    }
    toc_titles.insert(clean_href.to_string(), trimmed.to_string());
    if !opf_dir.is_empty()
        && let Some(rel) = clean_href.strip_prefix(opf_dir).map(|s| s.trim_start_matches('/'))
        && !rel.is_empty()
    {
        toc_titles.insert(rel.to_string(), trimmed.to_string());
    }
    if let Some(fname) = Path::new(clean_href).file_name().and_then(|os| os.to_str()) {
        toc_titles.insert(fname.to_string(), trimmed.to_string());
    }
}

fn parse_loose_anchor_titles(
    xml: &[u8],
    base_dir: &str,
    opf_dir: &str,
    toc_titles: &mut HashMap<String, String>,
) {
    let mut reader = quick_xml::Reader::from_reader(strip_utf8_bom(xml));
    reader.config_mut().trim_text(true);
    let mut buf = Vec::with_capacity(512);
    let mut current_href = String::new();
    let mut current_text = String::new();
    let mut in_a = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if local_name(e.name().into_inner()).eq_ignore_ascii_case(b"a") => {
                in_a = true;
                current_text.clear();
                current_href = extract_attr_by_name(e, b"href").unwrap_or_default();
            }
            Ok(Event::Text(e)) if in_a => {
                if let Ok(s) = e.unescape() {
                    if !current_text.is_empty() {
                        current_text.push(' ');
                    }
                    current_text.push_str(&s);
                }
            }
            Ok(Event::End(ref e)) if in_a && local_name(e.name().into_inner()).eq_ignore_ascii_case(b"a") => {
                in_a = false;
                let normalized = normalize_path(base_dir, &current_href);
                insert_toc_title_entry(&normalized, &current_text, opf_dir, toc_titles);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
}

#[derive(Default)]
struct NcxParserState {
    root_nodes: Vec<EbookTocNode>,
    stack: Vec<EbookTocNode>,
    in_nav_label: bool,
    in_text: bool,
    label_text: String,
}

impl NcxParserState {
    fn on_start(&mut self, e: &BytesStart<'_>, base_dir: &str, spine_items: &[SpineItem]) {
        let tag = local_name(e.name().into_inner());
        if tag.eq_ignore_ascii_case(b"navPoint") {
            let node_id = extract_attr_by_name(e, b"id").unwrap_or_default();
            self.stack.push(EbookTocNode {
                id: node_id,
                ..EbookTocNode::default()
            });
        } else if tag.eq_ignore_ascii_case(b"navLabel") {
            self.in_nav_label = true;
            self.label_text.clear();
        } else if tag.eq_ignore_ascii_case(b"text") && (self.in_nav_label || !self.stack.is_empty()) {
            self.in_text = true;
        } else if tag.eq_ignore_ascii_case(b"content") {
            self.apply_content_src(e, base_dir, spine_items);
        }
    }

    fn on_empty(&mut self, e: &BytesStart<'_>, base_dir: &str, spine_items: &[SpineItem]) {
        if local_name(e.name().into_inner()).eq_ignore_ascii_case(b"content") {
            self.apply_content_src(e, base_dir, spine_items);
        }
    }

    fn apply_content_src(&mut self, e: &BytesStart<'_>, base_dir: &str, spine_items: &[SpineItem]) {
        let src = extract_attr_by_name(e, b"src").unwrap_or_default();
        if let Some(curr) = self.stack.last_mut() {
            let normalized = normalize_path(base_dir, &src);
            curr.target_index = resolve_spine_index(&normalized, spine_items);
            curr.href = normalized;
        }
    }

    fn on_end(&mut self, e: &BytesEnd<'_>) {
        let tag = local_name(e.name().into_inner());
        if tag.eq_ignore_ascii_case(b"text") {
            self.in_text = false;
            if let Some(curr) = self.stack.last_mut()
                && curr.title.is_empty()
            {
                curr.title = self.label_text.trim().to_string();
            }
        } else if tag.eq_ignore_ascii_case(b"navLabel") {
            self.in_nav_label = false;
            if let Some(curr) = self.stack.last_mut() {
                curr.title = self.label_text.trim().to_string();
            }
        } else if tag.eq_ignore_ascii_case(b"navPoint") {
            pop_node_to_parent(&mut self.stack, &mut self.root_nodes);
        }
    }
}

#[derive(Default)]
struct NavXhtmlParserState {
    root_nodes: Vec<EbookTocNode>,
    stack: Vec<EbookTocNode>,
    in_toc_nav: bool,
    in_a: bool,
    current_a_href: String,
    current_a_id: String,
    text_buf: String,
}

impl NavXhtmlParserState {
    fn on_start(&mut self, e: &BytesStart<'_>) {
        match local_name(e.name().into_inner()) {
            b"nav" => {
                if is_toc_nav_tag(e) || !self.in_toc_nav {
                    self.in_toc_nav = true;
                }
            }
            b"li" if self.in_toc_nav => {
                let node_id = extract_attr_by_name(e, b"id").unwrap_or_default();
                self.stack.push(EbookTocNode {
                    id: node_id,
                    ..EbookTocNode::default()
                });
            }
            b"a" if self.in_toc_nav => {
                self.in_a = true;
                self.text_buf.clear();
                self.current_a_href = extract_attr_by_name(e, b"href").unwrap_or_default();
                self.current_a_id = extract_attr_by_name(e, b"id").unwrap_or_default();
            }
            _ => {}
        }
    }

    fn append_text(&mut self, s: &str) {
        if !self.text_buf.is_empty() {
            self.text_buf.push(' ');
        }
        self.text_buf.push_str(s);
    }

    fn on_end(&mut self, e: &BytesEnd<'_>, base_dir: &str, spine_items: &[SpineItem]) {
        match local_name(e.name().into_inner()) {
            b"a" if self.in_a => {
                self.in_a = false;
                self.finalize_anchor(base_dir, spine_items);
            }
            b"li" if self.in_toc_nav => pop_node_to_parent(&mut self.stack, &mut self.root_nodes),
            b"nav" if self.in_toc_nav => self.in_toc_nav = false,
            _ => {}
        }
    }

    fn finalize_anchor(&mut self, base_dir: &str, spine_items: &[SpineItem]) {
        let Some(curr) = self.stack.last_mut() else {
            return;
        };
        if curr.title.is_empty() {
            curr.title = self.text_buf.split_whitespace().collect::<Vec<_>>().join(" ");
        }
        if curr.href.is_empty() && !self.current_a_href.is_empty() {
            let normalized = normalize_path(base_dir, &self.current_a_href);
            curr.target_index = resolve_spine_index(&normalized, spine_items);
            curr.href = normalized;
        }
        if curr.id.is_empty() && !self.current_a_id.is_empty() {
            curr.id = self.current_a_id.clone();
        }
    }
}

fn extract_attr_by_name(e: &BytesStart<'_>, target_key: &[u8]) -> Option<String> {
    for attr in e.attributes().flatten() {
        if local_name(attr.key.into_inner()).eq_ignore_ascii_case(target_key)
            && let Ok(v) = attr.unescape_value()
        {
            return Some(v.into_owned());
        }
    }
    None
}

fn is_toc_nav_tag(e: &BytesStart<'_>) -> bool {
    for attr in e.attributes().flatten() {
        let k = local_name(attr.key.into_inner());
        let Ok(v) = attr.unescape_value() else {
            continue;
        };
        if (k == b"type" && v.split_whitespace().any(|p| p == "toc"))
            || (k == b"role" && v == "doc-toc")
            || (k == b"id" && v == "toc")
        {
            return true;
        }
    }
    false
}

fn pop_node_to_parent(stack: &mut Vec<EbookTocNode>, root_nodes: &mut Vec<EbookTocNode>) {
    if let Some(node) = stack.pop() {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(node);
        } else {
            root_nodes.push(node);
        }
    }
}
