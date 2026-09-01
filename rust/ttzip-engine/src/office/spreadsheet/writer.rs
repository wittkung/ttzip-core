// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust XLSX spreadsheet builder and binary packager.
//!
//! Generates fully compliant Microsoft Excel OpenXML (.xlsx) files with shared strings,
//! formulas, multi-sheet structures, styles, and high-throughput ZIP assembly.

use std::collections::HashMap;
use crate::office::types::{col_to_a1, OfficeCellValue, OfficeError, OfficeResult};
use crate::types::TTZipEncryptionMethod;
use crate::zip::writer::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

/// Cell formatting options for XLSX writer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellFormat {
    pub bold: bool,
    pub italic: bool,
    pub number_format: Option<String>,
}

/// Internal cell payload representation.
#[derive(Debug, Clone, Default)]
struct CellRecord {
    value: OfficeCellValue,
    formula: Option<String>,
    format: CellFormat,
}

/// In-memory worksheet data.
#[derive(Debug, Clone, Default)]
struct WorksheetData {
    name: String,
    cells: HashMap<(u32, u32), CellRecord>,
}

/// Pure Safe Rust XLSX Spreadsheet Writer.
#[derive(Debug, Default)]
pub struct TTZipSpreadsheetWriter {
    sheets: Vec<WorksheetData>,
}

impl TTZipSpreadsheetWriter {
    /// Creates a new empty spreadsheet writer.
    pub fn new() -> Self {
        Self { sheets: Vec::new() }
    }

    /// Adds a new sheet with the given name and returns its index.
    pub fn add_sheet(&mut self, name: &str) -> OfficeResult<usize> {
        let sheet_name = if name.trim().is_empty() {
            format!("Sheet{}", self.sheets.len() + 1)
        } else {
            name.trim().to_string()
        };

        if self.sheets.iter().any(|s| s.name == sheet_name) {
            return Err(OfficeError::CellParseError(format!("Sheet '{sheet_name}' already exists")));
        }

        let idx = self.sheets.len();
        self.sheets.push(WorksheetData {
            name: sheet_name,
            cells: HashMap::new(),
        });
        Ok(idx)
    }

    /// Writes a generic cell value at the given 0-indexed row and column.
    pub fn write_cell(
        &mut self,
        sheet_idx: usize,
        row: u32,
        col: u32,
        value: OfficeCellValue,
    ) -> OfficeResult<()> {
        let sheet = self.sheets.get_mut(sheet_idx).ok_or_else(|| {
            OfficeError::SheetNotFound(format!("Sheet index {sheet_idx} out of bounds"))
        })?;

        let entry = sheet.cells.entry((row, col)).or_default();
        entry.value = value;
        Ok(())
    }

    /// Writes a string text value.
    pub fn write_string(
        &mut self,
        sheet_idx: usize,
        row: u32,
        col: u32,
        text: &str,
    ) -> OfficeResult<()> {
        self.write_cell(sheet_idx, row, col, OfficeCellValue::String(text.to_string()))
    }

    /// Writes a floating-point numeric value.
    pub fn write_number(
        &mut self,
        sheet_idx: usize,
        row: u32,
        col: u32,
        num: f64,
    ) -> OfficeResult<()> {
        self.write_cell(sheet_idx, row, col, OfficeCellValue::Float(num))
    }

    /// Writes an integer numeric value.
    pub fn write_int(
        &mut self,
        sheet_idx: usize,
        row: u32,
        col: u32,
        num: i64,
    ) -> OfficeResult<()> {
        self.write_cell(sheet_idx, row, col, OfficeCellValue::Int(num))
    }

    /// Writes a boolean value.
    pub fn write_bool(
        &mut self,
        sheet_idx: usize,
        row: u32,
        col: u32,
        val: bool,
    ) -> OfficeResult<()> {
        self.write_cell(sheet_idx, row, col, OfficeCellValue::Bool(val))
    }

