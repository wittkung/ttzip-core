// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Exhaustive test suite for TTZip Office spreadsheet and Word document microkernel.

use std::collections::HashMap;

use super::document::TTZipDocxParser;
use super::spreadsheet::{TTZipFormulaEngine, TTZipSpreadsheetParser, TTZipSpreadsheetWriter};
use super::types::{
    a1_to_col, col_to_a1, OfficeCellAddress, OfficeCellValue, OfficeFormat, OfficeRange,
};
use crate::types::TTZipEncryptionMethod;
use crate::zip::writer::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

#[test]
fn test_office_format_detection() {
    assert_eq!(OfficeFormat::from_path_or_extension("report.xlsx"), OfficeFormat::Xlsx);
    assert_eq!(OfficeFormat::from_path_or_extension("data.xls"), OfficeFormat::Xls);
    assert_eq!(OfficeFormat::from_path_or_extension("sheet.ods"), OfficeFormat::Ods);
    assert_eq!(OfficeFormat::from_path_or_extension("document.docx"), OfficeFormat::Docx);
    assert_eq!(OfficeFormat::from_path_or_extension("binary.xlsb"), OfficeFormat::Xlsb);
    assert_eq!(OfficeFormat::from_path_or_extension("unknown.xyz"), OfficeFormat::Unknown);

    assert!(OfficeFormat::Xlsx.is_spreadsheet());
    assert!(OfficeFormat::Docx.is_document());
    assert!(!OfficeFormat::Docx.is_spreadsheet());
}

#[test]
fn test_cell_coordinates_and_a1_conversion() {
    assert_eq!(col_to_a1(0), "A");
    assert_eq!(col_to_a1(25), "Z");
    assert_eq!(col_to_a1(26), "AA");
    assert_eq!(col_to_a1(27), "AB");
    assert_eq!(col_to_a1(701), "ZZ");
    assert_eq!(col_to_a1(702), "AAA");

    assert_eq!(a1_to_col("A").unwrap(), 0);
    assert_eq!(a1_to_col("Z").unwrap(), 25);
    assert_eq!(a1_to_col("AA").unwrap(), 26);
    assert_eq!(a1_to_col("AB").unwrap(), 27);
    assert_eq!(a1_to_col("ZZ").unwrap(), 701);
    assert_eq!(a1_to_col("AAA").unwrap(), 702);

    let addr1 = OfficeCellAddress::from_row_col(0, 0);
    assert_eq!(addr1.a1, "A1");

    let addr2 = OfficeCellAddress::from_a1("B5").unwrap();
    assert_eq!(addr2.row, 4);
    assert_eq!(addr2.col, 1);
    assert_eq!(addr2.a1, "B5");

    let addr3 = OfficeCellAddress::from_a1("$AA$100").unwrap();
    assert_eq!(addr3.row, 99);
    assert_eq!(addr3.col, 26);

    assert!(OfficeCellAddress::from_a1("").is_err());
    assert!(OfficeCellAddress::from_a1("A0").is_err());
    assert!(OfficeCellAddress::from_a1("123").is_err());
}

#[test]
fn test_cell_value_conversions() {
    let empty = OfficeCellValue::Empty;
    assert!(empty.is_empty());
    assert_eq!(empty.as_string(), "");
    assert_eq!(empty.as_f64(), Some(0.0));

    let str_val = OfficeCellValue::String("42.5".to_string());
    assert_eq!(str_val.as_string(), "42.5");
    assert_eq!(str_val.as_f64(), Some(42.5));
    assert_eq!(str_val.as_i64(), None);

    let int_val = OfficeCellValue::Int(100);
    assert_eq!(int_val.as_string(), "100");
    assert_eq!(int_val.as_f64(), Some(100.0));
    assert_eq!(int_val.as_i64(), Some(100));
    assert_eq!(int_val.as_bool(), Some(true));

    let bool_val = OfficeCellValue::Bool(false);
    assert_eq!(bool_val.as_string(), "FALSE");
    assert_eq!(bool_val.as_bool(), Some(false));

    let err_val = OfficeCellValue::Error("#CYCLE!".to_string());
    assert!(err_val.is_error());
    assert_eq!(err_val.as_string(), "#CYCLE!");
}

