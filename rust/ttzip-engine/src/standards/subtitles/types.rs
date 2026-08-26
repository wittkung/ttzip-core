// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Subtitle AST types and data structures.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported subtitle formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubtitleFormat { Ass, Srt, Vtt }

/// 8-bit RGBA color representation for subtitle styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubtitleColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// Alpha channel: 255 = fully opaque, 0 = fully transparent.
    pub a: u8,
}

impl SubtitleColor {
    #[inline] pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self { Self { r, g, b, a } }
    #[inline] pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self { Self { r, g, b, a: 255 } }

    /// Parses ASS hex color `&H[AA][BB][GG][RR]` (alpha 00 = opaque, FF = transparent).
    pub fn from_ass_hex(s: &str) -> Option<Self> {
        let trimmed = s.trim().trim_start_matches("&H").trim_start_matches("&h").trim_end_matches('&');
        if trimmed.is_empty() { return None; }
        let val = u32::from_str_radix(trimmed, 16).ok()?;
        if trimmed.len() <= 6 {
            Some(Self::from_rgb((val & 0xFF) as u8, ((val >> 8) & 0xFF) as u8, ((val >> 16) & 0xFF) as u8))
        } else {
            let a_ass = ((val >> 24) & 0xFF) as u8;
            Some(Self::from_rgba((val & 0xFF) as u8, ((val >> 8) & 0xFF) as u8, ((val >> 16) & 0xFF) as u8, 255u8.saturating_sub(a_ass)))
        }
    }

    /// Parses HTML / CSS hex color (`#RRGGBB`, `#RGB`, `#RRGGBBAA`).
    pub fn from_html_hex(s: &str) -> Option<Self> {
        let t = s.trim().trim_start_matches('#');
        match t.len() {
            3 => Some(Self::from_rgb(
                u8::from_str_radix(&t[0..1], 16).ok()?.saturating_mul(17),
                u8::from_str_radix(&t[1..2], 16).ok()?.saturating_mul(17),
                u8::from_str_radix(&t[2..3], 16).ok()?.saturating_mul(17),
            )),
            6 => Some(Self::from_rgb(
                u8::from_str_radix(&t[0..2], 16).ok()?,
                u8::from_str_radix(&t[2..4], 16).ok()?,
                u8::from_str_radix(&t[4..6], 16).ok()?,
            )),
            8 => Some(Self::from_rgba(
                u8::from_str_radix(&t[0..2], 16).ok()?,
                u8::from_str_radix(&t[2..4], 16).ok()?,
                u8::from_str_radix(&t[4..6], 16).ok()?,
                u8::from_str_radix(&t[6..8], 16).ok()?,
            )),
            _ => None,
        }
    }
}

/// Subtitle alignment on screen (numpad mapping 1-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubtitleAlignment {
    BottomLeft = 1, BottomCenter = 2, BottomRight = 3,
    MiddleLeft = 4, MiddleCenter = 5, MiddleRight = 6,
    TopLeft = 7, TopCenter = 8, TopRight = 9,
}

impl SubtitleAlignment {
    pub fn from_numpad(val: u32) -> Option<Self> {
        match val {
            1 => Some(Self::BottomLeft), 2 => Some(Self::BottomCenter), 3 => Some(Self::BottomRight),
            4 => Some(Self::MiddleLeft), 5 => Some(Self::MiddleCenter), 6 => Some(Self::MiddleRight),
            7 => Some(Self::TopLeft), 8 => Some(Self::TopCenter), 9 => Some(Self::TopRight),
            _ => None,
        }
    }

    pub fn from_legacy_ssa(val: u32) -> Option<Self> {
        match val {
            1 => Some(Self::BottomLeft), 2 => Some(Self::BottomCenter), 3 => Some(Self::BottomRight),
            5 => Some(Self::TopLeft), 6 => Some(Self::TopCenter), 7 => Some(Self::TopRight),
            9 => Some(Self::MiddleLeft), 10 => Some(Self::MiddleCenter), 11 => Some(Self::MiddleRight),
            _ => None,
        }
    }
}

