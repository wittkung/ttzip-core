// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 1: Office Spreadsheet Formula AST Depth, Token Limit, and Tarjan Cycle Fuse.
//!
//! Prevents stack overflow, parser exhaustion, and calculation deadlocks in Excel/Spreadsheet
//! formula evaluation by enforcing:
//! 1. Maximum AST expression nesting depth (<= 32 levels).
//! 2. Maximum formula token count (<= 1,024 tokens).
//! 3. Strongly Connected Components (Tarjan algorithm) circular dependency detection and breaking.

use std::collections::{HashMap, HashSet};

use super::{OfficeDefenseError, MAX_FORMULA_DEPTH, MAX_FORMULA_TOKENS};

/// A normalized cell coordinate representation (e.g. "Sheet1!A1" or "A1").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CellCoord {
    pub sheet: Option<String>,
    pub col: u32,
    pub row: u32,
}

impl CellCoord {
    /// Creates a cell coordinate on default sheet.
    pub const fn new(col: u32, row: u32) -> Self {
        Self {
            sheet: None,
            col,
            row,
        }
    }

    /// Creates a qualified cell coordinate with sheet name.
    pub fn with_sheet(sheet: impl Into<String>, col: u32, row: u32) -> Self {
        Self {
            sheet: Some(sheet.into()),
            col,
            row,
        }
    }

    /// Formats the coordinate as an A1-style reference string.
    pub fn to_a1_string(&self) -> String {
        let col_str = col_index_to_str(self.col);
        match &self.sheet {
            Some(s) => format!("{}!{}{}", s, col_str, self.row),
            None => format!("{}{}", col_str, self.row),
        }
    }
}

/// Helper converting 1-based column index to Excel column string (e.g. 1 -> A, 27 -> AA).
pub fn col_index_to_str(mut col: u32) -> String {
    let mut result = Vec::new();
    while col > 0 {
        col -= 1;
        let rem = (col % 26) as u8;
        result.push(b'A' + rem);
        col /= 26;
    }
    result.reverse();
    if result.is_empty() {
        "A".to_string()
    } else {
        String::from_utf8(result).unwrap_or_else(|_| "A".to_string())
    }
}

/// Formula analysis summary containing depth, token count, and referenced cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormulaInspection {
    pub max_depth: usize,
    pub token_count: usize,
    pub referenced_cells: Vec<String>,
}

/// Guard enforcing AST depth limits, token limits, and cyclic dependency fuses on formulas.
#[derive(Debug, Clone)]
pub struct FormulaDepthGuard {
    max_depth: usize,
    max_tokens: usize,
}

impl Default for FormulaDepthGuard {
    fn default() -> Self {
        Self::new(MAX_FORMULA_DEPTH, MAX_FORMULA_TOKENS)
    }
}

impl FormulaDepthGuard {
    /// Creates a new guard with configured max depth and token limits.
    pub const fn new(max_depth: usize, max_tokens: usize) -> Self {
        Self {
            max_depth,
            max_tokens,
        }
    }

    /// Inspects and validates a raw formula string (e.g. `=SUM(A1:B10) + IF(C1>0, 1, 0)`).
    pub fn inspect_formula(&self, formula: &str) -> Result<FormulaInspection, OfficeDefenseError> {
        let trimmed = formula.trim().strip_prefix('=').unwrap_or(formula.trim());
        if trimmed.is_empty() {
            return Ok(FormulaInspection {
                max_depth: 0,
                token_count: 0,
                referenced_cells: Vec::new(),
            });
        }

        let mut current_depth = 0usize;
        let mut max_observed_depth = 0usize;
        let mut token_count = 0usize;
        let mut in_string = false;
        let mut in_quote = false;
        let mut referenced_cells = Vec::new();

        let chars: Vec<char> = trimmed.chars().collect();
        let len = chars.len();
        let mut idx = 0;

        while idx < len {
            let ch = chars[idx];

            // Handle string literal boundaries
            if ch == '"' && !in_quote {
                in_string = !in_string;
                idx += 1;
                continue;
            }
            if ch == '\'' && !in_string {
                in_quote = !in_quote;
                idx += 1;
                continue;
            }

            if in_string || in_quote {
                idx += 1;
                continue;
            }

            // Detect token boundaries
            if ch.is_whitespace() {
                idx += 1;
                continue;
            }

            if ch == '(' || ch == '{' || ch == '[' {
                current_depth = current_depth.saturating_add(1);
                if current_depth > max_observed_depth {
                    max_observed_depth = current_depth;
                }
                if current_depth > self.max_depth {
                    return Err(OfficeDefenseError::FormulaDepthExceeded {
                        depth: current_depth,
                        limit: self.max_depth,
                    });
                }
                token_count = token_count.saturating_add(1);
                idx += 1;
            } else if ch == ')' || ch == '}' || ch == ']' {
                current_depth = current_depth.saturating_sub(1);
                token_count = token_count.saturating_add(1);
                idx += 1;
            } else if ch == '+' || ch == '-' || ch == '*' || ch == '/' || ch == '^' || ch == '&' || ch == '=' || ch == '<' || ch == '>' || ch == ',' || ch == ':' {
                token_count = token_count.saturating_add(1);
                idx += 1;
            } else if ch.is_alphanumeric() || ch == '_' || ch == '$' || ch == '!' {
                // Parse identifier (function name, cell reference, or range)
                let start = idx;
                while idx < len && (chars[idx].is_alphanumeric() || chars[idx] == '_' || chars[idx] == '$' || chars[idx] == '!') {
                    idx += 1;
                }
                let ident: String = chars[start..idx].iter().collect();
                token_count = token_count.saturating_add(1);

                if is_potential_cell_ref(&ident) {
                    referenced_cells.push(ident);
                }
            } else {
                token_count = token_count.saturating_add(1);
                idx += 1;
            }

            if token_count > self.max_tokens {
                return Err(OfficeDefenseError::FormulaTokensExceeded {
                    tokens: token_count,
                    limit: self.max_tokens,
                });
            }
        }

        Ok(FormulaInspection {
            max_depth: max_observed_depth,
            token_count,
            referenced_cells,
        })
    }

