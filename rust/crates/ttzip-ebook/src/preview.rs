// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use ttzip_archive::mmap_guard::safe_map_file;
use ttzip_archive::zip::ZipArchive;

use crate::container::{
    extract_html_heading, find_and_extract_zip_entry, opf_parent_dir, resolve_epub_opf_path,
};
use crate::navigation::EbookTocNode;
use crate::ncx::{collect_toc_title_map, parse_nav_xhtml, parse_ncx};
use crate::opf::{ParsedOpfPackage, parse_opf_xml};

#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("Corrupt EPUB: {0}")]
    Corrupt(String),
    #[error("Missing EPUB resource: {0}")]
    Missing(String),
    #[error("Invalid EPUB XML: {0}")]
    Xml(String),
}

pub struct PreviewSpineItem {
    pub id: String,
    pub href: String,
    pub media_type: String,
    pub play_order: u32,
    pub is_linear: bool,
}

pub struct PreviewTocNode {
    pub id: String,
    pub title: String,
    pub href: String,
    pub play_order: u32,
    pub level: u32,
    pub children: Vec<Self>,
}

pub struct FallbackChapter {
    pub title: String,
    pub href: String,
}

pub struct FallbackBook {
    pub title: String,
    pub chapters: Vec<FallbackChapter>,
    pub cover_path: Option<String>,
}

pub fn open_and_parse_opf(
    data: &[u8],
) -> Result<(ZipArchive<'_>, String, ParsedOpfPackage), PreviewError> {
    let zip = ZipArchive::open_slice(data).map_err(|e| PreviewError::Corrupt(format!("{e:?}")))?;

    let opf_path = resolve_epub_opf_path(&zip)
        .ok_or_else(|| PreviewError::Missing("META-INF/container.xml or *.opf".into()))?;
    let opf_bytes = find_and_extract_zip_entry(&zip, &opf_path)
        .ok_or_else(|| PreviewError::Missing(opf_path.clone()))?;
    let opf_dir = opf_parent_dir(&opf_path).to_string();
    let opf = parse_opf_xml(&opf_bytes, &opf_dir).map_err(|e| PreviewError::Xml(e.to_string()))?;

    Ok((zip, opf_dir, opf))
}

pub fn title(opf: &ParsedOpfPackage, file_name: Option<&str>) -> String {
    opf.metadata
        .title
        .clone()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| {
            file_name
                .and_then(|f| Path::new(f).file_stem().and_then(|s| s.to_str()))
                .unwrap_or("Untitled Book")
                .to_string()
        })
}

pub fn resource(data: &[u8], href: &str) -> Result<Vec<u8>, PreviewError> {
    let zip = ZipArchive::open_slice(data)
        .map_err(|error| PreviewError::Corrupt(format!("{error:?}")))?;
    let clean_href = href.split('#').next().unwrap_or(href);
    find_and_extract_zip_entry(&zip, clean_href)
        .ok_or_else(|| PreviewError::Missing(clean_href.into()))
}