#[test]
fn test_office_range_operations() {
    let range = OfficeRange {
        sheet_name: "Sheet1".to_string(),
        start: OfficeCellAddress::from_row_col(0, 0),
        end: OfficeCellAddress::from_row_col(1, 1),
        values: vec![
            vec![OfficeCellValue::Int(1), OfficeCellValue::Int(2)],
            vec![OfficeCellValue::Int(3), OfficeCellValue::Int(4)],
        ],
    };

    assert_eq!(range.row_count(), 2);
    assert_eq!(range.col_count(), 2);
    assert_eq!(range.get_cell(0, 1), Some(&OfficeCellValue::Int(2)));
    assert_eq!(range.get_cell(1, 0), Some(&OfficeCellValue::Int(3)));
    assert_eq!(range.get_cell(2, 2), None);
}

#[test]
fn test_formula_arithmetic_and_logic() {
    let engine = TTZipFormulaEngine::new();
    let grid = HashMap::new();

    let res1 = engine.evaluate_formula("=1 + 2 * 3", &grid).unwrap();
    assert_eq!(res1, OfficeCellValue::Float(7.0));

    let res2 = engine.evaluate_formula("=(10 - 2) / 4", &grid).unwrap();
    assert_eq!(res2, OfficeCellValue::Float(2.0));

    let res3 = engine.evaluate_formula("=2 ^ 3", &grid).unwrap();
    assert_eq!(res3, OfficeCellValue::Float(8.0));

    let res4 = engine.evaluate_formula("=10 > 5", &grid).unwrap();
    assert_eq!(res4, OfficeCellValue::Bool(true));

    let res5 = engine.evaluate_formula("=\"Hello \" & \"World\"", &grid).unwrap();
    assert_eq!(res5, OfficeCellValue::String("Hello World".to_string()));
}

