// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! CSS3 selector compilation and streaming element matching engine.
//!
//! Supports tag names, IDs (`#id`), classes (`.class`), attribute selectors
//! (`[attr]`, `[attr=val]`, `[attr^=val]`, `[attr$=val]`, `[attr*=val]`, `[attr~=val]`, `[attr|=val]`),
//! compound combinations, and comma-separated selector lists.

use crate::html::types::{HtmlError, HtmlResult};
use serde::{Deserialize, Serialize};

/// Attribute comparison operator used in CSS3 attribute selectors.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AttributeMatcher {
    /// Attribute presence check: `[attr]`
    Exists,
    /// Exact match: `[attr="value"]` or `[attr=value]`
    Exact(String),
    /// Prefix match: `[attr^="prefix"]`
    Prefix(String),
    /// Suffix match: `[attr$="suffix"]`
    Suffix(String),
    /// Substring match: `[attr*="sub"]`
    Substring(String),
    /// Whitespace-separated list includes item: `[attr~="item"]`
    Includes(String),
    /// Hyphen-separated prefix match: `[attr|="val"]` (matches `val` or `val-*`)
    DashPrefix(String),
}

impl AttributeMatcher {
    /// Tests whether an actual attribute value satisfies this matcher.
    #[must_use]
    pub fn matches(&self, actual_val: &str) -> bool {
        match self {
            Self::Exists => true,
            Self::Exact(expected) => actual_val == expected,
            Self::Prefix(prefix) => actual_val.starts_with(prefix),
            Self::Suffix(suffix) => actual_val.ends_with(suffix),
            Self::Substring(sub) => actual_val.contains(sub),
            Self::Includes(target) => {
                actual_val.split_whitespace().any(|word| word == target)
            }
            Self::DashPrefix(prefix) => {
                actual_val == prefix
                    || (actual_val.starts_with(prefix)
                        && actual_val.as_bytes().get(prefix.len()) == Some(&b'-'))
            }
        }
    }
}

/// A simple CSS selector component containing tag, ID, class, and attribute filters.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SimpleSelector {
    /// Optional target tag name (e.g. `img`, `div`, `*`).
    pub tag: Option<String>,
    /// Optional ID selector (e.g. `main` for `#main`).
    pub id: Option<String>,
    /// Required class names (e.g. `['thumb', 'card']` for `.thumb.card`).
    pub classes: Vec<String>,
    /// Required attribute conditions (name and matcher).
    pub attributes: Vec<(String, AttributeMatcher)>,
}

impl SimpleSelector {
    /// Tests whether an HTML element matches this simple selector.
    pub fn matches<'a, F>(&self, tag_name: &str, get_attr: F) -> bool
    where
        F: Fn(&str) -> Option<&'a str>,
    {
        // 1. Tag name check
        if let Some(ref expected_tag) = self.tag {
            if expected_tag != "*" && !expected_tag.eq_ignore_ascii_case(tag_name) {
                return false;
            }
        }

        // 2. ID check
        if let Some(ref expected_id) = self.id {
            match get_attr("id") {
                Some(actual_id) if actual_id == expected_id => {}
                _ => return false,
            }
        }

        // 3. Class list check
        if !self.classes.is_empty() {
            let actual_class_attr = get_attr("class").unwrap_or_default();
            let actual_classes: Vec<&str> = actual_class_attr.split_whitespace().collect();
            for required_class in &self.classes {
                if !actual_classes.iter().any(|&c| c == required_class) {
                    return false;
                }
            }
        }

        // 4. Attribute checks
        for (attr_name, matcher) in &self.attributes {
            match get_attr(attr_name) {
                Some(val) => {
                    if !matcher.matches(val) {
                        return false;
                    }
                }
                None => return false,
            }
        }

        true
    }
}

/// A compiled CSS3 selector supporting comma-separated alternatives (e.g. `img[src], link[href]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledSelector {
    /// Original raw selector string.
    raw: String,
    /// Disjunctive simple selector branches (matching any branch satisfies the selector).
    branches: Vec<SimpleSelector>,
}

