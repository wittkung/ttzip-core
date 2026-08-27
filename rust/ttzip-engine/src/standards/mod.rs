// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Format standards compliance and 16-format magic sniffing subsystem.

pub mod anchors;
pub mod checkers;
pub mod demuxer;
#[cfg(feature = "probe")]
pub mod document_stream;
pub mod extra_fields;
pub mod ffi;
#[cfg(feature = "probe")]
pub mod image_pipeline;
#[cfg(feature = "probe")]
pub mod metadata_probe;
pub mod report;
pub mod signatures;
pub mod sniffer;
pub mod subtitles;
pub mod syntax_highlight;

pub use anchors::Anchor;
pub use checkers::{check_compliance_buffer, check_compliance_file};
pub use demuxer::{
    demux_media_tracks_from_slice, demux_media_tracks_two_pass, parse_mkv_demux, parse_mp4_demux,
    MediaAttachment, MediaChapter, MediaDemuxSummary, MediaTrackInfo, MediaTrackType,
};
#[cfg(feature = "probe")]
pub use document_stream::*;
pub use extra_fields::{ParsedExtraFields, RawExtraField, RawExtraFieldsIter};
pub use ffi::*;
#[cfg(feature = "probe")]
pub use image_pipeline::*;
#[cfg(feature = "probe")]
pub use metadata_probe::*;
pub use report::{ComplianceIssue, ComplianceReport, ComplianceSeverity, ComplianceStandard, StandardCitation};
pub use signatures::{CompoundFormat, DetectedFormat, SignatureEntry, PRIORITIZED_SIGNATURES};
pub use sniffer::{detect_format_buffer, detect_format_file, SniffResult};
pub use subtitles::{
    detect_subtitle_format, find_active_subtitles_at, parse_ass_script, parse_ass_spans,
    parse_ass_timestamp, parse_html_spans, parse_srt_script, parse_srt_vtt_timestamp,
    parse_subtitle_script, parse_timed_text_script, parse_vtt_script, SubtitleAlignment,
    SubtitleColor, SubtitleDialogue, SubtitleFormat, SubtitleScript, SubtitleSpan, SubtitleStyle,
    SubtitleTimeline,
};
pub use syntax_highlight::*;