fn project_toc_nodes(
    nodes: Vec<EbookTocNode>,
    id_prefix: &str,
    level: u32,
    order_counter: &mut u32,
) -> Vec<PreviewTocNode> {
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
        out.push(PreviewTocNode {
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

fn read_navigation(
    zip: &ZipArchive<'_>,
    opf_dir: &str,
    path: &str,
    parse: impl FnOnce(&[u8], &str, &[crate::SpineItem]) -> crate::EbookResult<Vec<EbookTocNode>>,
) -> Option<Vec<EbookTocNode>> {
    let bytes = find_and_extract_zip_entry(zip, path)?;
    let base = if path.contains('/') {
        opf_parent_dir(path)
    } else {
        opf_dir
    };
    let nodes = parse(&bytes, base, &[]).ok()?;
    (!nodes.is_empty()).then_some(nodes)
}

pub fn toc(zip: &ZipArchive<'_>, opf_dir: &str, opf: &ParsedOpfPackage) -> Vec<PreviewTocNode> {
    // A publication that provides both documents uses its NCX for legacy preview parity.
    if let Some(nodes) = opf
        .ncx_href
        .as_deref()
        .and_then(|path| read_navigation(zip, opf_dir, path, parse_ncx))
    {
        return project_toc_nodes(nodes, "np", 0, &mut 1);
    }

    if let Some(nodes) = opf
        .nav_href
        .as_deref()
        .and_then(|path| read_navigation(zip, opf_dir, path, parse_nav_xhtml))
    {
        return project_toc_nodes(nodes, "nav", 0, &mut 1);
    }

    spine(opf)
        .into_iter()
        .map(|s| PreviewTocNode {
            id: s.id,
            title: format!("Chapter {}", s.play_order),
            href: s.href,
            play_order: s.play_order,
            level: 0,
            children: Vec::new(),
        })
        .collect()
}

pub fn spine(opf: &ParsedOpfPackage) -> Vec<PreviewSpineItem> {
    let mut spine_items = Vec::with_capacity(opf.spine_raw.len());

    for (idx, (idref, is_linear)) in opf.spine_raw.iter().enumerate() {
        if let Some(item) = opf.manifest.get(idref) {
            spine_items.push(PreviewSpineItem {
                id: idref.clone(),
                href: item.href.clone(),
                media_type: item.media_type.clone(),
                play_order: (idx + 1) as u32,
                is_linear: *is_linear,
            });
        }
    }

    spine_items
}

fn resolve_chapter_title(
    zip: &ZipArchive<'_>,
    full_href: &str,
    toc_titles: &HashMap<String, String>,
    book_title: &str,
) -> String {
    let file_name = Path::new(full_href)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(full_href);

    let from_toc = toc_titles
        .get(full_href)
        .or_else(|| toc_titles.get(file_name))
        .cloned();

    let chapter_title = from_toc.or_else(|| {
        let ch_bytes = find_and_extract_zip_entry(zip, full_href)?;
        let ch_str = String::from_utf8_lossy(&ch_bytes);
        extract_html_heading(&ch_str).filter(|t| t != book_title && !t.trim().is_empty())
    });

    chapter_title.unwrap_or_else(|| {
        Path::new(full_href)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Chapter")
            .to_string()
    })
}

fn build_ordered_chapters(
    zip: &ZipArchive<'_>,
    opf: &ParsedOpfPackage,
    toc_titles: &HashMap<String, String>,
    book_title: &str,
) -> Vec<FallbackChapter> {
    let mut ordered_chapters = Vec::new();
    for (idref, _) in &opf.spine_raw {
        if let Some(item) = opf.manifest.get(idref) {
            let full_href = item.href.clone();
            let base_title = resolve_chapter_title(zip, &full_href, toc_titles, book_title);
            let idx = ordered_chapters.len();
            let is_prefixed = base_title.starts_with('第')
                || base_title.contains('章')
                || base_title.to_lowercase().contains("chapter");
            let display_title = if is_prefixed {
                base_title
            } else {
                format!("Chapter {} · {}", idx + 1, base_title)
            };
            ordered_chapters.push(FallbackChapter {
                title: display_title,
                href: full_href,
            });
        }
    }
    ordered_chapters
}

pub fn fallback_chapters(zip: &ZipArchive<'_>) -> Vec<FallbackChapter> {
    let mut html_entries: Vec<String> = zip
        .entries()
        .iter()
        .filter(|e| {
            let l = e.rel_path.to_lowercase();
            l.ends_with(".xhtml") || l.ends_with(".html") || l.ends_with(".htm")
        })
        .map(|e| e.rel_path.clone())
        .collect();
    html_entries.sort();

    html_entries
        .into_iter()
        .enumerate()
        .map(|(idx, entry_path)| {
            let stem = Path::new(&entry_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Chapter");
            FallbackChapter {
                title: format!("Chapter {} · {stem}", idx + 1),
                href: entry_path,
            }
        })
        .collect()
}

pub fn fallback_book(path: &Path) -> Option<FallbackBook> {
    if !path.exists() {
        return None;
    }

    let file = File::open(path).ok()?;
    let mmap = safe_map_file(&file).ok()?;
    let (zip, opf_dir, opf) = open_and_parse_opf(&mmap).ok()?;

    let default_title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled Book")
        .to_string();
    let book_title = opf
        .metadata
        .title
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or(default_title);

    let toc_titles = collect_toc_title_map(
        &zip,
        opf.ncx_href.as_deref(),
        opf.nav_href.as_deref(),
        &opf_dir,
    );

    let mut ordered_chapters = build_ordered_chapters(&zip, &opf, &toc_titles, &book_title);
    if ordered_chapters.is_empty() {
        ordered_chapters = fallback_chapters(&zip);
    }
    if ordered_chapters.is_empty() {
        return None;
    }

    Some(FallbackBook {
        title: book_title,
        chapters: ordered_chapters,
        cover_path: opf.metadata.cover_image_href,
    })
}
