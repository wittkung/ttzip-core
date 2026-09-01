// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Official Office Open XML (DOCX, XLSX, PPTX) Compliance & Differential Oracle Test Suite.
//!
//! Validates standard ECMA-376 / ISO/IEC 29500 Office Open XML specifications:
//! 1. **Dublin Core & App Properties**: `docProps/core.xml` and `docProps/app.xml` compliance.
//! 2. **DOCX WordprocessingML**: Hierarchical heading extraction, outline levels, paragraph tokenization.
//! 3. **XLSX SpreadsheetML**: Workbook sheet discovery, shared string table (SST) dereferencing.
//! 4. **PPTX PresentationML**: Slide shape tree, placeholder title identification, text box aggregation.
//! 5. **UniFFI High-Level Facade**: End-to-end container introspection and metadata extraction.

use std::collections::HashMap;

use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::security::office_defense::{
    col_index_to_str, col_str_to_index, CellCoord, FormulaDepthGuard, OfficeDefenseError,
    OfficeMacroSandboxGuard, OfficeMemoryBudgetGuard, OfficeSecurityConfig, OfficeSecurityPipeline,
    SensitiveOfficeBuffer, SheetDimensionsGuard, SstQuotaGuard, DEFAULT_MAX_OFFICE_BUDGET,
    MAX_FORMULA_DEPTH, MAX_FORMULA_TOKENS, MAX_SHEET_COLS, MAX_SHEET_ROWS,
    MAX_VIEWPORT_ACTIVE_CELLS,
};
use ttzip_engine::uniffi_api::xml_meta::office::{
    parse_office_metadata_from_slice, parse_office_outline_from_slice,
};
use ttzip_engine::xml::OfficeXmlExtractor;

// ============================================================================
// Synthetic Test ZIP Helper
// ============================================================================

fn create_test_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip_data = Vec::new();
    let mut cd_entries = Vec::new();

    for (name, content) in files {
        let lfh_offset = zip_data.len() as u32;
        let crc = crc32_fast(0, content);
        let name_bytes = name.as_bytes();

        zip_data.extend_from_slice(&0x04034b50u32.to_le_bytes());
        zip_data.extend_from_slice(&20u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&crc.to_le_bytes());
        zip_data.extend_from_slice(&(content.len() as u32).to_le_bytes());
        zip_data.extend_from_slice(&(content.len() as u32).to_le_bytes());
        zip_data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(name_bytes);
        zip_data.extend_from_slice(content);

        cd_entries.push((name_bytes.to_vec(), crc, content.len() as u32, lfh_offset));
    }

    let cd_offset = zip_data.len() as u32;
    for (name_bytes, crc, size, lfh_offset) in &cd_entries {
        zip_data.extend_from_slice(&0x02014b50u32.to_le_bytes());
        zip_data.extend_from_slice(&20u16.to_le_bytes());
        zip_data.extend_from_slice(&20u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&crc.to_le_bytes());
        zip_data.extend_from_slice(&size.to_le_bytes());
        zip_data.extend_from_slice(&size.to_le_bytes());
        zip_data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u32.to_le_bytes());
        zip_data.extend_from_slice(&lfh_offset.to_le_bytes());
        zip_data.extend_from_slice(name_bytes);
    }

    let cd_size = (zip_data.len() as u32) - cd_offset;
    let entry_count = cd_entries.len() as u16;

    zip_data.extend_from_slice(&0x06054b50u32.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&entry_count.to_le_bytes());
    zip_data.extend_from_slice(&entry_count.to_le_bytes());
    zip_data.extend_from_slice(&cd_size.to_le_bytes());
    zip_data.extend_from_slice(&cd_offset.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());

    zip_data
}

// ============================================================================
// 1. Dublin Core & Extended App Properties Tests
// ============================================================================

