// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TTZip E-book Format Detector, OPF Package Parser, and Unified Parsing Microkernel.
//!
//! Handles EPUB 2/3, MOBI, and AZW3 container recognition, Dublin Core metadata extraction,
//! and TOC/Spine graph construction.

use std::collections::HashMap;
use quick_xml::events::Event;

use crate::ebook::mobi::EbookMobiDecoder;
use crate::ebook::navigation::{local_name, EbookNavigationExtractor, EbookTocNode, SpineItem};
use crate::ebook::resource::{clean_container_path, normalize_path, EbookResource, EbookResourceExtractor};
use crate::ebook::{EbookError, EbookResult};
use crate::zip::ZipArchive;

/// Recognized electronic book container formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EbookFormat {
    /// EPUB 2.0 / 2.0.1 Open eBook Publication structure.
    Epub2,
    /// EPUB 3.0 / 3.2 / 3.3 Modern Web Standards Publication structure.
    Epub3,
    /// Mobipocket standard format (MOBI 6/7).
    Mobi,
    /// Amazon Kindle Format 8 (AZW3 / KF8).
    Azw3,
    /// Unrecognized e-book format.
    #[default]
    Unknown,
}

/// Extracted publication metadata (Dublin Core and EXTH extensions).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EbookMetadata {
    /// Main publication title.
    pub title: Option<String>,
    /// List of creators / authors.
    pub authors: Vec<String>,
    /// Publishing house or entity.
    pub publisher: Option<String>,
    /// Primary BCP-47 language tag (e.g., `en`, `zh-CN`).
    pub language: Option<String>,
    /// Book synopsis or description.
    pub description: Option<String>,
    /// Copyright or distribution rights statement.
    pub rights: Option<String>,
    /// Canonical unique identifiers (ISBN, UUID, ASIN, DOI).
    pub identifiers: Vec<String>,
    /// Publication or release date (ISO-8601 or free text).
    pub publication_date: Option<String>,
    /// Container relative path or URI to the cover image.
    pub cover_image_href: Option<String>,
}

/// Individual item declared in the EPUB OPF package manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpubManifestItem {
    /// Item identifier attribute in OPF.
    pub id: String,
    /// Container relative path to the resource.
    pub href: String,
    /// MIME content type of the file.
    pub media_type: String,
    /// Space-delimited properties (e.g., `cover-image`, `nav`, `scripted`).
    pub properties: Option<String>,
}

/// High-throughput e-book parser microkernel.
pub struct TTZipEbookParser<'a> {
    raw_data: &'a [u8],
    format: EbookFormat,
    metadata: EbookMetadata,
    spine: Vec<SpineItem>,
    manifest: HashMap<String, EpubManifestItem>,
    toc: Vec<EbookTocNode>,
    opf_path: String,
    opf_dir: String,
    zip: Option<ZipArchive<'a>>,
    mobi: Option<EbookMobiDecoder<'a>>,
}

impl<'a> TTZipEbookParser<'a> {
    /// Automatically detects e-book format and parses metadata, spine, and navigation structure from a byte slice.
    pub fn open_from_bytes(data: &'a [u8]) -> EbookResult<Self> {
        // 1. Probe for MOBI / AZW3 / PalmDOC format
        if let Ok(mobi_decoder) = EbookMobiDecoder::parse(data) {
            let format = if mobi_decoder.is_azw3() {
                EbookFormat::Azw3
            } else {
                EbookFormat::Mobi
            };

            let title = mobi_decoder.title();
            let authors = mobi_decoder.authors();
            let publisher = mobi_decoder.publisher();
            let description = mobi_decoder.description();
            let publication_date = mobi_decoder.publication_date();
            let rights = mobi_decoder.rights();
            let mut identifiers = Vec::new();
            if let Some(id) = mobi_decoder.asin_or_isbn() {
                identifiers.push(id);
            }

            let cover_image_href = if mobi_decoder.extract_cover_image().is_some() {
                Some("cover.jpg".to_string())
            } else {
                None
            };

            let metadata = EbookMetadata {
                title,
                authors,
                publisher,
                language: None,
                description,
                rights,
                identifiers,
                publication_date,
                cover_image_href,
            };

            return Ok(Self {
                raw_data: data,
                format,
                metadata,
                spine: Vec::new(),
                manifest: HashMap::new(),
                toc: Vec::new(),
                opf_path: String::new(),
                opf_dir: String::new(),
                zip: None,
                mobi: Some(mobi_decoder),
            });
        }

        // 2. Probe for EPUB (ZIP archive container)
        if let Ok(zip) = ZipArchive::open_slice(data) {
            if is_epub_container(&zip) {
                return Self::parse_epub_container(data, zip);
            }
        }

        Err(EbookError::UnsupportedFormat(
            "Payload does not match EPUB, MOBI, or AZW3 signature".to_string(),
        ))
    }

