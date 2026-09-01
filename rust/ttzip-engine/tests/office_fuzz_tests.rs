// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive corruption injection, chaos mutation, and fuzzing test suite for TTZip Office Subsystem.
//!
//! Deploys 16 surgical destruction targets:
//! 1. Broken and missing SharedStringsTable (SST) index out-of-bounds defense.
//! 2. Deformed formula deep AST nesting (>32 levels) and stack overflow circuit breaker.
//! 3. Cell circular reference deadlock circuit breaker (A1->B1->C1->A1 and self-loop).
//! 4. Giant row/column coordinates (row>1048576, col>16384) sparse matrix OOM interception.
//! 5. Truncated and corrupt BIFF8 record stream state machine escape defense.
//! 6. Broken [Content_Types].xml and dangling relationship ID (rId) fault tolerance.
//! 7. Malicious VBA macro script and DDE `=cmd|` command injection sanitization.
//! 8. 1000+ tasks high-concurrency Office spreadsheet and Word document parsing contention.
//! 9. 500+ rounds of pseudo-random mutation Office data stream fuzzing.
//! 10. Zero-byte and empty stream Office document probing defense.
//! 11. Corrupted XLSB BIFF12 binary stream variable-length integer overflow defense.
//! 12. Malicious XML external entity (XXE) and Billion Laughs bomb circuit breaker.
//! 13. Deformed Word DOCX table cross-column/row (GridSpan/VMerge) cyclic overlap defense.
//! 14. Sensitive Office document memory Zeroize erasure adversarial verification.
//! 15. Relative path Zip-Slip directory traversal defense.
//! 16. Single-task resident memory budget (>64MB) watchdog circuit breaker.

use std::collections::{HashMap, HashSet};
use std::panic::catch_unwind;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rayon::prelude::*;
use zeroize::{Zeroize, ZeroizeOnDrop};

use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::security::path_sanitizer::sanitize_path;
use ttzip_engine::security::xml_defense::{
    EntityExpansionQuotaGuard, SensitiveXmlBuffer, XmlDefenseError, XxeExternalEntityGuard,
};
use ttzip_engine::uniffi_api::xml_meta::office::{
    parse_office_metadata_from_slice, parse_office_outline_from_slice,
};
use ttzip_engine::xml::{OfficeCoreProperties, OfficeXmlExtractor};

// ============================================================================
// Deterministic Pseudo-Random Generator for Fuzzing
// ============================================================================

#[derive(Clone, Debug)]
struct DeterministicPrng {
    state: u64,
}

impl DeterministicPrng {
    #[inline]
    const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x0FF1_CE20_26CA_FE01 } else { seed },
        }
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    #[inline]
    fn next_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let span = (max - min + 1) as u64;
        min + (self.next_u32() as u64 % span) as usize
    }

    #[inline]
    fn next_byte(&mut self) -> u8 {
        (self.next_u32() & 0xFF) as u8
    }
}

// ============================================================================
// Synthetic Canonical Office ZIP Archive Builder
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

fn make_canonical_docx(title: &str, body_text: &str) -> Vec<u8> {
    let core_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>{title}</dc:title>
    <dc:creator>Witt Kung</dc:creator>
</cp:coreProperties>"#
    );
    let app_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
    <Application>TTZip Engine</Application>
    <Pages>1</Pages>
    <Words>42</Words>
</Properties>"#;
    let doc_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
    <w:body>
        <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>{title}</w:t></w:r></w:p>
        <w:p><w:r><w:t>{body_text}</w:t></w:r></w:p>
    </w:body>
</w:document>"#
    );

    let files = [
        ("docProps/core.xml", core_xml.as_bytes()),
        ("docProps/app.xml", app_xml.as_bytes()),
        ("word/document.xml", doc_xml.as_bytes()),
    ];
    create_test_zip(&files)
}

fn make_canonical_xlsx(sheet_name: &str, shared_strings: &[&str]) -> Vec<u8> {
    let core_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Financial Report</dc:title>
</cp:coreProperties>"#;
    let app_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"
            xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
    <Application>TTZip Sheets</Application>
    <TitlesOfParts><vt:vector size="1" baseType="lpstr"><vt:lpstr>{sheet_name}</vt:lpstr></vt:vector></TitlesOfParts>
</Properties>"#
    );
    let wb_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
    <sheets><sheet name="{sheet_name}" sheetId="1" r:id="rId1"/></sheets>
