// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Streaming OpenXML (XLSX) Worksheet Extractor, Shared String Resolver,
//! Cell Coordinate Grid Builder, and Dynamic Formula Evaluation Engine.

use std::io::Cursor;

use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;

pub use super::formula::{evaluate_spreadsheet_formula, format_coordinate, parse_cell_coordinate};
use super::types::{UniFFICell, UniFFICellValue, UniFFIOfficeError, UniFFISheetData, UniFFISheetRow};
use crate::uniffi_api::xml_meta::office::{parse_app_properties_xml, parse_xlsx_workbook_sheets};
use crate::uniffi_api::xml_meta::types::UniFFIDocumentMetadata;
use crate::zip::reader::ZipArchive;

/// Extracts worksheet names from an XLSX archive byte slice.
pub fn extract_xlsx_sheet_names(bytes: &[u8]) -> Result<Vec<String>, UniFFIOfficeError> {
    let zip = ZipArchive::open_slice(bytes)
        .map_err(|e| UniFFIOfficeError::corrupted(format!("Failed to open XLSX archive: {e:?}")))?;

    // 1. Try reading xl/workbook.xml
    if let Some(wb_bytes) = find_zip_entry(&zip, "xl/workbook.xml") {
        let mut meta = UniFFIDocumentMetadata::default();
        meta.format_name = "XLSX".to_string();
        parse_xlsx_workbook_sheets(&wb_bytes, &mut meta);
        if !meta.sheet_names.is_empty() {
            return Ok(meta.sheet_names);
        }
    }

    // 2. Fallback to docProps/app.xml
    if let Some(app_bytes) = find_zip_entry(&zip, "docProps/app.xml") {
        let mut meta = UniFFIDocumentMetadata::default();
        meta.format_name = "XLSX".to_string();
        parse_app_properties_xml(&app_bytes, &mut meta);
        if !meta.sheet_names.is_empty() {
            return Ok(meta.sheet_names);
        }
    }

    // 3. Fallback to scanning xl/worksheets/sheet*.xml entries
    let mut detected_sheets = Vec::new();
    for entry in zip.entries() {
        let p = entry.rel_path.to_lowercase();
        if p.starts_with("xl/worksheets/sheet") && p.ends_with(".xml") {
            let filename = p.trim_start_matches("xl/worksheets/").trim_end_matches(".xml");
            let sheet_num = filename.trim_start_matches("sheet");
            let name = format!("Sheet{}", if sheet_num.is_empty() { "1" } else { sheet_num });
            if !detected_sheets.contains(&name) {
                detected_sheets.push(name);
            }
        }
    }

    if !detected_sheets.is_empty() {
        detected_sheets.sort();
        Ok(detected_sheets)
    } else {
        Ok(vec!["Sheet1".to_string()])
    }
}

/// Extracts sheet data and parsed cell grids from an XLSX archive byte slice.
pub fn extract_xlsx_sheet_data(
    bytes: &[u8],
    sheet_name_or_index: &str,
    max_rows: Option<u32>,
) -> Result<UniFFISheetData, UniFFIOfficeError> {
    let zip = ZipArchive::open_slice(bytes)
        .map_err(|e| UniFFIOfficeError::corrupted(format!("Failed to open XLSX archive: {e:?}")))?;

    // 1. Read shared strings pool (xl/sharedStrings.xml)
    let shared_strings = if let Some(sst_bytes) = find_zip_entry(&zip, "xl/sharedStrings.xml") {
        parse_shared_strings_xml(&sst_bytes)
    } else {
        Vec::new()
    };
    let sst_count = shared_strings.len() as u32;

    // 2. Discover sheets and target worksheet path
    let sheet_names = extract_xlsx_sheet_names(bytes)?;
    let (target_sheet_name, target_sheet_idx, target_path) =
        resolve_sheet_path(&zip, &sheet_names, sheet_name_or_index)?;

    // 3. Extract target sheet XML bytes
    let sheet_xml_bytes = find_zip_entry(&zip, &target_path).ok_or_else(|| {
        UniFFIOfficeError::sheet_not_found(format!("Worksheet path '{target_path}' not found"))
    })?;

    // 4. Parse worksheet grid
    let (dimension_ref, rows, max_cols) =
        parse_worksheet_xml(&sheet_xml_bytes, &shared_strings, max_rows)?;

    let total_rows = rows.len() as u32;

    Ok(UniFFISheetData {
        sheet_name: target_sheet_name,
        sheet_index: target_sheet_idx,
        total_rows,
        total_cols: max_cols,
        dimension_ref,
        rows,
        shared_strings_count: sst_count,
    })
}

// ============================================================================
// Internal XLSX Parsing Helpers
// ============================================================================

