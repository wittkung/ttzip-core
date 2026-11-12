// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! EPUB Standard Publication Container Resolution, OPF Manifest Parsing,
//! NCX/NAV Hierarchical TOC Extraction, Spine Ordering, and Chapter Reading.

use std::path::Path;

use super::helpers::{
    extract_html_heading, extract_resource_from_zip, find_and_extract_entry, strip_html_tags,
};
use super::types::{
    UniFFIEbookChapter, UniFFIEbookError, UniFFIEbookFormat, UniFFIEbookMetadata,
    UniFFIEbookResource, UniFFIEbookSpineItem, UniFFIEbookTocNode,
};
use crate::ebook::container::{opf_parent_dir, resolve_epub_opf_path};
use crate::ebook::navigation::EbookTocNode;
use crate::ebook::ncx::{parse_nav_xhtml, parse_ncx};
use crate::ebook::opf::{parse_opf_xml, ParsedOpfPackage};
use crate::zip::reader::ZipArchive;

fn open_and_parse_opf(
    data: &[u8],
) -> Result<(ZipArchive<'_>, String, ParsedOpfPackage), UniFFIEbookError> {
    let zip = ZipArchive::open_slice(data)
        .map_err(|e| UniFFIEbookError::corrupted(format!("{e:?}")))?;

    let opf_path = resolve_epub_opf_path(&zip)
        .ok_or_else(|| UniFFIEbookError::not_found("META-INF/container.xml or *.opf"))?;
    let opf_bytes = find_and_extract_entry(&zip, &opf_path)
        .ok_or_else(|| UniFFIEbookError::not_found(&opf_path))?;
    let opf_dir = opf_parent_dir(&opf_path).to_string();
    let opf = parse_opf_xml(&opf_bytes, &opf_dir)
        .map_err(|e| UniFFIEbookError::xml_err(e.to_string()))?;

    Ok((zip, opf_dir, opf))
}

pub(crate) fn parse_epub_metadata(
    data: &[u8],
    file_name: Option<&str>,
) -> Result<UniFFIEbookMetadata, UniFFIEbookError> {
    let (zip, _, opf) = open_and_parse_opf(data)?;

    let title = opf.metadata.title.filter(|t| !t.is_empty()).unwrap_or_else(|| {
        file_name
            .and_then(|f| Path::new(f).file_stem().and_then(|s| s.to_str()))
            .unwrap_or("Untitled Book")
            .to_string()
    });

    let cover_path = opf.metadata.cover_image_href;
    let has_cover = cover_path
        .as_ref()
        .is_some_and(|p| find_and_extract_entry(&zip, p).is_some());

    Ok(UniFFIEbookMetadata {
        title,
        authors: opf.metadata.authors,
        publisher: opf.metadata.publisher,
        language: opf.metadata.language,
        identifier: opf.metadata.identifiers.into_iter().next(),
        description: opf.metadata.description,
        publication_date: opf.metadata.publication_date,
        modified_date: opf.modified_date,
        rights: opf.metadata.rights,
        format: UniFFIEbookFormat::Epub,
        total_chapters: opf.spine_raw.len() as u32,
        total_resources: opf.manifest.len() as u32,
        file_size_bytes: data.len() as u64,
        has_cover,
        cover_path,
        extra_metadata: opf.extra_metadata,
    })
}

fn project_toc_nodes(
    nodes: Vec<EbookTocNode>,
    id_prefix: &str,
    level: u32,
    order_counter: &mut u32,
) -> Vec<UniFFIEbookTocNode> {
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        let play_order = *order_counter;
        *order_counter += 1;
        let id = if node.id.is_empty() {
            format!("{id_prefix}_{play_order}")
        } else {
            node.id
        };
        let title = if node.title.is_empty() {
            format!("Section {play_order}")
        } else {
            node.title
        };
        let children = project_toc_nodes(node.children, id_prefix, level + 1, order_counter);
        out.push(UniFFIEbookTocNode {
            id,
            title,
            href: node.href,
            play_order,
            level,
            children,
        });
    }
    out
}

pub(crate) fn parse_epub_toc(data: &[u8]) -> Result<Vec<UniFFIEbookTocNode>, UniFFIEbookError> {
    let (zip, opf_dir, opf) = open_and_parse_opf(data)?;

    if let Some(ref ncx_path) = opf.ncx_href
        && let Some(bytes) = find_and_extract_entry(&zip, ncx_path)
    {
        let base_dir = if ncx_path.contains('/') {
            opf_parent_dir(ncx_path)
        } else {
            &opf_dir
        };
        if let Ok(nodes) = parse_ncx(&bytes, base_dir, &[])
            && !nodes.is_empty()
        {
            let mut order = 1u32;
            return Ok(project_toc_nodes(nodes, "np", 0, &mut order));
        }
    }

    if let Some(ref nav_path) = opf.nav_href
        && let Some(bytes) = find_and_extract_entry(&zip, nav_path)
    {
        let base_dir = if nav_path.contains('/') {
            opf_parent_dir(nav_path)
        } else {
            &opf_dir
        };
        if let Ok(nodes) = parse_nav_xhtml(&bytes, base_dir, &[])
            && !nodes.is_empty()
        {
            let mut order = 1u32;
            return Ok(project_toc_nodes(nodes, "nav", 0, &mut order));
        }
    }

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

pub(crate) fn parse_epub_spine(
    data: &[u8],
) -> Result<Vec<UniFFIEbookSpineItem>, UniFFIEbookError> {
    let (_, _, opf) = open_and_parse_opf(data)?;
    let mut spine_items = Vec::with_capacity(opf.spine_raw.len());

    for (idx, (idref, is_linear)) in opf.spine_raw.into_iter().enumerate() {
        if let Some(item) = opf.manifest.get(&idref) {
            spine_items.push(UniFFIEbookSpineItem {
                id: idref,
                href: item.href.clone(),
                media_type: item.media_type.clone(),
                play_order: (idx + 1) as u32,
                is_linear,
            });
        }
    }

    Ok(spine_items)
}

pub(crate) fn extract_epub_chapter(
    data: &[u8],
    href: &str,
) -> Result<UniFFIEbookChapter, UniFFIEbookError> {
    let zip = ZipArchive::open_slice(data)
        .map_err(|e| UniFFIEbookError::corrupted(format!("{e:?}")))?;
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