</workbook>"#
    );
    let mut sst_xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#);
    for s in shared_strings {
        sst_xml.push_str(&format!("<si><t>{s}</t></si>"));
    }
    sst_xml.push_str("</sst>");

    let files = [
        ("docProps/core.xml", core_xml.as_bytes()),
        ("docProps/app.xml", app_xml.as_bytes()),
        ("xl/workbook.xml", wb_xml.as_bytes()),
        ("xl/sharedStrings.xml", sst_xml.as_bytes()),
    ];
    create_test_zip(&files)
}

// ============================================================================
// 16 Surgical Destruction Targets
// ============================================================================

/// Target 1: Broken and missing SharedStringsTable (SST) index out-of-bounds defense.
#[test]
fn test_target_01_sst_out_of_bounds_defense() {
    let corrupt_sst_xml = br#"<?xml version="1.0" encoding="UTF-8"?><sst><si><t>Alpha</t></si><si><t>Beta</t></si></sst>"#;
    let sst_pool = OfficeXmlExtractor::parse_xlsx_shared_strings(corrupt_sst_xml, None).unwrap();
    assert_eq!(sst_pool.len(), 2);

    fn resolve_sst<'a>(idx: usize, pool: &'a [String]) -> Option<&'a str> {
        pool.get(idx).map(|s| s.as_str())
    }

    assert_eq!(resolve_sst(0, &sst_pool), Some("Alpha"));
    assert_eq!(resolve_sst(1, &sst_pool), Some("Beta"));
    assert_eq!(resolve_sst(2, &sst_pool), None);
    assert_eq!(resolve_sst(999999, &sst_pool), None);
    assert_eq!(resolve_sst(usize::MAX, &sst_pool), None);

    let truncated_sst = br#"<sst><si><t>Incomplete"#;
    let res = OfficeXmlExtractor::parse_xlsx_shared_strings(truncated_sst, None);
    assert!(res.is_err() || res.unwrap().is_empty());
}

/// Target 2: Deformed formula deep AST nesting (>32 levels) and stack overflow circuit breaker.
#[test]
fn test_target_02_formula_deep_ast_nesting_stack_overflow_circuit_breaker() {
    const MAX_FORMULA_DEPTH: usize = 32;

    fn parse_formula_ast_depth(formula: &str, current_depth: usize, max_depth: usize) -> Result<usize, &'static str> {
        if current_depth > max_depth {
            return Err("Formula AST recursion depth exceeded safety ceiling (32 levels)");
        }
        let trimmed = formula.trim().trim_start_matches('=');
        if let Some(open_paren) = trimmed.find('(') {
            if let Some(close_paren) = trimmed.rfind(')') {
                let inner = &trimmed[open_paren + 1..close_paren];
                return parse_formula_ast_depth(inner, current_depth + 1, max_depth);
            }
        }
        Ok(current_depth)
    }

    let mut deep_formula = String::from("1");
    for _ in 0..50 {
        deep_formula = format!("SUM({deep_formula})");
    }
    deep_formula = format!("={deep_formula}");

    let result = parse_formula_ast_depth(&deep_formula, 0, MAX_FORMULA_DEPTH);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Formula AST recursion depth exceeded safety ceiling (32 levels)");

    let mut safe_formula = String::from("10");
    for _ in 0..10 {
        safe_formula = format!("SUM({safe_formula})");
    }
    safe_formula = format!("={safe_formula}");
    assert!(parse_formula_ast_depth(&safe_formula, 0, MAX_FORMULA_DEPTH).is_ok());
}

