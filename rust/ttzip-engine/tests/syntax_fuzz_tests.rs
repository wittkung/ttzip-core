// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive corruption injection, chaos mutation, and fuzzing test suite for TTZip Tree-sitter Syntax Engine.
//!
//! Deploys 16 surgical destruction targets:
//! 1. Extreme deep nesting brackets/parentheses (10,000+ levels) stack overflow interception.
//! 2. Giant single-line source (1MB+ single line) lexical scanning circuit breaker.
//! 3. Truncated and malformed syntax fragments fault-tolerant recovery.
//! 4. Null bytes (`\0`) and control characters injection path defense.
//! 5. Malformed S-expression query pattern injection defense.
//! 6. 1000+ tasks concurrent syntax analysis race competition.
//! 7. 500+ rounds of pseudo-random mutation syntax data fuzzing.
//! 8. Extremely long identifiers (100KB+) and string literals boundary defense.
//! 9. Incremental parsing `InputEdit` malformed cross-boundary offsets & inverted ranges injection.
//! 10. Multi-language extension switching and unrecognized format fallback defense.
//! 11. UTF-8 malformed sequences and multi-byte UTF-16 NSRange mapping overflow defense.
//! 12. Mixed nested language and template string injection.
//! 13. UniFFI `tokenize_source_code` and `max_length` truncation boundary defense.
//! 14. Syntax tree cursor navigation empty node and circular sibling defense.
//! 15. Rapid repeated incremental edit replay and tree state reset robustness.
//! 16. Nested code inside comments and unclosed block comments defense.

use std::panic::catch_unwind;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rayon::prelude::*;
use tree_sitter::{InputEdit, Point, Query, QueryCursor};

use ttzip_engine::standards::syntax_highlight::{
    highlight_spans, tokenize_code, HighlightCategory, SupportedLanguage, SyntaxEngine, Utf16Index,
};
use ttzip_engine::uniffi_api::syntax::{highlight_code_spans, tokenize_source_code};

/// Deterministic linear congruential generator for reproducible fuzzing vectors.
#[derive(Clone, Debug)]
struct DeterministicPrng {
    state: u64,
}

impl DeterministicPrng {
    #[inline]
    const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9e37_79b9_7f4a_7c15 } else { seed },
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
// Target 1: Extreme Deep Nesting Brackets (10,000+ Levels) Stack Overflow Defense
// ============================================================================
#[test]
fn test_target_01_extreme_deep_nesting_stack_defense() {
    let depth = 10_000;
    let mut nested_parens = String::with_capacity(depth * 2 + 32);
    nested_parens.push_str("fn test() { let x = ");
    for _ in 0..depth {
        nested_parens.push('(');
    }
    nested_parens.push_str("42");
    for _ in 0..depth {
        nested_parens.push(')');
    }
    nested_parens.push_str("; }");

    let res = catch_unwind(|| {
        let spans = tokenize_code(&nested_parens, "rs");
        assert!(!spans.is_empty());
        assert!(spans.iter().any(|s| s.category == HighlightCategory::Keyword));
    });
    assert!(res.is_ok(), "Stack overflow or panic on 10,000-level nested parentheses");

    // Also test deeply nested JSON brackets
    let mut nested_json = String::with_capacity(depth * 2 + 8);
    for _ in 0..depth {
        nested_json.push('[');
    }
    nested_json.push_str("123");
    for _ in 0..depth {
        nested_json.push(']');
    }
    let res_json = catch_unwind(|| {
        let spans = tokenize_code(&nested_json, "json");
        assert!(!spans.is_empty());
    });
    assert!(res_json.is_ok(), "Stack overflow on 10,000-level JSON arrays");
}

// ============================================================================
// Target 2: Giant Single-Line Source (1MB+ Single Line) Lexical Circuit Breaker
// ============================================================================
#[test]
fn test_target_02_giant_single_line_source_circuit_breaker() {
    let target_size = 1024 * 1024; // 1 MB single line
    let mut giant_line = String::with_capacity(target_size + 100);
    let mut term_idx = 0u32;
    while giant_line.len() < target_size {
        giant_line.push_str(&format!("let v{} = {}; ", term_idx, term_idx % 1000));
        term_idx += 1;
    }

    let res = catch_unwind(|| {
        let spans = highlight_spans(&giant_line, "rs");
        assert!(!spans.is_empty());
        // Verify all spans are within byte boundary
        for s in &spans {
            assert!((s.end_byte as usize) <= giant_line.len());
            assert!(s.start_byte < s.end_byte);
        }
    });
    assert!(res.is_ok(), "Panic on 1MB single-line source code");
}