#[test]
fn test_office_core_properties_extraction() {
    let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/"
                   xmlns:dcterms="http://purl.org/dc/terms/"
                   xmlns:dcmitype="http://purl.org/dc/dcmitype/"
                   xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
    <dc:title>Q3 Financial Summary</dc:title>
    <dc:subject>Quarterly Financial Results</dc:subject>
    <dc:creator>Witt Kung</dc:creator>
    <dc:description>Executive briefing and revenue breakdown.</dc:description>
    <cp:keywords>Finance, Revenue, Q3, TTZip</cp:keywords>
    <cp:lastModifiedBy>Senior Auditor</cp:lastModifiedBy>
    <cp:revision>4</cp:revision>
    <dcterms:created xsi:type="dcterms:W3CDTF">2026-09-01T09:00:00Z</dcterms:created>
    <dcterms:modified xsi:type="dcterms:W3CDTF">2026-09-01T15:30:00Z</dcterms:modified>
    <cp:category>Corporate Finance</cp:category>
    <cp:contentStatus>Final Approved</cp:contentStatus>
</cp:coreProperties>"#;

    let props = OfficeXmlExtractor::parse_core_properties(xml).unwrap();
    assert_eq!(props.title.as_deref(), Some("Q3 Financial Summary"));
    assert_eq!(props.subject.as_deref(), Some("Quarterly Financial Results"));
    assert_eq!(props.creator.as_deref(), Some("Witt Kung"));
    assert_eq!(props.description.as_deref(), Some("Executive briefing and revenue breakdown."));
    assert_eq!(props.keywords.as_deref(), Some("Finance, Revenue, Q3, TTZip"));
    assert_eq!(props.last_modified_by.as_deref(), Some("Senior Auditor"));
    assert_eq!(props.revision.as_deref(), Some("4"));
    assert_eq!(props.created.as_deref(), Some("2026-09-01T09:00:00Z"));
    assert_eq!(props.modified.as_deref(), Some("2026-09-01T15:30:00Z"));
    assert_eq!(props.category.as_deref(), Some("Corporate Finance"));
    assert_eq!(props.content_status.as_deref(), Some("Final Approved"));
}

#[test]
fn test_office_app_properties_extraction() {
    let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"
            xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
    <Application>Microsoft Macintosh Excel</Application>
    <AppVersion>16.0300</AppVersion>
    <TotalTime>120</TotalTime>
    <Pages>5</Pages>
    <Words>1450</Words>
    <Characters>8700</Characters>
    <CharactersWithSpaces>10150</CharactersWithSpaces>
    <Lines>120</Lines>
    <Paragraphs>35</Paragraphs>
    <Slides>12</Slides>
    <Notes>3</Notes>
    <HiddenSlides>1</HiddenSlides>
    <Company>Antigravity Technologies</Company>
</Properties>"#;

    let props = OfficeXmlExtractor::parse_app_properties(xml).unwrap();
    assert_eq!(props.application.as_deref(), Some("Microsoft Macintosh Excel"));
    assert_eq!(props.app_version.as_deref(), Some("16.0300"));
    assert_eq!(props.total_time_mins, Some(120));
    assert_eq!(props.pages, Some(5));
    assert_eq!(props.words, Some(1450));
    assert_eq!(props.characters, Some(8700));
    assert_eq!(props.characters_with_spaces, Some(10150));
    assert_eq!(props.lines, Some(120));
    assert_eq!(props.paragraphs, Some(35));
    assert_eq!(props.slides, Some(12));
    assert_eq!(props.notes, Some(3));
    assert_eq!(props.hidden_slides, Some(1));
    assert_eq!(props.company.as_deref(), Some("Antigravity Technologies"));
}

// ============================================================================
// 2. DOCX WordprocessingML Outline & Headings
// ============================================================================

#[test]
fn test_docx_document_outline_and_headings() {
    let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
    <w:body>
        <w:p>
            <w:pPr><w:pStyle w:val="Title"/></w:pPr>
            <w:r><w:t>Project Architecture Specification</w:t></w:r>
        </w:p>
        <w:p>
            <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
            <w:r><w:t>1. Overview</w:t></w:r>
        </w:p>
        <w:p>
            <w:r><w:t>This document defines the high-level architecture of TTZip microkernel.</w:t></w:r>
        </w:p>
        <w:p>
            <w:pPr><w:pStyle w:val="Heading2"/></w:pPr>
            <w:r><w:t>1.1 Memory Architecture</w:t></w:r>
        </w:p>
        <w:p>
            <w:r><w:t>Zero-copy buffers are used across all FFI boundaries.</w:t></w:r>
        </w:p>
    </w:body>
</w:document>"#;

    let outline = OfficeXmlExtractor::parse_docx_document(xml).unwrap();
    assert_eq!(outline.paragraph_count, 5);
    assert_eq!(outline.headings.len(), 3);

    assert_eq!(outline.headings[0].level, 0);
    assert_eq!(outline.headings[0].style, "Title");
    assert_eq!(outline.headings[0].text, "Project Architecture Specification");

    assert_eq!(outline.headings[1].level, 1);
    assert_eq!(outline.headings[1].style, "Heading1");
    assert_eq!(outline.headings[1].text, "1. Overview");

    assert_eq!(outline.headings[2].level, 2);
    assert_eq!(outline.headings[2].style, "Heading2");
    assert_eq!(outline.headings[2].text, "1.1 Memory Architecture");

    assert!(outline.full_text.contains("Zero-copy buffers are used"));
}

