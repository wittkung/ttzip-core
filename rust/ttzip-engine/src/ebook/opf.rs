// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Canonical OPF XML package parser, spine resolution, cover discovery, and TOC extraction.

use std::collections::HashMap;
use quick_xml::events::{BytesEnd, BytesStart, Event};

pub use crate::ebook::container::{is_epub_container, parse_container_rootfile};
use crate::ebook::container::strip_utf8_bom;
use crate::ebook::navigation::{local_name, EbookTocNode, SpineItem};
use crate::ebook::ncx::{parse_nav_xhtml, parse_ncx};
use crate::ebook::parser::{EbookFormat, EbookMetadata, EpubManifestItem};
use crate::ebook::resource::{normalize_path, EbookResourceExtractor};
use crate::ebook::EbookResult;
use crate::zip::ZipArchive;

/// Structured OPF package representation shared across all EPUB metadata, TOC, and stream parsers.
pub struct ParsedOpfPackage {
    pub format: EbookFormat,
    pub metadata: EbookMetadata,
    pub modified_date: Option<String>,
    pub extra_metadata: HashMap<String, String>,
    pub manifest: HashMap<String, EpubManifestItem>,
    pub spine_raw: Vec<(String, bool)>,
    pub cover_meta_id: Option<String>,
    pub spine_toc_id: Option<String>,
    pub nav_xhtml_id: Option<String>,
    pub ncx_href: Option<String>,
    pub nav_href: Option<String>,
}

struct OpfParseState {
    format: EbookFormat,
    metadata: EbookMetadata,
    modified_date: Option<String>,
    extra_metadata: HashMap<String, String>,
    manifest: HashMap<String, EpubManifestItem>,
    spine_raw: Vec<(String, bool)>,
    cover_meta_id: Option<String>,
    spine_toc_id: Option<String>,
    nav_xhtml_id: Option<String>,
    ncx_href: Option<String>,
    nav_href: Option<String>,
    in_metadata: bool,
    in_manifest: bool,
    in_spine: bool,
    in_dcterms_modified: bool,
    current_element: Vec<u8>,
    text_buffer: String,
}

impl Default for OpfParseState {
    fn default() -> Self {
        Self {
            format: EbookFormat::Epub2,
            metadata: EbookMetadata::default(),
            modified_date: None,
            extra_metadata: HashMap::new(),
            manifest: HashMap::new(),
            spine_raw: Vec::new(),
            cover_meta_id: None,
            spine_toc_id: None,
            nav_xhtml_id: None,
            ncx_href: None,
            nav_href: None,
            in_metadata: false,
            in_manifest: false,
            in_spine: false,
            in_dcterms_modified: false,
            current_element: Vec::new(),
            text_buffer: String::new(),
        }
    }
}

impl OpfParseState {
    fn on_start_tag(&mut self, e: &BytesStart<'_>, opf_dir: &str) {
        let name = local_name(e.name().into_inner()).to_vec();
        match name.as_slice() {
            b"package" => self.parse_package_version(e),
            b"metadata" => self.in_metadata = true,
            b"manifest" => self.in_manifest = true,
            b"spine" => {
                self.in_spine = true;
                self.parse_spine_toc_attr(e);
            }
            b"item" if self.in_manifest => self.parse_manifest_item(e, opf_dir),
            b"itemref" if self.in_spine => self.parse_spine_itemref(e),
            b"meta" if self.in_metadata => self.parse_metadata_meta(e, false),
            _ => {}
        }
        self.current_element = name;
        self.text_buffer.clear();
    }

    fn parse_package_version(&mut self, e: &BytesStart<'_>) {
        for attr in e.attributes().flatten() {
            if local_name(attr.key.into_inner()) == b"version"
                && let Ok(v) = attr.unescape_value()
            {
                self.format = if v.starts_with('3') {
                    EbookFormat::Epub3
                } else {
                    EbookFormat::Epub2
                };
            }
        }
    }

    fn parse_spine_toc_attr(&mut self, e: &BytesStart<'_>) {
        for attr in e.attributes().flatten() {
            if local_name(attr.key.into_inner()) == b"toc"
                && let Ok(v) = attr.unescape_value()
            {
                self.spine_toc_id = Some(v.to_string());
            }
        }
    }