    /// Verifies that a directed cell dependency graph contains no cyclic loops using Tarjan's SCC.
    pub fn verify_dependency_dag(
        &self,
        graph: &HashMap<String, Vec<String>>,
    ) -> Result<(), OfficeDefenseError> {
        let mut tarjan = TarjanScc::new(graph);
        tarjan.run()
    }
}

/// Checks whether an identifier looks like a cell reference (e.g. "A1", "$B$12", "Sheet1!C3").
fn is_potential_cell_ref(ident: &str) -> bool {
    let target = if let Some((_, rest)) = ident.split_once('!') {
        rest
    } else {
        ident
    };
    let clean = target.replace('$', "");
    if clean.is_empty() {
        return false;
    }

    let mut letters = 0;
    let mut digits = 0;
    for ch in clean.chars() {
        if ch.is_ascii_alphabetic() && digits == 0 {
            letters += 1;
        } else if ch.is_ascii_digit() && letters > 0 {
            digits += 1;
        } else {
            return false;
        }
    }
    letters > 0 && letters <= 3 && digits > 0 && digits <= 7
}

// ============================================================================
// Tarjan's Strongly Connected Components (SCC) Cycle Detection
// ============================================================================

struct TarjanScc<'a> {
    graph: &'a HashMap<String, Vec<String>>,
    index: usize,
    indices: HashMap<&'a str, usize>,
    lowlink: HashMap<&'a str, usize>,
    on_stack: HashSet<&'a str>,
    stack: Vec<&'a str>,
}

impl<'a> TarjanScc<'a> {
    fn new(graph: &'a HashMap<String, Vec<String>>) -> Self {
        Self {
            graph,
            index: 0,
            indices: HashMap::new(),
            lowlink: HashMap::new(),
            on_stack: HashSet::new(),
            stack: Vec::new(),
        }
    }

    fn run(&mut self) -> Result<(), OfficeDefenseError> {
        for node in self.graph.keys() {
            if !self.indices.contains_key(node.as_str()) {
                self.strong_connect(node.as_str())?;
            }
        }
        Ok(())
    }

    fn strong_connect(&mut self, u: &'a str) -> Result<(), OfficeDefenseError> {
        self.indices.insert(u, self.index);
        self.lowlink.insert(u, self.index);
        self.index += 1;
        self.stack.push(u);
        self.on_stack.insert(u);

        if let Some(neighbors) = self.graph.get(u) {
            for v in neighbors {
                let v_str = v.as_str();
                // Self-loop direct cycle check
                if v_str == u {
                    return Err(OfficeDefenseError::FormulaCycleDetected {
                        cycle: format!("{} -> {}", u, u),
                    });
                }

                if !self.indices.contains_key(v_str) {
                    if self.graph.contains_key(v) {
                        self.strong_connect(v_str)?;
                        let u_low = self.lowlink[u];
                        let v_low = self.lowlink[v_str];
                        self.lowlink.insert(u, u_low.min(v_low));
                    }
                } else if self.on_stack.contains(v_str) {
                    let u_low = self.lowlink[u];
                    let v_idx = self.indices[v_str];
                    self.lowlink.insert(u, u_low.min(v_idx));
                }
            }
        }

        if self.lowlink.get(u) == self.indices.get(u) {
            let mut component = Vec::new();
            while let Some(w) = self.stack.pop() {
                self.on_stack.remove(w);
                component.push(w);
                if w == u {
                    break;
                }
            }

            if component.len() > 1 {
                component.reverse();
                let cycle_str = component.join(" -> ") + " -> " + component[0];
                return Err(OfficeDefenseError::FormulaCycleDetected { cycle: cycle_str });
            }
        }

        Ok(())
    }
}