impl CompiledSelector {
    /// Parses a CSS3 selector string into a compiled selector structure.
    pub fn parse(selector_str: &str) -> HtmlResult<Self> {
        let trimmed = selector_str.trim();
        if trimmed.is_empty() {
            return Err(HtmlError::SelectorParseError(
                "Selector string cannot be empty".to_string(),
            ));
        }

        let mut branches = Vec::new();
        // Split on top-level commas (outside brackets)
        for part in split_selector_list(trimmed) {
            let simple = parse_simple_selector(part)?;
            branches.push(simple);
        }

        if branches.is_empty() {
            return Err(HtmlError::SelectorParseError(format!(
                "No valid selector branches parsed from '{}'",
                selector_str
            )));
        }

        Ok(Self {
            raw: trimmed.to_string(),
            branches,
        })
    }

    /// Returns the raw selector string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Returns the parsed simple selector branches.
    #[must_use]
    pub fn branches(&self) -> &[SimpleSelector] {
        &self.branches
    }

    /// Tests whether an HTML element matches this compiled selector.
    pub fn matches<'a, F>(&self, tag_name: &str, get_attr: F) -> bool
    where
        F: Fn(&str) -> Option<&'a str>,
    {
        self.branches.iter().any(|b| b.matches(tag_name, &get_attr))
    }

    /// Tests whether an HTML element with slice of key-value attributes matches this selector.
    #[must_use]
    pub fn matches_attributes(&self, tag_name: &str, attributes: &[(String, String)]) -> bool {
        self.matches(tag_name, |key| {
            attributes
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v.as_str())
        })
    }
}

/// Helper function splitting comma-separated selector strings while respecting bracket nesting.
fn split_selector_list(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut in_quote = None;
    let mut start = 0;

    for (i, c) in input.char_indices() {
        match c {
            '"' | '\'' => {
                if in_quote == Some(c) {
                    in_quote = None;
                } else if in_quote.is_none() {
                    in_quote = Some(c);
                }
            }
            '[' if in_quote.is_none() => depth += 1,
            ']' if in_quote.is_none() && depth > 0 => depth -= 1,
            ',' if in_quote.is_none() && depth == 0 => {
                let chunk = input[start..i].trim();
                if !chunk.is_empty() {
                    parts.push(chunk);
                }
                start = i + 1;
            }
            _ => {}
        }
    }

    let tail = input[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}

/// Parses an individual simple selector unit (e.g. `img.avatar#thumb[src^="data:"]`).
fn parse_simple_selector(chunk: &str) -> HtmlResult<SimpleSelector> {
    let s = chunk.trim();
    if s.is_empty() {
        return Err(HtmlError::SelectorParseError(
            "Empty selector segment".to_string(),
        ));
    }

    let mut selector = SimpleSelector::default();
    let chars: Vec<char> = s.chars().collect();
    let mut idx = 0;
    let len = chars.len();

    // 1. Optional leading Tag Name or Universal Selector `*`
    if idx < len && chars[idx] != '#' && chars[idx] != '.' && chars[idx] != '[' {
        let tag_start = idx;
        while idx < len
            && chars[idx] != '#'
            && chars[idx] != '.'
            && chars[idx] != '['
            && !chars[idx].is_whitespace()
        {
            idx += 1;
        }
        let tag_name: String = chars[tag_start..idx].iter().collect();
        if !tag_name.is_empty() {
            selector.tag = Some(tag_name);
        }
    }

    // 2. Parse remaining specifiers: #id, .class, [attr]
    while idx < len {
        if chars[idx].is_whitespace() {
            idx += 1;
            continue;
        }

        match chars[idx] {
            '#' => {
                // ID selector
                idx += 1;
                let id_start = idx;
                while idx < len
                    && chars[idx] != '#'
                    && chars[idx] != '.'
                    && chars[idx] != '['
                    && !chars[idx].is_whitespace()
                {
                    idx += 1;
                }
                let id_val: String = chars[id_start..idx].iter().collect();
                if id_val.is_empty() {
                    return Err(HtmlError::SelectorParseError(format!(
                        "Invalid empty ID selector in '{}'",
                        chunk
                    )));
                }
                selector.id = Some(id_val);
            }
            '.' => {
                // Class selector
                idx += 1;
                let class_start = idx;
                while idx < len
                    && chars[idx] != '#'
                    && chars[idx] != '.'
                    && chars[idx] != '['
                    && !chars[idx].is_whitespace()
                {
                    idx += 1;
                }
                let class_val: String = chars[class_start..idx].iter().collect();
                if class_val.is_empty() {
                    return Err(HtmlError::SelectorParseError(format!(
                        "Invalid empty class selector in '{}'",
                        chunk
                    )));
                }
                selector.classes.push(class_val);
            }
            '[' => {
                // Attribute selector
                idx += 1;
                let attr_start = idx;
                let mut in_q = None;
                while idx < len {
                    if (chars[idx] == '"' || chars[idx] == '\'') && in_q == Some(chars[idx]) {
                        in_q = None;
                    } else if (chars[idx] == '"' || chars[idx] == '\'') && in_q.is_none() {
                        in_q = Some(chars[idx]);
                    } else if chars[idx] == ']' && in_q.is_none() {
                        break;
                    }
                    idx += 1;
                }

                if idx >= len || chars[idx] != ']' {
                    return Err(HtmlError::SelectorParseError(format!(
                        "Unclosed attribute bracket in '{}'",
                        chunk
                    )));
                }

                let inner: String = chars[attr_start..idx].iter().collect();
                idx += 1; // Consume ']'

                let (attr_name, matcher) = parse_attribute_spec(&inner)?;
                selector.attributes.push((attr_name, matcher));
            }
            other => {
                return Err(HtmlError::SelectorParseError(format!(
                    "Unexpected character '{}' in selector '{}'",
                    other, chunk
                )));
            }
        }
    }

    Ok(selector)
}

/// Parses the contents inside `[...]` into attribute name and `AttributeMatcher`.
fn parse_attribute_spec(spec: &str) -> HtmlResult<(String, AttributeMatcher)> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return Err(HtmlError::SelectorParseError(
            "Empty attribute selector inside brackets".to_string(),
        ));
    }

    // Check for 2-char operators first: ^=, $=, *=, ~=, |=
    let op_2char = ["^=", "$=", "*=", "~=", "|="];
    for op in op_2char {
        if let Some(pos) = trimmed.find(op) {
            let name = trimmed[..pos].trim().to_string();
            let raw_val = trimmed[pos + 2..].trim();
            let val = unquote(raw_val);
            let matcher = match op {
                "^=" => AttributeMatcher::Prefix(val),
                "$=" => AttributeMatcher::Suffix(val),
                "*=" => AttributeMatcher::Substring(val),
                "~=" => AttributeMatcher::Includes(val),
                "|=" => AttributeMatcher::DashPrefix(val),
                _ => unreachable!(),
            };
            return Ok((name, matcher));
        }
    }

    // Check for exact `=` operator
    if let Some(pos) = trimmed.find('=') {
        let name = trimmed[..pos].trim().to_string();
        let raw_val = trimmed[pos + 1..].trim();
        let val = unquote(raw_val);
        return Ok((name, AttributeMatcher::Exact(val)));
    }

    // Simple existence check: `[attr]`
    Ok((trimmed.to_string(), AttributeMatcher::Exists))
}