    fn on_empty_tag(&mut self, e: &BytesStart<'_>, opf_dir: &str) {
        let name = local_name(e.name().into_inner());
        if self.in_manifest && name == b"item" {
            self.parse_manifest_item(e, opf_dir);
        } else if self.in_spine && name == b"itemref" {
            self.parse_spine_itemref(e);
        } else if self.in_metadata && name == b"meta" {
            self.parse_metadata_meta(e, true);
        }
    }

    fn parse_manifest_item(&mut self, e: &BytesStart<'_>, opf_dir: &str) {
        let mut id = String::new();
        let mut href = String::new();
        let mut media_type = String::new();
        let mut properties: Option<String> = None;

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

        if id.is_empty() {
            return;
        }
        let normalized_href = normalize_path(opf_dir, &href);
        self.classify_manifest_item(&id, &href, &normalized_href, &media_type, properties.as_deref());
        self.manifest.insert(
            id.clone(),
            EpubManifestItem {
                id,
                href: normalized_href,
                media_type,
                properties,
            },
        );
    }

    fn classify_manifest_item(
        &mut self,
        id: &str,
        raw_href: &str,
        normalized_href: &str,
        media_type: &str,
        properties: Option<&str>,
    ) {
        if media_type == "application/x-dtbncx+xml" || raw_href.to_lowercase().ends_with(".ncx") {
            self.ncx_href = Some(normalized_href.to_string());
        }
        let has_nav_prop = properties.is_some_and(|p| p.split_whitespace().any(|tok| tok == "nav"));
        if has_nav_prop {
            self.nav_xhtml_id = Some(id.to_string());
            self.nav_href = Some(normalized_href.to_string());
        }
    }

    fn parse_spine_itemref(&mut self, e: &BytesStart<'_>) {
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
            self.spine_raw.push((idref, linear));
        }
    }

    fn parse_metadata_meta(&mut self, e: &BytesStart<'_>, is_empty: bool) {
        let mut meta_name = String::new();
        let mut meta_content = String::new();
        let mut meta_property = String::new();
        for attr in e.attributes().flatten() {
            let k = local_name(attr.key.into_inner());
            if let Ok(v) = attr.unescape_value() {
                match k {
                    b"name" => meta_name = v.to_string(),
                    b"content" => meta_content = v.to_string(),
                    b"property" => meta_property = v.to_string(),
                    _ => {}
                }
            }
        }
        if meta_name == "cover" && !meta_content.is_empty() {
            self.cover_meta_id = Some(meta_content.clone());
        }
        if !meta_name.is_empty() && !meta_content.is_empty() {
            self.extra_metadata.insert(meta_name, meta_content.clone());
        }
        if meta_property == "dcterms:modified" {
            if is_empty && !meta_content.is_empty() {
                self.modified_date = Some(meta_content);
            } else {
                self.in_dcterms_modified = true;
            }
        }
    }

    fn on_end_tag(&mut self, e: &BytesEnd<'_>) {
        let local = local_name(e.name().into_inner());
        match local {
            b"metadata" => self.in_metadata = false,
            b"manifest" => self.in_manifest = false,
            b"spine" => self.in_spine = false,
            _ if self.in_metadata => {
                let val = self.text_buffer.trim().to_string();
                if self.in_dcterms_modified && local == b"meta" {
                    self.in_dcterms_modified = false;
                    if !val.is_empty() {
                        self.modified_date = Some(val);
                    }
                } else if !val.is_empty() {
                    self.commit_metadata_value(val);
                }
            }
            _ => {}
        }
        self.current_element.clear();
        self.text_buffer.clear();
    }

    fn commit_metadata_value(&mut self, val: String) {
        match self.current_element.as_slice() {
            b"title" => set_if_empty(&mut self.metadata.title, val),
            b"creator" => self.metadata.authors.push(val),
            b"publisher" => set_if_empty(&mut self.metadata.publisher, val),
            b"language" => set_if_empty(&mut self.metadata.language, val),
            b"description" => set_if_empty(&mut self.metadata.description, val),
            b"rights" => set_if_empty(&mut self.metadata.rights, val),
            b"identifier" => self.metadata.identifiers.push(val),
            b"date" => set_if_empty(&mut self.metadata.publication_date, val),
            b"modified" => set_if_empty(&mut self.modified_date, val),
            b"meta" => {}
            other => {
                if let Ok(tag) = std::str::from_utf8(other) {
                    self.extra_metadata.insert(tag.to_string(), val);
                }
            }
        }
    }

    fn append_metadata_text(&mut self, text: Option<String>) {
        if self.in_metadata
            && let Some(s) = text
        {
            self.text_buffer.push_str(&s);
        }
    }

    fn into_parsed_package(mut self) -> ParsedOpfPackage {
        resolve_cover_href(
            &mut self.metadata,
            self.cover_meta_id.clone(),
            &self.manifest,
        );

        if self.ncx_href.is_none()
            && let Some(ref toc_id) = self.spine_toc_id
            && let Some(item) = self.manifest.get(toc_id)
        {
            self.ncx_href = Some(item.href.clone());
        }

        ParsedOpfPackage {
            format: self.format,
            metadata: self.metadata,
            modified_date: self.modified_date,
            extra_metadata: self.extra_metadata,
            manifest: self.manifest,
            spine_raw: self.spine_raw,
            cover_meta_id: self.cover_meta_id,
            spine_toc_id: self.spine_toc_id,
            nav_xhtml_id: self.nav_xhtml_id,
            ncx_href: self.ncx_href,
            nav_href: self.nav_href,
        }
    }
}