// ============================================================================
// 3. XLSX SpreadsheetML Workbook & Shared Strings
// ============================================================================

#[test]
fn test_xlsx_workbook_and_shared_strings_compliance() {
    let wb_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
    <workbookPr date1904="true"/>
    <sheets>
        <sheet name="Income Statement" sheetId="1" state="visible" r:id="rId1"/>
        <sheet name="Balance Sheet" sheetId="2" state="visible" r:id="rId2"/>
        <sheet name="Hidden Metrics" sheetId="3" state="hidden" r:id="rId3"/>
    </sheets>
</workbook>"#;

    let meta = OfficeXmlExtractor::parse_xlsx_workbook(wb_xml).unwrap();
    assert!(meta.date_1904);
    assert_eq!(meta.sheets.len(), 3);

    assert_eq!(meta.sheets[0].name, "Income Statement");
    assert_eq!(meta.sheets[0].sheet_id, 1);
    assert_eq!(meta.sheets[0].r_id, "rId1");

    assert_eq!(meta.sheets[1].name, "Balance Sheet");
    assert_eq!(meta.sheets[2].state.as_deref(), Some("hidden"));

    let sst_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="4" uniqueCount="3">
    <si><t>Revenue</t></si>
    <si><t>Operating Expenses</t></si>
    <si><t>Net Income</t></si>
</sst>"#;

    let pool = OfficeXmlExtractor::parse_xlsx_shared_strings(sst_xml, None).unwrap();
    assert_eq!(pool.len(), 3);
    assert_eq!(pool[0], "Revenue");
    assert_eq!(pool[1], "Operating Expenses");
    assert_eq!(pool[2], "Net Income");

    let pool_limited = OfficeXmlExtractor::parse_xlsx_shared_strings(sst_xml, Some(2)).unwrap();
    assert_eq!(pool_limited.len(), 2);
}

// ============================================================================
// 4. PPTX PresentationML Slides
// ============================================================================

#[test]
fn test_pptx_slide_outline_compliance() {
    let slide_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
    <p:cSld>
        <p:spTree>
            <p:sp>
                <p:nvSpPr>
                    <p:nvPr><p:ph type="ctrTitle"/></p:nvPr>
                </p:nvSpPr>
                <p:txBody><a:p><a:r><a:t>Quarterly Executive Deck</a:t></a:r></a:p></p:txBody>
            </p:sp>
            <p:sp>
                <p:nvSpPr><p:nvPr><p:ph type="body"/></p:nvPr></p:nvSpPr>
                <p:txBody>
                    <a:p><a:r><a:t>Bullet 1: 45% YoY Growth</a:t></a:r></a:p>
                    <a:p><a:r><a:t>Bullet 2: Zero Defect Delivery</a:t></a:r></a:p>
                </p:txBody>
            </p:sp>
        </p:spTree>
    </p:cSld>
</p:sld>"#;

    let outline = OfficeXmlExtractor::parse_pptx_slide(slide_xml, 1).unwrap();
    assert_eq!(outline.slide_number, 1);
    assert_eq!(outline.title.as_deref(), Some("Quarterly Executive Deck"));
    assert_eq!(outline.text_boxes.len(), 2);
    assert!(outline.full_text.contains("45% YoY Growth"));
    assert!(outline.full_text.contains("Zero Defect Delivery"));
}

// ============================================================================
// 5. UniFFI High-Level Extraction Facade
// ============================================================================

