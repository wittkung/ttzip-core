// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Subtitle parsing, AST manipulation, and microsecond-level timeline search subsystem.

#![forbid(unsafe_code)]

pub mod ass_parser;
pub mod srt_vtt_parser;
pub mod timeline;
pub mod types;

#[cfg(test)]
mod tests;

pub use ass_parser::{parse_ass_script, parse_ass_spans, parse_ass_timestamp};
pub use srt_vtt_parser::{
    parse_html_spans, parse_srt_script, parse_srt_vtt_timestamp, parse_timed_text_script,
    parse_vtt_script,
};
pub use timeline::SubtitleTimeline;
pub use types::{
    SubtitleAlignment, SubtitleColor, SubtitleDialogue, SubtitleFormat, SubtitleScript,
    SubtitleSpan, SubtitleStyle,
};

/// Detects the most likely subtitle format from the content header or structure.
pub fn detect_subtitle_format(content: &str) -> SubtitleFormat {
    let trimmed = content.trim_start();
    if trimmed.starts_with("[Script Info]") || trimmed.contains("[Events]") || trimmed.contains("[V4+ Styles]") {
        SubtitleFormat::Ass
    } else if trimmed.starts_with("WEBVTT") {
        SubtitleFormat::Vtt
    } else {
        SubtitleFormat::Srt
    }
}

/// Universal entrypoint to parse subtitle text into a structured AST.
pub fn parse_subtitle_script(content: &str, hint: Option<SubtitleFormat>) -> SubtitleScript {
    let format = hint.unwrap_or_else(|| detect_subtitle_format(content));
    match format {
        SubtitleFormat::Ass => parse_ass_script(content),
        SubtitleFormat::Srt => parse_srt_script(content),
        SubtitleFormat::Vtt => parse_vtt_script(content),
    }
}

/// Convenience function to retrieve active dialogue lines at a specific millisecond timestamp.
pub fn find_active_subtitles_at(script: &SubtitleScript, timestamp_ms: i64) -> Vec<SubtitleDialogue> {
    let timeline = SubtitleTimeline::from_script(script);
    timeline.find_active_dialogues(timestamp_ms)
}
