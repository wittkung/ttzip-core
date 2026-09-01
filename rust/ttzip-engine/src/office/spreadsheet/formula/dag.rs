// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Dependency graph construction, Tarjan SCC cycle detection, and topological sorting.

use std::collections::{HashMap, HashSet};
use super::ast::FormulaExpr;
use crate::office::types::OfficeCellAddress;

/// Collects all cell references required by an expression.
pub fn collect_deps(expr: &FormulaExpr, deps: &mut Vec<OfficeCellAddress>) {
    match expr {
        FormulaExpr::Cell(addr) => deps.push(addr.clone()),
        FormulaExpr::Range(start, end) => {
            let min_r = start.row.min(end.row);
            let max_r = start.row.max(end.row);
            let min_c = start.col.min(end.col);
            let max_c = start.col.max(end.col);
            for r in min_r..=max_r {
                for c in min_c..=max_c {
                    deps.push(OfficeCellAddress::from_row_col(r, c));
                }
            }
        }
        FormulaExpr::Unary(_, inner) => collect_deps(inner, deps),
        FormulaExpr::Binary(_, left, right) => {
            collect_deps(left, deps);
            collect_deps(right, deps);
        }
        FormulaExpr::Function(_, args) => {
            for arg in args {
                collect_deps(arg, deps);
            }
        }
        _ => {}
    }
}

/// Tarjan state machine context for finding Strongly Connected Components.
struct TarjanContext<'a> {
    dag: &'a HashMap<OfficeCellAddress, Vec<OfficeCellAddress>>,
    index: usize,
    indices: HashMap<OfficeCellAddress, usize>,
    lowlink: HashMap<OfficeCellAddress, usize>,
    on_stack: HashSet<OfficeCellAddress>,
    stack: Vec<OfficeCellAddress>,
    cycle_nodes: HashSet<OfficeCellAddress>,
}

impl<'a> TarjanContext<'a> {
    fn new(dag: &'a HashMap<OfficeCellAddress, Vec<OfficeCellAddress>>) -> Self {
        Self {
            dag,
            index: 0,
            indices: HashMap::new(),
            lowlink: HashMap::new(),
            on_stack: HashSet::new(),
            stack: Vec::new(),
            cycle_nodes: HashSet::new(),
        }
    }

    fn visit(&mut self, u: &OfficeCellAddress) {
        self.indices.insert(u.clone(), self.index);
        self.lowlink.insert(u.clone(), self.index);
        self.index += 1;
        self.stack.push(u.clone());
        self.on_stack.insert(u.clone());

        if let Some(neighbors) = self.dag.get(u) {
            for v in neighbors {
                if !self.dag.contains_key(v) {
                    continue;
                }
                if !self.indices.contains_key(v) {
                    self.visit(v);
                    let v_low = *self.lowlink.get(v).unwrap();
                    let u_low = self.lowlink.get_mut(u).unwrap();
                    *u_low = (*u_low).min(v_low);
                } else if self.on_stack.contains(v) {
                    let v_idx = *self.indices.get(v).unwrap();
                    let u_low = self.lowlink.get_mut(u).unwrap();
                    *u_low = (*u_low).min(v_idx);
                }
            }
        }

        if self.lowlink.get(u) == self.indices.get(u) {
            let mut scc = Vec::new();
            loop {
                let w = self.stack.pop().unwrap();
                self.on_stack.remove(&w);
                scc.push(w.clone());
                if &w == u {
                    break;
                }
            }
            if scc.len() > 1 {
                for node in scc {
                    self.cycle_nodes.insert(node);
                }
            } else if let Some(node) = scc.first() {
                if let Some(neighbors) = self.dag.get(node) {
                    if neighbors.contains(node) {
                        self.cycle_nodes.insert(node.clone());
                    }
                }
            }
        }
    }
}

/// Detects cycles in the dependency DAG using Tarjan's Strongly Connected Components algorithm.
pub fn detect_dag_cycles(
    dag: &HashMap<OfficeCellAddress, Vec<OfficeCellAddress>>,
) -> HashSet<OfficeCellAddress> {
    let mut ctx = TarjanContext::new(dag);
    for node in dag.keys() {
        if !ctx.indices.contains_key(node) {
            ctx.visit(node);
        }
    }
    ctx.cycle_nodes
}

/// Performs topological sorting on acyclic nodes.
pub fn topo_sort_dag(
    node: &OfficeCellAddress,
    dag: &HashMap<OfficeCellAddress, Vec<OfficeCellAddress>>,
    cycle_cells: &HashSet<OfficeCellAddress>,
    visited: &mut HashSet<OfficeCellAddress>,
    order: &mut Vec<OfficeCellAddress>,
) {
    if visited.contains(node) || cycle_cells.contains(node) {
        return;
    }
    visited.insert(node.clone());
    if let Some(deps) = dag.get(node) {
        for dep in deps {
            if dag.contains_key(dep) {
                topo_sort_dag(dep, dag, cycle_cells, visited, order);
            }
        }
    }
    order.push(node.clone());
}
