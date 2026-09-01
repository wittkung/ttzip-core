// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use std::collections::HashMap;
use std::io::Cursor;

use super::*;

#[test]
fn test_formula_depth_guard_valid_and_exceeded() {
    let guard = FormulaDepthGuard::new(5, 100);

    // Depth 3 is within limit 5
    let res = guard.inspect_formula("=SUM(IF(A1>0, SIN(B1), COS(C1)))");
    assert!(res.is_ok());
    let inspection = res.unwrap();
    assert!(inspection.max_depth <= 5);
    assert!(!inspection.referenced_cells.is_empty());

    // Depth 6 exceeds limit 5
    let deep = "=(1+(2*(3+(4*(5+(6*7))))))";
    let err = guard.inspect_formula(deep);
    assert!(matches!(
        err,
        Err(OfficeDefenseError::FormulaDepthExceeded { depth: 6, limit: 5 })
    ));
}

#[test]
fn test_formula_tokens_limit_exceeded() {
    let guard = FormulaDepthGuard::new(32, 10);
    let many_tokens = "=1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10 + 11";
    let err = guard.inspect_formula(many_tokens);
    assert!(matches!(
        err,
        Err(OfficeDefenseError::FormulaTokensExceeded { .. })
    ));
}

#[test]
fn test_formula_tarjan_cycle_detection() {
    let guard = FormulaDepthGuard::default();

    // Valid DAG: A1 -> B1 -> C1, A1 -> C1
    let mut valid_dag = HashMap::new();
    valid_dag.insert("A1".to_string(), vec!["B1".to_string(), "C1".to_string()]);
    valid_dag.insert("B1".to_string(), vec!["C1".to_string()]);
    valid_dag.insert("C1".to_string(), vec![]);
    assert!(guard.verify_dependency_dag(&valid_dag).is_ok());

    // Self loop: A1 -> A1
    let mut self_loop = HashMap::new();
    self_loop.insert("A1".to_string(), vec!["A1".to_string()]);
    let err = guard.verify_dependency_dag(&self_loop);
    assert!(matches!(err, Err(OfficeDefenseError::FormulaCycleDetected { .. })));

    // 3-Node cycle: A1 -> B1 -> C1 -> A1
    let mut cycle_graph = HashMap::new();
    cycle_graph.insert("A1".to_string(), vec!["B1".to_string()]);
    cycle_graph.insert("B1".to_string(), vec!["C1".to_string()]);
    cycle_graph.insert("C1".to_string(), vec!["A1".to_string()]);
    let err = guard.verify_dependency_dag(&cycle_graph);
    assert!(matches!(err, Err(OfficeDefenseError::FormulaCycleDetected { .. })));
}

#[test]
fn test_sheet_dimensions_guard() {
    let mut guard = SheetDimensionsGuard::new(1_048_576, 16_384, 100);

    // Valid dimension
    let range = guard.parse_and_validate_dimension("A1:XFD1048576").unwrap();
    assert_eq!(range.start_col, 1);
    assert_eq!(range.start_row, 1);
    assert_eq!(range.end_col, 16_384);
    assert_eq!(range.end_row, 1_048_576);

    // Column out of bounds
    assert!(col_str_to_index("XFE").unwrap() > 16_384);
    let err = guard.parse_and_validate_dimension("A1:XFE100");
    assert!(matches!(err, Err(OfficeDefenseError::ColumnOutOfBounds { .. })));

    // Row out of bounds
    let err = guard.parse_and_validate_dimension("A1:Z1048577");
    assert!(matches!(err, Err(OfficeDefenseError::RowOutOfBounds { .. })));

    // Active cells quota
    assert!(guard.register_active_cells(80).is_ok());
    assert!(guard.register_active_cells(30).is_err());
    guard.reset();
    assert_eq!(guard.active_cells(), 0);
}

