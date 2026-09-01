// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! EPUB Standard Publication Container Resolution, OPF Manifest Parsing,
//! NCX/NAV Hierarchical TOC Extraction, Spine Ordering, and Chapter Reading.

use std::collections::HashMap;
use std::path::Path;

use super::helpers::{
    extract_html_heading, find_and_extract_entry, extract_resource_from_zip, resolve_path,
    strip_html_tags,
};
use super::types::{
    UniFFIEbookChapter, UniFFIEbookError, UniFFIEbookFormat, UniFFIEbookMetadata,
    UniFFIEbookResource, UniFFIEbookSpineItem, UniFFIEbookTocNode,
};
use crate::zip::reader::ZipArchive;

#[derive(Debug, Clone)]
pub(crate) struct ManifestItem {
    pub href: String,
    pub media_type: String,
    pub _properties: String,
}

pub(crate) fn open_epub_zip(
    data: &[u8],
) -> Result<(ZipArchive<'_>, String, String), UniFFIEbookError> {
    let zip = ZipArchive::open_slice(data)
        .map_err(|e| UniFFIEbookError::corrupted(format!("{e:?}")))?;

    let opf_path = resolve_opf_path(&zip)?;
    let opf_bytes = find_and_extract_entry(&zip, &opf_path)
        .ok_or_else(|| UniFFIEbookError::not_found(&opf_path))?;
    let opf_xml = std::str::from_utf8(&opf_bytes)
        .map_err(|e| UniFFIEbookError::corrupted(e.to_string()))?
        .to_string();

    Ok((zip, opf_path, opf_xml))
}

pub(crate) fn resolve_opf_path(zip: &ZipArchive<'_>) -> Result<String, UniFFIEbookError> {
    if let Some(bytes) = find_and_extract_entry(zip, "META-INF/container.xml") {
        if let Ok(s) = std::str::from_utf8(&bytes) {
            if let Ok(doc) = roxmltree::Document::parse(s) {
                if let Some(rf) = doc.descendants().find(|n| n.tag_name().name() == "rootfile") {
                    if let Some(p) = rf.attribute("full-path") {
                        return Ok(p.to_string());
                    }
                }
            }
        }
    }
    zip.entries()
        .iter()
        .find(|e| e.rel_path.to_lowercase().ends_with(".opf"))
        .map(|e| e.rel_path.clone())
        .ok_or_else(|| UniFFIEbookError::not_found("META-INF/container.xml or *.opf"))
}

pub(crate) fn parse_epub_manifest(
    doc: &roxmltree::Document<'_>,
) -> (
    HashMap<String, ManifestItem>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let mut manifest = HashMap::new();
    let mut ncx = None;
    let mut nav = None;
    let mut cover_item_id = None;

    if let Some(node) = doc.descendants().find(|n| n.tag_name().name() == "manifest") {
        for item in node.children().filter(|n| n.tag_name().name() == "item") {
            if let (Some(id), Some(href)) = (item.attribute("id"), item.attribute("href")) {
                let media_type = item.attribute("media-type").unwrap_or("").to_string();
                let properties = item.attribute("properties").unwrap_or("").to_string();

                if media_type == "application/x-dtbncx+xml" || href.to_lowercase().ends_with(".ncx")
                {
                    ncx = Some(href.to_string());
                }
                if properties.contains("nav") {
                    nav = Some(href.to_string());
                }
                if properties.contains("cover-image") || id == "cover" || id == "cover-image" {
                    cover_item_id = Some(id.to_string());
                }

                manifest.insert(
                    id.to_string(),
                    ManifestItem {
                        href: href.to_string(),
                        media_type,
                        _properties: properties,
                    },
                );
            }
        }
    }

    (manifest, ncx, nav, cover_item_id)
}

