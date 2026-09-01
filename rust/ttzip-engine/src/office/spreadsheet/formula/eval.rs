// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Formula evaluation engine and built-in spreadsheet functions.

use std::collections::HashMap;
use super::ast::{BinaryOp, FormulaExpr, UnaryOp};
use crate::office::types::{OfficeCellAddress, OfficeCellValue, OfficeError, OfficeResult};

/// Evaluates a parsed formula AST against a grid of cell values.
pub fn evaluate_expr(
    expr: &FormulaExpr,
    grid: &HashMap<OfficeCellAddress, OfficeCellValue>,
) -> OfficeResult<OfficeCellValue> {
    match expr {
        FormulaExpr::Number(n) => Ok(OfficeCellValue::Float(*n)),
        FormulaExpr::Text(s) => Ok(OfficeCellValue::String(s.clone())),
        FormulaExpr::Bool(b) => Ok(OfficeCellValue::Bool(*b)),
        FormulaExpr::Cell(addr) => Ok(grid.get(addr).cloned().unwrap_or(OfficeCellValue::Empty)),
        FormulaExpr::Range(start, _) => {
            Ok(grid.get(start).cloned().unwrap_or(OfficeCellValue::Empty))
        }
        FormulaExpr::Unary(op, inner) => {
            let val = evaluate_expr(inner, grid)?;
            let num = val.as_f64().unwrap_or(0.0);
            match op {
                UnaryOp::Plus => Ok(OfficeCellValue::Float(num)),
                UnaryOp::Minus => Ok(OfficeCellValue::Float(-num)),
            }
        }
        FormulaExpr::Binary(op, left, right) => {
            let left_val = evaluate_expr(left, grid)?;
            let right_val = evaluate_expr(right, grid)?;
            eval_binary_op(*op, &left_val, &right_val)
        }
        FormulaExpr::Function(name, args) => eval_function(name, args, grid),
    }
}

pub fn eval_binary_op(op: BinaryOp, left: &OfficeCellValue, right: &OfficeCellValue) -> OfficeResult<OfficeCellValue> {
    if left.is_error() {
        return Ok(left.clone());
    }
    if right.is_error() {
        return Ok(right.clone());
    }

    match op {
        BinaryOp::Concat => Ok(OfficeCellValue::String(format!("{}{}", left.as_string(), right.as_string()))),
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Pow => {
            let l = left.as_f64().unwrap_or(0.0);
            let r = right.as_f64().unwrap_or(0.0);
            match op {
                BinaryOp::Add => Ok(OfficeCellValue::Float(l + r)),
                BinaryOp::Sub => Ok(OfficeCellValue::Float(l - r)),
                BinaryOp::Mul => Ok(OfficeCellValue::Float(l * r)),
                BinaryOp::Div => {
                    if r == 0.0 {
                        Ok(OfficeCellValue::Error("#DIV/0!".to_string()))
                    } else {
                        Ok(OfficeCellValue::Float(l / r))
                    }
                }
                BinaryOp::Pow => Ok(OfficeCellValue::Float(l.powf(r))),
                _ => unreachable!(),
            }
        }
        BinaryOp::Eq => Ok(OfficeCellValue::Bool(left.as_string().eq_ignore_ascii_case(&right.as_string()))),
        BinaryOp::NotEq => Ok(OfficeCellValue::Bool(!left.as_string().eq_ignore_ascii_case(&right.as_string()))),
        BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => {
            let l = left.as_f64().unwrap_or(0.0);
            let r = right.as_f64().unwrap_or(0.0);
            let b = match op {
                BinaryOp::Lt => l < r,
                BinaryOp::LtEq => l <= r,
                BinaryOp::Gt => l > r,
                BinaryOp::GtEq => l >= r,
                _ => unreachable!(),
            };
            Ok(OfficeCellValue::Bool(b))
        }
    }
}

