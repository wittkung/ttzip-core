// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Zero-allocation lifetime-borrowed DSL Lexer for archive filter expressions.

use std::fmt;

/// Lexical tokens borrowing slices directly from query string with zero allocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DslToken<'a> {
    Identifier(&'a str),
    Colon,
    StringLiteral(&'a str),
    NumberLiteral(i64),
    And,
    Or,
    Not,
    LeftParen,
    RightParen,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Equals,
    Comma,
}

impl<'a> DslToken<'a> {
    const fn static_symbol(&self) -> &'static str {
        match self {
            DslToken::Colon => ":",
            DslToken::And => "AND",
            DslToken::Or => "OR",
            DslToken::Not => "NOT",
            DslToken::LeftParen => "(",
            DslToken::RightParen => ")",
            DslToken::GreaterThan => ">",
            DslToken::LessThan => "<",
            DslToken::GreaterThanOrEqual => ">=",
            DslToken::LessThanOrEqual => "<=",
            DslToken::Equals => "=",
            DslToken::Comma => ",",
            DslToken::Identifier(_) | DslToken::StringLiteral(_) | DslToken::NumberLiteral(_) => "",
        }
    }
}

impl<'a> fmt::Display for DslToken<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DslToken::Identifier(val) => write!(f, "ID({})", val),
            DslToken::StringLiteral(str_val) => write!(f, "\"{}\"", str_val),
            DslToken::NumberLiteral(num) => write!(f, "NUM({})", num),
            other => f.write_str(other.static_symbol()),
        }
    }
}

/// DSL parsing and tokenization error cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DslParseError {
    UnexpectedToken {
        expected: &'static str,
        found: Option<String>,
    },
    InvalidSyntax {
        message: &'static str,
        position: usize,
    },
    UnknownField(String),
    InvalidSizeFormat(String),
    InvalidDateFormat(String),
    EmptyQuery,
}

impl fmt::Display for DslParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DslParseError::UnexpectedToken { expected, found } => {
                let token_desc = found.as_deref().unwrap_or("EOF");
                write!(f, "Unexpected token '{}', expected: {}", token_desc, expected)
            }
            DslParseError::InvalidSyntax { message, position } => {
                write!(f, "Syntax error at position {}: {}", position, message)
            }
            DslParseError::UnknownField(name) => write!(f, "Unknown filter field '{}'", name),
            DslParseError::InvalidSizeFormat(val) => write!(f, "Invalid size specification '{}'", val),
            DslParseError::InvalidDateFormat(val) => write!(f, "Invalid date specification '{}'", val),
            DslParseError::EmptyQuery => write!(f, "Empty search query"),
        }
    }
}

impl std::error::Error for DslParseError {}

/// Zero-allocation lexer over a borrowed query string slice.
pub struct DslLexer<'a> {
    input: &'a str,
}

impl<'a> DslLexer<'a> {
    #[inline]
    pub fn new(input: &'a str) -> Self {
        Self { input }
    }

    fn lex_punctuation(b: u8) -> Option<DslToken<'static>> {
        match b {
            b'(' => Some(DslToken::LeftParen),
            b')' => Some(DslToken::RightParen),
            b':' => Some(DslToken::Colon),
            b',' => Some(DslToken::Comma),
            _ => None,
        }
    }

    fn lex_operator(bytes: &[u8], idx: usize) -> Option<(DslToken<'static>, usize)> {
        let b = bytes[idx];
        let next = bytes.get(idx + 1).copied();
        match (b, next) {
            (b'>', Some(b'=')) => Some((DslToken::GreaterThanOrEqual, 2)),
            (b'>', _) => Some((DslToken::GreaterThan, 1)),
            (b'<', Some(b'=')) => Some((DslToken::LessThanOrEqual, 2)),
            (b'<', _) => Some((DslToken::LessThan, 1)),
            (b'=', Some(b'=')) => Some((DslToken::Equals, 2)),
            (b'=', _) => Some((DslToken::Equals, 1)),
            (b'!', Some(b'=')) => Some((DslToken::Identifier("!="), 2)),
            (b'!', _) => Some((DslToken::Not, 1)),
            (b'&', Some(b'&')) => Some((DslToken::And, 2)),
            (b'|', Some(b'|')) => Some((DslToken::Or, 2)),
            _ => None,
        }
    }

    fn lex_quoted_literal(&self, start_idx: usize) -> Result<(DslToken<'a>, usize), DslParseError> {
        let bytes = self.input.as_bytes();
        let quote = bytes[start_idx];
        let content_start = start_idx + 1;
        let mut idx = content_start;

        while idx < bytes.len() {
            if bytes[idx] == quote {
                let literal = &self.input[content_start..idx];
                return Ok((DslToken::StringLiteral(literal), idx + 1));
            }
            idx += 1;
        }

        Err(DslParseError::InvalidSyntax {
            message: "Unterminated string literal",
            position: idx,
        })
    }

    #[inline]
    fn is_word_delimiter(bytes: &[u8], idx: usize) -> bool {
        let c = bytes[idx];
        if c.is_ascii_whitespace()
            || matches!(
                c,
                b'(' | b')' | b':' | b',' | b'>' | b'<' | b'=' | b'"' | b'\''
            )
        {
            return true;
        }
        let next = bytes.get(idx + 1).copied();
        matches!((c, next), (b'&', Some(b'&')) | (b'|', Some(b'|')))
    }

    fn classify_word(word: &'a str) -> DslToken<'a> {
        if word.eq_ignore_ascii_case("AND") {
            DslToken::And
        } else if word.eq_ignore_ascii_case("OR") {
            DslToken::Or
        } else if word.eq_ignore_ascii_case("NOT") {
            DslToken::Not
        } else if let Ok(num) = word.parse::<i64>() {
            DslToken::NumberLiteral(num)
        } else {
            DslToken::Identifier(word)
        }
    }

    fn lex_word(&self, start: usize) -> Result<(DslToken<'a>, usize), DslParseError> {
        let bytes = self.input.as_bytes();
        let mut idx = start;
        while idx < bytes.len() && !Self::is_word_delimiter(bytes, idx) {
            idx += 1;
        }
        if start == idx {
            return Err(DslParseError::InvalidSyntax {
                message: "Unexpected character",
                position: start,
            });
        }
        Ok((Self::classify_word(&self.input[start..idx]), idx))
    }

    /// Tokenizes the input string into a list of borrowed tokens.
    pub fn tokenize(&self) -> Result<Vec<DslToken<'a>>, DslParseError> {
        let bytes = self.input.as_bytes();
        let len = bytes.len();
        let mut idx = 0;
        let mut tokens = Vec::new();

        while idx < len {
            let b = bytes[idx];
            if b.is_ascii_whitespace() {
                idx += 1;
            } else if let Some(tok) = Self::lex_punctuation(b) {
                tokens.push(tok);
                idx += 1;
            } else if let Some((tok, step)) = Self::lex_operator(bytes, idx) {
                tokens.push(tok);
                idx += step;
            } else if b == b'"' || b == b'\'' {
                let (tok, next_idx) = self.lex_quoted_literal(idx)?;
                tokens.push(tok);
                idx = next_idx;
            } else {
                let (tok, next_idx) = self.lex_word(idx)?;
                tokens.push(tok);
                idx = next_idx;
            }
        }

        Ok(tokens)
    }
}