/// Strips optional surrounding quotes ('...' or "...") from attribute value literals.
fn unquote(s: &str) -> String {
    let trimmed = s.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

/// High-throughput CSS3 selector dispatch and evaluation engine.
#[derive(Debug, Clone, Default)]
pub struct HtmlSelectorEngine {
    rules: Vec<(CompiledSelector, usize)>,
}

impl HtmlSelectorEngine {
    /// Creates a new empty selector engine.
    #[must_use]
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Registers a CSS3 selector string mapped to a unique rule identifier.
    pub fn register(&mut self, selector_str: &str, rule_id: usize) -> HtmlResult<&mut Self> {
        let compiled = CompiledSelector::parse(selector_str)?;
        self.rules.push((compiled, rule_id));
        Ok(self)
    }

    /// Evaluates an element against all registered selector rules, returning matching rule IDs.
    pub fn evaluate<'a, F>(&self, tag_name: &str, get_attr: F) -> Vec<usize>
    where
        F: Fn(&str) -> Option<&'a str>,
    {
        self.rules
            .iter()
            .filter_map(|(sel, id)| {
                if sel.matches(tag_name, &get_attr) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Returns true if at least one registered selector matches the element.
    pub fn has_match<'a, F>(&self, tag_name: &str, get_attr: F) -> bool
    where
        F: Fn(&str) -> Option<&'a str>,
    {
        self.rules.iter().any(|(sel, _)| sel.matches(tag_name, &get_attr))
    }

    /// Returns the total number of registered selector rules.
    #[must_use]
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Clears all registered rules.
    pub fn clear(&mut self) {
        self.rules.clear();
    }
}