fn eval_function(
    name: &str,
    args: &[FormulaExpr],
    grid: &HashMap<OfficeCellAddress, OfficeCellValue>,
) -> OfficeResult<OfficeCellValue> {
    let upper_name = name.to_ascii_uppercase();
    match upper_name.as_str() {
        "SUM" => {
            let vals = expand_args_to_values(args, grid)?;
            let sum: f64 = vals.iter().filter_map(|v| v.as_f64()).sum();
            Ok(OfficeCellValue::Float(sum))
        }
        "AVERAGE" => {
            let vals = expand_args_to_values(args, grid)?;
            let nums: Vec<f64> = vals.iter().filter_map(|v| v.as_f64()).collect();
            if nums.is_empty() {
                Ok(OfficeCellValue::Error("#DIV/0!".to_string()))
            } else {
                let avg = nums.iter().sum::<f64>() / (nums.len() as f64);
                Ok(OfficeCellValue::Float(avg))
            }
        }
        "MIN" => {
            let vals = expand_args_to_values(args, grid)?;
            let nums: Vec<f64> = vals.iter().filter_map(|v| v.as_f64()).collect();
            let min = nums.into_iter().fold(f64::INFINITY, f64::min);
            if min.is_infinite() {
                Ok(OfficeCellValue::Float(0.0))
            } else {
                Ok(OfficeCellValue::Float(min))
            }
        }
        "MAX" => {
            let vals = expand_args_to_values(args, grid)?;
            let nums: Vec<f64> = vals.iter().filter_map(|v| v.as_f64()).collect();
            let max = nums.into_iter().fold(f64::NEG_INFINITY, f64::max);
            if max.is_infinite() {
                Ok(OfficeCellValue::Float(0.0))
            } else {
                Ok(OfficeCellValue::Float(max))
            }
        }
        "COUNT" => {
            let vals = expand_args_to_values(args, grid)?;
            let cnt = vals.iter().filter(|v| v.as_f64().is_some() && !v.is_empty()).count();
            Ok(OfficeCellValue::Int(cnt as i64))
        }
        "COUNTA" => {
            let vals = expand_args_to_values(args, grid)?;
            let cnt = vals.iter().filter(|v| !v.is_empty()).count();
            Ok(OfficeCellValue::Int(cnt as i64))
        }
        "COUNTBLANK" => {
            let vals = expand_args_to_values(args, grid)?;
            let cnt = vals.iter().filter(|v| v.is_empty()).count();
            Ok(OfficeCellValue::Int(cnt as i64))
        }
        "IF" => {
            if args.is_empty() {
                return Err(OfficeError::EvaluationError("IF requires at least 2 arguments".to_string()));
            }
            let cond_val = evaluate_expr(&args[0], grid)?;
            let cond = cond_val.as_bool().unwrap_or(false);
            if cond {
                if args.len() > 1 {
                    evaluate_expr(&args[1], grid)
                } else {
                    Ok(OfficeCellValue::Bool(true))
                }
            } else if args.len() > 2 {
                evaluate_expr(&args[2], grid)
            } else {
                Ok(OfficeCellValue::Bool(false))
            }
        }
        "IFS" => {
            let mut i = 0;
            while i < args.len() {
                let cond = evaluate_expr(&args[i], grid)?.as_bool().unwrap_or(false);
                if cond {
                    if i + 1 < args.len() {
                        return evaluate_expr(&args[i + 1], grid);
                    } else {
                        return Ok(OfficeCellValue::Bool(true));
                    }
                }
                i += 2;
            }
            Ok(OfficeCellValue::Error("#N/A".to_string()))
        }
        "AND" => {
            let vals = expand_args_to_values(args, grid)?;
            let res = vals.iter().all(|v| v.as_bool().unwrap_or(false));
            Ok(OfficeCellValue::Bool(res))
        }
        "OR" => {
            let vals = expand_args_to_values(args, grid)?;
            let res = vals.iter().any(|v| v.as_bool().unwrap_or(false));
            Ok(OfficeCellValue::Bool(res))
        }
        "NOT" => {
            if args.is_empty() {
                return Ok(OfficeCellValue::Bool(true));
            }
            let val = evaluate_expr(&args[0], grid)?.as_bool().unwrap_or(false);
            Ok(OfficeCellValue::Bool(!val))
        }
        "CONCAT" | "CONCATENATE" => {
            let vals = expand_args_to_values(args, grid)?;
            let joined: String = vals.iter().map(|v| v.as_string()).collect();
            Ok(OfficeCellValue::String(joined))
        }
        "TEXTJOIN" => {
            if args.len() < 3 {
                return Err(OfficeError::EvaluationError("TEXTJOIN requires delimiter, ignore_empty, text...".to_string()));
            }
            let delim = evaluate_expr(&args[0], grid)?.as_string();
            let ignore_empty = evaluate_expr(&args[1], grid)?.as_bool().unwrap_or(true);
            let vals = expand_args_to_values(&args[2..], grid)?;
            let strings: Vec<String> = vals
                .into_iter()
                .filter(|v| !ignore_empty || !v.is_empty())
                .map(|v| v.as_string())
                .collect();
            Ok(OfficeCellValue::String(strings.join(&delim)))
        }
        "LEFT" => {
            let s = args.first().map(|a| evaluate_expr(a, grid)).transpose()?.unwrap_or_default().as_string();
            let len = args.get(1).map(|a| evaluate_expr(a, grid)).transpose()?.and_then(|v| v.as_i64()).unwrap_or(1).max(0) as usize;
            let res: String = s.chars().take(len).collect();
            Ok(OfficeCellValue::String(res))
        }
        "RIGHT" => {
            let s = args.first().map(|a| evaluate_expr(a, grid)).transpose()?.unwrap_or_default().as_string();
            let len = args.get(1).map(|a| evaluate_expr(a, grid)).transpose()?.and_then(|v| v.as_i64()).unwrap_or(1).max(0) as usize;
            let char_count = s.chars().count();
            let skip = char_count.saturating_sub(len);
            let res: String = s.chars().skip(skip).collect();
            Ok(OfficeCellValue::String(res))
        }
        "MID" => {
            let s = args.first().map(|a| evaluate_expr(a, grid)).transpose()?.unwrap_or_default().as_string();
            let start_1based = args.get(1).map(|a| evaluate_expr(a, grid)).transpose()?.and_then(|v| v.as_i64()).unwrap_or(1).max(1) as usize;
            let len = args.get(2).map(|a| evaluate_expr(a, grid)).transpose()?.and_then(|v| v.as_i64()).unwrap_or(0).max(0) as usize;
            let res: String = s.chars().skip(start_1based - 1).take(len).collect();
            Ok(OfficeCellValue::String(res))
        }
        "LEN" => {
            let s = args.first().map(|a| evaluate_expr(a, grid)).transpose()?.unwrap_or_default().as_string();
            Ok(OfficeCellValue::Int(s.chars().count() as i64))
        }
        "TRIM" => {
            let s = args.first().map(|a| evaluate_expr(a, grid)).transpose()?.unwrap_or_default().as_string();
            Ok(OfficeCellValue::String(s.trim().to_string()))
        }
        "UPPER" => {
            let s = args.first().map(|a| evaluate_expr(a, grid)).transpose()?.unwrap_or_default().as_string();
            Ok(OfficeCellValue::String(s.to_uppercase()))
        }
        "LOWER" => {
            let s = args.first().map(|a| evaluate_expr(a, grid)).transpose()?.unwrap_or_default().as_string();
            Ok(OfficeCellValue::String(s.to_lowercase()))
        }
        "ABS" => {
            let num = args.first().map(|a| evaluate_expr(a, grid)).transpose()?.and_then(|v| v.as_f64()).unwrap_or(0.0);
            Ok(OfficeCellValue::Float(num.abs()))
        }
        "ROUND" => {
            let num = args.first().map(|a| evaluate_expr(a, grid)).transpose()?.and_then(|v| v.as_f64()).unwrap_or(0.0);
            let decimals = args.get(1).map(|a| evaluate_expr(a, grid)).transpose()?.and_then(|v| v.as_i64()).unwrap_or(0);
            let factor = 10.0_f64.powi(decimals as i32);
            let rounded = (num * factor).round() / factor;
            Ok(OfficeCellValue::Float(rounded))
        }
        "POWER" => {
            let base = args.first().map(|a| evaluate_expr(a, grid)).transpose()?.and_then(|v| v.as_f64()).unwrap_or(0.0);
            let exp = args.get(1).map(|a| evaluate_expr(a, grid)).transpose()?.and_then(|v| v.as_f64()).unwrap_or(1.0);
            Ok(OfficeCellValue::Float(base.powf(exp)))
        }
        "SQRT" => {
            let num = args.first().map(|a| evaluate_expr(a, grid)).transpose()?.and_then(|v| v.as_f64()).unwrap_or(0.0);
            if num < 0.0 {
                Ok(OfficeCellValue::Error("#NUM!".to_string()))
            } else {
                Ok(OfficeCellValue::Float(num.sqrt()))
            }
        }
        "MOD" => {
            let n = args.first().map(|a| evaluate_expr(a, grid)).transpose()?.and_then(|v| v.as_f64()).unwrap_or(0.0);
            let d = args.get(1).map(|a| evaluate_expr(a, grid)).transpose()?.and_then(|v| v.as_f64()).unwrap_or(1.0);
            if d == 0.0 {
                Ok(OfficeCellValue::Error("#DIV/0!".to_string()))
            } else {
                Ok(OfficeCellValue::Float(n % d))
            }
        }
        "VLOOKUP" => eval_vlookup(args, grid),
        "INDEX" => eval_index(args, grid),
        "MATCH" => eval_match(args, grid),
        _ => Err(OfficeError::EvaluationError(format!("Unsupported function: {name}"))),
    }
}