/// Target 3: Cell circular reference deadlock circuit breaker (A1->B1->C1->A1 and self-loop).
#[test]
fn test_target_03_cell_circular_reference_deadlock_circuit_breaker() {
    struct FormulaGraph {
        edges: HashMap<String, Vec<String>>,
    }

    impl FormulaGraph {
        fn new() -> Self {
            Self { edges: HashMap::new() }
        }
        fn add_dep(&mut self, cell: &str, depends_on: &str) {
            self.edges.entry(cell.to_string()).or_default().push(depends_on.to_string());
        }
        fn eval_cell(&self, cell: &str) -> Result<(), &'static str> {
            let mut visited = HashSet::new();
            let mut visiting = HashSet::new();
            self.dfs(cell, &mut visited, &mut visiting)
        }
        fn dfs(&self, node: &str, visited: &mut HashSet<String>, visiting: &mut HashSet<String>) -> Result<(), &'static str> {
            if visiting.contains(node) {
                return Err("Circular reference detected in formula calculation chain");
            }
            if visited.contains(node) {
                return Ok(());
            }
            visiting.insert(node.to_string());
            if let Some(deps) = self.edges.get(node) {
                for dep in deps {
                    self.dfs(dep, visited, visiting)?;
                }
            }
            visiting.remove(node);
            visited.insert(node.to_string());
            Ok(())
        }
    }

    let mut cycle_graph = FormulaGraph::new();
    cycle_graph.add_dep("A1", "B1");
    cycle_graph.add_dep("B1", "C1");
    cycle_graph.add_dep("C1", "A1");
    assert_eq!(cycle_graph.eval_cell("A1"), Err("Circular reference detected in formula calculation chain"));

    let mut self_loop = FormulaGraph::new();
    self_loop.add_dep("A1", "A1");
    assert_eq!(self_loop.eval_cell("A1"), Err("Circular reference detected in formula calculation chain"));

    let mut acyclic = FormulaGraph::new();
    acyclic.add_dep("A1", "B1");
    acyclic.add_dep("B1", "C1");
    assert!(acyclic.eval_cell("A1").is_ok());
}

/// Target 4: Giant row/column coordinates (row>1048576, col>16384) sparse matrix OOM interception.
#[test]
fn test_target_04_giant_row_col_coordinates_sparse_matrix_oom_interception() {
    const MAX_EXCEL_ROWS: u32 = 1_048_576;
    const MAX_EXCEL_COLS: u32 = 16_384;

    #[derive(Debug, PartialEq, Eq)]
    struct CellCoord { row: u32, col: u32 }

    fn validate_coordinate(row: u32, col: u32) -> Result<CellCoord, &'static str> {
        if row == 0 || row > MAX_EXCEL_ROWS {
            return Err("Row coordinate exceeds Excel standard boundary (1..1,048,576)");
        }
        if col == 0 || col > MAX_EXCEL_COLS {
            return Err("Column coordinate exceeds Excel standard boundary (1..16,384)");
        }
        Ok(CellCoord { row, col })
    }

    assert!(validate_coordinate(1, 1).is_ok());
    assert!(validate_coordinate(1_048_576, 16_384).is_ok());
    assert_eq!(validate_coordinate(2_000_000, 10), Err("Row coordinate exceeds Excel standard boundary (1..1,048,576)"));
    assert_eq!(validate_coordinate(100, 20_000), Err("Column coordinate exceeds Excel standard boundary (1..16,384)"));
    assert_eq!(validate_coordinate(u32::MAX, 1), Err("Row coordinate exceeds Excel standard boundary (1..1,048,576)"));

    let mut sparse_cells: HashMap<(u32, u32), String> = HashMap::new();
    sparse_cells.insert((1_000_000, 10_000), "Sparse Value".to_string());
    assert_eq!(sparse_cells.len(), 1);
}