#[test]
fn test_formula_built_in_functions() {
    let engine = TTZipFormulaEngine::new();
    let mut grid = HashMap::new();

    grid.insert(OfficeCellAddress::from_a1("A1").unwrap(), OfficeCellValue::Int(10));
    grid.insert(OfficeCellAddress::from_a1("A2").unwrap(), OfficeCellValue::Int(20));
    grid.insert(OfficeCellAddress::from_a1("A3").unwrap(), OfficeCellValue::Int(30));
    grid.insert(OfficeCellAddress::from_a1("A4").unwrap(), OfficeCellValue::Int(40));

    // Math & Statistics
    assert_eq!(
        engine.evaluate_formula("=SUM(A1:A4)", &grid).unwrap(),
        OfficeCellValue::Float(100.0)
    );
    assert_eq!(
        engine.evaluate_formula("=AVERAGE(A1:A4)", &grid).unwrap(),
        OfficeCellValue::Float(25.0)
    );
    assert_eq!(
        engine.evaluate_formula("=MIN(A1:A4)", &grid).unwrap(),
        OfficeCellValue::Float(10.0)
    );
    assert_eq!(
        engine.evaluate_formula("=MAX(A1:A4)", &grid).unwrap(),
        OfficeCellValue::Float(40.0)
    );
    assert_eq!(
        engine.evaluate_formula("=COUNT(A1:A4)", &grid).unwrap(),
        OfficeCellValue::Int(4)
    );

    // Logic
    assert_eq!(
        engine.evaluate_formula("=IF(SUM(A1:A4) > 50, \"High\", \"Low\")", &grid).unwrap(),
        OfficeCellValue::String("High".to_string())
    );
    assert_eq!(
        engine.evaluate_formula("=AND(A1 = 10, A2 = 20)", &grid).unwrap(),
        OfficeCellValue::Bool(true)
    );
    assert_eq!(
        engine.evaluate_formula("=OR(A1 = 999, A2 = 20)", &grid).unwrap(),
        OfficeCellValue::Bool(true)
    );
    assert_eq!(
        engine.evaluate_formula("=NOT(A1 = 10)", &grid).unwrap(),
        OfficeCellValue::Bool(false)
    );

    // Text functions
    assert_eq!(
        engine.evaluate_formula("=CONCAT(\"A\", \"-\", \"B\")", &grid).unwrap(),
        OfficeCellValue::String("A-B".to_string())
    );
    assert_eq!(
        engine.evaluate_formula("=TEXTJOIN(\", \", TRUE, \"Apple\", \"Banana\")", &grid).unwrap(),
        OfficeCellValue::String("Apple, Banana".to_string())
    );
    assert_eq!(
        engine.evaluate_formula("=LEFT(\"TTZip\", 2)", &grid).unwrap(),
        OfficeCellValue::String("TT".to_string())
    );
    assert_eq!(
        engine.evaluate_formula("=RIGHT(\"TTZip\", 3)", &grid).unwrap(),
        OfficeCellValue::String("Zip".to_string())
    );
    assert_eq!(
        engine.evaluate_formula("=MID(\"TTZipEngine\", 3, 3)", &grid).unwrap(),
        OfficeCellValue::String("Zip".to_string())
    );
    assert_eq!(
        engine.evaluate_formula("=LEN(\"TTZip\")", &grid).unwrap(),
        OfficeCellValue::Int(5)
    );
    assert_eq!(
        engine.evaluate_formula("=TRIM(\"  Clean  \")", &grid).unwrap(),
        OfficeCellValue::String("Clean".to_string())
    );
    assert_eq!(
        engine.evaluate_formula("=UPPER(\"ttzip\")", &grid).unwrap(),
        OfficeCellValue::String("TTZIP".to_string())
    );
    assert_eq!(
        engine.evaluate_formula("=LOWER(\"TTZIP\")", &grid).unwrap(),
        OfficeCellValue::String("ttzip".to_string())
    );

    // Math helpers
    assert_eq!(
        engine.evaluate_formula("=ABS(-42)", &grid).unwrap(),
        OfficeCellValue::Float(42.0)
    );
    assert_eq!(
        engine.evaluate_formula("=ROUND(3.14159, 2)", &grid).unwrap(),
        OfficeCellValue::Float(3.14)
    );
    assert_eq!(
        engine.evaluate_formula("=POWER(2, 4)", &grid).unwrap(),
        OfficeCellValue::Float(16.0)
    );
    assert_eq!(
        engine.evaluate_formula("=SQRT(64)", &grid).unwrap(),
        OfficeCellValue::Float(8.0)
    );
    assert_eq!(
        engine.evaluate_formula("=MOD(10, 3)", &grid).unwrap(),
        OfficeCellValue::Float(1.0)
    );
}

#[test]
fn test_formula_lookup_and_index_match() {
    let engine = TTZipFormulaEngine::new();
    let mut grid = HashMap::new();

    // Table:
    // A1: "Alice", B1: 100
    // A2: "Bob",   B2: 200
    // A3: "Carol", B3: 300
    grid.insert(OfficeCellAddress::from_a1("A1").unwrap(), OfficeCellValue::String("Alice".to_string()));
    grid.insert(OfficeCellAddress::from_a1("B1").unwrap(), OfficeCellValue::Int(100));
    grid.insert(OfficeCellAddress::from_a1("A2").unwrap(), OfficeCellValue::String("Bob".to_string()));
    grid.insert(OfficeCellAddress::from_a1("B2").unwrap(), OfficeCellValue::Int(200));
    grid.insert(OfficeCellAddress::from_a1("A3").unwrap(), OfficeCellValue::String("Carol".to_string()));
    grid.insert(OfficeCellAddress::from_a1("B3").unwrap(), OfficeCellValue::Int(300));

    // VLOOKUP
    let vlookup_res = engine.evaluate_formula("=VLOOKUP(\"Bob\", A1:B3, 2, FALSE)", &grid).unwrap();
    assert_eq!(vlookup_res, OfficeCellValue::Int(200));

    // MATCH
    let match_res = engine.evaluate_formula("=MATCH(\"Carol\", A1:A3, 0)", &grid).unwrap();
    assert_eq!(match_res, OfficeCellValue::Int(3));

    // INDEX
    let index_res = engine.evaluate_formula("=INDEX(A1:B3, 2, 2)", &grid).unwrap();
    assert_eq!(index_res, OfficeCellValue::Int(200));
}