pub fn expand_args_to_values(
    args: &[FormulaExpr],
    grid: &HashMap<OfficeCellAddress, OfficeCellValue>,
) -> OfficeResult<Vec<OfficeCellValue>> {
    let mut results = Vec::new();
    for arg in args {
        match arg {
            FormulaExpr::Range(start, end) => {
                let min_r = start.row.min(end.row);
                let max_r = start.row.max(end.row);
                let min_c = start.col.min(end.col);
                let max_c = start.col.max(end.col);
                for r in min_r..=max_r {
                    for c in min_c..=max_c {
                        let addr = OfficeCellAddress::from_row_col(r, c);
                        let val = grid.get(&addr).cloned().unwrap_or(OfficeCellValue::Empty);
                        results.push(val);
                    }
                }
            }
            _ => {
                let val = evaluate_expr(arg, grid)?;
                results.push(val);
            }
        }
    }
    Ok(results)
}

fn eval_vlookup(
    args: &[FormulaExpr],
    grid: &HashMap<OfficeCellAddress, OfficeCellValue>,
) -> OfficeResult<OfficeCellValue> {
    if args.len() < 3 {
        return Err(OfficeError::EvaluationError("VLOOKUP requires lookup_value, table_array, col_index".to_string()));
    }
    let lookup_val = evaluate_expr(&args[0], grid)?;
    let range_expr = &args[1];
    let col_idx_1based = evaluate_expr(&args[2], grid)?.as_i64().unwrap_or(1);
    let exact_match = if args.len() > 3 {
        !evaluate_expr(&args[3], grid)?.as_bool().unwrap_or(true)
    } else {
        false
    };

    if let FormulaExpr::Range(start, end) = range_expr {
        let min_r = start.row.min(end.row);
        let max_r = start.row.max(end.row);
        let min_c = start.col.min(end.col);
        let max_c = start.col.max(end.col);

        let target_col = min_c + (col_idx_1based as u32).saturating_sub(1);
        if target_col > max_c {
            return Ok(OfficeCellValue::Error("#REF!".to_string()));
        }

        for r in min_r..=max_r {
            let lookup_cell_addr = OfficeCellAddress::from_row_col(r, min_c);
            let cell_val = grid.get(&lookup_cell_addr).cloned().unwrap_or(OfficeCellValue::Empty);
            let matched = if exact_match {
                cell_val.as_string().eq_ignore_ascii_case(&lookup_val.as_string())
            } else {
                cell_val == lookup_val || cell_val.as_string() == lookup_val.as_string()
            };

            if matched {
                let res_addr = OfficeCellAddress::from_row_col(r, target_col);
                return Ok(grid.get(&res_addr).cloned().unwrap_or(OfficeCellValue::Empty));
            }
        }
        Ok(OfficeCellValue::Error("#N/A".to_string()))
    } else {
        Ok(OfficeCellValue::Error("#VALUE!".to_string()))
    }
}