#[test]
fn test_uniffi_office_metadata_and_outline_pipeline() {
    let core_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Compliance Master Document</dc:title>
    <dc:creator>Witt Kung</dc:creator>
</cp:coreProperties>"#;

    let app_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
    <Application>TTZip Test Suite</Application>
    <Pages>3</Pages>
    <Words>120</Words>
</Properties>"#;

    let doc_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
    <w:body>
        <w:p><w:pPr><w:pStyle w:val="Heading 1"/></w:pPr><w:r><w:t>Introduction</w:t></w:r></w:p>
        <w:p><w:r><w:t>Body text of the compliance document.</w:t></w:r></w:p>
    </w:body>
</w:document>"#;

    let files = [
        ("docProps/core.xml", core_xml.as_slice()),
        ("docProps/app.xml", app_xml.as_slice()),
        ("word/document.xml", doc_xml.as_slice()),
    ];
    let zip_bytes = create_test_zip(&files);

    let meta = parse_office_metadata_from_slice(&zip_bytes).unwrap();
    assert_eq!(meta.format_name, "DOCX");
    assert_eq!(meta.title.as_deref(), Some("Compliance Master Document"));
    assert_eq!(meta.author.as_deref(), Some("Witt Kung"));
    assert_eq!(meta.page_count, 3);
    assert_eq!(meta.word_count, 120);

    let outline = parse_office_outline_from_slice(&zip_bytes).unwrap();
    assert_eq!(outline.document_type, "Word Processing");
    assert_eq!(outline.headings.len(), 1);
    assert_eq!(outline.headings[0], "Introduction");
    assert!(outline.summary_preview.contains("Body text"));
}

// ============================================================================
// 6. ECMA-376 Dimension & Coordinate Oracles
// ============================================================================

#[test]
fn test_ecma376_sheet_coordinate_and_dimension_oracle() {
    let guard = SheetDimensionsGuard::default();

    assert_eq!(col_str_to_index("A"), Some(1));
    assert_eq!(col_str_to_index("Z"), Some(26));
    assert_eq!(col_str_to_index("AA"), Some(27));
    assert_eq!(col_str_to_index("XFD"), Some(16_384));

    assert_eq!(col_index_to_str(1), "A");
    assert_eq!(col_index_to_str(26), "Z");
    assert_eq!(col_index_to_str(27), "AA");
    assert_eq!(col_index_to_str(16_384), "XFD");

    let range = guard
        .parse_and_validate_dimension("B2:D10")
        .expect("Failed to parse standard dimension");
    assert_eq!(range.start_col, 2);
    assert_eq!(range.start_row, 2);
    assert_eq!(range.end_col, 4);
    assert_eq!(range.end_row, 10);
    assert_eq!(range.theoretical_cell_count(), 27);

    let coord = CellCoord::with_sheet("Financials", 28, 100);
    assert_eq!(coord.to_a1_string(), "Financials!AB100");
}

// ============================================================================
// 7. 6-Layer Security Defense Adversarial Test Matrix
// ============================================================================

#[test]
fn test_adversarial_formula_depth_and_token_bombs() {
    let strict_guard = FormulaDepthGuard::new(32, 1024);

    // 1. Stack overflow AST depth explosion (> 32 levels)
    let mut deep_formula = String::from("=1");
    for _ in 0..35 {
        deep_formula = format!("(1 + {})", deep_formula);
    }
    let depth_err = strict_guard.inspect_formula(&deep_formula);
    assert!(matches!(
        depth_err,
        Err(OfficeDefenseError::FormulaDepthExceeded { .. })
    ));

    // 2. Token exhaustion bomb (> 1024 tokens)
    let token_bomb = (0..1200)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(" + ");
    let full_bomb = format!("={}", token_bomb);
    let token_err = strict_guard.inspect_formula(&full_bomb);
    assert!(matches!(
        token_err,
        Err(OfficeDefenseError::FormulaTokensExceeded { .. })
    ));
}

#[test]
fn test_adversarial_tarjan_circular_reference_fuses() {
    let guard = FormulaDepthGuard::default();

    // 1. Direct self-reference loop: A1 -> A1
    let mut self_loop = HashMap::new();
    self_loop.insert("A1".to_string(), vec!["A1".to_string()]);
    let err1 = guard.verify_dependency_dag(&self_loop);
    assert!(matches!(
        err1,
        Err(OfficeDefenseError::FormulaCycleDetected { .. })
    ));

    // 2. Mutual 2-node cycle: A1 -> B1 -> A1
    let mut pair_cycle = HashMap::new();
    pair_cycle.insert("A1".to_string(), vec!["B1".to_string()]);
    pair_cycle.insert("B1".to_string(), vec!["A1".to_string()]);
    let err2 = guard.verify_dependency_dag(&pair_cycle);
    assert!(matches!(
        err2,
        Err(OfficeDefenseError::FormulaCycleDetected { .. })
    ));

    // 3. Complex 4-node cycle embedded in larger graph: A -> B -> C -> D -> B
    let mut complex = HashMap::new();
    complex.insert("Root".to_string(), vec!["A".to_string()]);
    complex.insert("A".to_string(), vec!["B".to_string()]);
    complex.insert("B".to_string(), vec!["C".to_string()]);
    complex.insert("C".to_string(), vec!["D".to_string()]);
    complex.insert("D".to_string(), vec!["B".to_string()]); // Cycle back to B
    let err3 = guard.verify_dependency_dag(&complex);
    assert!(matches!(
        err3,
        Err(OfficeDefenseError::FormulaCycleDetected { .. })
    ));
}

