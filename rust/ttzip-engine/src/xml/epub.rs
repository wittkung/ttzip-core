// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! EPUB e-book streaming container resolution, OPF package parsing, TOC trees, and cover extraction.
//!
//! Provides streaming SAX parsers for `META-INF/container.xml`, `content.opf` (metadata,
//! manifest, spine), EPUB 2 NCX (`toc.ncx`), and EPUB 3 navigation documents (`nav.xhtml`).

use std::collections::HashMap;
use quick_xml::events::Event;

use super::parser::TTZipXmlParser;
use super::XmlError;

/// Dublin Core metadata and publication details extracted from `content.opf`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpubPackageMetadata {
    pub title: Option<String>,
    pub creators: Vec<String>,
    pub contributors: Vec<String>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub identifier: Option<String>,
    pub description: Option<String>,
    pub publication_date: Option<String>,
    pub modified_date: Option<String>,
    pub rights: Option<String>,
    pub subjects: Vec<String>,
}

/// Type alias for EPUB package metadata.
pub type EpubMetadata = EpubPackageMetadata;

/// An individual item in the EPUB OPF manifest table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpubManifestItem {
    pub id: String,
    pub href: String,
    pub media_type: String,
    pub properties: Option<String>,
}

/// An entry in the EPUB reading order (spine).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpubSpineItem {
    pub idref: String,
    pub linear: bool,
}

/// A node in the hierarchical Table of Contents tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpubTocNode {
    pub title: String,
    pub href: String,
    pub play_order: u32,
    pub children: Vec<EpubTocNode>,
}

/// Complete Table of Contents tree structure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpubToc {
    pub nodes: Vec<EpubTocNode>,
}

/// Full package manifest and reading order extracted from `content.opf`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpubPackage {
    pub metadata: EpubMetadata,
    pub manifest: HashMap<String, EpubManifestItem>,
    pub spine: Vec<EpubSpineItem>,
    pub cover_image_href: Option<String>,
    pub toc_ncx_href: Option<String>,
    pub nav_xhtml_href: Option<String>,
}

/// Extractor for EPUB 2 and EPUB 3 electronic book metadata and structure.
pub struct EpubMetadataExtractor;

