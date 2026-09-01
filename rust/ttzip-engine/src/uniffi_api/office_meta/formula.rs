// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Dynamic Spreadsheet Formula Evaluation Engine and Cell Coordinate Utilities.

use std::collections::HashMap;

use super::types::{UniFFICell, UniFFICellValue, UniFFIOfficeError};

/// Parses an Excel coordinate like "A1", "BC15", "$D$5" into 1-based (row, col) tuple.
pub fn parse_cell_coordinate(coord: &str, fallback_row: u32) -> (u32, u32) {
    if coord.is_empty() {
        return (fallback_row, 1);
    }

    let clean = coord.replace('$', "");
    let mut col_str = String::new();
    let mut row_str = String::new();

    for ch in clean.chars() {
        if ch.is_ascii_alphabetic() {
            col_str.push(ch.to_ascii_uppercase());
        } else if ch.is_ascii_digit() {
            row_str.push(ch);
        }
    }

    let col_num = if col_str.is_empty() {
        1
    } else {
        let mut n = 0u32;
        for byte in col_str.bytes() {
            n = n.saturating_mul(26).saturating_add((byte - b'A' + 1) as u32);
        }
        n
    };

    let row_num = row_str.parse::<u32>().unwrap_or(fallback_row);
    (row_num, col_num)
}

/// Formats 1-based (row, col) back to alphanumeric coordinate string (e.g. 1, 1 -> "A1").
pub fn format_coordinate(row: u32, mut col: u32) -> String {
    let mut col_str = String::new();
    while col > 0 {
        let rem = ((col - 1) % 26) as u8;
        col_str.insert(0, (b'A' + rem) as char);
        col = (col - 1) / 26;
    }
    if col_str.is_empty() {
        col_str.push('A');
    }
    format!("{col_str}{row}")
}

// ============================================================================
// Dynamic Formula Evaluation Engine
// ============================================================================

/// Evaluates a standalone formula or formula with context cells.
pub fn evaluate_spreadsheet_formula(
    formula: &str,
    context_cells: Option<&[UniFFICell]>,
) -> Result<UniFFICellValue, UniFFIOfficeError> {
    let trimmed = formula.trim().trim_start_matches('=').trim();
    if trimmed.is_empty() {
        return Ok(UniFFICellValue::Empty);
    }

    // Build context lookup table
    let mut cell_map = HashMap::new();
    if let Some(cells) = context_cells {
        for c in cells {
            cell_map.insert(c.coordinate.to_uppercase(), c.value.clone());
        }
    }

    let upper = trimmed.to_uppercase();

    // 1. Function evaluation: SUM, AVERAGE, MIN, MAX, COUNT, IF, CONCAT
    if upper.starts_with("SUM(") && upper.ends_with(')') {
        let inner = &trimmed[4..trimmed.len() - 1];
        let vals = resolve_numeric_args(inner, &cell_map)?;
        let sum: f64 = vals.iter().sum();
        return Ok(UniFFICellValue::Number { value: sum });
    }

    if upper.starts_with("AVERAGE(") && upper.ends_with(')') {
        let inner = &trimmed[8..trimmed.len() - 1];
        let vals = resolve_numeric_args(inner, &cell_map)?;
        if vals.is_empty() {
            return Ok(UniFFICellValue::Error {
                message: "DIV/0!".to_string(),
            });
        }
        let avg: f64 = vals.iter().sum::<f64>() / (vals.len() as f64);
        return Ok(UniFFICellValue::Number { value: avg });
    }

    if upper.starts_with("MIN(") && upper.ends_with(')') {
        let inner = &trimmed[4..trimmed.len() - 1];
        let vals = resolve_numeric_args(inner, &cell_map)?;
        let min_val = vals.into_iter().fold(f64::INFINITY, f64::min);
        return Ok(UniFFICellValue::Number {
            value: if min_val.is_infinite() { 0.0 } else { min_val },
        });
    }

    if upper.starts_with("MAX(") && upper.ends_with(')') {
        let inner = &trimmed[4..trimmed.len() - 1];
        let vals = resolve_numeric_args(inner, &cell_map)?;
        let max_val = vals.into_iter().fold(f64::NEG_INFINITY, f64::max);
        return Ok(UniFFICellValue::Number {
            value: if max_val.is_infinite() { 0.0 } else { max_val },
        });
    }

    if upper.starts_with("COUNT(") && upper.ends_with(')') {
        let inner = &trimmed[6..trimmed.len() - 1];
        let vals = resolve_numeric_args(inner, &cell_map)?;
        return Ok(UniFFICellValue::Number {
            value: vals.len() as f64,
        });
    }

    if upper.starts_with("CONCAT(") && upper.ends_with(')') {
        let inner = &trimmed[7..trimmed.len() - 1];
        let res = evaluate_concat(inner, &cell_map);
        return Ok(UniFFICellValue::Text { value: res });
    }

    if upper.starts_with("CONCATENATE(") && upper.ends_with(')') {
        let inner = &trimmed[12..trimmed.len() - 1];
        let res = evaluate_concat(inner, &cell_map);
        return Ok(UniFFICellValue::Text { value: res });
    }

    if upper.starts_with("IF(") && upper.ends_with(')') {
        let inner = &trimmed[3..trimmed.len() - 1];
        return evaluate_if_function(inner, &cell_map);
    }

    // 2. Arithmetic expression evaluation (e.g. "(10 + 20) * 3" or "A1 + B1")
    let resolved_expr = substitute_cell_references(trimmed, &cell_map);
    match evaluate_arithmetic_expression(&resolved_expr) {
        Ok(num) => Ok(UniFFICellValue::Number { value: num }),
        Err(_) => {
            if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
                Ok(UniFFICellValue::Text {
                    value: trimmed[1..trimmed.len() - 1].to_string(),
                })
            } else {
                Err(UniFFIOfficeError::formula_err(formula, "Invalid formula expression"))
            }
        }
    }
}