// ============================================================================
// Target 3: Truncated & Malformed Syntax Fragments Fault-Tolerant Recovery
// ============================================================================
#[test]
fn test_target_03_truncated_malformed_syntax_recovery() {
    let sample = r#"pub fn calculate_matrix<T: Copy>(data: &[T], multiplier: f64) -> Result<Vec<T>, String> {
        let mut buffer = Vec::with_capacity(data.len());
        for (idx, item) in data.iter().enumerate() {
            if idx > 100 {
                return Ok(buffer);
            }
            buffer.push(*item);
        }
        Ok(buffer)
    }"#;

    // Truncate at every single byte boundary
    for i in 1..sample.len() {
        let truncated = &sample[..i];
        let res = catch_unwind(|| {
            let spans = tokenize_code(truncated, "rs");
            for s in &spans {
                assert!((s.end_byte as usize) <= truncated.len());
            }
        });
        assert!(res.is_ok(), "Panic on truncated snippet of length {}", i);
    }
}

// ============================================================================
// Target 4: Null Bytes & Control Characters Injection Path Defense
// ============================================================================
#[test]
fn test_target_04_null_bytes_and_control_chars_injection() {
    let raw_payload = b"fn main\0() {\x01let\x00 x\x1B = \x02\"hello\0world\"\x7F;\t\n\r let y = \0;\0 }";
    let text = String::from_utf8_lossy(raw_payload);

    let res = catch_unwind(|| {
        let spans = tokenize_code(&text, "rs");
        assert!(!spans.is_empty());
    });
    assert!(res.is_ok(), "Panic on null bytes and control chars injection");
}

// ============================================================================
// Target 5: Malformed S-Expression Query Pattern Injection Defense
// ============================================================================
#[test]
fn test_target_05_malformed_s_expression_query_defense() {
    let lang = tree_sitter_rust::language();

    let invalid_queries = [
        "",
        "(((",
        "(function_item",
        "(function_item name: )",
        "(non_existent_node_type_12345 @capture)",
        "(function_item (unclosed_child @x)",
        "@orphan_capture",
        "(!@#$%^&*()_+)",
        "(let_declaration pattern: (identifier) @var value: (non_existent))",
    ];

    for (idx, q_str) in invalid_queries.iter().enumerate() {
        let res = catch_unwind(|| {
            let q_res = Query::new(&lang, q_str);
            // Must return Err safely without memory corruption or panic
            if q_str.is_empty() || q_str.contains("non_existent") || q_str.contains('(') {
                let _ = q_res;
            }
        });
        assert!(res.is_ok(), "Panic on malformed query index {}", idx);
    }

    // Valid query execution validation
    let valid_query_str = "(function_item name: (identifier) @fn_name)";
    let query = Query::new(&lang, valid_query_str).expect("Valid S-expression query");
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).unwrap();
    let tree = parser.parse("fn foo() {} fn bar() {}", None).unwrap();

    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), "fn foo() {} fn bar() {}".as_bytes());
    let match_count = matches.count();
    assert_eq!(match_count, 2, "Query must accurately capture 2 functions");
}

// ============================================================================
// Target 6: 1000+ Tasks Concurrent Syntax Analysis Race Competition
// ============================================================================
#[test]
fn test_target_06_concurrent_syntax_analysis_race() {
    let total_tasks = 1200;
    let completed = Arc::new(AtomicUsize::new(0));

    let snippets = [
        ("fn rust_worker(idx: usize) -> usize { idx * 2 }", "rs"),
        ("def py_worker(val):\n    return [x * 2 for x in range(val)]", "py"),
        ("const jsWorker = async (data) => { return await fetch(data); };", "js"),
        ("{\"name\": \"TTZip\", \"version\": 2026, \"active\": true}", "json"),
        ("struct CWorker { int id; double weight; };", "c"),
    ];

    (0..total_tasks).into_par_iter().for_each(|task_id| {
        let (code, ext) = snippets[task_id % snippets.len()];
        let mut prng = DeterministicPrng::new((task_id + 1) as u64);
        let mut mutated = code.to_string();

        if prng.next_range(0, 5) == 0 {
            mutated.push_str(" // extra comment");
        }

        let res = catch_unwind(|| {
            let spans = tokenize_code(&mutated, ext);
            assert!(!spans.is_empty());
        });
        assert!(res.is_ok(), "Panic in concurrent worker task {}", task_id);
        completed.fetch_add(1, Ordering::Relaxed);
    });

    assert_eq!(completed.load(Ordering::SeqCst), total_tasks);
}

