// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Subtitle AST and Timeline Scaffolding.
//!
//! Exposes strongly-typed SubtitleScript AST models, styling tokens,
//! and timeline search capabilities to Swift, Kotlin, and Python.

use std::collections::HashMap;

use super::types::TTZipError;
use crate::standards::subtitles::{
    SubtitleAlignment, SubtitleColor, SubtitleDialogue, SubtitleFormat, SubtitleScript,
    SubtitleSpan, SubtitleStyle,
};

/// Supported subtitle formats exposed to Swift.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum UniFFISubtitleFormat {
    Ass,
    Srt,
    Vtt,
}

impl From<SubtitleFormat> for UniFFISubtitleFormat {
    fn from(f: SubtitleFormat) -> Self { match f { SubtitleFormat::Ass => Self::Ass, SubtitleFormat::Srt => Self::Srt, SubtitleFormat::Vtt => Self::Vtt } }
}
impl From<UniFFISubtitleFormat> for SubtitleFormat {
    fn from(f: UniFFISubtitleFormat) -> Self { match f { UniFFISubtitleFormat::Ass => Self::Ass, UniFFISubtitleFormat::Srt => Self::Srt, UniFFISubtitleFormat::Vtt => Self::Vtt } }
}

/// 8-bit RGBA color representation for subtitle styling across FFI boundary.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Record)]
pub struct UniFFISubtitleColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl From<SubtitleColor> for UniFFISubtitleColor {
    fn from(c: SubtitleColor) -> Self { Self { r: c.r, g: c.g, b: c.b, a: c.a } }
}
impl From<UniFFISubtitleColor> for SubtitleColor {
    fn from(c: UniFFISubtitleColor) -> Self { Self { r: c.r, g: c.g, b: c.b, a: c.a } }
}

/// Subtitle alignment on screen (numpad mapping 1-9).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum UniFFISubtitleAlignment {
    BottomLeft, BottomCenter, BottomRight,
    MiddleLeft, MiddleCenter, MiddleRight,
    TopLeft, TopCenter, TopRight,
}

impl From<SubtitleAlignment> for UniFFISubtitleAlignment {
    fn from(a: SubtitleAlignment) -> Self { match a { SubtitleAlignment::BottomLeft => Self::BottomLeft, SubtitleAlignment::BottomCenter => Self::BottomCenter, SubtitleAlignment::BottomRight => Self::BottomRight, SubtitleAlignment::MiddleLeft => Self::MiddleLeft, SubtitleAlignment::MiddleCenter => Self::MiddleCenter, SubtitleAlignment::MiddleRight => Self::MiddleRight, SubtitleAlignment::TopLeft => Self::TopLeft, SubtitleAlignment::TopCenter => Self::TopCenter, SubtitleAlignment::TopRight => Self::TopRight } }
}
impl From<UniFFISubtitleAlignment> for SubtitleAlignment {
    fn from(a: UniFFISubtitleAlignment) -> Self { match a { UniFFISubtitleAlignment::BottomLeft => Self::BottomLeft, UniFFISubtitleAlignment::BottomCenter => Self::BottomCenter, UniFFISubtitleAlignment::BottomRight => Self::BottomRight, UniFFISubtitleAlignment::MiddleLeft => Self::MiddleLeft, UniFFISubtitleAlignment::MiddleCenter => Self::MiddleCenter, UniFFISubtitleAlignment::MiddleRight => Self::MiddleRight, UniFFISubtitleAlignment::TopLeft => Self::TopLeft, UniFFISubtitleAlignment::TopCenter => Self::TopCenter, UniFFISubtitleAlignment::TopRight => Self::TopRight } }
}

/// 2D Cartesian coordinates for explicit subtitle positioning.
#[derive(Copy, Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFISubtitlePosition {
    pub x: f32,
    pub y: f32,
}

/// An inline styled span of text inside a subtitle dialogue line.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFISubtitleSpan {
    pub text: String,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strikeout: Option<bool>,
    pub primary_color: Option<UniFFISubtitleColor>,
    pub secondary_color: Option<UniFFISubtitleColor>,
    pub outline_color: Option<UniFFISubtitleColor>,
    pub shadow_color: Option<UniFFISubtitleColor>,
    pub font_name: Option<String>,
    pub font_size: Option<f32>,
    pub position: Option<UniFFISubtitlePosition>,
    pub alignment: Option<UniFFISubtitleAlignment>,
}