fn resolve_numeric_args(
    args_str: &str,
    cell_map: &HashMap<String, UniFFICellValue>,
) -> Result<Vec<f64>, UniFFIOfficeError> {
    let mut results = Vec::new();
    let parts = split_formula_args(args_str);

    for part in parts {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }

        if let Some((start_coord, end_coord)) = p.split_once(':') {
            let (r1, c1) = parse_cell_coordinate(start_coord.trim(), 1);
            let (r2, c2) = parse_cell_coordinate(end_coord.trim(), 1);
            let min_r = r1.min(r2);
            let max_r = r1.max(r2);
            let min_c = c1.min(c2);
            let max_c = c1.max(c2);

            for r in min_r..=max_r {
                for c in min_c..=max_c {
                    let coord = format_coordinate(r, c);
                    if let Some(val) = cell_map.get(&coord) {
                        if let Some(num) = val.as_number() {
                            results.push(num);
                        }
                    }
                }
            }
        } else if let Some(val) = cell_map.get(&p.to_uppercase()) {
            if let Some(num) = val.as_number() {
                results.push(num);
            }
        } else if let Ok(n) = p.parse::<f64>() {
            results.push(n);
        } else if let Ok(val) = evaluate_arithmetic_expression(p) {
            results.push(val);
        }
    }

    Ok(results)
}