    /// Writes a formula expression (e.g. `SUM(A1:A10)` or `=A1*2`).
    pub fn write_formula(
        &mut self,
        sheet_idx: usize,
        row: u32,
        col: u32,
        formula: &str,
    ) -> OfficeResult<()> {
        let sheet = self.sheets.get_mut(sheet_idx).ok_or_else(|| {
            OfficeError::SheetNotFound(format!("Sheet index {sheet_idx} out of bounds"))
        })?;

        let clean_formula = formula.trim().trim_start_matches('=').to_string();
        let entry = sheet.cells.entry((row, col)).or_default();
        entry.formula = Some(clean_formula);
        Ok(())
    }

    /// Sets bold and italic formatting for a cell.
    pub fn set_cell_format(
        &mut self,
        sheet_idx: usize,
        row: u32,
        col: u32,
        bold: bool,
        italic: bool,
    ) -> OfficeResult<()> {
        let sheet = self.sheets.get_mut(sheet_idx).ok_or_else(|| {
            OfficeError::SheetNotFound(format!("Sheet index {sheet_idx} out of bounds"))
        })?;

        let entry = sheet.cells.entry((row, col)).or_default();
        entry.format.bold = bold;
        entry.format.italic = italic;
        Ok(())
    }

    /// Serializes the workbook into a standard OpenXML XLSX byte buffer.
    pub fn save_to_buffer(&mut self) -> OfficeResult<Vec<u8>> {
        if self.sheets.is_empty() {
            self.add_sheet("Sheet1")?;
        }

        // 1. Collect all shared strings across all worksheets
        let mut sst: Vec<String> = Vec::new();
        let mut sst_map: HashMap<String, usize> = HashMap::new();

        for sheet in &self.sheets {
            let mut coords: Vec<(u32, u32)> = sheet.cells.keys().cloned().collect();
            coords.sort();
            for coord in coords {
                if let Some(record) = sheet.cells.get(&coord) {
                    if let OfficeCellValue::String(ref s) = record.value {
                        if !sst_map.contains_key(s) {
                            let idx = sst.len();
                            sst_map.insert(s.clone(), idx);
                            sst.push(s.clone());
                        }
                    }
                }
            }
        }

        let mut zip_items = Vec::new();

        // 2. Generate [Content_Types].xml
        let content_types_xml = generate_content_types_xml(self.sheets.len());
        zip_items.push(make_zip_item("[Content_Types].xml", content_types_xml.into_bytes()));

        // 3. Generate _rels/.rels
        let rels_xml = generate_root_rels_xml();
        zip_items.push(make_zip_item("_rels/.rels", rels_xml.into_bytes()));

        // 4. Generate docProps/app.xml & docProps/core.xml
        let app_xml = generate_app_xml(&self.sheets);
        zip_items.push(make_zip_item("docProps/app.xml", app_xml.into_bytes()));

        let core_xml = generate_core_xml();
        zip_items.push(make_zip_item("docProps/core.xml", core_xml.into_bytes()));

        // 5. Generate xl/_rels/workbook.xml.rels
        let wb_rels_xml = generate_workbook_rels_xml(self.sheets.len());
        zip_items.push(make_zip_item("xl/_rels/workbook.xml.rels", wb_rels_xml.into_bytes()));

        // 6. Generate xl/workbook.xml
        let wb_xml = generate_workbook_xml(&self.sheets);
        zip_items.push(make_zip_item("xl/workbook.xml", wb_xml.into_bytes()));

        // 7. Generate xl/styles.xml
        let styles_xml = generate_styles_xml();
        zip_items.push(make_zip_item("xl/styles.xml", styles_xml.into_bytes()));

        // 8. Generate xl/sharedStrings.xml
        let sst_xml = generate_shared_strings_xml(&sst);
        zip_items.push(make_zip_item("xl/sharedStrings.xml", sst_xml.into_bytes()));

        // 9. Generate each xl/worksheets/sheetN.xml
        for (i, sheet) in self.sheets.iter().enumerate() {
            let sheet_xml = generate_worksheet_xml(sheet, &sst_map);
            let path = format!("xl/worksheets/sheet{}.xml", i + 1);
            zip_items.push(make_zip_item(&path, sheet_xml.into_bytes()));
        }

        // 10. Compress and assemble ZIP container
        let compressed_items = compress_items_parallel(zip_items, 6, TTZipEncryptionMethod::None, None, 4)
            .map_err(OfficeError::Zip)?;

        assemble_zip_archive(&compressed_items).map_err(OfficeError::Zip)
    }
}