impl EpubMetadataExtractor {
    /// Resolves the root OPF file path from `META-INF/container.xml`.
    pub fn parse_container_xml(container_xml: &[u8]) -> Result<String, XmlError> {
        let mut parser = TTZipXmlParser::from_slice(container_xml);
        let mut buf = Vec::with_capacity(512);

        loop {
            match parser.read_event_into(&mut buf)? {
                Event::Start(ref e) | Event::Empty(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    if local == b"rootfile" {
                        if let Some(path) = TTZipXmlParser::get_attribute(e, b"full-path") {
                            if !path.trim().is_empty() {
                                return Ok(path.into_owned());
                            }
                        }
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Err(XmlError::NotFound(
            "Missing <rootfile full-path=...> in META-INF/container.xml".to_string(),
        ))
    }

    /// Parses `content.opf` extracting metadata, manifest entries, spine order, and TOC/cover routes.
    pub fn parse_opf(opf_xml: &[u8]) -> Result<EpubPackage, XmlError> {
        let mut parser = TTZipXmlParser::from_slice(opf_xml);
        let mut pkg = EpubPackage {
            manifest: HashMap::with_capacity(128),
            spine: Vec::with_capacity(128),
            ..Default::default()
        };
        let mut buf = Vec::with_capacity(512);

        let mut in_metadata = false;
        let mut in_manifest = false;
        let mut in_spine = false;

        let mut cover_meta_id: Option<String> = None;
        let mut spine_toc_id: Option<String> = None;

        loop {
            match parser.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"metadata" => in_metadata = true,
                        b"manifest" => in_manifest = true,
                        b"spine" => {
                            in_spine = true;
                            if let Some(toc) = TTZipXmlParser::get_attribute(e, b"toc") {
                                spine_toc_id = Some(toc.into_owned());
                            }
                        }
                        b"item" if in_manifest => {
                            let mut id = String::new();
                            let mut href = String::new();
                            let mut media_type = String::new();
                            let mut properties = None;
                            for attr in e.attributes().flatten() {
                                match TTZipXmlParser::local_name(attr.key) {
                                    b"id" => id = String::from_utf8_lossy(&attr.value).into_owned(),
                                    b"href" => href = String::from_utf8_lossy(&attr.value).into_owned(),
                                    b"media-type" => media_type = String::from_utf8_lossy(&attr.value).into_owned(),
                                    b"properties" => properties = Some(String::from_utf8_lossy(&attr.value).into_owned()),
                                    _ => {}
                                }
                            }
                            if !id.is_empty() {
                                pkg.manifest.insert(id.clone(), EpubManifestItem { id, href, media_type, properties });
                            }
                        }
                        b"itemref" if in_spine => {
                            let mut idref = String::new();
                            let mut linear = true;
                            for attr in e.attributes().flatten() {
                                match TTZipXmlParser::local_name(attr.key) {
                                    b"idref" => idref = String::from_utf8_lossy(&attr.value).into_owned(),
                                    b"linear" => linear = attr.value.as_ref() != b"no",
                                    _ => {}
                                }
                            }
                            if !idref.is_empty() {
                                pkg.spine.push(EpubSpineItem { idref, linear });
                            }
                        }
                        b"title" if in_metadata => {
                            pkg.metadata.title = Some(parser.read_element_text(b"title")?);
                        }
                        b"creator" if in_metadata => {
                            let creator = parser.read_element_text(b"creator")?;
                            if !creator.trim().is_empty() {
                                pkg.metadata.creators.push(creator.trim().to_string());
                            }
                        }
                        b"contributor" if in_metadata => {
                            let contrib = parser.read_element_text(b"contributor")?;
                            if !contrib.trim().is_empty() {
                                pkg.metadata.contributors.push(contrib.trim().to_string());
                            }
                        }
                        b"publisher" if in_metadata => {
                            pkg.metadata.publisher =
                                Some(parser.read_element_text(b"publisher")?);
                        }
                        b"language" if in_metadata => {
                            pkg.metadata.language = Some(parser.read_element_text(b"language")?);
                        }
                        b"identifier" if in_metadata => {
                            let id_text = parser.read_element_text(b"identifier")?;
                            if pkg.metadata.identifier.is_none() && !id_text.trim().is_empty() {
                                pkg.metadata.identifier = Some(id_text.trim().to_string());
                            }
                        }
                        b"description" if in_metadata => {
                            pkg.metadata.description =
                                Some(parser.read_element_text(b"description")?);
                        }
                        b"date" if in_metadata => {
                            pkg.metadata.publication_date =
                                Some(parser.read_element_text(b"date")?);
                        }
                        b"rights" if in_metadata => {
                            pkg.metadata.rights = Some(parser.read_element_text(b"rights")?);
                        }
                        b"subject" if in_metadata => {
                            let subj = parser.read_element_text(b"subject")?;
                            if !subj.trim().is_empty() {
                                pkg.metadata.subjects.push(subj.trim().to_string());
                            }
                        }
                        b"meta" if in_metadata => {
                            if let Some(name) = TTZipXmlParser::get_attribute(e, b"name") {
                                if name == "cover" {
                                    if let Some(content) =
                                        TTZipXmlParser::get_attribute(e, b"content")
                                    {
                                        cover_meta_id = Some(content.into_owned());
                                    }
                                }
                            }
                            if let Some(prop) = TTZipXmlParser::get_attribute(e, b"property") {
                                if prop == "dcterms:modified" {
                                    pkg.metadata.modified_date =
                                        Some(parser.read_element_text(b"meta")?);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Event::Empty(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    if in_manifest && local == b"item" {
                        let mut id = String::new();
                        let mut href = String::new();
                        let mut media_type = String::new();
                        let mut properties = None;
                        for attr in e.attributes().flatten() {
                            match TTZipXmlParser::local_name(attr.key) {
                                b"id" => id = String::from_utf8_lossy(&attr.value).into_owned(),
                                b"href" => href = String::from_utf8_lossy(&attr.value).into_owned(),
                                b"media-type" => media_type = String::from_utf8_lossy(&attr.value).into_owned(),
                                b"properties" => properties = Some(String::from_utf8_lossy(&attr.value).into_owned()),
                                _ => {}
                            }
                        }

                        if !id.is_empty() {
                            let item = EpubManifestItem {
                                id: id.clone(),
                                href,
                                media_type,
                                properties,
                            };
                            pkg.manifest.insert(id, item);
                        }
                    } else if in_spine && local == b"itemref" {
                        let mut idref = String::new();
                        let mut linear = true;
                        for attr in e.attributes().flatten() {
                            match TTZipXmlParser::local_name(attr.key) {
                                b"idref" => idref = String::from_utf8_lossy(&attr.value).into_owned(),
                                b"linear" => linear = attr.value.as_ref() != b"no",
                                _ => {}
                            }
                        }

                        if !idref.is_empty() {
                            pkg.spine.push(EpubSpineItem { idref, linear });
                        }
                    } else if in_metadata && local == b"meta" {
                        if let Some(name) = TTZipXmlParser::get_attribute(e, b"name") {
                            if name == "cover" {
                                if let Some(content) = TTZipXmlParser::get_attribute(e, b"content")
                                {
                                    cover_meta_id = Some(content.into_owned());
                                }
                            }
                        }
                    }
                }
                Event::End(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"metadata" => in_metadata = false,
                        b"manifest" => in_manifest = false,
                        b"spine" => in_spine = false,
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        // Resolve cover image href
        if let Some(cover_id) = cover_meta_id {
            if let Some(item) = pkg.manifest.get(&cover_id) {
                pkg.cover_image_href = Some(item.href.clone());
            }
        }
        if pkg.cover_image_href.is_none() {
            for item in pkg.manifest.values() {
                if let Some(props) = &item.properties {
                    if props.split_whitespace().any(|p| p == "cover-image") {
                        pkg.cover_image_href = Some(item.href.clone());
                        break;
                    }
                }
            }
        }

        // Resolve TOC NCX href
        if let Some(toc_id) = spine_toc_id {
            if let Some(item) = pkg.manifest.get(&toc_id) {
                pkg.toc_ncx_href = Some(item.href.clone());
            }
        }
        if pkg.toc_ncx_href.is_none() {
            for item in pkg.manifest.values() {
                if item.media_type == "application/x-dtbncx+xml" {
                    pkg.toc_ncx_href = Some(item.href.clone());
                    break;
                }
            }
        }

        // Resolve Nav XHTML href
        for item in pkg.manifest.values() {
            if let Some(props) = &item.properties {
                if props.split_whitespace().any(|p| p == "nav") {
                    pkg.nav_xhtml_href = Some(item.href.clone());
                    break;
                }
            }
        }

        Ok(pkg)
    }

    /// Parses EPUB 2 Table of Contents from `toc.ncx`.
    pub fn parse_ncx_toc(ncx_xml: &[u8]) -> Result<EpubToc, XmlError> {
        let mut parser = TTZipXmlParser::from_slice(ncx_xml);
        let mut root_nodes: Vec<EpubTocNode> = Vec::new();
        let mut stack: Vec<EpubTocNode> = Vec::new();
        let mut buf = Vec::with_capacity(512);

        let mut in_nav_label = false;
        let mut in_text = false;
        let mut label_text = String::with_capacity(64);

        loop {
            match parser.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"navPoint" => {
                            let play_order = TTZipXmlParser::get_attribute(e, b"playOrder")
                                .and_then(|s| s.trim().parse::<u32>().ok())
                                .unwrap_or(0);
                            stack.push(EpubTocNode {
                                title: String::new(),
                                href: String::new(),
                                play_order,
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
                    let local = TTZipXmlParser::local_name(e.name());
                    if local == b"content" {
                        if let Some(src) = TTZipXmlParser::get_attribute(e, b"src") {
                            if let Some(current) = stack.last_mut() {
                                current.href = src.into_owned();
                            }
                        }
                    }
                }
                Event::Text(ref e) if in_text => {
                    let text = TTZipXmlParser::decode_text(e)?;
                    label_text.push_str(&text);
                }
                Event::End(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"text" => in_text = false,
                        b"navLabel" => {
                            in_nav_label = false;
                            if let Some(current) = stack.last_mut() {
                                current.title = label_text.trim().to_string();
                            }
                        }
                        b"navPoint" => {
                            if let Some(completed_node) = stack.pop() {
                                if let Some(parent) = stack.last_mut() {
                                    parent.children.push(completed_node);
                                } else {
                                    root_nodes.push(completed_node);
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

        Ok(EpubToc { nodes: root_nodes })
    }

    /// Parses EPUB 3 Navigation Document (`nav.xhtml`) Table of Contents.
    pub fn parse_nav_xhtml(nav_xhtml: &[u8]) -> Result<EpubToc, XmlError> {
        let mut parser = TTZipXmlParser::from_slice(nav_xhtml);
        let mut root_nodes: Vec<EpubTocNode> = Vec::new();
        let mut stack: Vec<EpubTocNode> = Vec::new();
        let mut buf = Vec::with_capacity(512);

        let mut in_toc_nav = false;
        let mut in_a = false;
        let mut link_text = String::with_capacity(64);
        let mut link_href = String::new();
        let mut play_order_counter = 1u32;

        loop {
            match parser.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"nav" => {
                            if let Some(epub_type) = TTZipXmlParser::get_attribute(e, b"epub:type")
                            {
                                if epub_type == "toc" {
                                    in_toc_nav = true;
                                }
                            } else if let Some(role) = TTZipXmlParser::get_attribute(e, b"role") {
                                if role == "doc-toc" {
                                    in_toc_nav = true;
                                }
                            } else if let Some(id) = TTZipXmlParser::get_attribute(e, b"id") {
                                if id == "toc" {
                                    in_toc_nav = true;
                                }
                            }
                        }
                        b"li" if in_toc_nav => {
                            let order = play_order_counter;
                            play_order_counter = play_order_counter.saturating_add(1);
                            stack.push(EpubTocNode {
                                title: String::new(),
                                href: String::new(),
                                play_order: order,
                                children: Vec::new(),
                            });
                        }
                        b"a" if in_toc_nav => {
                            in_a = true;
                            link_text.clear();
                            link_href = TTZipXmlParser::get_attribute(e, b"href")
                                .map(|s| s.into_owned())
                                .unwrap_or_default();
                        }
                        _ => {}
                    }
                }
                Event::Text(ref e) if in_a => {
                    let text = TTZipXmlParser::decode_text(e)?;
                    link_text.push_str(&text);
                }
                Event::End(ref e) => {
                    let local = TTZipXmlParser::local_name(e.name());
                    match local {
                        b"a" => {
                            in_a = false;
                            if let Some(current) = stack.last_mut() {
                                current.title = link_text.trim().to_string();
                                current.href = link_href.clone();
                            }
                        }
                        b"li" if in_toc_nav => {
                            if let Some(completed_node) = stack.pop() {
                                if let Some(parent) = stack.last_mut() {
                                    parent.children.push(completed_node);
                                } else {
                                    root_nodes.push(completed_node);
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

        Ok(EpubToc { nodes: root_nodes })
    }
}