fn split_formula_args(args_str: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let mut in_quote = false;

    for ch in args_str.chars() {
        match ch {
            '"' => {
                in_quote = !in_quote;
                current.push(ch);
            }
            '(' if !in_quote => {
                depth += 1;
                current.push(ch);
            }
            ')' if !in_quote => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if !in_quote && depth == 0 => {
                args.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        args.push(current.trim().to_string());
    }
    args
}

fn evaluate_concat(args_str: &str, cell_map: &HashMap<String, UniFFICellValue>) -> String {
    let args = split_formula_args(args_str);
    let mut out = String::new();
    for arg in args {
        let a = arg.trim();
        if a.starts_with('"') && a.ends_with('"') && a.len() >= 2 {
            out.push_str(&a[1..a.len() - 1]);
        } else if let Some(cell) = cell_map.get(&a.to_uppercase()) {
            out.push_str(&cell.to_display_string());
        } else {
            out.push_str(a);
        }
    }
    out
}

fn evaluate_if_function(
    args_str: &str,
    cell_map: &HashMap<String, UniFFICellValue>,
) -> Result<UniFFICellValue, UniFFIOfficeError> {
    let args = split_formula_args(args_str);
    if args.len() < 2 {
        return Err(UniFFIOfficeError::formula_err(
            args_str,
            "IF function requires at least 2 arguments: IF(condition, true_value, [false_value])",
        ));
    }

    let cond_str = &args[0];
    let true_expr = &args[1];
    let false_expr = args.get(2).map(|s| s.as_str()).unwrap_or("0");

    let is_true = evaluate_condition(cond_str, cell_map)?;

    let target_expr = if is_true { true_expr } else { false_expr }.trim();
    if target_expr.starts_with('"') && target_expr.ends_with('"') && target_expr.len() >= 2 {
        Ok(UniFFICellValue::Text {
            value: target_expr[1..target_expr.len() - 1].to_string(),
        })
    } else if let Some(cell) = cell_map.get(&target_expr.to_uppercase()) {
        Ok(cell.clone())
    } else if let Ok(num) = target_expr.parse::<f64>() {
        Ok(UniFFICellValue::Number { value: num })
    } else {
        Ok(UniFFICellValue::Text {
            value: target_expr.to_string(),
        })
    }
}

fn evaluate_condition(
    cond: &str,
    cell_map: &HashMap<String, UniFFICellValue>,
) -> Result<bool, UniFFIOfficeError> {
    let clean = cond.trim();
    for op in [">=", "<=", "<>", "!=", "=", ">", "<"] {
        if let Some((lhs_str, rhs_str)) = clean.split_once(op) {
            let lhs = resolve_operand(lhs_str.trim(), cell_map);
            let rhs = resolve_operand(rhs_str.trim(), cell_map);

            if let (Some(l_num), Some(r_num)) = (lhs.as_number(), rhs.as_number()) {
                return Ok(match op {
                    ">=" => l_num >= r_num,
                    "<=" => l_num <= r_num,
                    "<>" | "!=" => (l_num - r_num).abs() > f64::EPSILON,
                    "=" => (l_num - r_num).abs() <= f64::EPSILON,
                    ">" => l_num > r_num,
                    "<" => l_num < r_num,
                    _ => false,
                });
            } else {
                let l_str = lhs.to_display_string();
                let r_str = rhs.to_display_string();
                return Ok(match op {
                    "=" => l_str == r_str,
                    "<>" | "!=" => l_str != r_str,
                    ">" => l_str > r_str,
                    "<" => l_str < r_str,
                    _ => false,
                });
            }
        }
    }

    if let Ok(num) = clean.parse::<f64>() {
        Ok(num != 0.0)
    } else {
        Ok(false)
    }
}

fn resolve_operand(op_str: &str, cell_map: &HashMap<String, UniFFICellValue>) -> UniFFICellValue {
    let clean = op_str.trim();
    if clean.starts_with('"') && clean.ends_with('"') && clean.len() >= 2 {
        UniFFICellValue::Text {
            value: clean[1..clean.len() - 1].to_string(),
        }
    } else if let Some(cell) = cell_map.get(&clean.to_uppercase()) {
        cell.clone()
    } else if let Ok(num) = clean.parse::<f64>() {
        UniFFICellValue::Number { value: num }
    } else {
        UniFFICellValue::Text {
            value: clean.to_string(),
        }
    }
}

fn substitute_cell_references(
    expr: &str,
    cell_map: &HashMap<String, UniFFICellValue>,
) -> String {
    let mut out = String::with_capacity(expr.len() + 16);
    let mut token = String::new();

    for ch in expr.chars() {
        if ch.is_ascii_alphanumeric() || ch == '$' {
            token.push(ch);
        } else {
            if !token.is_empty() {
                let upper = token.replace('$', "").to_uppercase();
                if let Some(cell) = cell_map.get(&upper) {
                    if let Some(num) = cell.as_number() {
                        out.push_str(&format!("{num}"));
                    } else {
                        out.push_str(&token);
                    }
                } else {
                    out.push_str(&token);
                }
                token.clear();
            }
            out.push(ch);
        }
    }

    if !token.is_empty() {
        let upper = token.replace('$', "").to_uppercase();
        if let Some(cell) = cell_map.get(&upper) {
            if let Some(num) = cell.as_number() {
                out.push_str(&format!("{num}"));
            } else {
                out.push_str(&token);
            }
        } else {
            out.push_str(&token);
        }
    }

    out
}

/// Simple recursive descent arithmetic parser for `+`, `-`, `*`, `/`, `^`, parentheses.
fn evaluate_arithmetic_expression(expr: &str) -> Result<f64, ()> {
    let tokens: Vec<char> = expr.chars().filter(|c| !c.is_whitespace()).collect();
    let mut pos = 0usize;
    parse_expr(&tokens, &mut pos)
}

fn parse_expr(tokens: &[char], pos: &mut usize) -> Result<f64, ()> {
    let mut val = parse_term(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos] {
            '+' => {
                *pos += 1;
                val += parse_term(tokens, pos)?;
            }
            '-' => {
                *pos += 1;
                val -= parse_term(tokens, pos)?;
            }
            _ => break,
        }
    }
    Ok(val)
}

fn parse_term(tokens: &[char], pos: &mut usize) -> Result<f64, ()> {
    let mut val = parse_factor(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos] {
            '*' => {
                *pos += 1;
                val *= parse_factor(tokens, pos)?;
            }
            '/' => {
                *pos += 1;
                let divisor = parse_factor(tokens, pos)?;
                if divisor.abs() < f64::EPSILON {
                    return Err(());
                }
                val /= divisor;
            }
            '^' => {
                *pos += 1;
                let exponent = parse_factor(tokens, pos)?;
                val = val.powf(exponent);
            }
            _ => break,
        }
    }
    Ok(val)
}

fn parse_factor(tokens: &[char], pos: &mut usize) -> Result<f64, ()> {
    if *pos >= tokens.len() {
        return Err(());
    }

    if tokens[*pos] == '+' {
        *pos += 1;
        return parse_factor(tokens, pos);
    }
    if tokens[*pos] == '-' {
        *pos += 1;
        return Ok(-parse_factor(tokens, pos)?);
    }
    if tokens[*pos] == '(' {
        *pos += 1;
        let val = parse_expr(tokens, pos)?;
        if *pos < tokens.len() && tokens[*pos] == ')' {
            *pos += 1;
            return Ok(val);
        } else {
            return Err(());
        }
    }

    let start = *pos;
    while *pos < tokens.len() && (tokens[*pos].is_ascii_digit() || tokens[*pos] == '.') {
        *pos += 1;
    }
    if start == *pos {
        return Err(());
    }

    let num_str: String = tokens[start..*pos].iter().collect();
    num_str.parse::<f64>().map_err(|_| ())
}
