// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Advanced SubStation Alpha (.ass / .ssa) AST parser.

use crate::standards::subtitles::types::{
    SubtitleAlignment, SubtitleColor, SubtitleDialogue, SubtitleFormat, SubtitleScript,
    SubtitleSpan, SubtitleStyle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssSection { None, ScriptInfo, Styles, Events }

/// Parses an ASS / SSA subtitle string into a structured SubtitleScript AST.
pub fn parse_ass_script(content: &str) -> SubtitleScript {
    let mut script = SubtitleScript::new(SubtitleFormat::Ass);
    let mut current_section = AssSection::None;
    let mut style_format_cols: Vec<String> = Vec::new();
    let mut event_format_cols: Vec<String> = Vec::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with("//") { continue; }
        if line.starts_with('[') && line.ends_with(']') {
            let s = &line[1..line.len().saturating_sub(1)];
            if s.eq_ignore_ascii_case("Script Info") { current_section = AssSection::ScriptInfo; }
            else if s.eq_ignore_ascii_case("V4+ Styles") || s.eq_ignore_ascii_case("V4 Styles") || s.eq_ignore_ascii_case("Styles") {
                current_section = AssSection::Styles;
            } else if s.eq_ignore_ascii_case("Events") { current_section = AssSection::Events; }
            else { current_section = AssSection::None; }
            continue;
        }

        match current_section {
            AssSection::ScriptInfo => parse_script_info_line(line, &mut script),
            AssSection::Styles => {
                if let Some(s) = line.strip_prefix("Format:") {
                    style_format_cols = s.split(',').map(|c| c.trim().to_ascii_lowercase()).collect();
                } else if let Some(s) = line.strip_prefix("Style:") {
                    if let Some(st) = parse_style_line(&style_format_cols, s) {
                        script.styles.insert(st.name.clone(), st);
                    }
                }
            }
            AssSection::Events => {
                if let Some(s) = line.strip_prefix("Format:") {
                    event_format_cols = s.split(',').map(|c| c.trim().to_ascii_lowercase()).collect();
                } else if let Some(s) = line.strip_prefix("Dialogue:") {
                    if let Some(d) = parse_dialogue_line(&event_format_cols, s) {
                        script.dialogues.push(d);
                    }
                }
            }
            AssSection::None => {}
        }
    }
    script
}

fn parse_script_info_line(line: &str, script: &mut SubtitleScript) {
    if let Some((k, v)) = line.split_once(':') {
        let (key, val) = (k.trim(), v.trim());
        if key.eq_ignore_ascii_case("Title") { script.title = Some(val.to_string()); }
        else if key.eq_ignore_ascii_case("ScriptType") { script.script_type = Some(val.to_string()); }
        else if key.eq_ignore_ascii_case("PlayResX") { script.play_res_x = val.parse::<u32>().ok(); }
        else if key.eq_ignore_ascii_case("PlayResY") { script.play_res_y = val.parse::<u32>().ok(); }
        else if key.eq_ignore_ascii_case("WrapStyle") { script.wrap_style = val.parse::<u32>().ok(); }
        else if key.eq_ignore_ascii_case("ScaledBorderAndShadow") {
            script.scaled_border_and_shadow = Some(val.eq_ignore_ascii_case("yes"));
        }
    }
}

pub fn parse_ass_timestamp(s: &str) -> Option<i64> {
    let mut parts = s.trim().split(':');
    let h = parts.next()?.parse::<i64>().ok()?;
    let m = parts.next()?.parse::<i64>().ok()?;
    let s_part = parts.next()?;
    let (sec_str, frac_str) = s_part.split_once('.').or_else(|| s_part.split_once(',')).unwrap_or((s_part, "0"));
    let sec = sec_str.parse::<i64>().ok()?;
    let ms = match frac_str.len() {
        0 => 0,
        1 => frac_str.parse::<i64>().ok()?.saturating_mul(100),
        2 => frac_str.parse::<i64>().ok()?.saturating_mul(10),
        3 => frac_str.parse::<i64>().ok()?,
        _ => frac_str.get(..3)?.parse::<i64>().ok()?,
    };
    Some(h.saturating_mul(3_600_000).saturating_add(m.saturating_mul(60_000)).saturating_add(sec.saturating_mul(1_000)).saturating_add(ms))
}