fn set_if_empty(slot: &mut Option<String>, val: String) {
    if slot.is_none() {
        *slot = Some(val);
    }
}

/// Parses an OPF package XML document into structured format, metadata, manifest, and spine data.
pub fn parse_opf_xml(xml: &[u8], opf_dir: &str) -> EbookResult<ParsedOpfPackage> {
    let mut reader = quick_xml::Reader::from_reader(strip_utf8_bom(xml));
    reader.config_mut().trim_text(true);

    let mut state = OpfParseState::default();
    let mut buf = Vec::with_capacity(512);

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) => state.on_start_tag(e, opf_dir),
            Event::Empty(ref e) => state.on_empty_tag(e, opf_dir),
            Event::Text(e) => state.append_metadata_text(e.unescape().ok().map(|s| s.into_owned())),
            Event::CData(e) => {
                state.append_metadata_text(std::str::from_utf8(e.as_ref()).ok().map(String::from));
            }
            Event::End(ref e) => state.on_end_tag(e),
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(state.into_parsed_package())
}

fn is_html_manifest_item(item: &EpubManifestItem) -> bool {
    item.media_type == "application/xhtml+xml"
        || item.media_type == "text/html"
        || item.href.ends_with(".xhtml")
        || item.href.ends_with(".html")
}

fn is_html_archive_path(path: &str) -> bool {
    path.ends_with(".xhtml") || path.ends_with(".html") || path.ends_with(".htm")
}

/// Resolves spine `<itemref>` entries against the OPF manifest with HTML fallback discovery.
pub fn resolve_epub_spine(
    spine_raw: Vec<(String, bool)>,
    manifest: &HashMap<String, EpubManifestItem>,
    zip: &ZipArchive<'_>,
) -> Vec<SpineItem> {
    let mut spine: Vec<SpineItem> = spine_raw
        .into_iter()
        .filter_map(|(idref, linear)| {
            let item = manifest.get(&idref)?;
            Some(SpineItem {
                idref,
                href: item.href.clone(),
                linear,
                media_type: item.media_type.clone(),
            })
        })
        .collect();

    if !spine.is_empty() {
        return spine;
    }

    for (id, item) in manifest {
        if is_html_manifest_item(item) {
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
            if is_html_archive_path(&entry.rel_path) {
                spine.push(SpineItem {
                    idref: entry.rel_path.clone(),
                    href: entry.rel_path.clone(),
                    linear: true,
                    media_type: "application/xhtml+xml".to_string(),
                });
            }
        }
    }
    spine.sort_by(|a, b| a.href.cmp(&b.href));
    spine
}