    /// Returns the raw binary payload slice.
    #[inline]
    pub fn raw_data(&self) -> &'a [u8] {
        self.raw_data
    }

    /// Returns the detected e-book format.
    #[inline]
    pub fn format(&self) -> EbookFormat {
        self.format
    }

    /// Returns the publication metadata.
    #[inline]
    pub fn metadata(&self) -> &EbookMetadata {
        &self.metadata
    }

    /// Returns the sequential reading spine.
    #[inline]
    pub fn spine(&self) -> &[SpineItem] {
        &self.spine
    }

    /// Returns the hierarchical Table of Contents.
    #[inline]
    pub fn toc(&self) -> &[EbookTocNode] {
        &self.toc
    }

    /// Returns the OPF manifest item lookup table.
    #[inline]
    pub fn manifest(&self) -> &HashMap<String, EpubManifestItem> {
        &self.manifest
    }

    /// Returns the OPF package root file path inside the container.
    #[inline]
    pub fn opf_path(&self) -> &str {
        &self.opf_path
    }

    /// Returns the raw underlying e-book byte slice.
    #[inline]
    pub fn raw_bytes(&self) -> &'a [u8] {
        self.raw_data
    }

    /// Returns the OPF base directory path.
    #[inline]
    pub fn opf_dir(&self) -> &str {
        &self.opf_dir
    }

    /// Extracts a resource by container relative path.
    pub fn extract_resource(&self, path: &str) -> EbookResult<EbookResource> {
        if let Some(ref zip) = self.zip {
            EbookResourceExtractor::extract_resource(zip, path, None)
        } else if let Some(ref mobi) = self.mobi {
            if path == "cover.jpg" || path == "cover.jpeg" || path == "cover.png" {
                if let Some(img_data) = mobi.extract_cover_image() {
                    return Ok(EbookResource {
                        path: path.to_string(),
                        media_type: "image/jpeg".to_string(),
                        data: img_data,
                    });
                }
            }
            Err(EbookError::NotFound(format!(
                "Direct resource extraction not supported for MOBI/AZW3: {path}"
            )))
        } else {
            Err(EbookError::NotFound("No container opened".to_string()))
        }
    }

    /// Extracts the text/XHTML payload of a specific spine chapter index.
    pub fn extract_chapter_text(&self, spine_index: usize) -> EbookResult<String> {
        let item = self.spine.get(spine_index).ok_or_else(|| {
            EbookError::NotFound(format!("Spine index out of bounds: {spine_index}"))
        })?;

        if let Some(ref zip) = self.zip {
            EbookResourceExtractor::extract_text(zip, &item.href)
        } else {
            Err(EbookError::NotFound(
                "Spine chapter extraction only supported on EPUB".to_string(),
            ))
        }
    }

    /// Extracts the binary cover image, if available.
    pub fn extract_cover(&self) -> EbookResult<Option<EbookResource>> {
        if let Some(ref mobi) = self.mobi {
            if let Some(cover_bytes) = mobi.extract_cover_image() {
                return Ok(Some(EbookResource {
                    path: "cover.jpg".to_string(),
                    media_type: "image/jpeg".to_string(),
                    data: cover_bytes,
                }));
            }
            return Ok(None);
        }

        if let Some(ref zip) = self.zip {
            if let Some(ref cover_href) = self.metadata.cover_image_href {
                if let Ok(res) = EbookResourceExtractor::extract_resource(zip, cover_href, None) {
                    return Ok(Some(res));
                }
            }
        }

        Ok(None)
    }

    /// Extracts full text from the e-book (MOBI decompressed records or EPUB spine concatenation).
    pub fn extract_full_text(&self) -> EbookResult<String> {
        if let Some(ref mobi) = self.mobi {
            return mobi.extract_full_text();
        }

        if let Some(ref zip) = self.zip {
            let mut full_text = String::new();
            for item in &self.spine {
                if let Ok(chapter_raw) = EbookResourceExtractor::extract_text(zip, &item.href) {
                    let stripped = strip_html_tags(&chapter_raw);
                    if !stripped.trim().is_empty() {
                        if !full_text.is_empty() {
                            full_text.push_str("\n\n");
                        }
                        full_text.push_str(stripped.trim());
                    }
                }
            }
            return Ok(full_text);
        }

        Ok(String::new())
    }

    /// Internal helper to parse full EPUB container and OPF package.
    fn parse_epub_container(raw_data: &'a [u8], zip: ZipArchive<'a>) -> EbookResult<Self> {
        // 1. Locate and parse META-INF/container.xml
        let container_xml = EbookResourceExtractor::extract_text(&zip, "META-INF/container.xml")
            .map_err(|_| EbookError::NotFound("Missing META-INF/container.xml in EPUB".to_string()))?;

        let opf_path = parse_container_rootfile(container_xml.as_bytes())?;
        let opf_dir = match opf_path.rfind('/') {
            Some(pos) => opf_path[..pos].to_string(),
            None => String::new(),
        };

        // 2. Extract and parse OPF package file
        let opf_xml = EbookResourceExtractor::extract_text(&zip, &opf_path)
            .map_err(|_| EbookError::NotFound(format!("Missing OPF package file at {opf_path}")))?;

        let (format, metadata, manifest, spine_raw, cover_meta_id, spine_toc_id, nav_xhtml_id) =
            parse_opf_xml(opf_xml.as_bytes(), &opf_dir)?;

        // 3. Resolve Spine items with manifest references
        let mut spine: Vec<SpineItem> = Vec::with_capacity(spine_raw.len());
        for (idref, linear) in spine_raw {
            if let Some(item) = manifest.get(&idref) {
                spine.push(SpineItem {
                    idref: idref.clone(),
                    href: item.href.clone(),
                    linear,
                    media_type: item.media_type.clone(),
                });
            }
        }

        // Self-heal broken or missing spine items from manifest or container entries
        if spine.is_empty() {
            for (id, item) in &manifest {
                if item.media_type == "application/xhtml+xml"
                    || item.media_type == "text/html"
                    || item.href.ends_with(".xhtml")
                    || item.href.ends_with(".html")
                {
                    spine.push(SpineItem {
                        idref: id.clone(),
                        href: item.href.clone(),
                        linear: true,
                        media_type: item.media_type.clone(),
                    });
                }
            }
            if spine.is_empty() {
                for entry in zip.entries() {
                    let path = &entry.rel_path;
                    if path.ends_with(".xhtml") || path.ends_with(".html") || path.ends_with(".htm")
                    {
                        spine.push(SpineItem {
                            idref: path.clone(),
                            href: path.clone(),
                            linear: true,
                            media_type: "application/xhtml+xml".to_string(),
                        });
                    }
                }
            }
            spine.sort_by(|a, b| a.href.cmp(&b.href));
        }

        // 4. Resolve Cover image href if not resolved in metadata
        let mut resolved_metadata = metadata;
        if resolved_metadata.cover_image_href.is_none() {
            if let Some(cover_id) = cover_meta_id {
                if let Some(item) = manifest.get(&cover_id) {
                    resolved_metadata.cover_image_href = Some(item.href.clone());
                }
            }
        }
        if resolved_metadata.cover_image_href.is_none() {
            for item in manifest.values() {
                if let Some(ref props) = item.properties {
                    if props.split_whitespace().any(|p| p == "cover-image") {
                        resolved_metadata.cover_image_href = Some(item.href.clone());
                        break;
                    }
                }
            }
        }

        // 5. Parse TOC navigation hierarchy
        let mut toc = Vec::new();

        // Check for EPUB 3 Navigation Document first
        let mut nav_href = None;
        if let Some(nav_id) = nav_xhtml_id {
            if let Some(item) = manifest.get(&nav_id) {
                nav_href = Some(item.href.clone());
            }
        }
        if nav_href.is_none() {
            for item in manifest.values() {
                if let Some(ref props) = item.properties {
                    if props.split_whitespace().any(|p| p == "nav") {
                        nav_href = Some(item.href.clone());
                        break;
                    }
                }
            }
        }

        if let Some(nav_path) = nav_href {
            if let Ok(nav_bytes) =
                EbookResourceExtractor::extract_resource(&zip, &nav_path, None)
            {
                let nav_dir = match nav_path.rfind('/') {
                    Some(pos) => nav_path[..pos].to_string(),
                    None => opf_dir.clone(),
                };
                if let Ok(nodes) =
                    EbookNavigationExtractor::parse_nav_xhtml(&nav_bytes.data, &nav_dir, &spine)
                {
                    if !nodes.is_empty() {
                        toc = nodes;
                    }
                }
            }
        }

        // Fallback to EPUB 2 NCX TOC if Nav XHTML was not present or returned 0 nodes
        if toc.is_empty() {
            let mut ncx_href = None;
            if let Some(toc_id) = spine_toc_id {
                if let Some(item) = manifest.get(&toc_id) {
                    ncx_href = Some(item.href.clone());
                }
            }
            if ncx_href.is_none() {
                for item in manifest.values() {
                    if item.media_type == "application/x-dtbncx+xml" {
                        ncx_href = Some(item.href.clone());
                        break;
                    }
                }
            }

            if let Some(ncx_path) = ncx_href {
                if let Ok(ncx_bytes) =
                    EbookResourceExtractor::extract_resource(&zip, &ncx_path, None)
                {
                    let ncx_dir = match ncx_path.rfind('/') {
                        Some(pos) => ncx_path[..pos].to_string(),
                        None => opf_dir.clone(),
                    };
                    if let Ok(nodes) =
                        EbookNavigationExtractor::parse_ncx(&ncx_bytes.data, &ncx_dir, &spine)
                    {
                        toc = nodes;
                    }
                }
            }
        }

        Ok(Self {
            raw_data,
            format,
            metadata: resolved_metadata,
            spine,
            manifest,
            toc,
            opf_path,
            opf_dir,
            zip: Some(zip),
            mobi: None,
        })
    }
}