pub(crate) fn parse_epub_metadata(
    data: &[u8],
    file_name: Option<&str>,
) -> Result<UniFFIEbookMetadata, UniFFIEbookError> {
    let (zip, opf_path, opf_xml) = open_epub_zip(data)?;
    let opf_dir = Path::new(&opf_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    let doc = roxmltree::Document::parse(&opf_xml)
        .map_err(|e| UniFFIEbookError::xml_err(e.to_string()))?;

    let (manifest, _, _, mut cover_item_id) = parse_epub_manifest(&doc);

    let mut title = String::new();
    let mut authors = Vec::new();
    let mut publisher = None;
    let mut language = None;
    let mut identifier = None;
    let mut description = None;
    let mut publication_date = None;
    let mut modified_date = None;
    let mut rights = None;
    let mut extra_metadata = HashMap::new();

    if let Some(node) = doc.descendants().find(|n| n.tag_name().name() == "metadata") {
        for c in node.children().filter(|n| n.is_element()) {
            let tag = c.tag_name().name();
            let text = c
                .text()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            match tag {
                "title" => {
                    if let Some(t) = text {
                        if title.is_empty() {
                            title = t;
                        }
                    }
                }
                "creator" => {
                    if let Some(a) = text {
                        authors.push(a);
                    }
                }
                "publisher" => publisher = text,
                "language" => language = text,
                "identifier" => {
                    if identifier.is_none() {
                        identifier = text;
                    }
                }
                "description" => description = text,
                "date" => publication_date = text,
                "rights" => rights = text,
                "meta" => {
                    if c.attribute("name") == Some("cover") {
                        if let Some(cid) = c.attribute("content") {
                            cover_item_id = Some(cid.to_string());
                        }
                    }
                    if c.attribute("property") == Some("dcterms:modified") {
                        modified_date = text.clone();
                    }
                    if let (Some(name), Some(content)) =
                        (c.attribute("name"), c.attribute("content"))
                    {
                        extra_metadata.insert(name.to_string(), content.to_string());
                    }
                }
                _ => {
                    if let Some(val) = text {
                        extra_metadata.insert(tag.to_string(), val);
                    }
                }
            }
        }
    }

    if title.is_empty() {
        title = file_name
            .map(|f| {
                Path::new(f)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Untitled Book")
            })
            .unwrap_or("Untitled Book")
            .to_string();
    }

    let spine_count = doc
        .descendants()
        .find(|n| n.tag_name().name() == "spine")
        .map(|s| {
            s.children()
                .filter(|n| n.tag_name().name() == "itemref")
                .count() as u32
        })
        .unwrap_or(0);

    let cover_path = cover_item_id
        .and_then(|cid| manifest.get(&cid))
        .map(|item| resolve_path(opf_dir, &item.href));

    let has_cover = cover_path
        .as_ref()
        .map(|p| find_and_extract_entry(&zip, p).is_some())
        .unwrap_or(false);

    Ok(UniFFIEbookMetadata {
        title,
        authors,
        publisher,
        language,
        identifier,
        description,
        publication_date,
        modified_date,
        rights,
        format: UniFFIEbookFormat::Epub,
        total_chapters: spine_count,
        total_resources: manifest.len() as u32,
        file_size_bytes: data.len() as u64,
        has_cover,
        cover_path,
        extra_metadata,
    })
}

pub(crate) fn parse_epub_toc(data: &[u8]) -> Result<Vec<UniFFIEbookTocNode>, UniFFIEbookError> {
    let (zip, opf_path, opf_xml) = open_epub_zip(data)?;
    let opf_dir = Path::new(&opf_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    let doc = roxmltree::Document::parse(&opf_xml)
        .map_err(|e| UniFFIEbookError::xml_err(e.to_string()))?;

    let (_, ncx_href, nav_href, _) = parse_epub_manifest(&doc);

    // 1. Try NCX TOC
    if let Some(ncx_rel) = ncx_href {
        let full = resolve_path(opf_dir, &ncx_rel);
        if let Some(bytes) = find_and_extract_entry(&zip, &full) {
            if let Ok(s) = std::str::from_utf8(&bytes) {
                if let Ok(ncx_doc) = roxmltree::Document::parse(s) {
                    if let Some(nav_map) =
                        ncx_doc.descendants().find(|n| n.tag_name().name() == "navMap")
                    {
                        let mut order_counter = 1u32;
                        let ncx_dir = Path::new(&full)
                            .parent()
                            .and_then(|p| p.to_str())
                            .unwrap_or("");
                        let nodes = parse_ncx_nav_points(&nav_map, ncx_dir, 0, &mut order_counter);
                        if !nodes.is_empty() {
                            return Ok(nodes);
                        }
                    }
                }
            }
        }
    }

    // 2. Try EPUB3 NAV TOC
    if let Some(nav_rel) = nav_href {
        let full = resolve_path(opf_dir, &nav_rel);
        if let Some(bytes) = find_and_extract_entry(&zip, &full) {
            if let Ok(s) = std::str::from_utf8(&bytes) {
                if let Ok(nav_doc) = roxmltree::Document::parse(s) {
                    let nav_dir = Path::new(&full)
                        .parent()
                        .and_then(|p| p.to_str())
                        .unwrap_or("");
                    if let Some(toc_nav) = nav_doc.descendants().find(|n| {
                        n.tag_name().name() == "nav"
                            && (n.attribute("epub:type") == Some("toc")
                                || n.attribute("type") == Some("toc")
                                || n.children().any(|c| c.tag_name().name() == "ol"))
                    }) {
                        if let Some(ol) = toc_nav.children().find(|n| n.tag_name().name() == "ol")
                        {
                            let mut order_counter = 1u32;
                            let nodes = parse_nav_ol(&ol, nav_dir, 0, &mut order_counter);
                            if !nodes.is_empty() {
                                return Ok(nodes);
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Fallback: generate flat TOC from spine items
    let spine = parse_epub_spine(data)?;
    Ok(spine
        .into_iter()
        .map(|s| UniFFIEbookTocNode {
            id: s.id.clone(),
            title: format!("Chapter {}", s.play_order),
            href: s.href,
            play_order: s.play_order,
            level: 0,
            children: Vec::new(),
        })
        .collect())
}

fn parse_ncx_nav_points<'a>(
    parent: &roxmltree::Node<'a, 'a>,
    base_dir: &str,
    level: u32,
    order_counter: &mut u32,
) -> Vec<UniFFIEbookTocNode> {
    let mut nodes = Vec::new();
    for np in parent.children().filter(|n| n.tag_name().name() == "navPoint") {
        let id = np.attribute("id").unwrap_or("").to_string();
        let play_order = np
            .attribute("playOrder")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or_else(|| {
                let cur = *order_counter;
                *order_counter += 1;
                cur
            });

        let title = np
            .descendants()
            .find(|n| n.tag_name().name() == "text")
            .and_then(|n| n.text())
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| format!("Section {play_order}"));

        let src = np
            .children()
            .find(|n| n.tag_name().name() == "content")
            .and_then(|n| n.attribute("src"))
            .unwrap_or("")
            .to_string();

        let href = resolve_path(base_dir, &src);
        let children = parse_ncx_nav_points(&np, base_dir, level + 1, order_counter);

        nodes.push(UniFFIEbookTocNode {
            id: if id.is_empty() {
                format!("np_{play_order}")
            } else {
                id
            },
            title,
            href,
            play_order,
            level,
            children,
        });
    }
    nodes
}

fn parse_nav_ol<'a>(
    ol: &roxmltree::Node<'a, 'a>,
    base_dir: &str,
    level: u32,
    order_counter: &mut u32,
) -> Vec<UniFFIEbookTocNode> {
    let mut nodes = Vec::new();
    for li in ol.children().filter(|n| n.tag_name().name() == "li") {
        let a = li.children().find(|n| n.tag_name().name() == "a");
        let src = a.and_then(|n| n.attribute("href")).unwrap_or("");
        let title = a
            .and_then(|n| n.text())
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| format!("Section {}", *order_counter));

        let play_order = *order_counter;
        *order_counter += 1;

        let href = resolve_path(base_dir, src);
        let mut children = Vec::new();
        if let Some(child_ol) = li.children().find(|n| n.tag_name().name() == "ol") {
            children = parse_nav_ol(&child_ol, base_dir, level + 1, order_counter);
        }

        nodes.push(UniFFIEbookTocNode {
            id: format!("nav_{play_order}"),
            title,
            href,
            play_order,
            level,
            children,
        });
    }
    nodes
}

pub(crate) fn parse_epub_spine(
    data: &[u8],
) -> Result<Vec<UniFFIEbookSpineItem>, UniFFIEbookError> {
    let (_, opf_path, opf_xml) = open_epub_zip(data)?;
    let opf_dir = Path::new(&opf_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    let doc = roxmltree::Document::parse(&opf_xml)
        .map_err(|e| UniFFIEbookError::xml_err(e.to_string()))?;

    let (manifest, _, _, _) = parse_epub_manifest(&doc);
    let mut spine_items = Vec::new();

    if let Some(spine_node) = doc.descendants().find(|n| n.tag_name().name() == "spine") {
        for (idx, itemref) in spine_node
            .children()
            .filter(|n| n.tag_name().name() == "itemref")
            .enumerate()
        {
            if let Some(idref) = itemref.attribute("idref") {
                if let Some(item) = manifest.get(idref) {
                    let href = resolve_path(opf_dir, &item.href);
                    let is_linear = itemref.attribute("linear") != Some("no");
                    spine_items.push(UniFFIEbookSpineItem {
                        id: idref.to_string(),
                        href,
                        media_type: item.media_type.clone(),
                        play_order: (idx + 1) as u32,
                        is_linear,
                    });
                }
            }
        }
    }

    Ok(spine_items)
}

pub(crate) fn extract_epub_chapter(
    data: &[u8],
    href: &str,
) -> Result<UniFFIEbookChapter, UniFFIEbookError> {
    let (zip, _, _) = open_epub_zip(data)?;
    let clean_href = href.split('#').next().unwrap_or(href);
    let bytes = find_and_extract_entry(&zip, clean_href)
        .ok_or_else(|| UniFFIEbookError::not_found(clean_href))?;

    let content_string = String::from_utf8_lossy(&bytes).to_string();
    let plain_text = strip_html_tags(&content_string);
    let character_count = plain_text.chars().count() as u32;
    let word_count = plain_text.split_whitespace().count() as u32;

    let title = extract_html_heading(&content_string).unwrap_or_else(|| {
        Path::new(clean_href)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Chapter")
            .to_string()
    });

    Ok(UniFFIEbookChapter {
        id: clean_href.to_string(),
        title,
        href: clean_href.to_string(),
        media_type: "application/xhtml+xml".to_string(),
        play_order: 1,
        content_string,
        character_count,
        word_count,
    })
}

pub(crate) fn extract_epub_cover(
    data: &[u8],
) -> Result<Option<UniFFIEbookResource>, UniFFIEbookError> {
    let meta = parse_epub_metadata(data, None)?;
    if let Some(path) = meta.cover_path {
        match extract_resource_from_zip(data, &path) {
            Ok(res) => Ok(Some(res)),
            Err(_) => Ok(None),
        }
    } else {
        Ok(None)
    }
}