#[test]
fn test_adversarial_sheet_dimensions_and_sparse_oom() {
    let mut guard = SheetDimensionsGuard::new(
        MAX_SHEET_ROWS,
        MAX_SHEET_COLS,
        MAX_VIEWPORT_ACTIVE_CELLS,
    );

    // 1. Row index out of bounds (> 1,048,576)
    let row_err = guard.parse_and_validate_dimension("A1:A1048577");
    assert!(matches!(
        row_err,
        Err(OfficeDefenseError::RowOutOfBounds { .. })
    ));

    // 2. Column index out of bounds (> 16,384, e.g. XFE)
    let col_err = guard.parse_and_validate_dimension("A1:XFE1");
    assert!(matches!(
        col_err,
        Err(OfficeDefenseError::ColumnOutOfBounds { .. })
    ));

    // 3. Sparse matrix OOM attack: A1:XFD1048576 declares ~17.1 billion cell span.
    let full_span = guard.parse_and_validate_dimension("A1:XFD1048576").unwrap();
    assert_eq!(full_span.theoretical_cell_count(), 17_179_869_184);

    // Active cell streaming flooding
    assert!(guard.register_active_cells(99_000).is_ok());
    assert!(guard.register_active_cells(2_000).is_err()); // Exceeds 100,000 ceiling
}

#[test]
fn test_adversarial_sst_quota_and_hashdos_resistance() {
    let mut guard = SstQuotaGuard::new(100, 1024, 10 * 1024);

    // 1. Single string entry size explosion (> 1024 bytes)
    let huge_entry = "Z".repeat(2048);
    let size_err = guard.add_entry(&huge_entry);
    assert!(matches!(
        size_err,
        Err(OfficeDefenseError::SstEntryTooLarge { .. })
    ));

    // 2. Unique entry count ceiling (> 100 unique entries)
    for i in 0..100 {
        assert!(guard.add_entry(&format!("UniqueKey_{i}")).is_ok());
    }
    let quota_err = guard.add_entry("UniqueKey_101");
    assert!(matches!(
        quota_err,
        Err(OfficeDefenseError::SstUniqueEntriesExceeded { .. })
    ));
}

#[test]
fn test_adversarial_macro_dde_and_unc_injection() {
    let sandbox = OfficeMacroSandboxGuard::new();

    // 1. Macro & ActiveX physical stripping
    assert!(sandbox.should_strip_entry("xl/vbaProject.bin"));
    assert!(sandbox.should_strip_entry("word/vbaProject.bin"));
    assert!(sandbox.should_strip_entry("ppt/vbaProject.bin"));
    assert!(sandbox.should_strip_entry("vbaData.xml"));
    assert!(sandbox.should_strip_entry("word/activeX/activeX1.bin"));
    assert!(!sandbox.should_strip_entry("word/document.xml"));

    // 2. DDE / Command execution formula injection
    assert!(sandbox.inspect_formula_security("=cmd|'/c calc.exe'!A0").is_err());
    assert!(sandbox.inspect_formula_security("+cmd|'/c powershell.exe -enc ...'!A0").is_err());
    assert!(sandbox.inspect_formula_security("=DDE(\"cmd\", \"/c calc.exe\", \"\")").is_err());
    assert!(sandbox.inspect_formula_security("=DDEAUTO(\"cmd\", \"/c calc.exe\", \"\")").is_err());
    assert!(sandbox.inspect_formula_security("=EXEC(\"calc.exe\")").is_err());
    assert!(sandbox.inspect_formula_security("=HYPERLINK(\"powershell:iex(new-object...)\", \"Click\")").is_err());
    assert!(sandbox.inspect_formula_security("=HYPERLINK(\"cmd.exe /c calc\", \"Run\")").is_err());
    assert!(sandbox.inspect_formula_security("=AVERAGE(A1:B10)").is_ok());

    // 3. UNC Path & Remote Template Neutralization
    assert!(sandbox.sanitize_relationship_target(r"\\10.0.0.1\share\payload.dotm", Some("External"), "attachedTemplate").is_err());
    assert!(sandbox.sanitize_relationship_target("http://malicious.org/exploit.dotm", Some("External"), "http://schemas.openxmlformats.org/officeDocument/2006/relationships/attachedTemplate").is_err());
    assert!(sandbox.sanitize_relationship_target("ms-msdt:/id DEV /skip force /sync no", Some("External"), "hyperlink").is_err());
}