/// Populates `metadata.cover_image_href` from `<meta name="cover">`, `properties="cover-image"`, or conventional IDs.
pub fn resolve_cover_href(
    metadata: &mut EbookMetadata,
    cover_meta_id: Option<String>,
    manifest: &HashMap<String, EpubManifestItem>,
) {
    if metadata.cover_image_href.is_none()
        && let Some(cover_id) = cover_meta_id
        && let Some(item) = manifest.get(&cover_id)
    {
        metadata.cover_image_href = Some(item.href.clone());
    }
    if metadata.cover_image_href.is_none() {
        metadata.cover_image_href = find_manifest_href_by_property(manifest, "cover-image");
    }
    if metadata.cover_image_href.is_none() {
        metadata.cover_image_href = ["cover", "cover-image"]
            .into_iter()
            .filter_map(|id| manifest.get(id))
            .find(|item| {
                !is_html_manifest_item(item)
                    && (item.media_type.is_empty() || item.media_type.starts_with("image/"))
            })
            .map(|item| item.href.clone());
    }
}

fn find_manifest_href_by_property(
    manifest: &HashMap<String, EpubManifestItem>,
    prop_name: &str,
) -> Option<String> {
    manifest
        .values()
        .find(|item| {
            item.properties
                .as_deref()
                .is_some_and(|props| props.split_whitespace().any(|p| p == prop_name))
        })
        .map(|item| item.href.clone())
}

fn parent_dir_or_fallback<'a>(path: &'a str, fallback: &'a str) -> &'a str {
    match path.rfind('/') {
        Some(pos) => &path[..pos],
        None => fallback,
    }
}

/// Extracts the hierarchical EPUB TOC from NAV XHTML or NCX.
pub fn extract_epub_toc(
    zip: &ZipArchive<'_>,
    opf_dir: &str,
    manifest: &HashMap<String, EpubManifestItem>,
    spine: &[SpineItem],
    nav_xhtml_id: Option<String>,
    spine_toc_id: Option<String>,
) -> Vec<EbookTocNode> {
    let nav_href = nav_xhtml_id
        .and_then(|id| manifest.get(&id).map(|item| item.href.clone()))
        .or_else(|| find_manifest_href_by_property(manifest, "nav"));

    if let Some(nav_path) = nav_href
        && let Ok(nav_bytes) = EbookResourceExtractor::extract_resource(zip, &nav_path, None)
    {
        let nav_dir = parent_dir_or_fallback(&nav_path, opf_dir);
        if let Ok(nodes) = parse_nav_xhtml(&nav_bytes.data, nav_dir, spine)
            && !nodes.is_empty()
        {
            return nodes;
        }
    }

    let ncx_href = spine_toc_id
        .and_then(|id| manifest.get(&id).map(|item| item.href.clone()))
        .or_else(|| {
            manifest
                .values()
                .find(|item| item.media_type == "application/x-dtbncx+xml")
                .map(|item| item.href.clone())
        });

    if let Some(ncx_path) = ncx_href
        && let Ok(ncx_bytes) = EbookResourceExtractor::extract_resource(zip, &ncx_path, None)
    {
        let ncx_dir = parent_dir_or_fallback(&ncx_path, opf_dir);
        if let Ok(nodes) = parse_ncx(&ncx_bytes.data, ncx_dir, spine) {
            return nodes;
        }
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_opf_xml_cover_image_property_outweighs_xhtml_cover_id() {
        let opf_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Cover Precedence Test</dc:title>
  </metadata>
  <manifest>
    <item id="cover" href="Text/cover.xhtml" media-type="application/xhtml+xml"/>
    <item id="cover-img" href="Images/cover.png" media-type="image/png" properties="cover-image"/>
  </manifest>
  <spine>
    <itemref idref="cover"/>
  </spine>
</package>"#;

        let parsed = parse_opf_xml(opf_xml, "OEBPS").expect("OPF XML should parse");
        assert_eq!(parsed.cover_meta_id, None);
        assert_eq!(
            parsed.metadata.cover_image_href.as_deref(),
            Some("OEBPS/Images/cover.png")
        );
    }

    #[test]
    fn test_parse_opf_xml_ignores_non_image_conventional_cover_id() {
        let opf_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>No Cover Image Test</dc:title>
  </metadata>
  <manifest>
    <item id="cover" href="Text/cover.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="cover"/>
  </spine>
</package>"#;

        let parsed = parse_opf_xml(opf_xml, "OEBPS").expect("OPF XML should parse");
        assert_eq!(parsed.metadata.cover_image_href, None);
    }
}