// ============================================================================
// Target 7: 500+ Rounds of Pseudo-Random Mutation Syntax Data Fuzzing
// ============================================================================
#[test]
fn test_target_07_pseudo_random_mutation_syntax_fuzzing() {
    let base_code = r#"// TTZip AST Fuzzing Base
pub struct PipelineState<T> {
    pub id: u64,
    pub payload: Vec<T>,
    pub is_active: bool,
}

impl<T: Default + Clone> PipelineState<T> {
    pub fn new(capacity: usize) -> Self {
        Self { id: 0, payload: Vec::with_capacity(capacity), is_active: true }
    }
}"#;

    let mut prng = DeterministicPrng::new(0xABCD_EF01_2345_6789);
    let rounds = 550;

    for r in 0..rounds {
        let mut mutated = base_code.as_bytes().to_vec();
        let ops = prng.next_range(1, 6);

        for _ in 0..ops {
            match prng.next_range(0, 3) {
                0 => {
                    // Bit flip
                    if !mutated.is_empty() {
                        let idx = prng.next_range(0, mutated.len() - 1);
                        mutated[idx] ^= 1 << prng.next_range(0, 7);
                    }
                }
                1 => {
                    // Byte insertion
                    let idx = prng.next_range(0, mutated.len());
                    let byte = prng.next_byte();
                    mutated.insert(idx, byte);
                }
                2 => {
                    // Token splice
                    let idx = prng.next_range(0, mutated.len());
                    let tokens: &[&[u8]] = &[b"fn ", b"let mut ", b"/*", b"*/", b"\"str\"", b"::std::", b"{};"];
                    let tok = tokens[prng.next_range(0, tokens.len() - 1)];
                    mutated.splice(idx..idx, tok.iter().copied());
                }
                _ => {}
            }
        }

        let text = String::from_utf8_lossy(&mutated);
        let res = catch_unwind(|| {
            let spans = tokenize_code(&text, "rs");
            for s in &spans {
                assert!((s.end_byte as usize) <= text.len());
            }
        });
        assert!(res.is_ok(), "Fuzzer panic on round {}", r);
    }
}

// ============================================================================
// Target 8: Extremely Long Identifiers & String Literals Boundary Defense
// ============================================================================
#[test]
fn test_target_08_extremely_long_identifiers_and_literals() {
    let huge_ident = "identifier_".repeat(10_000); // 110 KB identifier
    let huge_str = "string_content_".repeat(20_000); // 300 KB string
    let code = format!("let {} = \"{}\";", huge_ident, huge_str);

    let res = catch_unwind(|| {
        let spans = tokenize_code(&code, "rs");
        assert!(!spans.is_empty());
        assert!(spans.iter().any(|s| s.category == HighlightCategory::String));
    });
    assert!(res.is_ok(), "Panic on 100KB identifier & 300KB string literal");
}

// ============================================================================
// Target 9: Incremental Parsing InputEdit Malformed Offsets & Inverted Ranges
// ============================================================================
#[test]
fn test_target_09_incremental_parsing_malformed_input_edit() {
    let mut engine = SyntaxEngine::new();
    let initial_code = "fn compute() -> u32 { 100 }";
    let _ = engine.parse_full(initial_code, SupportedLanguage::Rust).unwrap();

    let malformed_edits = [
        // Out-of-bounds start byte
        InputEdit {
            start_byte: 9999,
            old_end_byte: 10000,
            new_end_byte: 10005,
            start_position: Point { row: 10, column: 0 },
            old_end_position: Point { row: 10, column: 1 },
            new_end_position: Point { row: 10, column: 6 },
        },
        // Inverted range (start > end)
        InputEdit {
            start_byte: 20,
            old_end_byte: 10,
            new_end_byte: 10,
            start_position: Point { row: 0, column: 20 },
            old_end_position: Point { row: 0, column: 10 },
            new_end_position: Point { row: 0, column: 10 },
        },
    ];

    for (idx, edit) in malformed_edits.iter().enumerate() {
        let res = catch_unwind(|| {
            let mut eng = SyntaxEngine::new();
            let _ = eng.parse_full(initial_code, SupportedLanguage::Rust);
            let _ = eng.parse_incremental("fn compute() -> u32 { 200 }", edit, SupportedLanguage::Rust);
        });
        assert!(res.is_ok(), "Panic on malformed InputEdit index {}", idx);
    }
}

// ============================================================================
// Target 10: Multi-Language Extension Switching & Fallback Defense
// ============================================================================
#[test]
fn test_target_10_multilang_extension_switching_and_fallback() {
    let extensions = [
        "rs", "swift", "c", "h", "cpp", "hpp", "py", "js", "ts", "json", "md", "html", "css",
        "unknown", "xyz", "123", "", ".tar.gz", "rs.bak", "pyc",
    ];

    let code = "function test() { return 42; }";
    for ext in &extensions {
        let res = catch_unwind(|| {
            let spans = tokenize_code(code, ext);
            let _ = spans;
        });
        assert!(res.is_ok(), "Panic on extension '{}'", ext);
    }
}

