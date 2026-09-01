// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Abstract Syntax Tree (AST) representations for spreadsheet formulas.

use crate::office::types::OfficeCellAddress;

/// AST node representing a parsed formula expression.
#[derive(Debug, Clone, PartialEq)]
pub enum FormulaExpr {
    Number(f64),
    Text(String),
    Bool(bool),
    Cell(OfficeCellAddress),
    Range(OfficeCellAddress, OfficeCellAddress),
    Unary(UnaryOp, Box<FormulaExpr>),
    Binary(BinaryOp, Box<FormulaExpr>, Box<FormulaExpr>),
    Function(String, Vec<FormulaExpr>),
}

/// Unary operators in formulas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
}

/// Binary operators in formulas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Concat,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
}
