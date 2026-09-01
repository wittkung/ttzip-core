// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust PDF Document Outline (Bookmarks / Table of Contents) Extractor.
//!
//! Streams and traverses the `/Outlines` doubly-linked hierarchy tree (`/First`, `/Last`, `/Next`),
//! resolving explicit page destinations, named actions, text styles, and hierarchical nesting
//! with cycle-safe graph protection.

use std::collections::HashSet;

use lopdf::{Dictionary, Object, ObjectId};

use super::parser::TTZipPdfParser;
use super::PdfError;

/// Navigation destination resolved from an outline node or interactive action.
#[derive(Debug, Clone, PartialEq)]
pub enum PdfDestination {
    /// 1-based page target with exact zoom/viewport coordinates (XYZ).
    FitCoordinates {
        page: u32,
        x: Option<f64>,
        y: Option<f64>,
        zoom: Option<f64>,
    },
    /// 1-based page target fitting the whole page or visible box.
    FitPage { page: u32 },
    /// 1-based page target without specific coordinates.
    PageNumber(u32),
    /// Named destination string resolved via `/Names` or `/Dests` dictionary.
    Named(String),
    /// External URL hyperlink target.
    Uri(String),
    /// Unresolved or unsupported destination format.
    Unknown,
}

/// A node in the hierarchical PDF outline (Table of Contents / Bookmarks) tree.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfOutlineNode {
    /// Display title of the bookmark item.
    pub title: String,
    /// 1-based target page number if deterministically resolved.
    pub page_number: Option<u32>,
    /// Full resolved destination parameters.
    pub destination: PdfDestination,
    /// 0-based hierarchical nesting level (0 for top-level, 1 for sub-section, etc.).
    pub level: u32,
    /// Whether this outline branch is expanded by default (`/Count > 0`).
    pub is_open: bool,
    /// Whether the title is formatted in bold text.
    pub is_bold: bool,
    /// Whether the title is formatted in italic text.
    pub is_italic: bool,
    /// RGB display color normalized in [0.0, 1.0] if explicitly configured.
    pub color_rgb: Option<[f32; 3]>,
    /// Child sub-sections nested underneath this node.
    pub children: Vec<PdfOutlineNode>,
}

/// Flattened representation of an outline entry for quick linear rendering and searching.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfFlatOutlineItem {
    pub title: String,
    pub page_number: Option<u32>,
    pub destination: PdfDestination,
    pub level: u32,
    pub is_open: bool,
    pub has_children: bool,
}

/// Pure Safe Rust PDF outline extractor.
pub struct PdfOutlineExtractor;

impl PdfOutlineExtractor {
    /// Maximum allowed outline recursion nesting depth to prevent stack overflow on hostile PDFs.
    const MAX_OUTLINE_DEPTH: u32 = 64;

    /// Extracts the full hierarchical outline tree from the document catalog.
    pub fn extract_outlines(parser: &TTZipPdfParser) -> Result<Vec<PdfOutlineNode>, PdfError> {
        let catalog = parser.catalog()?;
        let outlines_obj = match catalog.get(b"Outlines") {
            Ok(obj) => parser.resolve_reference(obj)?,
            Err(_) => return Ok(Vec::new()),
        };

        let outlines_dict = match outlines_obj {
            Object::Dictionary(dict) => dict,
            _ => return Ok(Vec::new()),
        };

        let mut visited_ids = HashSet::new();
        let mut roots = Vec::new();

        if let Ok(first_obj) = outlines_dict.get(b"First") {
            if let Some(first_id) = Self::resolve_id(first_obj) {
                Self::traverse_sibling_chain(parser, first_id, 0, &mut visited_ids, &mut roots)?;
            }
        }

        Ok(roots)
    }

    /// Converts a tree of `PdfOutlineNode` items into a flat linear list preserving DFS hierarchy order.
    pub fn flatten_outlines(nodes: &[PdfOutlineNode]) -> Vec<PdfFlatOutlineItem> {
        let mut flat = Vec::new();
        for node in nodes {
            Self::flatten_recursive(node, &mut flat);
        }
        flat
    }

    fn flatten_recursive(node: &PdfOutlineNode, out: &mut Vec<PdfFlatOutlineItem>) {
        out.push(PdfFlatOutlineItem {
            title: node.title.clone(),
            page_number: node.page_number,
            destination: node.destination.clone(),
            level: node.level,
            is_open: node.is_open,
            has_children: !node.children.is_empty(),
        });
        for child in &node.children {
            Self::flatten_recursive(child, out);
        }
    }

    fn resolve_id(obj: &Object) -> Option<ObjectId> {
        match obj {
            Object::Reference(id) => Some(*id),
            _ => None,
        }
    }