fn make_zip_item(path: &str, data: Vec<u8>) -> ZipInputItem {
    ZipInputItem {
        rel_path: path.to_string(),
        data,
        mtime_epoch_secs: 1772500000,
        mode: 0o100644,
        is_directory: false,
    }
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn generate_content_types_xml(sheet_count: usize) -> String {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
  <Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
"#);
    for i in 1..=sheet_count {
        xml.push_str(&format!(
            r#"  <Override PartName="/xl/worksheets/sheet{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
"#
        ));
    }
    xml.push_str("</Types>");
    xml
}

fn generate_root_rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#.to_string()
}

fn generate_app_xml(sheets: &[WorksheetData]) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>TTZip Native Engine</Application>
  <DocSecurity>0</DocSecurity>
  <ScaleCrop>false</ScaleCrop>
  <HeadingPairs>
    <vt:vector size="2" baseType="variant" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
      <vt:variant><vt:lpstr>Worksheets</vt:lpstr></vt:variant>
      <vt:variant><vt:i4>{}</vt:i4></vt:variant>
    </vt:vector>
  </HeadingPairs>
  <TitlesOfParts>
    <vt:vector size="{}" baseType="lpstr" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
"#,
        sheets.len(),
        sheets.len()
    );

    for s in sheets {
        xml.push_str(&format!("      <vt:lpstr>{}</vt:lpstr>\n", escape_xml(&s.name)));
    }

    xml.push_str(
        r#"    </vt:vector>
  </TitlesOfParts>
  <Company>TTZip</Company>
  <LinksUpToDate>false</LinksUpToDate>
  <SharedDoc>false</SharedDoc>
  <HyperlinksChanged>false</HyperlinksChanged>
  <AppVersion>1.0000</AppVersion>
</Properties>"#,
    );
    xml
}

fn generate_core_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <dc:creator>TTZip</dc:creator>
  <cp:lastModifiedBy>TTZip</cp:lastModifiedBy>
  <dcterms:created xsi:type="dcterms:W3CDTF">2026-03-01T12:00:00Z</dcterms:created>
  <dcterms:modified xsi:type="dcterms:W3CDTF">2026-03-01T12:00:00Z</dcterms:modified>
</cp:coreProperties>"#.to_string()
}

fn generate_workbook_rels_xml(sheet_count: usize) -> String {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
  <Relationship Id="rIdSST" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/>
"#);
    for i in 1..=sheet_count {
        xml.push_str(&format!(
            r#"  <Relationship Id="rIdSheet{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{i}.xml"/>
"#
        ));
    }
    xml.push_str("</Relationships>");
    xml
}

fn generate_workbook_xml(sheets: &[WorksheetData]) -> String {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
"#);
    for (i, s) in sheets.iter().enumerate() {
        let sheet_id = i + 1;
        xml.push_str(&format!(
            r#"    <sheet name="{}" sheetId="{sheet_id}" r:id="rIdSheet{sheet_id}"/>
"#,
            escape_xml(&s.name)
        ));
    }
    xml.push_str("  </sheets>\n</workbook>");
    xml
}

fn generate_styles_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <fonts count="3">
    <font><sz val="11"/><color theme="1"/><name val="Calibri"/><family val="2"/></font>
    <font><b/><sz val="11"/><color theme="1"/><name val="Calibri"/><family val="2"/></font>
    <font><i/><sz val="11"/><color theme="1"/><name val="Calibri"/><family val="2"/></font>
  </fonts>
  <fills count="2">
    <fill><patternFill patternType="none"/></fill>
    <fill><patternFill patternType="gray125"/></fill>
  </fills>
  <borders count="1">
    <border><left/><right/><top/><bottom/><diagonal/></border>
  </borders>
  <cellStyleXfs count="1">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0"/>
  </cellStyleXfs>
  <cellXfs count="3">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>
    <xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/>
    <xf numFmtId="0" fontId="2" fillId="0" borderId="0" xfId="0" applyFont="1"/>
  </cellXfs>