impl From<SubtitleSpan> for UniFFISubtitleSpan {
    fn from(s: SubtitleSpan) -> Self {
        Self { text: s.text, bold: s.bold, italic: s.italic, underline: s.underline, strikeout: s.strikeout, primary_color: s.primary_color.map(Into::into), secondary_color: s.secondary_color.map(Into::into), outline_color: s.outline_color.map(Into::into), shadow_color: s.shadow_color.map(Into::into), font_name: s.font_name, font_size: s.font_size, position: s.position.map(|(x, y)| UniFFISubtitlePosition { x, y }), alignment: s.alignment.map(Into::into) }
    }
}
impl From<UniFFISubtitleSpan> for SubtitleSpan {
    fn from(s: UniFFISubtitleSpan) -> Self {
        Self { text: s.text, bold: s.bold, italic: s.italic, underline: s.underline, strikeout: s.strikeout, primary_color: s.primary_color.map(Into::into), secondary_color: s.secondary_color.map(Into::into), outline_color: s.outline_color.map(Into::into), shadow_color: s.shadow_color.map(Into::into), font_name: s.font_name, font_size: s.font_size, position: s.position.map(|p| (p.x, p.y)), alignment: s.alignment.map(Into::into) }
    }
}

/// V4+ Style definition for ASS subtitle scripts.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFISubtitleStyle {
    pub name: String,
    pub font_name: String,
    pub font_size: f32,
    pub primary_color: UniFFISubtitleColor,
    pub secondary_color: UniFFISubtitleColor,
    pub outline_color: UniFFISubtitleColor,
    pub back_color: UniFFISubtitleColor,
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
    pub alignment: UniFFISubtitleAlignment,
    pub margin_l: u32,
    pub margin_r: u32,
    pub margin_v: u32,
    pub encoding: u32,
}

impl From<SubtitleStyle> for UniFFISubtitleStyle {
    fn from(s: SubtitleStyle) -> Self {
        Self { name: s.name, font_name: s.font_name, font_size: s.font_size, primary_color: s.primary_color.into(), secondary_color: s.secondary_color.into(), outline_color: s.outline_color.into(), back_color: s.back_color.into(), bold: s.bold, italic: s.italic, underline: s.underline, strikeout: s.strikeout, scale_x: s.scale_x, scale_y: s.scale_y, spacing: s.spacing, angle: s.angle, border_style: s.border_style, outline: s.outline, shadow: s.shadow, alignment: s.alignment.into(), margin_l: s.margin_l, margin_r: s.margin_r, margin_v: s.margin_v, encoding: s.encoding }
    }
}
impl From<UniFFISubtitleStyle> for SubtitleStyle {
    fn from(s: UniFFISubtitleStyle) -> Self {
        Self { name: s.name, font_name: s.font_name, font_size: s.font_size, primary_color: s.primary_color.into(), secondary_color: s.secondary_color.into(), outline_color: s.outline_color.into(), back_color: s.back_color.into(), bold: s.bold, italic: s.italic, underline: s.underline, strikeout: s.strikeout, scale_x: s.scale_x, scale_y: s.scale_y, spacing: s.spacing, angle: s.angle, border_style: s.border_style, outline: s.outline, shadow: s.shadow, alignment: s.alignment.into(), margin_l: s.margin_l, margin_r: s.margin_r, margin_v: s.margin_v, encoding: s.encoding }
    }
}

/// A parsed subtitle dialogue event.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFISubtitleDialogue {
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
    pub spans: Vec<UniFFISubtitleSpan>,
}