fn eval_index(
    args: &[FormulaExpr],
    grid: &HashMap<OfficeCellAddress, OfficeCellValue>,
) -> OfficeResult<OfficeCellValue> {
    if args.len() < 2 {
        return Err(OfficeError::EvaluationError("INDEX requires array, row_num".to_string()));
    }
    if let FormulaExpr::Range(start, end) = &args[0] {
        let row_num = evaluate_expr(&args[1], grid)?.as_i64().unwrap_or(1);
        let col_num = if args.len() > 2 {
            evaluate_expr(&args[2], grid)?.as_i64().unwrap_or(1)
        } else {
            1
        };

        let min_r = start.row.min(end.row);
        let max_r = start.row.max(end.row);
        let min_c = start.col.min(end.col);
        let max_c = start.col.max(end.col);

        let target_r = min_r + (row_num as u32).saturating_sub(1);
        let target_c = min_c + (col_num as u32).saturating_sub(1);

        if target_r > max_r || target_c > max_c {
            return Ok(OfficeCellValue::Error("#REF!".to_string()));
        }

        let addr = OfficeCellAddress::from_row_col(target_r, target_c);
        Ok(grid.get(&addr).cloned().unwrap_or(OfficeCellValue::Empty))
    } else {
        Ok(OfficeCellValue::Error("#VALUE!".to_string()))
    }
}