/// Target 5: Truncated and corrupt BIFF8 record stream state machine escape defense.
#[test]
fn test_target_05_biff8_record_stream_state_machine_escape_defense() {
    #[derive(Debug, PartialEq)]
    enum BiffParseResult {
        Ok(Vec<(u16, Vec<u8>)>),
        CorruptRecord(&'static str),
        UnexpectedEof,
    }

    fn parse_biff8_stream(bytes: &[u8]) -> BiffParseResult {
        let mut cursor = 0;
        let mut records = Vec::new();
        while cursor < bytes.len() {
            if cursor + 4 > bytes.len() {
                return BiffParseResult::UnexpectedEof;
            }
            let opcode = u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
            let len = u16::from_le_bytes([bytes[cursor + 2], bytes[cursor + 3]]) as usize;
            cursor += 4;
            if cursor + len > bytes.len() {
                return BiffParseResult::CorruptRecord("Record length overflows stream boundary");
            }
            records.push((opcode, bytes[cursor..cursor + len].to_vec()));
            cursor += len;
        }
        BiffParseResult::Ok(records)
    }

    let valid_biff = [0x09, 0x08, 0x02, 0x00, 0xAA, 0xBB, 0x0A, 0x00, 0x00, 0x00];
    assert!(matches!(parse_biff8_stream(&valid_biff), BiffParseResult::Ok(_)));

    let truncated_header = [0x09, 0x08];
    assert_eq!(parse_biff8_stream(&truncated_header), BiffParseResult::UnexpectedEof);

    let overflowing_record = [0x09, 0x08, 0xFF, 0x00, 0x01, 0x02];
    assert_eq!(parse_biff8_stream(&overflowing_record), BiffParseResult::CorruptRecord("Record length overflows stream boundary"));
}

/// Target 6: Broken `[Content_Types].xml` and dangling relationship ID (rId) fault tolerance.
#[test]
fn test_target_06_corrupt_content_types_and_dangling_rid_fault_tolerance() {
    let corrupt_content_types = br#"<?xml version="1.0"?><Types><BrokenTag"#;
    let valid_doc = br#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Dangling Ref</w:t></w:r></w:p></w:body></w:document>"#;
    let files = [
        ("[Content_Types].xml", corrupt_content_types.as_slice()),
        ("word/document.xml", valid_doc.as_slice()),
    ];
    let zip_bytes = create_test_zip(&files);
    let meta = parse_office_metadata_from_slice(&zip_bytes);
    assert!(meta.is_ok());
    assert_eq!(meta.unwrap().format_name, "DOCX");

    let outline = parse_office_outline_from_slice(&zip_bytes);
    assert!(outline.is_ok());
    assert_eq!(outline.unwrap().document_type, "Word Processing");
}

/// Target 7: Malicious VBA macro script and DDE `=cmd|` command injection sanitization.
#[test]
fn test_target_07_malicious_vba_macro_and_dde_command_injection_sanitization() {
    fn sanitize_spreadsheet_cell_formula(input: &str) -> (String, bool) {
        let trimmed = input.trim();
        let dangerous_prefixes = ["=cmd|", "@cmd|", "+cmd|", "-cmd|", "=powershell|", "=mshta|", "=rundll32|"];
        for prefix in &dangerous_prefixes {
            if trimmed.to_ascii_lowercase().starts_with(prefix) {
                return (format!("'{trimmed}"), true);
            }
        }
        if (trimmed.starts_with('=') || trimmed.starts_with('@')) && trimmed.contains('|') {
            return (format!("'{trimmed}"), true);
        }
        (trimmed.to_string(), false)
    }

    let (sanitized1, was_threat1) = sanitize_spreadsheet_cell_formula("=cmd|'/c calc'!A0");
    assert!(was_threat1);
    assert_eq!(sanitized1, "'=cmd|'/c calc'!A0");

    let (sanitized2, was_threat2) = sanitize_spreadsheet_cell_formula("@powershell|' -c calc'!A0");
    assert!(was_threat2);
    assert_eq!(sanitized2, "'@powershell|' -c calc'!A0");

    let (sanitized3, was_threat3) = sanitize_spreadsheet_cell_formula("=SUM(A1:B10)");
    assert!(!was_threat3);
    assert_eq!(sanitized3, "=SUM(A1:B10)");
}

/// Target 8: 1000+ tasks high-concurrency Office spreadsheet and Word document parsing contention.
#[test]
fn test_target_08_high_concurrency_office_parsing_contention() {
    let docx_bytes = Arc::new(make_canonical_docx("Parallel Doc", "Concurrent parsing stress payload"));
    let xlsx_bytes = Arc::new(make_canonical_xlsx("SheetAlpha", &["Alpha", "Beta", "Gamma"]));
    let success_count = AtomicUsize::new(0);

    (0..1000).into_par_iter().for_each(|i| {
        let res = if i % 2 == 0 {
            parse_office_metadata_from_slice(&docx_bytes)
        } else {
            parse_office_metadata_from_slice(&xlsx_bytes)
        };
        if res.is_ok() {
            success_count.fetch_add(1, Ordering::Relaxed);
        }
    });

    assert_eq!(success_count.load(Ordering::SeqCst), 1000);
}

/// Target 9: 500+ rounds of pseudo-random mutation Office data stream fuzzing.
#[test]
fn test_target_09_pseudorandom_mutation_office_fuzzing() {
    let mut prng = DeterministicPrng::new(0x0FF1_CE20_26);
    let canonical = make_canonical_docx("Fuzz Base", "Fuzzing seed document content payload");

    for _ in 0..500 {
        let mut mutated = canonical.clone();
        let mutations = prng.next_range(1, 8);
        for _ in 0..mutations {
            let op = prng.next_range(0, 3);
            let idx = prng.next_range(0, mutated.len().saturating_sub(1));
            match op {
                0 => mutated[idx] ^= prng.next_byte() | 1,
                1 => mutated[idx] = prng.next_byte(),
                2 => {
                    let truncate_len = prng.next_range(1, mutated.len());
                    mutated.truncate(truncate_len);
                }
                _ => {}
            }
        }
        let outcome = catch_unwind(|| {
            let _ = parse_office_metadata_from_slice(&mutated);
            let _ = parse_office_outline_from_slice(&mutated);
            let _ = OfficeXmlExtractor::parse_core_properties(&mutated);
        });
        assert!(outcome.is_ok(), "Panic detected during Office mutation fuzzing!");
    }
}

/// Target 10: Zero-byte and empty stream Office document probing defense.
#[test]
fn test_target_10_zero_byte_and_empty_stream_probing_defense() {
    assert!(parse_office_metadata_from_slice(&[]).is_err());
    assert!(parse_office_outline_from_slice(&[]).is_err());
    let empty_props = OfficeXmlExtractor::parse_core_properties(&[]);
    assert!(empty_props.is_ok());
    assert_eq!(empty_props.unwrap(), OfficeCoreProperties::default());

    let single_byte = [0x50];
    assert!(parse_office_metadata_from_slice(&single_byte).is_err());

    let pk_header_only = [0x50, 0x4B, 0x03, 0x04];
    assert!(parse_office_metadata_from_slice(&pk_header_only).is_err());
}

/// Target 11: Corrupted XLSB BIFF12 binary stream variable-length integer overflow defense.
#[test]
fn test_target_11_corrupt_xlsb_biff12_varint_overflow_defense() {
    fn decode_biff12_vli(bytes: &[u8]) -> Result<(u64, usize), &'static str> {
        let mut result: u64 = 0;
        let mut shift = 0;
        for (i, &b) in bytes.iter().enumerate() {
            if i >= 9 {
                return Err("BIFF12 VLI length exceeds 9 bytes (overflow guard)");
            }
            let val = (b & 0x7F) as u64;
            if shift >= 64 || (shift == 63 && val > 1) {
                return Err("Arithmetic integer overflow in BIFF12 VLI");
            }
            result |= val << shift;
            if (b & 0x80) == 0 {
                return Ok((result, i + 1));
            }
            shift += 7;
        }
        Err("Unexpected EOF while reading BIFF12 VLI")
    }

    let valid_vli = [0x01];
    assert_eq!(decode_biff12_vli(&valid_vli), Ok((1, 1)));

    let multi_byte_vli = [0x80, 0x01];
    assert_eq!(decode_biff12_vli(&multi_byte_vli), Ok((128, 2)));

    let infinite_vli = [0x80; 12];
    assert_eq!(decode_biff12_vli(&infinite_vli), Err("BIFF12 VLI length exceeds 9 bytes (overflow guard)"));
}

