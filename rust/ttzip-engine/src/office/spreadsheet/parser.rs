// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust XLSX, XLS, XLSB, and ODS spreadsheet parser.
//!
//! Provides zero-copy archive unpacking, Shared String Table (SST) parsing,
//! sheet discovery, and high-throughput cell matrix reading.

use std::collections::HashMap;
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::office::types::{OfficeCellAddress, OfficeCellValue, OfficeError, OfficeFormat, OfficeRange, OfficeResult};
use crate::zip::ZipArchive;

/// High-throughput spreadsheet parser supporting OpenXML (XLSX) and OpenDocument (ODS).
pub struct TTZipSpreadsheetParser<'a> {
    format: OfficeFormat,
    sheet_order: Vec<String>,
    sheet_paths: HashMap<String, String>,
    shared_strings: Vec<String>,
    zip: Option<ZipArchive<'a>>,
    ods_content: Option<String>,
}

impl<'a> TTZipSpreadsheetParser<'a> {
    /// Opens and indexes a spreadsheet container from a byte slice.
    pub fn open_from_bytes(data: &'a [u8]) -> OfficeResult<Self> {
        if data.len() < 4 {
            return Err(OfficeError::UnsupportedFormat("Data payload too short".to_string()));
        }

        // 1. Check for standard ZIP container (PK\x03\x04)
        if data.starts_with(b"PK\x03\x04") || data.starts_with(b"PK\x05\x06") {
            let zip = ZipArchive::open_slice(data).map_err(OfficeError::Zip)?;
            
            // Check if XLSX or ODS
            let mut is_xlsx = false;
            let mut is_ods = false;

            for entry in zip.entries() {
                let name = &entry.rel_path;
                if name.starts_with("xl/") || name == "[Content_Types].xml" {
                    is_xlsx = true;
                }
                if name == "content.xml" || name.starts_with("mimetype") {
                    is_ods = true;
                }
            }

            if is_xlsx {
                let mut parser = Self {
                    format: OfficeFormat::Xlsx,
                    sheet_order: Vec::new(),
                    sheet_paths: HashMap::new(),
                    shared_strings: Vec::new(),
                    zip: Some(zip),
                    ods_content: None,
                };
                parser.init_xlsx()?;
                return Ok(parser);
            } else if is_ods {
                let mut parser = Self {
                    format: OfficeFormat::Ods,
                    sheet_order: Vec::new(),
                    sheet_paths: HashMap::new(),
                    shared_strings: Vec::new(),
                    zip: Some(zip),
                    ods_content: None,
                };
                parser.init_ods()?;
                return Ok(parser);
            }
        }

        Err(OfficeError::UnsupportedFormat("Unrecognized spreadsheet container format".to_string()))
    }

    /// Returns list of sheet names in the workbook.
    pub fn sheet_names(&self) -> Vec<String> {
        self.sheet_order.clone()
    }

    /// Reads a full sheet range by name.
    pub fn read_sheet(&mut self, name: &str) -> OfficeResult<OfficeRange> {
        let rows = self.read_sheet_rows_stream(name)?;
        if rows.is_empty() {
            return Ok(OfficeRange::empty(name));
        }

        let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let row_count = rows.len() as u32;
        let col_count = max_cols.max(1) as u32;

        let start = OfficeCellAddress::from_row_col(0, 0);
        let end = OfficeCellAddress::from_row_col(row_count.saturating_sub(1), col_count.saturating_sub(1));

        Ok(OfficeRange {
            sheet_name: name.to_string(),
            start,
            end,
            values: rows,
        })
    }

    /// Reads sheet rows as a 2D matrix of cell values.
    pub fn read_sheet_rows_stream(&self, name: &str) -> OfficeResult<Vec<Vec<OfficeCellValue>>> {
        match self.format {
            OfficeFormat::Xlsx => self.read_xlsx_sheet_rows(name),
            OfficeFormat::Ods => self.read_ods_sheet_rows(name),
            _ => Err(OfficeError::UnsupportedFormat(format!("Unsupported format {:?}", self.format))),
        }
    }