#[test]
fn test_sst_quota_guard() {
    let mut guard = SstQuotaGuard::new(5, 64, 256);

    // Add strings
    let id1 = guard.add_entry("Hello").unwrap();
    let id2 = guard.add_entry("World").unwrap();
    let id3 = guard.add_entry("Hello").unwrap();
    assert_eq!(id1, id3);
    assert_ne!(id1, id2);
    assert_eq!(guard.unique_count(), 2);

    // String entry too large (>64 bytes)
    let large_str = "A".repeat(100);
    let err = guard.add_entry(&large_str);
    assert!(matches!(err, Err(OfficeDefenseError::SstEntryTooLarge { .. })));

    // Streaming XML test
    let xml_data = r#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3" uniqueCount="2">
    <si><t>Alpha</t></si>
    <si><t>Beta</t></si>
    <si><t>Alpha</t></si>
</sst>"#;
    let mut stream_guard = SstQuotaGuard::default();
    let report = stream_guard.parse_sst_stream(Cursor::new(xml_data.as_bytes())).unwrap();
    assert_eq!(report.unique_entries, 2);
    assert_eq!(stream_guard.get(0), Some("Alpha"));
    assert_eq!(stream_guard.get(1), Some("Beta"));
}

#[test]
fn test_macro_sandbox_guard() {
    let guard = OfficeMacroSandboxGuard::new();

    // Dangerous macro files
    assert!(guard.should_strip_entry("xl/vbaProject.bin"));
    assert!(guard.should_strip_entry("word/activeX/activeX1.bin"));
    assert!(guard.should_strip_entry("customMacros.vba"));
    assert!(!guard.should_strip_entry("xl/worksheets/sheet1.xml"));

    // DDE and command injection
    assert!(guard.inspect_formula_security("=cmd|'/C calc'!A0").is_err());
    assert!(guard.inspect_formula_security("=DDE(\"server\", \"topic\", \"item\")").is_err());
    assert!(guard.inspect_formula_security("+DDEAUTO(\"server\", \"topic\", \"item\")").is_err());
    assert!(guard.inspect_formula_security("=HYPERLINK(\"powershell.exe -enc ...\", \"Open\")").is_err());
    assert!(guard.inspect_formula_security("=SUM(A1:B10)").is_ok());

    // External UNC relationship neutralization
    assert!(guard.sanitize_relationship_target(r"\\attacker.com\share", Some("External"), "hyperlink").is_err());
    assert!(guard.sanitize_relationship_target("http://evil.com/template.dotm", Some("External"), "http://schemas.openxmlformats.org/officeDocument/2006/relationships/attachedTemplate").is_err());
    assert!(guard.sanitize_relationship_target("sheet2.xml", None, "worksheet").is_ok());
}

#[test]
fn test_memory_budget_guard() {
    let guard = OfficeMemoryBudgetGuard::new(1024, 512, 512);

    {
        let permit = guard.allocate(600).unwrap();
        assert_eq!(permit.size(), 600);
        assert_eq!(guard.allocated_bytes(), 600);

        // Exceeds remaining budget (1024 - 600 = 424 < 500)
        let err = guard.allocate(500);
        assert!(matches!(err, Err(OfficeDefenseError::MemoryBudgetExceeded { .. })));
    }

    // Permit dropped -> bytes released
    assert_eq!(guard.allocated_bytes(), 0);
    assert!(guard.validate_sheet_size(600).is_err());
    assert!(guard.validate_sheet_size(400).is_ok());
}

#[test]
fn test_sensitive_office_buffer() {
    let mut buf = SensitiveOfficeBuffer::with_capacity(32);
    buf.extend_from_slice(b"Confidential Corporate SpreadSheet");
    assert_eq!(buf.len(), 34);
    assert_eq!(buf.as_str().unwrap(), "Confidential Corporate SpreadSheet");

    // Redacted debug format
    let debug_str = format!("{:?}", buf);
    assert!(debug_str.contains("[REDACTED_SENSITIVE_OFFICE_DATA]"));
    assert!(!debug_str.contains("Confidential"));

    buf.clear();
    assert!(buf.is_empty());
}
