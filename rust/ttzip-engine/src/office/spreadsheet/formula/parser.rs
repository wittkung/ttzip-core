// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pratt expression parsing and lexical analysis for spreadsheet formulas.

use super::ast::{BinaryOp, FormulaExpr, UnaryOp};
use crate::office::types::{OfficeCellAddress, OfficeError, OfficeResult};

/// Token representation during lexical analysis.
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Text(String),
    CellOrIdent(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Ampersand,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    LParen,
    RParen,
    Comma,
    Colon,
    Eof,
}

/// Parses a formula expression string into an AST using Pratt parsing.
pub fn parse_formula_expr(formula: &str) -> OfficeResult<FormulaExpr> {
    let tokens = tokenize(formula)?;
    let mut parser = PrattParser::new(tokens);
    parser.parse_expression(0)
}

struct PrattParser {
    tokens: Vec<Token>,
    pos: usize,
}

impl PrattParser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            tok
        } else {
            Token::Eof
        }
    }

    fn parse_expression(&mut self, min_bp: u8) -> OfficeResult<FormulaExpr> {
        let mut left = self.parse_prefix()?;

        loop {
            let tok = self.peek().clone();
            if let Some((l_bp, r_bp, op)) = infix_binding_power(&tok) {
                if l_bp < min_bp {
                    break;
                }
                self.advance();
                let right = self.parse_expression(r_bp)?;
                left = FormulaExpr::Binary(op, Box::new(left), Box::new(right));
                continue;
            }

            if tok == Token::Colon {
                if 90 < min_bp {
                    break;
                }
                self.advance();
                let right = self.parse_expression(91)?;
                match (left, right) {
                    (FormulaExpr::Cell(s), FormulaExpr::Cell(e)) => {
                        left = FormulaExpr::Range(s, e);
                    }
                    _ => return Err(OfficeError::InvalidFormula("Colon range expects cell operands".to_string())),
                }
                continue;
            }

            break;
        }

        Ok(left)
    }

    fn parse_prefix(&mut self) -> OfficeResult<FormulaExpr> {
        let tok = self.advance();
        match tok {
            Token::Number(n) => Ok(FormulaExpr::Number(n)),
            Token::Text(s) => Ok(FormulaExpr::Text(s)),
            Token::Plus => {
                let inner = self.parse_expression(70)?;
                Ok(FormulaExpr::Unary(UnaryOp::Plus, Box::new(inner)))
            }
            Token::Minus => {
                let inner = self.parse_expression(70)?;
                Ok(FormulaExpr::Unary(UnaryOp::Minus, Box::new(inner)))
            }
            Token::LParen => {
                let expr = self.parse_expression(0)?;
                if self.advance() != Token::RParen {
                    return Err(OfficeError::InvalidFormula("Expected closing parenthesis ')'".to_string()));
                }
                Ok(expr)
            }
            Token::CellOrIdent(s) => {
                if self.peek() == &Token::LParen {
                    self.advance();
                    let mut args = Vec::new();
                    if self.peek() != &Token::RParen {
                        loop {
                            let arg = self.parse_expression(0)?;
                            args.push(arg);
                            if self.peek() == &Token::Comma {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    if self.advance() != Token::RParen {
                        return Err(OfficeError::InvalidFormula(format!("Expected ')' for function {s}")));
                    }
                    Ok(FormulaExpr::Function(s, args))
                } else if let Ok(addr) = OfficeCellAddress::from_a1(&s) {
                    Ok(FormulaExpr::Cell(addr))
                } else if s.eq_ignore_ascii_case("TRUE") {
                    Ok(FormulaExpr::Bool(true))
                } else if s.eq_ignore_ascii_case("FALSE") {
                    Ok(FormulaExpr::Bool(false))
                } else {
                    Ok(FormulaExpr::Text(s))
                }
            }
            _ => Err(OfficeError::InvalidFormula(format!("Unexpected token: {tok:?}"))),
        }
    }
}

fn infix_binding_power(tok: &Token) -> Option<(u8, u8, BinaryOp)> {
    match tok {
        Token::Caret => Some((80, 79, BinaryOp::Pow)),
        Token::Star => Some((60, 61, BinaryOp::Mul)),
        Token::Slash => Some((60, 61, BinaryOp::Div)),
        Token::Plus => Some((50, 51, BinaryOp::Add)),
        Token::Minus => Some((50, 51, BinaryOp::Sub)),
        Token::Ampersand => Some((40, 41, BinaryOp::Concat)),
        Token::Eq => Some((30, 31, BinaryOp::Eq)),
        Token::NotEq => Some((30, 31, BinaryOp::NotEq)),
        Token::Lt => Some((30, 31, BinaryOp::Lt)),
        Token::LtEq => Some((30, 31, BinaryOp::LtEq)),
        Token::Gt => Some((30, 31, BinaryOp::Gt)),
        Token::GtEq => Some((30, 31, BinaryOp::GtEq)),
        _ => None,
    }
}

fn tokenize(input: &str) -> OfficeResult<Vec<Token>> {
    let mut chars = input.trim().trim_start_matches('=').trim().chars().peekable();
    let mut tokens = Vec::new();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }

        if ch == '"' {
            chars.next();
            let mut s = String::new();
            while let Some(c) = chars.next() {
                if c == '"' {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        s.push('"');
                    } else {
                        break;
                    }
                } else {
                    s.push(c);
                }
            }
            tokens.push(Token::Text(s));
            continue;
        }

        if ch.is_ascii_digit() || (ch == '.' && chars.clone().nth(1).map(|c| c.is_ascii_digit()).unwrap_or(false)) {
            let mut num_str = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() || c == '.' {
                    num_str.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            let n: f64 = num_str.parse().map_err(|_| OfficeError::InvalidFormula(format!("Invalid number: {num_str}")))?;
            tokens.push(Token::Number(n));
            continue;
        }

        if ch.is_ascii_alphabetic() || ch == '_' || ch == '$' {
            let mut ident = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
                    ident.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(Token::CellOrIdent(ident));
            continue;
        }

        match ch {
            '+' => { chars.next(); tokens.push(Token::Plus); }
            '-' => { chars.next(); tokens.push(Token::Minus); }
            '*' => { chars.next(); tokens.push(Token::Star); }
            '/' => { chars.next(); tokens.push(Token::Slash); }
            '^' => { chars.next(); tokens.push(Token::Caret); }
            '&' => { chars.next(); tokens.push(Token::Ampersand); }
            '(' => { chars.next(); tokens.push(Token::LParen); }
            ')' => { chars.next(); tokens.push(Token::RParen); }
            ',' => { chars.next(); tokens.push(Token::Comma); }
            ':' => { chars.next(); tokens.push(Token::Colon); }
            '=' => { chars.next(); tokens.push(Token::Eq); }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::LtEq);
                } else if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push(Token::NotEq);
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::GtEq);
                } else {
                    tokens.push(Token::Gt);
                }
            }
            _ => {
                chars.next();
            }
        }
    }

    tokens.push(Token::Eof);
    Ok(tokens)
}