#[test]
fn test_formula_cycle_detection_tarjan() {
    let mut engine = TTZipFormulaEngine::new();
    let mut formulas = HashMap::new();
    let mut grid = HashMap::new();

    // Circular loop: A1 -> B1 -> C1 -> A1
    let a1 = OfficeCellAddress::from_a1("A1").unwrap();
    let b1 = OfficeCellAddress::from_a1("B1").unwrap();
    let c1 = OfficeCellAddress::from_a1("C1").unwrap();

    formulas.insert(a1.clone(), "=B1 + 1".to_string());
    formulas.insert(b1.clone(), "=C1 * 2".to_string());
    formulas.insert(c1.clone(), "=A1 - 5".to_string());

    // Non-circular cell: D1 = 10 + 20
    let d1 = OfficeCellAddress::from_a1("D1").unwrap();
    formulas.insert(d1.clone(), "=10 + 20".to_string());

    engine.recalculate_all(&formulas, &mut grid).unwrap();

    assert_eq!(grid.get(&a1), Some(&OfficeCellValue::Error("#CYCLE!".to_string())));
    assert_eq!(grid.get(&b1), Some(&OfficeCellValue::Error("#CYCLE!".to_string())));
    assert_eq!(grid.get(&c1), Some(&OfficeCellValue::Error("#CYCLE!".to_string())));
    assert_eq!(grid.get(&d1), Some(&OfficeCellValue::Float(30.0)));
}

#[test]
fn test_spreadsheet_writer_and_parser_roundtrip() {
    let mut writer = TTZipSpreadsheetWriter::new();
    let sheet0 = writer.add_sheet("SalesData").unwrap();

    writer.write_string(sheet0, 0, 0, "Quarter").unwrap();
    writer.write_string(sheet0, 0, 1, "Revenue").unwrap();
    writer.set_cell_format(sheet0, 0, 0, true, false).unwrap();
    writer.set_cell_format(sheet0, 0, 1, true, false).unwrap();

    writer.write_string(sheet0, 1, 0, "Q1").unwrap();
    writer.write_number(sheet0, 1, 1, 15000.5).unwrap();

    writer.write_string(sheet0, 2, 0, "Q2").unwrap();
    writer.write_number(sheet0, 2, 1, 23000.0).unwrap();

    writer.write_string(sheet0, 3, 0, "Total").unwrap();
    writer.write_formula(sheet0, 3, 1, "SUM(B2:B3)").unwrap();

    // Sheet 2
    let sheet1 = writer.add_sheet("Summary").unwrap();
    writer.write_string(sheet1, 0, 0, "Status").unwrap();
    writer.write_bool(sheet1, 0, 1, true).unwrap();

    let xlsx_bytes = writer.save_to_buffer().unwrap();
    assert!(!xlsx_bytes.is_empty());
    assert!(xlsx_bytes.starts_with(b"PK\x03\x04"));

    // Parse back with TTZipSpreadsheetParser
    let mut parser = TTZipSpreadsheetParser::open_from_bytes(&xlsx_bytes).unwrap();
    let names = parser.sheet_names();
    assert_eq!(names, vec!["SalesData".to_string(), "Summary".to_string()]);

    let range = parser.read_sheet("SalesData").unwrap();
    assert_eq!(range.row_count(), 4);
    assert_eq!(range.col_count(), 2);

    assert_eq!(range.get_cell(0, 0), Some(&OfficeCellValue::String("Quarter".to_string())));
    assert_eq!(range.get_cell(0, 1), Some(&OfficeCellValue::String("Revenue".to_string())));
    assert_eq!(range.get_cell(1, 0), Some(&OfficeCellValue::String("Q1".to_string())));
    assert_eq!(range.get_cell(1, 1), Some(&OfficeCellValue::Float(15000.5)));
    assert_eq!(range.get_cell(2, 0), Some(&OfficeCellValue::String("Q2".to_string())));
    assert_eq!(range.get_cell(2, 1), Some(&OfficeCellValue::Int(23000)));

    let summary_range = parser.read_sheet("Summary").unwrap();
    assert_eq!(summary_range.get_cell(0, 0), Some(&OfficeCellValue::String("Status".to_string())));
    assert_eq!(summary_range.get_cell(0, 1), Some(&OfficeCellValue::Bool(true)));
}