</styleSheet>"#.to_string()
}

fn generate_shared_strings_xml(sst: &[String]) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="{}" uniqueCount="{}">
"#,
        sst.len(),
        sst.len()
    );

    for s in sst {
        xml.push_str(&format!("  <si><t>{}</t></si>\n", escape_xml(s)));
    }

    xml.push_str("</sst>");
    xml
}

fn generate_worksheet_xml(sheet: &WorksheetData, sst_map: &HashMap<String, usize>) -> String {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheetData>
"#);

    // Group cells by row
    let mut rows_map: HashMap<u32, Vec<(u32, &CellRecord)>> = HashMap::new();
    for (&(r, c), record) in &sheet.cells {
        rows_map.entry(r).or_default().push((c, record));
    }

    let mut row_indices: Vec<u32> = rows_map.keys().cloned().collect();
    row_indices.sort();

    for row_idx in row_indices {
        let mut cols = rows_map.remove(&row_idx).unwrap_or_default();
        cols.sort_by_key(|&(c, _)| c);

        let row_1based = row_idx + 1;
        xml.push_str(&format!("    <row r=\"{row_1based}\">\n"));

        for (col_idx, record) in cols {
            let col_letter = col_to_a1(col_idx);
            let cell_ref = format!("{col_letter}{row_1based}");

            let style_idx = if record.format.bold {
                1
            } else if record.format.italic {
                2
            } else {
                0
            };
            let style_attr = if style_idx > 0 {
                format!(" s=\"{style_idx}\"")
            } else {
                String::new()
            };

            if let Some(ref f) = record.formula {
                xml.push_str(&format!("      <c r=\"{cell_ref}\"{style_attr}><f>{}</f>", escape_xml(f)));
                match &record.value {
                    OfficeCellValue::Int(i) => xml.push_str(&format!("<v>{i}</v>")),
                    OfficeCellValue::Float(fl) => xml.push_str(&format!("<v>{fl}</v>")),
                    OfficeCellValue::String(s) => xml.push_str(&format!("<v>{}</v>", escape_xml(s))),
                    _ => {}
                }
                xml.push_str("</c>\n");
            } else {
                match &record.value {
                    OfficeCellValue::Empty => {}
                    OfficeCellValue::String(s) => {
                        if let Some(&sst_idx) = sst_map.get(s) {
                            xml.push_str(&format!("      <c r=\"{cell_ref}\" t=\"s\"{style_attr}><v>{sst_idx}</v></c>\n"));
                        } else {
                            xml.push_str(&format!("      <c r=\"{cell_ref}\" t=\"inlineStr\"{style_attr}><is><t>{}</t></is></c>\n", escape_xml(s)));
                        }
                    }
                    OfficeCellValue::Int(i) => {
                        xml.push_str(&format!("      <c r=\"{cell_ref}\"{style_attr}><v>{i}</v></c>\n"));
                    }
                    OfficeCellValue::Float(fl) => {
                        xml.push_str(&format!("      <c r=\"{cell_ref}\"{style_attr}><v>{fl}</v></c>\n"));
                    }
                    OfficeCellValue::Bool(b) => {
                        let val_str = if *b { "1" } else { "0" };
                        xml.push_str(&format!("      <c r=\"{cell_ref}\" t=\"b\"{style_attr}><v>{val_str}</v></c>\n"));
                    }
                    OfficeCellValue::DateTime(dt) => {
                        xml.push_str(&format!("      <c r=\"{cell_ref}\" t=\"inlineStr\"{style_attr}><is><t>{}</t></is></c>\n", escape_xml(dt)));
                    }
                    OfficeCellValue::Error(e) => {
                        xml.push_str(&format!("      <c r=\"{cell_ref}\" t=\"e\"{style_attr}><v>{}</v></c>\n", escape_xml(e)));
                    }
                }
            }
        }

        xml.push_str("    </row>\n");
    }

    xml.push_str("  </sheetData>\n</worksheet>");
    xml
}