fn find_zip_entry<'a>(zip: &ZipArchive<'a>, target_path: &str) -> Option<Vec<u8>> {
    let target_norm = target_path.to_lowercase().replace('\\', "/");
    for (idx, entry) in zip.entries().iter().enumerate() {
        if entry.rel_path.to_lowercase().replace('\\', "/") == target_norm {
            return zip.extract_entry_bytes(idx, None).ok();
        }
    }
    None
}

fn resolve_sheet_path<'a>(
    zip: &ZipArchive<'a>,
    sheet_names: &[String],
    sheet_name_or_index: &str,
) -> Result<(String, u32, String), UniFFIOfficeError> {
    let trimmed = sheet_name_or_index.trim();

    // Check if numeric index (1-based)
    if let Ok(idx) = trimmed.parse::<usize>() {
        if idx >= 1 && idx <= sheet_names.len() {
            let name = &sheet_names[idx - 1];
            let path = format!("xl/worksheets/sheet{idx}.xml");
            if entry_exists(zip, &path) {
                return Ok((name.clone(), idx as u32, path));
            }
        }
    }

    // Match by exact or case-insensitive sheet name
    for (i, name) in sheet_names.iter().enumerate() {
        if name.eq_ignore_ascii_case(trimmed) {
            let idx = (i + 1) as u32;
            let path = format!("xl/worksheets/sheet{idx}.xml");
            if entry_exists(zip, &path) {
                return Ok((name.clone(), idx, path));
            }
            // Fallback: search for any sheetN.xml in archive
            for entry in zip.entries() {
                if entry.rel_path.starts_with("xl/worksheets/") && entry.rel_path.ends_with(".xml")
                {
                    return Ok((name.clone(), idx, entry.rel_path.clone()));
                }
            }
        }
    }

    // Fallback: Default to first worksheet found
    if let Some(first_name) = sheet_names.first() {
        let path = "xl/worksheets/sheet1.xml".to_string();
        if entry_exists(zip, &path) {
            return Ok((first_name.clone(), 1, path));
        }
        for entry in zip.entries() {
            if entry.rel_path.starts_with("xl/worksheets/") && entry.rel_path.ends_with(".xml") {
                return Ok((first_name.clone(), 1, entry.rel_path.clone()));
            }
        }
    }

    Err(UniFFIOfficeError::sheet_not_found(sheet_name_or_index))
}

fn entry_exists<'a>(zip: &ZipArchive<'a>, target_path: &str) -> bool {
    let target_norm = target_path.to_lowercase().replace('\\', "/");
    zip.entries()
        .iter()
        .any(|e| e.rel_path.to_lowercase().replace('\\', "/") == target_norm)
}