    // --- XLSX Parsing Helpers ---

    fn init_xlsx(&mut self) -> OfficeResult<()> {
        let zip = self.zip.as_ref().ok_or_else(|| OfficeError::Corrupt("Missing ZIP reader".to_string()))?;

        // 1. Extract shared strings if present
        if let Some((idx, _)) = zip.entries().iter().enumerate().find(|(_, e)| e.rel_path == "xl/sharedStrings.xml") {
            let sst_bytes = zip.extract_entry_bytes(idx, None).map_err(OfficeError::Zip)?;
            self.shared_strings = parse_shared_strings_xml(&sst_bytes)?;
        }

        // 2. Parse relationships to map r:id to path
        let mut rels_map: HashMap<String, String> = HashMap::new();
        if let Some((idx, _)) = zip.entries().iter().enumerate().find(|(_, e)| e.rel_path == "xl/_rels/workbook.xml.rels") {
            let rels_bytes = zip.extract_entry_bytes(idx, None).map_err(OfficeError::Zip)?;
            rels_map = parse_relationships_xml(&rels_bytes)?;
        }

        // 3. Parse workbook sheets
        if let Some((idx, _)) = zip.entries().iter().enumerate().find(|(_, e)| e.rel_path == "xl/workbook.xml") {
            let wb_bytes = zip.extract_entry_bytes(idx, None).map_err(OfficeError::Zip)?;
            let sheets = parse_workbook_xml(&wb_bytes)?;
            for (sheet_name, r_id) in sheets {
                self.sheet_order.push(sheet_name.clone());
                let target_path = if let Some(target) = rels_map.get(&r_id) {
                    if target.starts_with("worksheets/") || target.starts_with("xl/") {
                        if target.starts_with("xl/") {
                            target.clone()
                        } else {
                            format!("xl/{target}")
                        }
                    } else {
                        format!("xl/{target}")
                    }
                } else {
                    format!("xl/worksheets/sheet{}.xml", self.sheet_order.len())
                };
                self.sheet_paths.insert(sheet_name, target_path);
            }
        }

        // Fallback: discover sheet xmls directly if workbook.xml parsing didn't find any
        if self.sheet_order.is_empty() {
            let mut sheet_files = Vec::new();
            for entry in zip.entries() {
                if entry.rel_path.starts_with("xl/worksheets/sheet") && entry.rel_path.ends_with(".xml") {
                    sheet_files.push(entry.rel_path.clone());
                }
            }
            sheet_files.sort();
            for (i, path) in sheet_files.into_iter().enumerate() {
                let name = format!("Sheet{}", i + 1);
                self.sheet_order.push(name.clone());
                self.sheet_paths.insert(name, path);
            }
        }

        Ok(())
    }

    fn read_xlsx_sheet_rows(&self, name: &str) -> OfficeResult<Vec<Vec<OfficeCellValue>>> {
        let path = self.sheet_paths.get(name).ok_or_else(|| OfficeError::SheetNotFound(name.to_string()))?;
        let zip = self.zip.as_ref().ok_or_else(|| OfficeError::Corrupt("Missing ZIP reader".to_string()))?;
        
        let entry_idx = zip
            .entries()
            .iter()
            .position(|e| e.rel_path == *path || e.rel_path == path.trim_start_matches('/'))
            .ok_or_else(|| OfficeError::SheetNotFound(format!("{name} ({path})")))?;

        let sheet_bytes = zip.extract_entry_bytes(entry_idx, None).map_err(OfficeError::Zip)?;
        parse_xlsx_sheet_xml(&sheet_bytes, &self.shared_strings)
    }

    // --- ODS Parsing Helpers ---