#[test]
fn test_docx_parser_and_markdown_export() {
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr>
        <w:pStyle w:val="Heading1"/>
      </w:pPr>
      <w:r>
        <w:t>Architecture Overview</w:t>
      </w:r>
    </w:p>
    <w:p>
      <w:r>
        <w:rPr><w:b/></w:rPr>
        <w:t>TTZip</w:t>
      </w:r>
      <w:r>
        <w:t> is a high performance </w:t>
      </w:r>
      <w:r>
        <w:rPr><w:i/></w:rPr>
        <w:t>Safe Rust</w:t>
      </w:r>
      <w:r>
        <w:t> microkernel engine.</w:t>
      </w:r>
    </w:p>
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Module</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Status</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>XLSX Engine</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Production</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
  </w:body>
</w:document>"#;

    // Create standard minimal DOCX container in memory
    let items = vec![
        ZipInputItem {
            rel_path: "word/document.xml".to_string(),
            data: doc_xml.as_bytes().to_vec(),
            mtime_epoch_secs: 1772500000,
            mode: 0o100644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "[Content_Types].xml".to_string(),
            data: b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"/>".to_vec(),
            mtime_epoch_secs: 1772500000,
            mode: 0o100644,
            is_directory: false,
        },
    ];

    let comp = compress_items_parallel(items, 6, TTZipEncryptionMethod::None, None, 1).unwrap();
    let docx_bytes = assemble_zip_archive(&comp).unwrap();

    let parser = TTZipDocxParser::open_from_bytes(&docx_bytes).unwrap();
    assert_eq!(parser.paragraphs().len(), 2);
    assert_eq!(parser.tables().len(), 1);

    let p0 = &parser.paragraphs()[0];
    assert_eq!(p0.text, "Architecture Overview");
    assert_eq!(p0.heading_level, Some(1));

    let p1 = &parser.paragraphs()[1];
    assert_eq!(p1.text, "TTZip is a high performance Safe Rust microkernel engine.");
    assert_eq!(p1.runs.len(), 4);
    assert!(p1.runs[0].bold);
    assert!(p1.runs[2].italic);

    let plain = parser.to_plain_text();
    assert!(plain.contains("Architecture Overview"));
    assert!(plain.contains("TTZip is a high performance"));
    assert!(plain.contains("Module\tStatus"));
    assert!(plain.contains("XLSX Engine\tProduction"));

    let markdown = parser.to_markdown();
    assert!(markdown.contains("# Architecture Overview"));
    assert!(markdown.contains("**TTZip**"));
    assert!(markdown.contains("*Safe Rust*"));
    assert!(markdown.contains("| Module | Status |"));
    assert!(markdown.contains("| --- | --- |"));
    assert!(markdown.contains("| XLSX Engine | Production |"));
}