    /// Traverses a doubly-linked sibling chain along `/Next` pointers.
    fn traverse_sibling_chain(
        parser: &TTZipPdfParser,
        start_id: ObjectId,
        level: u32,
        visited_ids: &mut HashSet<ObjectId>,
        result: &mut Vec<PdfOutlineNode>,
    ) -> Result<(), PdfError> {
        if level > Self::MAX_OUTLINE_DEPTH {
            return Ok(());
        }

        let mut current_id = start_id;

        loop {
            // Anti-cycle protection: avoid revisiting nodes
            if !visited_ids.insert(current_id) {
                break;
            }

            let item_obj = parser.get_object(current_id)?;
            let item_dict = match item_obj {
                Object::Dictionary(dict) => dict,
                _ => break,
            };

            // Extract node properties
            let title = Self::extract_title(parser, item_dict).unwrap_or_else(|| "Untitled".to_string());
            let (destination, page_number) = Self::extract_destination(parser, item_dict);

            let count = item_dict
                .get(b"Count")
                .ok()
                .and_then(|o| parser.resolve_reference(o).ok())
                .and_then(|o| match o {
                    Object::Integer(c) => Some(*c),
                    _ => None,
                })
                .unwrap_or(0);

            let is_open = count > 0;

            let flags = item_dict
                .get(b"F")
                .ok()
                .and_then(|o| parser.resolve_reference(o).ok())
                .and_then(|o| match o {
                    Object::Integer(f) => Some(*f),
                    _ => None,
                })
                .unwrap_or(0);

            let is_italic = (flags & 1) != 0;
            let is_bold = (flags & 2) != 0;

            let color_rgb = Self::extract_color(parser, item_dict);

            // Traverse children linked by /First
            let mut children = Vec::new();
            if let Ok(first_child) = item_dict.get(b"First") {
                if let Some(child_id) = Self::resolve_id(first_child) {
                    Self::traverse_sibling_chain(parser, child_id, level + 1, visited_ids, &mut children)?;
                }
            }

            result.push(PdfOutlineNode {
                title,
                page_number,
                destination,
                level,
                is_open,
                is_bold,
                is_italic,
                color_rgb,
                children,
            });

            // Follow /Next link
            match item_dict.get(b"Next") {
                Ok(next_obj) => match Self::resolve_id(next_obj) {
                    Some(next_id) => current_id = next_id,
                    None => break,
                },
                Err(_) => break,
            }
        }

        Ok(())
    }

    /// Decodes the title string of an outline item.
    fn extract_title(parser: &TTZipPdfParser, dict: &Dictionary) -> Option<String> {
        let title_obj = dict.get(b"Title").ok()?;
        parser.resolve_string(title_obj)
    }

    /// Resolves RGB color array `[r, g, b]` if defined in `/C`.
    fn extract_color(parser: &TTZipPdfParser, dict: &Dictionary) -> Option<[f32; 3]> {
        let c_obj = dict.get(b"C").ok()?;
        let deref = parser.resolve_reference(c_obj).ok()?;
        if let Object::Array(arr) = deref {
            if arr.len() == 3 {
                let mut rgb = [0.0f32; 3];
                for (i, v) in arr.iter().enumerate() {
                    let num_obj = parser.resolve_reference(v).ok()?;
                    rgb[i] = match num_obj {
                        Object::Integer(n) => *n as f32,
                        Object::Real(f) => *f,
                        _ => 0.0,
                    };
                }
                return Some(rgb);
            }
        }
        None
    }

    /// Extracts destination or action target from outline dictionary.
    fn extract_destination(
        parser: &TTZipPdfParser,
        dict: &Dictionary,
    ) -> (PdfDestination, Option<u32>) {
        // Direct /Dest
        if let Ok(dest_obj) = dict.get(b"Dest") {
            if let Ok(deref) = parser.resolve_reference(dest_obj) {
                return Self::resolve_destination_object(parser, deref);
            }
        }

        // Action /A
        if let Ok(a_obj) = dict.get(b"A") {
            if let Ok(Object::Dictionary(a_dict)) = parser.resolve_reference(a_obj) {
                let s_name = a_dict
                    .get(b"S")
                    .ok()
                    .and_then(|s| parser.resolve_reference(s).ok())
                    .and_then(|s| match s {
                        Object::Name(n) => Some(n.as_slice()),
                        _ => None,
                    });

                if s_name == Some(b"GoTo") {
                    if let Ok(d_obj) = a_dict.get(b"D") {
                        if let Ok(d_deref) = parser.resolve_reference(d_obj) {
                            return Self::resolve_destination_object(parser, d_deref);
                        }
                    }
                } else if s_name == Some(b"URI") {
                    if let Ok(uri_obj) = a_dict.get(b"URI") {
                        if let Some(uri_str) = parser.resolve_string(uri_obj) {
                            return (PdfDestination::Uri(uri_str), None);
                        }
                    }
                }
            }
        }

        (PdfDestination::Unknown, None)
    }