fn parse_style_line(cols: &[String], payload: &str) -> Option<SubtitleStyle> {
    let fields: Vec<&str> = payload.split(',').map(|s| s.trim()).collect();
    if fields.is_empty() { return None; }
    let mut st = SubtitleStyle::default();
    for (i, col) in cols.iter().enumerate() {
        let val = match fields.get(i) { Some(v) => *v, None => break };
        match col.as_str() {
            "name" => st.name = val.to_string(),
            "fontname" => st.font_name = val.to_string(),
            "fontsize" => if let Ok(v) = val.parse::<f32>() { st.font_size = v; },
            "primarycolour" => if let Some(c) = SubtitleColor::from_ass_hex(val) { st.primary_color = c; },
            "secondarycolour" => if let Some(c) = SubtitleColor::from_ass_hex(val) { st.secondary_color = c; },
            "outlinecolour" | "bordercolour" => if let Some(c) = SubtitleColor::from_ass_hex(val) { st.outline_color = c; },
            "backcolour" => if let Some(c) = SubtitleColor::from_ass_hex(val) { st.back_color = c; },
            "bold" => st.bold = val == "1" || val == "-1",
            "italic" => st.italic = val == "1" || val == "-1",
            "underline" => st.underline = val == "1" || val == "-1",
            "strikeout" => st.strikeout = val == "1" || val == "-1",
            "scalex" => if let Ok(v) = val.parse::<f32>() { st.scale_x = v; },
            "scaley" => if let Ok(v) = val.parse::<f32>() { st.scale_y = v; },
            "spacing" => if let Ok(v) = val.parse::<f32>() { st.spacing = v; },
            "angle" => if let Ok(v) = val.parse::<f32>() { st.angle = v; },
            "borderstyle" => if let Ok(v) = val.parse::<u32>() { st.border_style = v; },
            "outline" => if let Ok(v) = val.parse::<f32>() { st.outline = v; },
            "shadow" => if let Ok(v) = val.parse::<f32>() { st.shadow = v; },
            "alignment" => if let Ok(v) = val.parse::<u32>() {
                if let Some(a) = SubtitleAlignment::from_numpad(v).or_else(|| SubtitleAlignment::from_legacy_ssa(v)) {
                    st.alignment = a;
                }
            },
            "marginl" => if let Ok(v) = val.parse::<u32>() { st.margin_l = v; },
            "marginr" => if let Ok(v) = val.parse::<u32>() { st.margin_r = v; },
            "marginv" => if let Ok(v) = val.parse::<u32>() { st.margin_v = v; },
            "encoding" => if let Ok(v) = val.parse::<u32>() { st.encoding = v; },
            _ => {}
        }
    }
    Some(st)
}

fn parse_dialogue_line(cols: &[String], payload: &str) -> Option<SubtitleDialogue> {
    let mut layer = 0u32;
    let mut start_ms = 0i64;
    let mut end_ms = 0i64;
    let mut style = "Default".to_string();
    let mut actor = String::new();
    let mut margin_l = 0u32;
    let mut margin_r = 0u32;
    let mut margin_v = 0u32;
    let mut effect = String::new();
    let mut raw_text = String::new();

    let num_cols = if cols.is_empty() { 10 } else { cols.len() };
    let mut parts = payload.splitn(num_cols, ',');
    for col in cols.iter() {
        let val = parts.next()?.trim();
        match col.as_str() {
            "layer" => layer = val.parse::<u32>().unwrap_or(0),
            "start" => start_ms = parse_ass_timestamp(val).unwrap_or(0),
            "end" => end_ms = parse_ass_timestamp(val).unwrap_or(0),
            "style" => style = val.to_string(),
            "name" | "actor" => actor = val.to_string(),
            "marginl" => margin_l = val.parse::<u32>().unwrap_or(0),
            "marginr" => margin_r = val.parse::<u32>().unwrap_or(0),
            "marginv" => margin_v = val.parse::<u32>().unwrap_or(0),
            "effect" => effect = val.to_string(),
            "text" => raw_text = val.to_string(),
            _ => {}
        }
    }
    if raw_text.is_empty() && cols.is_empty() { raw_text = payload.to_string(); }
    let (spans, plain_text) = parse_ass_spans(&raw_text);
    Some(SubtitleDialogue {
        layer, start_ms, end_ms, style, actor, margin_l, margin_r, margin_v, effect,
        raw_text, plain_text, spans,
    })
}

