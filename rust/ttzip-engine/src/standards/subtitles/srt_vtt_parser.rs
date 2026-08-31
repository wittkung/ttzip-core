// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! SubRip (.srt) and WebVTT (.vtt) subtitle AST parser.

use crate::standards::subtitles::types::{
    SubtitleColor, SubtitleDialogue, SubtitleFormat, SubtitleScript, SubtitleSpan,
};

/// Parses an SRT subtitle string into a structured SubtitleScript AST.
pub fn parse_srt_script(content: &str) -> SubtitleScript {
    parse_timed_text_script(content, SubtitleFormat::Srt)
}

/// Parses a WebVTT subtitle string into a structured SubtitleScript AST.
pub fn parse_vtt_script(content: &str) -> SubtitleScript {
    parse_timed_text_script(content, SubtitleFormat::Vtt)
}

/// Common block parser for SRT and WebVTT text streams.
pub fn parse_timed_text_script(content: &str, format: SubtitleFormat) -> SubtitleScript {
    let mut script = SubtitleScript::new(format);
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    let blocks = normalized.split("\n\n");

    for block in blocks {
        let trimmed_block = block.trim();
        if trimmed_block.is_empty() { continue; }
        if format == SubtitleFormat::Vtt && (trimmed_block.starts_with("WEBVTT") || trimmed_block.starts_with("NOTE")) {
            continue;
        }

        let lines: Vec<&str> = trimmed_block.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() { continue; }

        let (time_line_idx, (start_ms, end_ms)) = match find_time_line(&lines) {
            Some(res) => res,
            None => continue,
        };

        let text_lines = &lines[time_line_idx + 1..];
        if text_lines.is_empty() { continue; }
        let raw_text = text_lines.join("\n");
        let (spans, plain_text, actor) = parse_html_spans(&raw_text);

        script.dialogues.push(SubtitleDialogue {
            layer: 0,
            start_ms,
            end_ms,
            style: "Default".to_string(),
            actor,
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: String::new(),
            raw_text,
            plain_text,
            spans,
        });
    }

    script
}

fn find_time_line(lines: &[&str]) -> Option<(usize, (i64, i64))> {
    for (i, &line) in lines.iter().enumerate() {
        if let Some((s_str, e_str)) = line.split_once("-->") {
            let start = parse_srt_vtt_timestamp(s_str.trim())?;
            let end_token = e_str.split_whitespace().next().unwrap_or(e_str.trim());
            let end = parse_srt_vtt_timestamp(end_token)?;
            return Some((i, (start, end)));
        }
    }
    None
}

/// Parses SRT (`HH:MM:SS,mmm`) or WebVTT (`HH:MM:SS.mmm` / `MM:SS.mmm`) timestamp into milliseconds.
pub fn parse_srt_vtt_timestamp(s: &str) -> Option<i64> {
    let t = s.trim();
    let parts: Vec<&str> = t.split(':').collect();
    match parts.len() {
        3 => {
            let h = parts.first()?.parse::<i64>().ok()?;
            let m = parts.get(1)?.parse::<i64>().ok()?;
            let (sec, ms) = parse_sec_millis(parts.get(2)?)?;
            Some(h.saturating_mul(3_600_000).saturating_add(m.saturating_mul(60_000)).saturating_add(sec.saturating_mul(1_000)).saturating_add(ms))
        }
        2 => {
            let m = parts.first()?.parse::<i64>().ok()?;
            let (sec, ms) = parse_sec_millis(parts.get(1)?)?;
            Some(m.saturating_mul(60_000).saturating_add(sec.saturating_mul(1_000)).saturating_add(ms))
        }
        _ => None,
    }
}

fn parse_sec_millis(s: &str) -> Option<(i64, i64)> {
    let (sec_str, frac_str) = s.split_once(',').or_else(|| s.split_once('.')).unwrap_or((s, "0"));
    let sec = sec_str.parse::<i64>().ok()?;
    let ms = match frac_str.len() {
        0 => 0,
        1 => frac_str.parse::<i64>().ok()?.saturating_mul(100),
        2 => frac_str.parse::<i64>().ok()?.saturating_mul(10),
        3 => frac_str.parse::<i64>().ok()?,
        _ => frac_str.get(..3)?.parse::<i64>().ok()?,
    };
    Some((sec, ms))
}

/// Extracts styled spans, plain text, and speaker from HTML / WebVTT cue tags.
pub fn parse_html_spans(raw_text: &str) -> (Vec<SubtitleSpan>, String, String) {
    let mut spans = Vec::new();
    let mut plain_text = String::new();
    let mut actor = String::new();
    let mut bold = false;
    let mut italic = false;
    let mut underline = false;
    let mut strikeout = false;
    let mut color_stack: Vec<SubtitleColor> = Vec::new();

    let mut chars = raw_text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '<' {
            let mut tag_buf = String::new();
            for inner in chars.by_ref() {
                if inner == '>' { break; }
                tag_buf.push(inner);
            }
            let tag = tag_buf.trim();
            if tag.eq_ignore_ascii_case("b") { bold = true; }
            else if tag.eq_ignore_ascii_case("/b") { bold = false; }
            else if tag.eq_ignore_ascii_case("i") { italic = true; }
            else if tag.eq_ignore_ascii_case("/i") { italic = false; }
            else if tag.eq_ignore_ascii_case("u") { underline = true; }
            else if tag.eq_ignore_ascii_case("/u") { underline = false; }
            else if tag.eq_ignore_ascii_case("s") || tag.eq_ignore_ascii_case("strike") { strikeout = true; }
            else if tag.eq_ignore_ascii_case("/s") || tag.eq_ignore_ascii_case("/strike") { strikeout = false; }
            else if tag.to_ascii_lowercase().starts_with("font") {
                if let Some(c) = extract_font_color(tag) { color_stack.push(c); }
            } else if tag.eq_ignore_ascii_case("/font") {
                color_stack.pop();
            } else if tag.starts_with("v ") || tag.starts_with("v.") {
                actor = tag.get(2..).unwrap_or("").trim().to_string();
            }
        } else {
            let mut text_buf = String::from(ch);
            while let Some(&next_ch) = chars.peek() {
                if next_ch == '<' { break; }
                if let Some(c) = chars.next() { text_buf.push(c); }
            }
            plain_text.push_str(&text_buf);
            spans.push(SubtitleSpan {
                text: text_buf,
                bold: if bold { Some(true) } else { None },
                italic: if italic { Some(true) } else { None },
                underline: if underline { Some(true) } else { None },
                strikeout: if strikeout { Some(true) } else { None },
                primary_color: color_stack.last().copied(),
                secondary_color: None,
                outline_color: None,
                shadow_color: None,
                font_name: None,
                font_size: None,
                position: None,
                alignment: None,
            });
        }
    }

    (spans, plain_text, actor)
}

fn extract_font_color(tag: &str) -> Option<SubtitleColor> {
    let lower = tag.to_ascii_lowercase();
    let idx = lower.find("color=")?;
    let rem = tag.get(idx + 6..)?.trim_start();
    let quote = rem.chars().next()?;
    let color_str = if quote == '"' || quote == '\'' {
        rem.get(1..)?.split(quote).next()?
    } else {
        rem.split_whitespace().next()?
    };
    SubtitleColor::from_html_hex(color_str)
}