fn eval_match(
    args: &[FormulaExpr],
    grid: &HashMap<OfficeCellAddress, OfficeCellValue>,
) -> OfficeResult<OfficeCellValue> {
    if args.len() < 2 {
        return Err(OfficeError::EvaluationError("MATCH requires lookup_value, lookup_array".to_string()));
    }
    let lookup_val = evaluate_expr(&args[0], grid)?;
    if let FormulaExpr::Range(start, end) = &args[1] {
        let min_r = start.row.min(end.row);
        let max_r = start.row.max(end.row);
        let min_c = start.col.min(end.col);
        let max_c = start.col.max(end.col);

        let is_row = min_r == max_r;
        let mut idx = 1;

        if is_row {
            for c in min_c..=max_c {
                let addr = OfficeCellAddress::from_row_col(min_r, c);
                let val = grid.get(&addr).cloned().unwrap_or(OfficeCellValue::Empty);
                if val.as_string().eq_ignore_ascii_case(&lookup_val.as_string()) {
                    return Ok(OfficeCellValue::Int(idx));
                }
                idx += 1;
            }
        } else {
            for r in min_r..=max_r {
                let addr = OfficeCellAddress::from_row_col(r, min_c);
                let val = grid.get(&addr).cloned().unwrap_or(OfficeCellValue::Empty);
                if val.as_string().eq_ignore_ascii_case(&lookup_val.as_string()) {
                    return Ok(OfficeCellValue::Int(idx));
                }
                idx += 1;
            }
        }
        Ok(OfficeCellValue::Error("#N/A".to_string()))
    } else {
        Ok(OfficeCellValue::Error("#VALUE!".to_string()))
    }
}