/// Parses inline ASS override tags and constructs a SubtitleSpan sequence and plain text.
pub fn parse_ass_spans(raw_text: &str) -> (Vec<SubtitleSpan>, String) {
    let mut spans = Vec::new();
    let mut plain_text = String::new();
    let mut cur = SubtitleSpan::plain("");
    let mut chars = raw_text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            let mut tag_content = String::new();
            for inner in chars.by_ref() {
                if inner == '}' { break; }
                tag_content.push(inner);
            }
            apply_ass_override_tags(&tag_content, &mut cur);
        } else if ch == '\\' {
            if let Some(&next_ch) = chars.peek() {
                if next_ch == 'N' || next_ch == 'n' {
                    chars.next();
                    plain_text.push('\n');
                    let mut s = cur.clone(); s.text = "\n".to_string(); spans.push(s);
                } else if next_ch == 'h' {
                    chars.next();
                    plain_text.push(' ');
                    let mut s = cur.clone(); s.text = " ".to_string(); spans.push(s);
                } else {
                    let mut buf = String::from('\\');
                    if let Some(c) = chars.next() { buf.push(c); }
                    plain_text.push_str(&buf);
                    let mut s = cur.clone(); s.text = buf; spans.push(s);
                }
            }
        } else {
            let mut buf = String::from(ch);
            while let Some(&next_ch) = chars.peek() {
                if next_ch == '{' || next_ch == '\\' { break; }
                if let Some(c) = chars.next() { buf.push(c); }
            }
            plain_text.push_str(&buf);
            let mut s = cur.clone(); s.text = buf; spans.push(s);
        }
    }
    (spans, plain_text)
}

fn apply_ass_override_tags(tags: &str, cur: &mut SubtitleSpan) {
    for tag in tags.split('\\') {
        let t = tag.trim();
        if t.is_empty() { continue; }
        if let Some(v) = t.strip_prefix("pos(") {
            if let Some(pos_body) = v.strip_suffix(')') {
                if let Some((x_str, y_str)) = pos_body.split_once(',') {
                    if let (Ok(x), Ok(y)) = (x_str.trim().parse::<f32>(), y_str.trim().parse::<f32>()) {
                        cur.position = Some((x, y));
                    }
                }
            }
        } else if let Some(v) = t.strip_prefix("an") {
            if let Ok(num) = v.parse::<u32>() { cur.alignment = SubtitleAlignment::from_numpad(num); }
        } else if let Some(v) = t.strip_prefix('a') {
            if let Ok(num) = v.parse::<u32>() { cur.alignment = SubtitleAlignment::from_legacy_ssa(num); }
        } else if t == "b1" { cur.bold = Some(true); }
        else if t == "b0" { cur.bold = Some(false); }
        else if t == "i1" { cur.italic = Some(true); }
        else if t == "i0" { cur.italic = Some(false); }
        else if t == "u1" { cur.underline = Some(true); }
        else if t == "u0" { cur.underline = Some(false); }
        else if t == "s1" { cur.strikeout = Some(true); }
        else if t == "s0" { cur.strikeout = Some(false); }
        else if let Some(v) = t.strip_prefix("1c").or_else(|| t.strip_prefix('c')) {
            cur.primary_color = SubtitleColor::from_ass_hex(v);
        } else if let Some(v) = t.strip_prefix("2c") {
            cur.secondary_color = SubtitleColor::from_ass_hex(v);
        } else if let Some(v) = t.strip_prefix("3c") {
            cur.outline_color = SubtitleColor::from_ass_hex(v);
        } else if let Some(v) = t.strip_prefix("4c") {
            cur.shadow_color = SubtitleColor::from_ass_hex(v);
        } else if let Some(v) = t.strip_prefix("fn") {
            cur.font_name = Some(v.to_string());
        } else if let Some(v) = t.strip_prefix("fs") {
            cur.font_size = v.parse::<f32>().ok();
        } else if t.starts_with('r') {
            *cur = SubtitleSpan::plain("");
        }
    }
}
