// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust spreadsheet formula evaluation microkernel.
//!
//! Features Pratt expression parsing, DAG dependency graph construction,
//! Tarjan strongly connected components (SCC) circular reference detection,
//! and 25+ built-in spreadsheet functions.

pub mod ast;
pub mod dag;
pub mod eval;
pub mod parser;

pub use ast::{BinaryOp, FormulaExpr, UnaryOp};
pub use dag::{collect_deps, detect_dag_cycles, topo_sort_dag};
pub use eval::{eval_binary_op, evaluate_expr, expand_args_to_values};
pub use parser::parse_formula_expr;

use std::collections::{HashMap, HashSet};
use crate::office::types::{OfficeCellAddress, OfficeCellValue, OfficeResult};

/// Pure Safe Rust Formula Engine.
#[derive(Debug, Default)]
pub struct TTZipFormulaEngine;

impl TTZipFormulaEngine {
    /// Creates a new formula engine instance.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates a raw formula string against a cell grid.
    pub fn evaluate_formula(
        &self,
        formula: &str,
        grid: &HashMap<OfficeCellAddress, OfficeCellValue>,
    ) -> OfficeResult<OfficeCellValue> {
        let expr = self.parse_expr(formula)?;
        self.evaluate_expr(&expr, grid)
    }

    /// Parses a formula string into an AST.
    pub fn parse_expr(&self, formula: &str) -> OfficeResult<FormulaExpr> {
        parse_formula_expr(formula)
    }

    /// Evaluates a parsed formula AST against a cell grid.
    pub fn evaluate_expr(
        &self,
        expr: &FormulaExpr,
        grid: &HashMap<OfficeCellAddress, OfficeCellValue>,
    ) -> OfficeResult<OfficeCellValue> {
        evaluate_expr(expr, grid)
    }

    /// Extracts all cell dependencies from an expression.
    pub fn extract_dependencies(&self, expr: &FormulaExpr) -> Vec<OfficeCellAddress> {
        let mut deps = Vec::new();
        collect_deps(expr, &mut deps);
        deps
    }

    /// Builds a dependency DAG mapping each cell formula to its required input cells.
    pub fn build_dependency_dag(
        &self,
        formulas: &HashMap<OfficeCellAddress, String>,
    ) -> OfficeResult<HashMap<OfficeCellAddress, Vec<OfficeCellAddress>>> {
        let mut dag = HashMap::new();
        for (addr, formula_str) in formulas {
            let expr = self.parse_expr(formula_str)?;
            let deps = self.extract_dependencies(&expr);
            dag.insert(addr.clone(), deps);
        }
        Ok(dag)
    }

    /// Detects circular reference cycles in the DAG using Tarjan's SCC algorithm.
    pub fn detect_cycles(
        &self,
        dag: &HashMap<OfficeCellAddress, Vec<OfficeCellAddress>>,
    ) -> HashSet<OfficeCellAddress> {
        detect_dag_cycles(dag)
    }

    /// Re-evaluates all formulas in dependency order, assigning `#CYCLE!` to circular nodes.
    pub fn recalculate_all(
        &mut self,
        formulas: &HashMap<OfficeCellAddress, String>,
        grid: &mut HashMap<OfficeCellAddress, OfficeCellValue>,
    ) -> OfficeResult<()> {
        let dag = self.build_dependency_dag(formulas)?;
        let cycle_cells = self.detect_cycles(&dag);

        for cell in &cycle_cells {
            grid.insert(cell.clone(), OfficeCellValue::Error("#CYCLE!".to_string()));
        }

        let mut visited = HashSet::new();
        let mut order = Vec::new();

        for node in dag.keys() {
            if !cycle_cells.contains(node) {
                topo_sort_dag(node, &dag, &cycle_cells, &mut visited, &mut order);
            }
        }

        for cell in order {
            if let Some(formula_str) = formulas.get(&cell) {
                let res = self.evaluate_formula(formula_str, grid)?;
                grid.insert(cell, res);
            }
        }

        Ok(())
    }
}