    /// Resolves an individual destination object (Array, String, Name).
    fn resolve_destination_object(
        parser: &TTZipPdfParser,
        obj: &Object,
    ) -> (PdfDestination, Option<u32>) {
        match obj {
            Object::Array(arr) if !arr.is_empty() => {
                let target_page = Self::resolve_page_from_dest_first_item(parser, &arr[0]);
                let fit_type = arr
                    .get(1)
                    .and_then(|o| parser.resolve_reference(o).ok())
                    .and_then(|o| match o {
                        Object::Name(n) => Some(n.as_slice()),
                        _ => None,
                    });

                if fit_type == Some(b"XYZ") {
                    let x = arr.get(2).and_then(|o| Self::get_number(parser, o));
                    let y = arr.get(3).and_then(|o| Self::get_number(parser, o));
                    let zoom = arr.get(4).and_then(|o| Self::get_number(parser, o));

                    if let Some(page) = target_page {
                        (
                            PdfDestination::FitCoordinates { page, x, y, zoom },
                            Some(page),
                        )
                    } else {
                        (PdfDestination::Unknown, None)
                    }
                } else if let Some(page) = target_page {
                    (PdfDestination::FitPage { page }, Some(page))
                } else {
                    (PdfDestination::Unknown, None)
                }
            }
            Object::String(bytes, _) => {
                let name = TTZipPdfParser::decode_pdf_string(bytes);
                let page = Self::lookup_named_destination(parser, &name);
                (PdfDestination::Named(name), page)
            }
            Object::Name(bytes) => {
                let name = String::from_utf8_lossy(bytes).to_string();
                let page = Self::lookup_named_destination(parser, &name);
                (PdfDestination::Named(name), page)
            }
            _ => (PdfDestination::Unknown, None),
        }
    }

    /// Resolves the target page number from the first item of a `/Dest` array.
    fn resolve_page_from_dest_first_item(parser: &TTZipPdfParser, item: &Object) -> Option<u32> {
        match item {
            Object::Reference(target_id) => {
                // Find matching page number in parser's page_map
                for (page_num, id) in parser.page_map() {
                    if id == target_id {
                        return Some(*page_num);
                    }
                }
                None
            }
            Object::Integer(page_idx) => {
                // 0-based page index
                let p = (*page_idx as u32) + 1;
                if p <= parser.page_count() {
                    Some(p)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Looks up a named destination in Catalog `/Names -> /Dests` or Catalog `/Dests`.
    fn lookup_named_destination(parser: &TTZipPdfParser, name: &str) -> Option<u32> {
        let catalog = parser.catalog().ok()?;

        // 1. Try Catalog /Dests dictionary
        if let Ok(dests_obj) = catalog.get(b"Dests") {
            if let Ok(Object::Dictionary(dests_dict)) = parser.resolve_reference(dests_obj) {
                if let Ok(dest_entry) = dests_dict.get(name.as_bytes()) {
                    if let Ok(deref) = parser.resolve_reference(dest_entry) {
                        let (_, page) = Self::resolve_destination_object(parser, deref);
                        if page.is_some() {
                            return page;
                        }
                    }
                }
            }
        }

        // 2. Try Catalog /Names -> /Dests name tree (simplified lookup)
        if let Ok(names_obj) = catalog.get(b"Names") {
            if let Ok(Object::Dictionary(names_dict)) = parser.resolve_reference(names_obj) {
                if let Ok(dests_tree_obj) = names_dict.get(b"Dests") {
                    if let Ok(Object::Dictionary(dests_tree)) = parser.resolve_reference(dests_tree_obj) {
                        if let Ok(names_arr_obj) = dests_tree.get(b"Names") {
                            if let Ok(Object::Array(arr)) = parser.resolve_reference(names_arr_obj) {
                                for chunk in arr.chunks_exact(2) {
                                    if let Some(key_str) = parser.resolve_string(&chunk[0]) {
                                        if key_str == name {
                                            if let Ok(deref) = parser.resolve_reference(&chunk[1]) {
                                                let (_, page) = Self::resolve_destination_object(parser, deref);
                                                if page.is_some() {
                                                    return page;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    fn get_number(parser: &TTZipPdfParser, obj: &Object) -> Option<f64> {
        let deref = parser.resolve_reference(obj).ok()?;
        match deref {
            Object::Integer(i) => Some(*i as f64),
            Object::Real(r) => Some((*r).into()),
            _ => None,
        }
    }
}