/// An inline styled span of text inside a subtitle dialogue line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleSpan {
    pub text: String,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strikeout: Option<bool>,
    pub primary_color: Option<SubtitleColor>,
    pub secondary_color: Option<SubtitleColor>,
    pub outline_color: Option<SubtitleColor>,
    pub shadow_color: Option<SubtitleColor>,
    pub font_name: Option<String>,
    pub font_size: Option<f32>,
    pub position: Option<(f32, f32)>,
    pub alignment: Option<SubtitleAlignment>,
}

impl SubtitleSpan {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(), bold: None, italic: None, underline: None, strikeout: None,
            primary_color: None, secondary_color: None, outline_color: None, shadow_color: None,
            font_name: None, font_size: None, position: None, alignment: None,
        }
    }
}

/// V4+ Style definition for ASS subtitle scripts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleStyle {
    pub name: String,
    pub font_name: String,
    pub font_size: f32,
    pub primary_color: SubtitleColor,
    pub secondary_color: SubtitleColor,
    pub outline_color: SubtitleColor,
    pub back_color: SubtitleColor,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    pub scale_x: f32,
    pub scale_y: f32,
    pub spacing: f32,
    pub angle: f32,
    pub border_style: u32,
    pub outline: f32,
    pub shadow: f32,
    pub alignment: SubtitleAlignment,
    pub margin_l: u32,
    pub margin_r: u32,
    pub margin_v: u32,
    pub encoding: u32,
}

impl Default for SubtitleStyle {
    fn default() -> Self {
        Self {
            name: "Default".to_string(), font_name: "Arial".to_string(), font_size: 20.0,
            primary_color: SubtitleColor::from_rgba(255, 255, 255, 255),
            secondary_color: SubtitleColor::from_rgba(255, 255, 0, 255),
            outline_color: SubtitleColor::from_rgba(0, 0, 0, 255),
            back_color: SubtitleColor::from_rgba(0, 0, 0, 128),
            bold: false, italic: false, underline: false, strikeout: false,
            scale_x: 100.0, scale_y: 100.0, spacing: 0.0, angle: 0.0, border_style: 1,
            outline: 2.0, shadow: 2.0, alignment: SubtitleAlignment::BottomCenter,
            margin_l: 10, margin_r: 10, margin_v: 10, encoding: 1,
        }
    }
}

/// A parsed subtitle dialogue event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleDialogue {
    pub layer: u32,
    pub start_ms: i64,
    pub end_ms: i64,
    pub style: String,
    pub actor: String,
    pub margin_l: u32,
    pub margin_r: u32,
    pub margin_v: u32,
    pub effect: String,
    pub raw_text: String,
    pub plain_text: String,
    pub spans: Vec<SubtitleSpan>,
}

impl SubtitleDialogue {
    #[inline] pub fn start_micros(&self) -> i64 { self.start_ms.saturating_mul(1000) }
    #[inline] pub fn end_micros(&self) -> i64 { self.end_ms.saturating_mul(1000) }
    #[inline] pub fn is_active_at(&self, ms: i64) -> bool { self.start_ms <= ms && ms < self.end_ms }
    #[inline] pub fn is_active_at_micros(&self, us: i64) -> bool { self.start_micros() <= us && us < self.end_micros() }
    #[inline] pub fn duration_ms(&self) -> i64 { self.end_ms.saturating_sub(self.start_ms).max(0) }
}

/// Complete parsed subtitle script AST document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleScript {
    pub format: SubtitleFormat,
    pub title: Option<String>,
    pub script_type: Option<String>,
    pub play_res_x: Option<u32>,
    pub play_res_y: Option<u32>,
    pub wrap_style: Option<u32>,
    pub scaled_border_and_shadow: Option<bool>,
    pub styles: HashMap<String, SubtitleStyle>,
    pub dialogues: Vec<SubtitleDialogue>,
}

impl SubtitleScript {
    pub fn new(format: SubtitleFormat) -> Self {
        Self {
            format, title: None, script_type: None, play_res_x: None, play_res_y: None,
            wrap_style: None, scaled_border_and_shadow: None,
            styles: HashMap::new(), dialogues: Vec::new(),
        }
    }
}