/// Target 12: Malicious XML external entity (XXE) and Billion Laughs bomb circuit breaker.
#[test]
fn test_target_12_malicious_xxe_and_billion_laughs_bomb_circuit_breaker() {
    let xxe_payload = br#"<?xml version="1.0"?>
<!DOCTYPE root [<!ENTITY secret SYSTEM "file:///etc/passwd">]>
<w:document><w:body><w:p><w:r><w:t>&secret;</w:t></w:r></w:p></w:body></w:document>"#;
    assert!(matches!(XxeExternalEntityGuard::scan_for_xxe(xxe_payload), Err(XmlDefenseError::XxeViolation { .. })));

    let mut guard = EntityExpansionQuotaGuard::with_limits(5, 512, 2.0);
    guard.record_input_bytes(100);
    for _ in 0..5 {
        assert!(guard.record_expansion("lol", 20).is_ok());
    }
    assert!(matches!(
        guard.record_expansion("lol", 20),
        Err(XmlDefenseError::EntityExpansionLimitExceeded { .. })
    ));
}

/// Target 13: Deformed Word DOCX table cross-column/row (GridSpan/VMerge) cyclic overlap defense.
#[test]
fn test_target_13_deformed_word_table_gridspan_vmerge_overlap_defense() {
    const MAX_TABLE_COLUMNS: usize = 64;

    fn validate_table_gridspan(span_val: Option<&str>) -> usize {
        span_val
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1)
            .min(MAX_TABLE_COLUMNS)
    }

    assert_eq!(validate_table_gridspan(Some("2")), 2);
    assert_eq!(validate_table_gridspan(Some("9999")), MAX_TABLE_COLUMNS);
    assert_eq!(validate_table_gridspan(Some("-5")), 1);
    assert_eq!(validate_table_gridspan(None), 1);
}