#[test]
fn test_adversarial_memory_budget_watchdog() {
    let budget = OfficeMemoryBudgetGuard::new(
        DEFAULT_MAX_OFFICE_BUDGET,
        16 * 1024 * 1024,
        32 * 1024 * 1024,
    );

    // 1. Allocate within budget
    let permit = budget.allocate(32 * 1024 * 1024).expect("Allocation failed");
    assert_eq!(budget.allocated_bytes(), 32 * 1024 * 1024);
    assert_eq!(budget.available_bytes(), 32 * 1024 * 1024);

    // 2. Exceed global budget (32MB + 40MB = 72MB > 64MB)
    let overflow_err = budget.allocate(40 * 1024 * 1024);
    assert!(matches!(
        overflow_err,
        Err(OfficeDefenseError::MemoryBudgetExceeded { .. })
    ));

    // 3. RAII auto-release on drop
    drop(permit);
    assert_eq!(budget.allocated_bytes(), 0);
    assert_eq!(budget.available_bytes(), DEFAULT_MAX_OFFICE_BUDGET);
}

#[test]
fn test_adversarial_sensitive_buffer_zeroize() {
    let mut sensitive_buf = SensitiveOfficeBuffer::with_capacity(64);
    sensitive_buf.extend_from_slice(b"TOP_SECRET_EXECUTIVE_SALARIES");
    assert_eq!(sensitive_buf.as_str().unwrap(), "TOP_SECRET_EXECUTIVE_SALARIES");

    // Redacted debug output
    let debug_repr = format!("{:?}", sensitive_buf);
    assert!(!debug_repr.contains("TOP_SECRET"));
    assert!(debug_repr.contains("[REDACTED_SENSITIVE_OFFICE_DATA]"));

    sensitive_buf.clear();
    assert!(sensitive_buf.is_empty());
}

// ============================================================================
// 7. End-to-End Office Security Pipeline Orchestration
// ============================================================================

#[test]
fn test_end_to_end_office_security_pipeline() {
    let config = OfficeSecurityConfig::default();
    let mut pipeline = OfficeSecurityPipeline::new(config);

    // 1. Package entry filtering
    assert!(!pipeline.filter_archive_entry("xl/vbaProject.bin"));
    assert!(!pipeline.filter_archive_entry("word/activeX/activeX1.bin"));
    assert!(pipeline.filter_archive_entry("xl/worksheets/sheet1.xml"));
    assert!(pipeline.filter_archive_entry("word/document.xml"));

    // 2. Formula evaluation validation
    let insp = pipeline
        .validate_formula("=SUM(A1:A5) + IF(B1 > 0, 10, -10)")
        .expect("Valid pipeline formula failed");
    assert!(insp.max_depth <= MAX_FORMULA_DEPTH);
    assert!(insp.token_count <= MAX_FORMULA_TOKENS);

    // 3. DDE injection block in pipeline
    let dde_err = pipeline.validate_formula("=cmd|'/C calc'!A0");
    assert!(matches!(
        dde_err,
        Err(OfficeDefenseError::DdeCommandBlocked { .. })
    ));

    // 4. Bounding box validation
    let range = pipeline
        .validate_sheet_dimension("A1:Z500")
        .expect("Valid dimension failed in pipeline");
    assert_eq!(range.end_col, 26);
    assert_eq!(range.end_row, 500);

    // 5. Memory permit validation
    {
        let permit = pipeline.allocate_memory(1024 * 1024).unwrap();
        assert_eq!(permit.size(), 1024 * 1024);
    }

    // 6. Security report verification
    let report = pipeline.generate_report();
    assert_eq!(report.inspected_formulas, 1);
    assert_eq!(report.stripped_entries.len(), 2);
}