impl From<SubtitleDialogue> for UniFFISubtitleDialogue {
    fn from(d: SubtitleDialogue) -> Self {
        Self { layer: d.layer, start_ms: d.start_ms, end_ms: d.end_ms, style: d.style, actor: d.actor, margin_l: d.margin_l, margin_r: d.margin_r, margin_v: d.margin_v, effect: d.effect, raw_text: d.raw_text, plain_text: d.plain_text, spans: d.spans.into_iter().map(Into::into).collect() }
    }
}
impl From<UniFFISubtitleDialogue> for SubtitleDialogue {
    fn from(d: UniFFISubtitleDialogue) -> Self {
        Self { layer: d.layer, start_ms: d.start_ms, end_ms: d.end_ms, style: d.style, actor: d.actor, margin_l: d.margin_l, margin_r: d.margin_r, margin_v: d.margin_v, effect: d.effect, raw_text: d.raw_text, plain_text: d.plain_text, spans: d.spans.into_iter().map(Into::into).collect() }
    }
}

/// Complete parsed subtitle script AST document exposed to Swift.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFISubtitleScript {
    pub format: UniFFISubtitleFormat,
    pub title: Option<String>,
    pub script_type: Option<String>,
    pub play_res_x: Option<u32>,
    pub play_res_y: Option<u32>,
    pub wrap_style: Option<u32>,
    pub scaled_border_and_shadow: Option<bool>,
    pub styles: HashMap<String, UniFFISubtitleStyle>,
    pub dialogues: Vec<UniFFISubtitleDialogue>,
}

impl From<SubtitleScript> for UniFFISubtitleScript {
    fn from(s: SubtitleScript) -> Self {
        Self { format: s.format.into(), title: s.title, script_type: s.script_type, play_res_x: s.play_res_x, play_res_y: s.play_res_y, wrap_style: s.wrap_style, scaled_border_and_shadow: s.scaled_border_and_shadow, styles: s.styles.into_iter().map(|(k, v)| (k, v.into())).collect(), dialogues: s.dialogues.into_iter().map(Into::into).collect() }
    }
}
impl From<UniFFISubtitleScript> for SubtitleScript {
    fn from(s: UniFFISubtitleScript) -> Self {
        Self { format: s.format.into(), title: s.title, script_type: s.script_type, play_res_x: s.play_res_x, play_res_y: s.play_res_y, wrap_style: s.wrap_style, scaled_border_and_shadow: s.scaled_border_and_shadow, styles: s.styles.into_iter().map(|(k, v)| (k, v.into())).collect(), dialogues: s.dialogues.into_iter().map(Into::into).collect() }
    }
}

/// Parses subtitle content with an optional format hint into a strongly-typed AST script.
#[uniffi::export]
pub fn parse_subtitle_script(content: String, format_name: String) -> Result<UniFFISubtitleScript, TTZipError> {
    let hint = match format_name.trim().to_lowercase().as_str() {
        "ass" | "ssa" => Some(SubtitleFormat::Ass),
        "srt" => Some(SubtitleFormat::Srt),
        "vtt" | "webvtt" => Some(SubtitleFormat::Vtt),
        "" | "auto" | "detect" => None,
        other => return Err(TTZipError::IoError { message: format!("Unsupported subtitle format hint: {other}") }),
    };
    let script = crate::standards::subtitles::parse_subtitle_script(&content, hint);
    Ok(script.into())
}

/// Retrieves all active subtitle dialogues at a specific millisecond timestamp using binary search.
#[uniffi::export]
pub fn find_active_subtitles_at(script: UniFFISubtitleScript, timestamp_ms: u64) -> Vec<UniFFISubtitleDialogue> {
    let rust_script: SubtitleScript = script.into();
    crate::standards::subtitles::find_active_subtitles_at(&rust_script, timestamp_ms as i64)
        .into_iter()
        .map(Into::into)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniffi_parse_subtitle_script_srt() {
        let srt = "1\n00:00:01,000 --> 00:00:04,000\nHello <i>World</i>\n";
        let script = parse_subtitle_script(srt.to_string(), "srt".to_string()).expect("parse srt failed");
        assert_eq!(script.format, UniFFISubtitleFormat::Srt);
        assert_eq!(script.dialogues.len(), 1);
        assert_eq!(script.dialogues[0].start_ms, 1000);
        assert_eq!(script.dialogues[0].end_ms, 4000);

        let active = find_active_subtitles_at(script, 2000);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].plain_text, "Hello World");
    }

    #[test]
    fn test_uniffi_parse_subtitle_script_invalid_hint() {
        let res = parse_subtitle_script("foo".to_string(), "unknown_format".to_string());
        assert!(res.is_err());
    }
}