    fn init_ods(&mut self) -> OfficeResult<()> {
        let zip = self.zip.as_ref().ok_or_else(|| OfficeError::Corrupt("Missing ZIP reader".to_string()))?;
        let entry_idx = zip
            .entries()
            .iter()
            .position(|e| e.rel_path == "content.xml")
            .ok_or_else(|| OfficeError::Corrupt("Missing ODS content.xml".to_string()))?;

        let content_bytes = zip.extract_entry_bytes(entry_idx, None).map_err(OfficeError::Zip)?;
        let xml_str = String::from_utf8(content_bytes).map_err(|e| OfficeError::Utf8(e.utf8_error()))?;
        
        let mut reader = Reader::from_str(&xml_str);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let local = local_name(e.name().into_inner());
                    if local == b"table" {
                        for attr in e.attributes().flatten() {
                            let attr_local = local_name(attr.key.into_inner());
                            if attr_local == b"name" {
                                if let Ok(val) = std::str::from_utf8(&attr.value) {
                                    self.sheet_order.push(val.to_string());
                                }
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(OfficeError::Xml(e)),
                _ => {}
            }
            buf.clear();
        }

        self.ods_content = Some(xml_str);
        Ok(())
    }

    fn read_ods_sheet_rows(&self, name: &str) -> OfficeResult<Vec<Vec<OfficeCellValue>>> {
        let content = self.ods_content.as_ref().ok_or_else(|| OfficeError::Corrupt("Missing ODS content".to_string()))?;
        parse_ods_sheet_xml(content, name)
    }
}

#[inline]
fn local_name(name: &[u8]) -> &[u8] {
    if let Some(pos) = name.iter().rposition(|&b| b == b':') {
        &name[pos + 1..]
    } else {
        name
    }
}

fn parse_shared_strings_xml(xml_bytes: &[u8]) -> OfficeResult<Vec<String>> {
    let mut reader = Reader::from_reader(xml_bytes);
    reader.config_mut().trim_text(false);

    let mut shared_strings = Vec::new();
    let mut current_string = String::new();
    let mut in_t = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = local_name(e.name().into_inner());
                if local == b"si" {
                    current_string.clear();
                } else if local == b"t" {
                    in_t = true;
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_t {
                    if let Ok(text) = e.unescape() {
                        current_string.push_str(&text);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = local_name(e.name().into_inner());
                if local == b"t" {
                    in_t = false;
                } else if local == b"si" {
                    shared_strings.push(current_string.clone());
                    current_string.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(OfficeError::Xml(e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(shared_strings)
}

fn parse_relationships_xml(xml_bytes: &[u8]) -> OfficeResult<HashMap<String, String>> {
    let mut reader = Reader::from_reader(xml_bytes);
    reader.config_mut().trim_text(true);
    let mut rels = HashMap::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = local_name(e.name().into_inner());
                if local == b"Relationship" {
                    let mut id = String::new();
                    let mut target = String::new();
                    for attr in e.attributes().flatten() {
                        let k = local_name(attr.key.into_inner());
                        if k == b"Id" {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                id = v.to_string();
                            }
                        } else if k == b"Target" {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                target = v.to_string();
                            }
                        }
                    }
                    if !id.is_empty() && !target.is_empty() {
                        rels.insert(id, target);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(OfficeError::Xml(e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(rels)
}

fn parse_workbook_xml(xml_bytes: &[u8]) -> OfficeResult<Vec<(String, String)>> {
    let mut reader = Reader::from_reader(xml_bytes);
    reader.config_mut().trim_text(true);
    let mut sheets = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = local_name(e.name().into_inner());
                if local == b"sheet" {
                    let mut name = String::new();
                    let mut r_id = String::new();
                    for attr in e.attributes().flatten() {
                        let k = local_name(attr.key.into_inner());
                        if k == b"name" {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                name = v.to_string();
                            }
                        } else if k == b"id" {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                r_id = v.to_string();
                            }
                        }
                    }
                    if !name.is_empty() {
                        sheets.push((name, r_id));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(OfficeError::Xml(e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(sheets)
}

fn parse_xlsx_sheet_xml(xml_bytes: &[u8], shared_strings: &[String]) -> OfficeResult<Vec<Vec<OfficeCellValue>>> {
    let mut reader = Reader::from_reader(xml_bytes);
    reader.config_mut().trim_text(true);

    let mut cell_map: HashMap<(u32, u32), OfficeCellValue> = HashMap::new();
    let mut max_row: u32 = 0;
    let mut max_col: u32 = 0;

    let mut current_row: u32 = 0;
    let mut current_col: u32 = 0;
    let mut current_cell_type = String::new();
    let mut current_val = String::new();
    let mut in_v = false;
    let mut in_is_t = false;
    let mut is_text = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = local_name(e.name().into_inner());
                if local == b"row" {
                    for attr in e.attributes().flatten() {
                        if local_name(attr.key.into_inner()) == b"r" {
                            if let Ok(s) = std::str::from_utf8(&attr.value) {
                                if let Ok(r) = s.parse::<u32>() {
                                    current_row = r.saturating_sub(1);
                                }
                            }
                        }
                    }
                } else if local == b"c" {
                    current_cell_type.clear();
                    current_val.clear();
                    is_text.clear();
                    for attr in e.attributes().flatten() {
                        let k = local_name(attr.key.into_inner());
                        if k == b"r" {
                            if let Ok(s) = std::str::from_utf8(&attr.value) {
                                if let Ok(addr) = OfficeCellAddress::from_a1(s) {
                                    current_row = addr.row;
                                    current_col = addr.col;
                                }
                            }
                        } else if k == b"t" {
                            if let Ok(s) = std::str::from_utf8(&attr.value) {
                                current_cell_type = s.to_string();
                            }
                        }
                    }
                } else if local == b"v" {
                    in_v = true;
                    current_val.clear();
                } else if local == b"t" {
                    in_is_t = true;
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_v {
                    if let Ok(t) = e.unescape() {
                        current_val.push_str(&t);
                    }
                } else if in_is_t {
                    if let Ok(t) = e.unescape() {
                        is_text.push_str(&t);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = local_name(e.name().into_inner());
                if local == b"v" {
                    in_v = false;
                } else if local == b"t" {
                    in_is_t = false;
                } else if local == b"c" {
                    let cell_value = match current_cell_type.as_str() {
                        "s" => {
                            if let Ok(idx) = current_val.trim().parse::<usize>() {
                                if let Some(str_val) = shared_strings.get(idx) {
                                    OfficeCellValue::String(str_val.clone())
                                } else {
                                    OfficeCellValue::String(current_val.clone())
                                }
                            } else {
                                OfficeCellValue::String(current_val.clone())
                            }
                        }
                        "inlineStr" => OfficeCellValue::String(is_text.clone()),
                        "b" => {
                            let b = current_val.trim() == "1" || current_val.trim().eq_ignore_ascii_case("true");
                            OfficeCellValue::Bool(b)
                        }
                        "e" => OfficeCellValue::Error(current_val.clone()),
                        "str" => OfficeCellValue::String(current_val.clone()),
                        _ => {
                            let val_trim = current_val.trim();
                            if val_trim.is_empty() {
                                OfficeCellValue::Empty
                            } else if let Ok(i) = val_trim.parse::<i64>() {
                                OfficeCellValue::Int(i)
                            } else if let Ok(f) = val_trim.parse::<f64>() {
                                OfficeCellValue::Float(f)
                            } else {
                                OfficeCellValue::String(val_trim.to_string())
                            }
                        }
                    };

                    if !cell_value.is_empty() {
                        if current_row > max_row {
                            max_row = current_row;
                        }
                        if current_col > max_col {
                            max_col = current_col;
                        }
                        cell_map.insert((current_row, current_col), cell_value);
                    }
                    current_col += 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(OfficeError::Xml(e)),
            _ => {}
        }
        buf.clear();
    }

    if cell_map.is_empty() {
        return Ok(Vec::new());
    }

    let total_rows = (max_row + 1) as usize;
    let total_cols = (max_col + 1) as usize;

    let mut result = vec![vec![OfficeCellValue::Empty; total_cols]; total_rows];
    for ((r, c), val) in cell_map {
        if (r as usize) < total_rows && (c as usize) < total_cols {
            result[r as usize][c as usize] = val;
        }
    }

    Ok(result)
}

fn parse_ods_sheet_xml(content: &str, target_sheet: &str) -> OfficeResult<Vec<Vec<OfficeCellValue>>> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut in_target_table = false;
    let mut rows: Vec<Vec<OfficeCellValue>> = Vec::new();
    let mut current_row: Vec<OfficeCellValue> = Vec::new();
    let mut current_cell_text = String::new();
    let mut current_val_type = String::new();
    let mut current_val_attr = String::new();
    let mut in_cell = false;
    let mut in_text = false;
    let mut cell_repeat: usize = 1;
    let mut row_repeat: usize = 1;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = local_name(e.name().into_inner());
                if local == b"table" {
                    let mut name = String::new();
                    for attr in e.attributes().flatten() {
                        if local_name(attr.key.into_inner()) == b"name" {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                name = v.to_string();
                            }
                        }
                    }
                    if name == target_sheet {
                        in_target_table = true;
                    }
                } else if in_target_table && local == b"table-row" {
                    current_row.clear();
                    row_repeat = 1;
                    for attr in e.attributes().flatten() {
                        if local_name(attr.key.into_inner()) == b"number-rows-repeated" {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                row_repeat = v.parse::<usize>().unwrap_or(1).min(1024);
                            }
                        }
                    }
                } else if in_target_table && local == b"table-cell" {
                    in_cell = true;
                    cell_repeat = 1;
                    current_cell_text.clear();
                    current_val_type.clear();
                    current_val_attr.clear();

                    for attr in e.attributes().flatten() {
                        let k = local_name(attr.key.into_inner());
                        if k == b"number-columns-repeated" {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                cell_repeat = v.parse::<usize>().unwrap_or(1).min(1024);
                            }
                        } else if k == b"value-type" {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                current_val_type = v.to_string();
                            }
                        } else if k == b"value" || k == b"string-value" {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                current_val_attr = v.to_string();
                            }
                        }
                    }
                } else if in_cell && (local == b"p" || local == b"text-p") {
                    in_text = true;
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_text {
                    if let Ok(t) = e.unescape() {
                        current_cell_text.push_str(&t);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = local_name(e.name().into_inner());
                if local == b"p" || local == b"text-p" {
                    in_text = false;
                } else if local == b"table-cell" {
                    in_cell = false;
                    let cell_value = if !current_val_attr.is_empty() {
                        if current_val_type == "float" || current_val_type == "currency" || current_val_type == "percentage" {
                            if let Ok(i) = current_val_attr.parse::<i64>() {
                                OfficeCellValue::Int(i)
                            } else if let Ok(f) = current_val_attr.parse::<f64>() {
                                OfficeCellValue::Float(f)
                            } else {
                                OfficeCellValue::String(current_val_attr.clone())
                            }
                        } else if current_val_type == "boolean" {
                            OfficeCellValue::Bool(current_val_attr == "true")
                        } else {
                            OfficeCellValue::String(current_val_attr.clone())
                        }
                    } else if !current_cell_text.is_empty() {
                        if let Ok(i) = current_cell_text.parse::<i64>() {
                            OfficeCellValue::Int(i)
                        } else if let Ok(f) = current_cell_text.parse::<f64>() {
                            OfficeCellValue::Float(f)
                        } else {
                            OfficeCellValue::String(current_cell_text.clone())
                        }
                    } else {
                        OfficeCellValue::Empty
                    };

                    for _ in 0..cell_repeat {
                        current_row.push(cell_value.clone());
                    }
                } else if in_target_table && local == b"table-row" {
                    // Trim trailing empty cells to save space
                    while current_row.last().map(|c| c.is_empty()).unwrap_or(false) {
                        current_row.pop();
                    }
                    if !current_row.is_empty() {
                        for _ in 0..row_repeat {
                            rows.push(current_row.clone());
                        }
                    }
                } else if local == b"table" && in_target_table {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(OfficeError::Xml(e)),
            _ => {}
        }
        buf.clear();
    }

    if rows.is_empty() {
        return Ok(Vec::new());
    }

    // Normalize column counts
    let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    for row in &mut rows {
        while row.len() < max_cols {
            row.push(OfficeCellValue::Empty);
        }
    }

    Ok(rows)
}