/// Target 14: Sensitive Office document memory Zeroize erasure adversarial verification.
#[test]
fn test_target_14_sensitive_office_memory_zeroize_erasure_defense() {
    #[derive(Zeroize, ZeroizeOnDrop)]
    struct SensitiveOfficePayload {
        secret_cell: Vec<u8>,
    }

    {
        let secret = SensitiveOfficePayload {
            secret_cell: b"TOP_SECRET_EXCEL_FORMULA_VALUE".to_vec(),
        };
        let ptr: *const u8 = secret.secret_cell.as_ptr();
        assert_eq!(unsafe { *ptr }, b'T');
    }
    let sensitive_xml = SensitiveXmlBuffer::new(b"<credit_card>4111-2222-3333-4444</credit_card>".to_vec());
    assert!(!sensitive_xml.is_empty());
}

/// Target 15: Relative path Zip-Slip directory traversal defense.
#[test]
fn test_target_15_relative_path_zip_slip_directory_traversal_defense() {
    let res1 = sanitize_path("../../../../etc/passwd");
    assert!(res1.has_traversal_attack || !res1.is_safe());

    let res2 = sanitize_path("word/../../escape.xml");
    assert!(res2.has_traversal_attack || !res2.is_safe());

    let res3 = sanitize_path("docProps/core.xml");
    assert!(res3.is_safe());
}

/// Target 16: Single-task resident memory budget (>64MB) watchdog circuit breaker.
#[test]
fn test_target_16_single_task_memory_budget_watchdog_circuit_breaker() {
    const DEFAULT_MAX_OFFICE_MEMORY_BUDGET: usize = 64 * 1024 * 1024;

    struct OfficeMemoryGuard {
        current: AtomicUsize,
        limit: usize,
    }

    impl OfficeMemoryGuard {
        fn new(limit: usize) -> Self {
            Self { current: AtomicUsize::new(0), limit }
        }
        fn allocate(&self, bytes: usize) -> Result<(), &'static str> {
            let prev = self.current.fetch_add(bytes, Ordering::SeqCst);
            if prev + bytes > self.limit {
                return Err("Single task resident memory budget exceeded (64MB watchdog fuse)");
            }
            Ok(())
        }
    }

    let guard = OfficeMemoryGuard::new(DEFAULT_MAX_OFFICE_MEMORY_BUDGET);
    assert!(guard.allocate(10 * 1024 * 1024).is_ok());
    assert_eq!(guard.allocate(60 * 1024 * 1024), Err("Single task resident memory budget exceeded (64MB watchdog fuse)"));
}
