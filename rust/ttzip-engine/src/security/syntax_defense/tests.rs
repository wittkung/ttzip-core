// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use super::*;

#[test]
fn test_ast_depth_guard_push_pop_and_limits() {
    let mut guard = AstDepthGuard::with_max_depth(5);
    assert_eq!(guard.current_depth(), 0);

    for i in 1..=5 {
        assert_eq!(guard.push_depth().unwrap(), i);
    }
    assert_eq!(guard.peak_depth(), 5);

    // 6th push exceeds limit
    let err = guard.push_depth().unwrap_err();
    assert!(matches!(
        err,
        SyntaxDefenseError::AstDepthExceeded {
            depth: 6,
            max_depth: 5
        }
    ));

    assert_eq!(guard.pop_depth(), 4);
    assert_eq!(guard.pop_depth(), 3);
    guard.reset();
    assert_eq!(guard.current_depth(), 0);
    assert_eq!(guard.peak_depth(), 0);
}

#[cfg(feature = "syntax")]
#[test]
fn test_ast_depth_guard_tree_inspection() {
    let mut guard = AstDepthGuard::new();
    let rust_code = "fn main() { let x = (1 + (2 * (3 - (4 / 2)))); }";
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::language())
        .unwrap();
    let tree = parser.parse(rust_code, None).unwrap();

    let depth = guard.inspect_tree(&tree).unwrap();
    assert!(depth > 0 && depth < 256);

    let mut shallow_guard = AstDepthGuard::with_max_depth(2);
    let err = shallow_guard.inspect_tree(&tree).unwrap_err();
    assert!(matches!(err, SyntaxDefenseError::AstDepthExceeded { .. }));
}

#[test]
fn test_error_recovery_timeout_guard_budget() {
    let mut guard = ErrorRecoveryTimeoutGuard::with_max_steps(100);
    assert_eq!(guard.step_count(), 0);

    assert_eq!(guard.record_steps(50).unwrap(), 50);
    assert_eq!(guard.step_count(), 50);
    guard.record_error_node();
    guard.record_branch();
    assert_eq!(guard.error_node_count(), 1);
    assert_eq!(guard.branch_count(), 1);

    assert_eq!(guard.record_steps(50).unwrap(), 100);

    let err = guard.record_step().unwrap_err();
    assert!(matches!(
        err,
        SyntaxDefenseError::ErrorRecoveryLimitExceeded {
            steps: 101,
            max_steps: 100
        }
    ));
}

#[cfg(feature = "syntax")]
#[test]
fn test_error_recovery_guard_tree_scan() {
    let mut guard = ErrorRecoveryTimeoutGuard::new();
    let malformed_code = "fn broken( { let x = ; }";
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::language())
        .unwrap();
    let tree = parser.parse(malformed_code, None).unwrap();

    let errors = guard.scan_for_error_nodes(&tree).unwrap();
    assert!(errors > 0);
    assert!(guard.step_count() > 0);
}

#[test]
fn test_query_pattern_safety_guard() {
    let guard = QueryPatternSafetyGuard::new();

    // Valid S-expression patterns
    assert!(guard.validate_pattern("(function_item (identifier) @fn)").is_ok());
    assert!(guard.validate_pattern("(struct_item name: (type_identifier) @name)").is_ok());

    // Deeply nested patterns exceeding limit
    let deep_guard = QueryPatternSafetyGuard::with_limits(3, 4096, 64);
    let deep_pattern = "((((node))))";
    let err = deep_guard.validate_pattern(deep_pattern).unwrap_err();
    assert!(matches!(err, SyntaxDefenseError::QueryPatternViolation { .. }));

    // Unbalanced parentheses
    assert!(guard.validate_pattern("(node (child)").is_err());
    assert!(guard.validate_pattern("(node))").is_err());

    // Unterminated string literal
    assert!(guard.validate_pattern("(node \"literal)").is_err());

    // Predicate quota
    let strict_guard = QueryPatternSafetyGuard::with_limits(32, 4096, 1);
    let multi_pred = "(node) #eq? @a \"x\" #match? @b \"y\"";
    assert!(strict_guard.validate_pattern(multi_pred).is_err());
}

#[test]
fn test_line_length_fuse_guard() {
    let guard = LineLengthFuseGuard::with_max_line_length(100);

    let short_source = "fn foo() {\n    let a = 1;\n    let b = 2;\n}\n";
    assert!(guard.scan_source(short_source).is_ok());

    let long_line = format!("let x = \"{}\";", "a".repeat(150));
    let err = guard.scan_source(&long_line).unwrap_err();
    assert!(matches!(
        err,
        SyntaxDefenseError::LineLengthExceeded {
            line_number: 1,
            len: _,
            max_len: 100
        }
    ));

    let sanitized = guard.sanitize_long_lines(&long_line, "/* TRUNCATED */");
    assert!(sanitized.contains("/* TRUNCATED */"));
    assert!(sanitized.len() <= 100 + "/* TRUNCATED */".len());
}

#[test]
fn test_parsing_timeout_guard() {
    let mut guard = ParsingTimeoutGuard::with_timeout_micros(100_000);
    assert!(!guard.is_expired());
    assert!(guard.check_timeout().is_ok());
    assert!(guard.remaining_micros() <= 100_000);

    guard.reset();
    assert!(guard.elapsed_micros() < 50_000);
}

#[test]
fn test_sensitive_token_buffer_security() {
    let mut buf = SensitiveTokenBuffer::new();
    assert!(buf.is_empty());

    buf.push_bytes(b"api_key_secret_12345").unwrap();
    assert_eq!(buf.len(), 20);
    assert_eq!(buf.as_str(), Some("api_key_secret_12345"));

    buf.push_token("secret", "api_key_secret_12345", 0, 20)
        .unwrap();
    assert_eq!(buf.token_count(), 1);

    let masked = buf.render_masked('*');
    assert_eq!(masked, "********************");

    buf.zeroize_and_clear();
    assert!(buf.is_empty());
    assert_eq!(buf.len(), 0);
    assert_eq!(buf.token_count(), 0);
}

#[cfg(feature = "syntax")]
#[test]
fn test_syntax_security_pipeline_end_to_end() {
    let mut pipeline = SyntaxSecurityPipeline::new();
    let rust_code = "pub fn calculate(a: u32, b: u32) -> u32 {\n    a + b\n}";

    let (tree, spans) = pipeline
        .parse_securely(
            rust_code,
            crate::standards::syntax_highlight::SupportedLanguage::Rust,
        )
        .unwrap();
    assert!(!spans.is_empty());
    assert_eq!(tree.root_node().kind(), "source_file");
}
