// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Export Layer for Office Document Metadata,
//! Spreadsheet Worksheets, Cell Grids, Dynamic Formulas, and DOCX Structures.

pub(crate) mod docx;
pub(crate) mod formula;
pub(crate) mod service;
pub mod types;
pub(crate) mod xlsx;

pub use docx::parse_docx_archive;
pub use formula::{evaluate_spreadsheet_formula, format_coordinate, parse_cell_coordinate};
pub use service::{
    uniffi_convert_docx_to_markdown, uniffi_evaluate_formula, uniffi_extract_docx_document,
    uniffi_extract_sheet_data, uniffi_extract_sheet_names, uniffi_probe_office_bytes,
    UniFFIOfficeService,
};
pub use types::{
    UniFFICell, UniFFICellValue, UniFFIDocxDocument, UniFFIDocxParagraph, UniFFIDocxTable,
    UniFFIDocxTableRow, UniFFIOfficeError, UniFFIOfficeFormat, UniFFISheetData, UniFFISheetRow,
};
pub use xlsx::{extract_xlsx_sheet_data, extract_xlsx_sheet_names};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

    fn make_test_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let items: Vec<ZipInputItem> = files
            .iter()
            .map(|(name, content)| ZipInputItem {
                rel_path: name.to_string(),
                data: content.to_vec(),
                mtime_epoch_secs: 1700000000,
                mode: 0o644,
                is_directory: false,
            })
            .collect();
        let compressed = compress_items_parallel(
            items,
            6,
            crate::types::TTZipEncryptionMethod::None,
            None,
            1,
        )
        .expect("zip compress");
        assemble_zip_archive(&compressed).expect("zip assemble")
    }

    fn make_synthetic_xlsx() -> Vec<u8> {
        let workbook_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Summary" sheetId="1" r:id="rId1"/>
    <sheet name="Q1_Expenses" sheetId="2" r:id="rId2"/>
  </sheets>
</workbook>"#;

        let shared_strings_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3" uniqueCount="3">
  <si><t>Revenue</t></si>
  <si><t>Cost of Goods</t></si>
  <si><t>Net Profit</t></si>
</sst>"#;

        let sheet1_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1">
      <c r="A1" t="s"><v>0</v></c>
      <c r="B1"><v>10000</v></c>
    </row>
    <row r="2">
      <c r="A2" t="s"><v>1</v></c>
      <c r="B2"><v>4000</v></c>
    </row>
    <row r="3">
      <c r="A3" t="s"><v>2</v></c>
      <c r="B3"><f>B1-B2</f><v>6000</v></c>
      <c r="C3" t="b"><v>1</v></c>
    </row>
  </sheetData>
</worksheet>"#;

        let sheet2_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B2"/>
  <sheetData>
    <row r="1">
      <c r="A1"><v>250</v></c>
    </row>
  </sheetData>
</worksheet>"#;

        make_test_zip(&[
            ("xl/workbook.xml", workbook_xml),
            ("xl/sharedStrings.xml", shared_strings_xml),
            ("xl/worksheets/sheet1.xml", sheet1_xml),
            ("xl/worksheets/sheet2.xml", sheet2_xml),
        ])
    }

    fn make_synthetic_docx() -> Vec<u8> {
        let core_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:title>TTZip Systems Architecture</dc:title>
  <dc:creator>Witt Kung</dc:creator>
</cp:coreProperties>"#;

        let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:t>Introduction to Microkernel</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>The TTZip engine utilizes zero-disk streaming architecture.</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>First key benefit: low memory</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>Second key benefit: high throughput</w:t></w:r>
    </w:p>
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Component</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Throughput</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Rust Core</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>850 MB/s</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
  </w:body>
</w:document>"#;

        make_test_zip(&[
            ("docProps/core.xml", core_xml),
            ("word/document.xml", document_xml),
        ])
    }

    #[test]
    fn test_office_probe_and_format_identification() {
        let xlsx_bytes = make_synthetic_xlsx();
        let fmt = uniffi_probe_office_bytes(xlsx_bytes, Some("financials.xlsx".to_string()))
            .expect("probe xlsx");
        assert_eq!(fmt, UniFFIOfficeFormat::Xlsx);

        let docx_bytes = make_synthetic_docx();
        let fmt_docx = uniffi_probe_office_bytes(docx_bytes, Some("paper.docx".to_string()))
            .expect("probe docx");
        assert_eq!(fmt_docx, UniFFIOfficeFormat::Docx);

        let unknown_bytes = vec![0x10, 0x20, 0x30, 0x40];
        let fmt_unk = uniffi_probe_office_bytes(unknown_bytes, Some("data.bin".to_string()))
            .expect("probe unknown");
        assert_eq!(fmt_unk, UniFFIOfficeFormat::Unknown);
    }

    #[test]
    fn test_xlsx_sheet_names_and_sheet_data() {
        let xlsx_bytes = make_synthetic_xlsx();

        // 1. Sheet names
        let names = uniffi_extract_sheet_names(xlsx_bytes.clone(), None).expect("sheet names");
        assert_eq!(names, vec!["Summary".to_string(), "Q1_Expenses".to_string()]);

        // 2. Sheet data (Summary)
        let data = uniffi_extract_sheet_data(xlsx_bytes.clone(), "Summary".to_string(), None, None)
            .expect("sheet data");
        assert_eq!(data.sheet_name, "Summary");
        assert_eq!(data.total_rows, 3);
        assert_eq!(data.total_cols, 3);
        assert_eq!(data.dimension_ref.as_deref(), Some("A1:C3"));

        // Row 1: A1 = "Revenue", B1 = 10000
        let row1 = &data.rows[0];
        assert_eq!(row1.row_number, 1);
        assert_eq!(row1.cells[0].coordinate, "A1");
        assert_eq!(
            row1.cells[0].value,
            UniFFICellValue::Text {
                value: "Revenue".to_string()
            }
        );
        assert_eq!(
            row1.cells[1].value,
            UniFFICellValue::Number { value: 10000.0 }
        );

        // Row 3: A3 = "Net Profit", B3 = Formula "B1-B2" with cached 6000, C3 = Boolean TRUE
        let row3 = &data.rows[2];
        assert_eq!(
            row3.cells[0].value,
            UniFFICellValue::Text {
                value: "Net Profit".to_string()
            }
        );
        assert_eq!(
            row3.cells[1].value,
            UniFFICellValue::Formula {
                expression: "B1-B2".to_string(),
                cached_value: Some("6000".to_string())
            }
        );
        assert_eq!(row3.cells[2].value, UniFFICellValue::Boolean { value: true });

        // 3. Sheet data by numeric index
        let data2 = uniffi_extract_sheet_data(xlsx_bytes, "2".to_string(), None, None)
            .expect("sheet 2 data");
        assert_eq!(data2.sheet_name, "Q1_Expenses");
        assert_eq!(data2.rows.len(), 1);
    }

    #[test]
    fn test_dynamic_formula_evaluation() {
        // 1. Standalone arithmetic expressions
        let res1 = uniffi_evaluate_formula("=(10 + 20) * 3.5".to_string(), None).expect("eval 1");
        assert_eq!(res1, UniFFICellValue::Number { value: 105.0 });

        let res2 = uniffi_evaluate_formula("=100 / 4 - 5".to_string(), None).expect("eval 2");
        assert_eq!(res2, UniFFICellValue::Number { value: 20.0 });

        let res3 = uniffi_evaluate_formula("=2 ^ 4".to_string(), None).expect("eval 3");
        assert_eq!(res3, UniFFICellValue::Number { value: 16.0 });

        // 2. Math functions: SUM, AVERAGE, MIN, MAX, COUNT
        let sum_res = uniffi_evaluate_formula("=SUM(10, 20, 30, 40)".to_string(), None).expect("sum");
        assert_eq!(sum_res, UniFFICellValue::Number { value: 100.0 });

        let avg_res = uniffi_evaluate_formula("=AVERAGE(10, 20, 30)".to_string(), None).expect("avg");
        assert_eq!(avg_res, UniFFICellValue::Number { value: 20.0 });

        let min_res = uniffi_evaluate_formula("=MIN(15, 3, 99)".to_string(), None).expect("min");
        assert_eq!(min_res, UniFFICellValue::Number { value: 3.0 });

        let max_res = uniffi_evaluate_formula("=MAX(15, 3, 99)".to_string(), None).expect("max");
        assert_eq!(max_res, UniFFICellValue::Number { value: 99.0 });

        let cnt_res = uniffi_evaluate_formula("=COUNT(1, 2, 3, 4, 5)".to_string(), None).expect("count");
        assert_eq!(cnt_res, UniFFICellValue::Number { value: 5.0 });

        // 3. Logic: IF and CONCAT
        let if_true = uniffi_evaluate_formula("=IF(10 > 5, 100, 200)".to_string(), None).expect("if true");
        assert_eq!(if_true, UniFFICellValue::Number { value: 100.0 });

        let if_false = uniffi_evaluate_formula("=IF(10 < 5, \"Yes\", \"No\")".to_string(), None).expect("if false");
        assert_eq!(
            if_false,
            UniFFICellValue::Text {
                value: "No".to_string()
            }
        );

        let concat_res =
            uniffi_evaluate_formula("=CONCAT(\"Hello \", \"TTZip!\")".to_string(), None).expect("concat");
        assert_eq!(
            concat_res,
            UniFFICellValue::Text {
                value: "Hello TTZip!".to_string()
            }
        );

        // 4. Formula with context cells
        let cells = vec![
            UniFFICell {
                row: 1,
                col: 1,
                coordinate: "A1".to_string(),
                value: UniFFICellValue::Number { value: 50.0 },
                formula: None,
            },
            UniFFICell {
                row: 2,
                col: 1,
                coordinate: "A2".to_string(),
                value: UniFFICellValue::Number { value: 150.0 },
                formula: None,
            },
            UniFFICell {
                row: 1,
                col: 2,
                coordinate: "B1".to_string(),
                value: UniFFICellValue::Number { value: 2.0 },
                formula: None,
            },
        ];

        let range_sum =
            uniffi_evaluate_formula("=SUM(A1:A2)".to_string(), Some(cells.clone())).expect("range sum");
        assert_eq!(range_sum, UniFFICellValue::Number { value: 200.0 });

        let cell_arithmetic =
            uniffi_evaluate_formula("=(A1 + A2) * B1".to_string(), Some(cells)).expect("cell arith");
        assert_eq!(cell_arithmetic, UniFFICellValue::Number { value: 400.0 });
    }

    #[test]
    fn test_docx_document_and_markdown_conversion() {
        let docx_bytes = make_synthetic_docx();

        // 1. DOCX structured extraction
        let doc = uniffi_extract_docx_document(docx_bytes.clone(), None).expect("docx extract");
        assert_eq!(doc.title.as_deref(), Some("TTZip Systems Architecture"));
        assert_eq!(doc.paragraphs.len(), 4);
        assert_eq!(doc.paragraphs[0].text, "Introduction to Microkernel");
        assert_eq!(doc.paragraphs[0].heading_level, Some(1));
        assert!(doc.paragraphs[2].is_list_item);

        // Table
        assert_eq!(doc.tables.len(), 1);
        assert_eq!(doc.tables[0].total_rows, 2);
        assert_eq!(doc.tables[0].headers, vec!["Component", "Throughput"]);

        // Metrics
        assert!(doc.total_words > 0);
        assert!(doc.total_characters > 0);

        // 2. Markdown output
        let md = uniffi_convert_docx_to_markdown(docx_bytes, None).expect("docx md");
        assert!(md.contains("# TTZip Systems Architecture"));
        assert!(md.contains("# Introduction to Microkernel"));
        assert!(md.contains("- First key benefit: low memory"));
        assert!(md.contains("| Component | Throughput |"));
        assert!(md.contains("| --- | --- |"));
        assert!(md.contains("| Rust Core | 850 MB/s |"));
    }

    #[test]
    fn test_uniffi_office_service_object_lifecycle() {
        let service = UniFFIOfficeService::new();
        let xlsx_bytes = make_synthetic_xlsx();
        let docx_bytes = make_synthetic_docx();

        let fmt_x = service.probe_bytes(xlsx_bytes.clone(), None).expect("probe x");
        assert_eq!(fmt_x, UniFFIOfficeFormat::Xlsx);

        let names = service.extract_sheet_names(xlsx_bytes.clone(), None).expect("names");
        assert_eq!(names.len(), 2);

        let data = service
            .extract_sheet_data(xlsx_bytes, "Summary".to_string(), None, None)
            .expect("data");
        assert_eq!(data.total_rows, 3);

        let eval = service
            .evaluate_formula("=10 * 10".to_string(), None)
            .expect("eval");
        assert_eq!(eval, UniFFICellValue::Number { value: 100.0 });

        let doc = service.extract_docx_document(docx_bytes.clone(), None).expect("doc");
        assert_eq!(doc.title.as_deref(), Some("TTZip Systems Architecture"));

        let md = service.convert_docx_to_markdown(docx_bytes, None).expect("md");
        assert!(md.contains("TTZip Systems Architecture"));
    }

    #[test]
    fn test_coordinate_math() {
        assert_eq!(parse_cell_coordinate("A1", 1), (1, 1));
        assert_eq!(parse_cell_coordinate("B5", 1), (5, 2));
        assert_eq!(parse_cell_coordinate("Z26", 1), (26, 26));
        assert_eq!(parse_cell_coordinate("AA10", 1), (10, 27));

        assert_eq!(format_coordinate(1, 1), "A1");
        assert_eq!(format_coordinate(5, 2), "B5");
        assert_eq!(format_coordinate(26, 26), "Z26");
        assert_eq!(format_coordinate(10, 27), "AA10");
    }
}