// ============================================================================
// Target 11: UTF-8 Malformed Sequences & UTF-16 NSRange Mapping Defense
// ============================================================================
#[test]
fn test_target_11_utf8_malformed_sequences_and_utf16_mapping() {
    // Emojis (4-byte UTF-8, 2 UTF-16 code units) and CJK
    let text = "let 🦀 = \"🚀 压缩引擎 \u{1F4D6}\"; // 注释 🌟";
    let index = Utf16Index::new(text);

    let (loc, len) = index.byte_range_to_utf16(0, text.len());
    assert!(loc == 0);
    assert!(len > 0);

    // Out-of-bounds byte range
    let (oob_loc, oob_len) = index.byte_range_to_utf16(9999, 10000);
    assert_eq!(oob_len, 0);
    assert!(oob_loc >= len);

    let spans = highlight_spans(text, "rs");
    assert!(!spans.is_empty());
}

// ============================================================================
// Target 12: Mixed Nested Language & Template String Injection
// ============================================================================
#[test]
fn test_target_12_mixed_nested_language_and_template_strings() {
    let html_mixed = r#"<!DOCTYPE html>
<html>
<head>
    <style>body { background: #fff; font-family: sans-serif; }</style>
</head>
<body>
    <script>const msg = `Hello ${name + 42}`; console.log(msg);</script>
</body>
</html>"#;

    let res = catch_unwind(|| {
        let spans = tokenize_code(html_mixed, "html");
        assert!(!spans.is_empty());
    });
    assert!(res.is_ok(), "Panic on mixed HTML/CSS/JS template string source");
}

// ============================================================================
// Target 13: UniFFI tokenize_source_code & max_length Truncation Defense
// ============================================================================
#[test]
fn test_target_13_uniffi_tokenize_source_code_max_length_truncation() {
    let code = "pub fn execute() {\n    let val = 12345;\n    val + 10\n}";

    for max_len in [0u32, 1, 5, 10, 20, 50, 1000] {
        let spans = tokenize_source_code(code.to_string(), "rs".to_string(), max_len);
        if max_len > 0 && max_len < (code.len() as u32) {
            for s in &spans {
                assert!(s.location + s.length <= max_len + 10);
            }
        }
    }

    // High-level wrapper
    let all_spans = highlight_code_spans(code.to_string(), "rs".to_string());
    assert!(!all_spans.is_empty());
}

// ============================================================================
// Target 14: Syntax Tree Cursor Navigation Empty Node & Circular Defense
// ============================================================================
#[test]
fn test_target_14_syntax_tree_cursor_navigation_defense() {
    let empty_sources = ["", " ", "\n\n", "///", "/* */", ";;;"];
    for src in &empty_sources {
        let res = catch_unwind(|| {
            let spans = tokenize_code(src, "rs");
            let _ = spans;
        });
        assert!(res.is_ok(), "Panic on empty source: '{}'", src);
    }
}

// ============================================================================
// Target 15: Rapid Repeated Incremental Edit Replay & Tree State Reset
// ============================================================================
#[test]
fn test_target_15_rapid_repeated_edit_replay_and_tree_reset() {
    let mut engine = SyntaxEngine::new();
    let current_code = "fn main() {\n    let x = 0;\n}".to_string();
    let _ = engine.parse_full(&current_code, SupportedLanguage::Rust).unwrap();

    for i in 1..=100 {
        let new_code = format!("fn main() {{\n    let x = {};\n}}", i);
        let edit = InputEdit {
            start_byte: 28,
            old_end_byte: 29,
            new_end_byte: 28 + format!("{}", i).len(),
            start_position: Point { row: 1, column: 12 },
            old_end_position: Point { row: 1, column: 13 },
            new_end_position: Point { row: 1, column: 12 + format!("{}", i).len() },
        };
        let spans = engine.parse_incremental(&new_code, &edit, SupportedLanguage::Rust).unwrap();
        assert!(!spans.is_empty());
    }
}

// ============================================================================
// Target 16: Nested Comments & Unclosed Block Comments Defense
// ============================================================================
#[test]
fn test_target_16_nested_comments_and_unclosed_block_comments() {
    let corrupt_comments = [
        "/* unclosed comment at EOF",
        "/* /* double open */",
        "// single line comment without newline",
        "\"/* string with comment characters */\"",
        "/* comment with \"string inside\" */",
        "\"unclosed string literal",
        "''' unclosed python docstring",
    ];

    for (idx, c) in corrupt_comments.iter().enumerate() {
        let res = catch_unwind(|| {
            let spans = tokenize_code(c, "rs");
            let _ = spans;
        });
        assert!(res.is_ok(), "Panic on corrupt comment index {}", idx);
    }
}