/// Checks if a ZIP archive conforms to the EPUB specification.
fn is_epub_container(zip: &ZipArchive<'_>) -> bool {
    let mut has_mimetype = false;
    let mut has_container = false;

    for entry in zip.entries() {
        let clean = clean_container_path(&entry.rel_path);
        if clean == "mimetype" {
            has_mimetype = true;
        } else if clean == "META-INF/container.xml" {
            has_container = true;
        }
    }

    has_container || has_mimetype
}

/// Parses the OPF full-path from `META-INF/container.xml`.
fn parse_container_rootfile(xml: &[u8]) -> EbookResult<String> {
    let mut reader = quick_xml::Reader::from_reader(xml);
    let mut buf = Vec::with_capacity(512);

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) | Event::Empty(ref e) => {
                if local_name(e.name().into_inner()) == b"rootfile" {
                    for attr in e.attributes().flatten() {
                        if local_name(attr.key.into_inner()) == b"full-path" {
                            if let Ok(v) = attr.unescape_value() {
                                return Ok(v.to_string());
                            }
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Err(EbookError::NotFound(
        "Missing <rootfile full-path=...> in META-INF/container.xml".to_string(),
    ))
}

type ParsedOpfResult = (
    EbookFormat,
    EbookMetadata,
    HashMap<String, EpubManifestItem>,
    Vec<(String, bool)>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// Parses an OPF package XML document into structured format, metadata, manifest, and spine tuples.
fn parse_opf_xml(xml: &[u8], opf_dir: &str) -> EbookResult<ParsedOpfResult> {
    let mut reader = quick_xml::Reader::from_reader(xml);
    reader.config_mut().trim_text(true);

    let mut format = EbookFormat::Epub2;
    let mut metadata = EbookMetadata::default();
    let mut manifest = HashMap::new();
    let mut spine_raw = Vec::new();
    let mut cover_meta_id = None;
    let mut spine_toc_id = None;
    let mut nav_xhtml_id = None;

    let mut in_metadata = false;
    let mut in_manifest = false;
    let mut in_spine = false;

    let mut current_element = Vec::new();
    let mut text_buffer = String::new();
    let mut buf = Vec::with_capacity(512);

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) => {
                let name = local_name(e.name().into_inner()).to_vec();
                match name.as_slice() {
                    b"package" => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.into_inner()) == b"version" {
                                if let Ok(v) = attr.unescape_value() {
                                    if v.starts_with('3') {
                                        format = EbookFormat::Epub3;
                                    } else {
                                        format = EbookFormat::Epub2;
                                    }
                                }
                            }
                        }
                    }
                    b"metadata" => in_metadata = true,
                    b"manifest" => in_manifest = true,
                    b"spine" => {
                        in_spine = true;
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.into_inner()) == b"toc" {
                                if let Ok(v) = attr.unescape_value() {
                                    spine_toc_id = Some(v.to_string());
                                }
                            }
                        }
                    }
                    _ => {}
                }
                current_element = name;
                text_buffer.clear();
            }
            Event::Empty(ref e) => {
                let name = local_name(e.name().into_inner());
                if in_manifest && name == b"item" {
                    let mut id = String::new();
                    let mut href = String::new();
                    let mut media_type = String::new();
                    let mut properties = None;

                    for attr in e.attributes().flatten() {
                        let k = local_name(attr.key.into_inner());
                        if let Ok(v) = attr.unescape_value() {
                            match k {
                                b"id" => id = v.to_string(),
                                b"href" => href = v.to_string(),
                                b"media-type" => media_type = v.to_string(),
                                b"properties" => properties = Some(v.to_string()),
                                _ => {}
                            }
                        }
                    }

                    if !id.is_empty() {
                        let normalized_href = normalize_path(opf_dir, &href);
                        if let Some(ref props) = properties {
                            if props.split_whitespace().any(|p| p == "nav") {
                                nav_xhtml_id = Some(id.clone());
                            }
                        }
                        manifest.insert(
                            id.clone(),
                            EpubManifestItem {
                                id,
                                href: normalized_href,
                                media_type,
                                properties,
                            },
                        );
                    }
                } else if in_spine && name == b"itemref" {
                    let mut idref = String::new();
                    let mut linear = true;

                    for attr in e.attributes().flatten() {
                        let k = local_name(attr.key.into_inner());
                        if let Ok(v) = attr.unescape_value() {
                            match k {
                                b"idref" => idref = v.to_string(),
                                b"linear" => linear = v != "no",
                                _ => {}
                            }
                        }
                    }

                    if !idref.is_empty() {
                        spine_raw.push((idref, linear));
                    }
                } else if in_metadata && name == b"meta" {
                    let mut meta_name = String::new();
                    let mut meta_content = String::new();
                    for attr in e.attributes().flatten() {
                        let k = local_name(attr.key.into_inner());
                        if let Ok(v) = attr.unescape_value() {
                            match k {
                                b"name" => meta_name = v.to_string(),
                                b"content" => meta_content = v.to_string(),
                                _ => {}
                            }
                        }
                    }
                    if meta_name == "cover" && !meta_content.is_empty() {
                        cover_meta_id = Some(meta_content);
                    }
                }
            }
            Event::Text(ref e) if in_metadata => {
                if let Ok(s) = e.unescape() {
                    text_buffer.push_str(&s);
                }
            }
            Event::CData(ref e) if in_metadata => {
                if let Ok(s) = std::str::from_utf8(e.as_ref()) {
                    text_buffer.push_str(s);
                }
            }
            Event::End(ref e) => {
                let name = local_name(e.name().into_inner());
                match name {
                    b"metadata" => in_metadata = false,
                    b"manifest" => in_manifest = false,
                    b"spine" => in_spine = false,
                    _ if in_metadata => {
                        let val = text_buffer.trim().to_string();
                        if !val.is_empty() {
                            match current_element.as_slice() {
                                b"title" if metadata.title.is_none() => {
                                    metadata.title = Some(val);
                                }
                                b"creator" => {
                                    metadata.authors.push(val);
                                }
                                b"publisher" if metadata.publisher.is_none() => {
                                    metadata.publisher = Some(val);
                                }
                                b"language" if metadata.language.is_none() => {
                                    metadata.language = Some(val);
                                }
                                b"description" if metadata.description.is_none() => {
                                    metadata.description = Some(val);
                                }
                                b"rights" if metadata.rights.is_none() => {
                                    metadata.rights = Some(val);
                                }
                                b"identifier" => {
                                    metadata.identifiers.push(val);
                                }
                                b"date" if metadata.publication_date.is_none() => {
                                    metadata.publication_date = Some(val);
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
                current_element.clear();
                text_buffer.clear();
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok((
        format,
        metadata,
        manifest,
        spine_raw,
        cover_meta_id,
        spine_toc_id,
        nav_xhtml_id,
    ))
}

/// Strips HTML/XML tags from a markup string to extract plain text paragraphs.
fn strip_html_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }

    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