fn parse_shared_strings_xml(xml_bytes: &[u8]) -> Vec<String> {
    let mut reader = XmlReader::from_reader(Cursor::new(xml_bytes));
    reader.config_mut().trim_text(false);

    let mut strings = Vec::with_capacity(128);
    let mut buf = Vec::with_capacity(512);
    let mut in_si = false;
    let mut in_t = false;
    let mut current_string = String::with_capacity(64);

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.local_name().as_ref() {
                b"si" => {
                    in_si = true;
                    current_string.clear();
                }
                b"t" if in_si => {
                    in_t = true;
                }
                _ => {}
            },
            Ok(Event::Text(ref e)) if in_t => {
                if let Ok(txt) = e.unescape() {
                    current_string.push_str(&txt);
                }
            }
            Ok(Event::End(ref e)) => match e.local_name().as_ref() {
                b"t" => in_t = false,
                b"si" => {
                    in_si = false;
                    strings.push(current_string.clone());
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    strings
}

fn parse_worksheet_xml(
    xml_bytes: &[u8],
    shared_strings: &[String],
    max_rows: Option<u32>,
) -> Result<(Option<String>, Vec<UniFFISheetRow>, u32), UniFFIOfficeError> {
    let mut reader = XmlReader::from_reader(Cursor::new(xml_bytes));
    reader.config_mut().trim_text(true);

    let mut dimension_ref = None;
    let mut rows: Vec<UniFFISheetRow> = Vec::new();
    let mut max_col_found = 0u32;
    let limit_rows = max_rows.unwrap_or(u32::MAX);

    let mut buf = Vec::with_capacity(512);
    let mut current_row_num = 0u32;
    let mut current_cells = Vec::new();
    let mut in_row = false;

    // Current cell parsing state
    let mut current_coord = String::new();
    let mut current_cell_type = String::new();
    let mut current_formula: Option<String> = None;
    let mut current_val_text = String::new();
    let mut in_f = false;
    let mut in_v = false;
    let mut in_is_t = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"dimension" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"ref" {
                                if let Ok(val) = String::from_utf8(attr.value.to_vec()) {
                                    dimension_ref = Some(val);
                                }
                            }
                        }
                    }
                    b"row" => {
                        if rows.len() as u32 >= limit_rows {
                            break;
                        }
                        in_row = true;
                        current_cells.clear();
                        current_row_num = 0;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"r" {
                                if let Ok(val) = String::from_utf8(attr.value.to_vec()) {
                                    current_row_num = val.parse::<u32>().unwrap_or(0);
                                }
                            }
                        }
                        if current_row_num == 0 {
                            current_row_num = (rows.len() + 1) as u32;
                        }
                    }
                    b"c" if in_row => {
                        current_coord.clear();
                        current_cell_type.clear();
                        current_formula = None;
                        current_val_text.clear();
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"r" {
                                if let Ok(val) = String::from_utf8(attr.value.to_vec()) {
                                    current_coord = val;
                                }
                            } else if attr.key.as_ref() == b"t" {
                                if let Ok(val) = String::from_utf8(attr.value.to_vec()) {
                                    current_cell_type = val;
                                }
                            }
                        }
                    }
                    b"f" if in_row => {
                        in_f = true;
                        current_val_text.clear();
                    }
                    b"v" if in_row => {
                        in_v = true;
                        current_val_text.clear();
                    }
                    b"t" if in_row => {
                        in_is_t = true;
                        current_val_text.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                if local.as_ref() == b"dimension" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"ref" {
                            if let Ok(val) = String::from_utf8(attr.value.to_vec()) {
                                dimension_ref = Some(val);
                            }
                        }
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_f || in_v || in_is_t {
                    if let Ok(txt) = e.unescape() {
                        current_val_text.push_str(&txt);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"f" if in_row => {
                        in_f = false;
                        let trimmed = current_val_text.trim();
                        if !trimmed.is_empty() {
                            current_formula = Some(trimmed.to_string());
                        }
                        current_val_text.clear();
                    }
                    b"v" if in_row => {
                        in_v = false;
                    }
                    b"t" if in_row => {
                        in_is_t = false;
                    }
                    b"c" if in_row => {
                        let (row_idx, col_idx) = parse_cell_coordinate(&current_coord, current_row_num);
                        if col_idx > max_col_found {
                            max_col_found = col_idx;
                        }

                        let typed_value = build_typed_cell_value(
                            &current_cell_type,
                            &current_val_text,
                            current_formula.as_deref(),
                            shared_strings,
                        );

                        current_cells.push(UniFFICell {
                            row: row_idx,
                            col: col_idx,
                            coordinate: if current_coord.is_empty() {
                                format_coordinate(row_idx, col_idx)
                            } else {
                                current_coord.clone()
                            },
                            value: typed_value,
                            formula: current_formula.take(),
                        });
                        current_val_text.clear();
                    }
                    b"row" if in_row => {
                        in_row = false;
                        if !current_cells.is_empty() {
                            rows.push(UniFFISheetRow {
                                row_number: current_row_num,
                                cells: current_cells.clone(),
                            });
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(UniFFIOfficeError::xml_err(format!("XLSX XML error: {e:?}")));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok((dimension_ref, rows, max_col_found))
}

fn build_typed_cell_value(
    cell_type: &str,
    val_text: &str,
    formula: Option<&str>,
    shared_strings: &[String],
) -> UniFFICellValue {
    let trimmed = val_text.trim();

    if let Some(f_expr) = formula {
        let cached = if !trimmed.is_empty() {
            Some(trimmed.to_string())
        } else {
            None
        };
        return UniFFICellValue::Formula {
            expression: f_expr.to_string(),
            cached_value: cached,
        };
    }

    match cell_type {
        "s" => {
            if let Ok(idx) = trimmed.parse::<usize>() {
                if let Some(str_val) = shared_strings.get(idx) {
                    return UniFFICellValue::Text {
                        value: str_val.clone(),
                    };
                }
            }
            UniFFICellValue::Text {
                value: trimmed.to_string(),
            }
        }
        "b" => {
            let b = trimmed == "1" || trimmed.eq_ignore_ascii_case("true");
            UniFFICellValue::Boolean { value: b }
        }
        "e" => UniFFICellValue::Error {
            message: trimmed.to_string(),
        },
        "str" | "inlineStr" => UniFFICellValue::Text {
            value: trimmed.to_string(),
        },
        _ => {
            if trimmed.is_empty() {
                UniFFICellValue::Empty
            } else if let Ok(num) = trimmed.parse::<f64>() {
                UniFFICellValue::Number { value: num }
            } else {
                UniFFICellValue::Text {
                    value: trimmed.to_string(),
                }
            }
        }
    }
}

