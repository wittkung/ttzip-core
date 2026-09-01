// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Modern Character Set Detection, Zero-Allocation Transcoding,
//! Mojibake Remediation, and Unicode Normalization Subsystem.
//!
//! # Architecture Components
//! - [`detector`]: Tri-gram language model probabilistic encoding detection via `chardetng` with SIMD/fast paths.
//! - [`transcoder`]: High-throughput zero-allocation transcoding between 40+ WHATWG encodings and UTF-8 via `encoding_rs`.
//! - [`remediator`]: Heuristic mojibake auto-detection and reverse-transcoding state machine.
//! - [`normalizer`]: macOS Unicode NFC canonical normalization and defensive Zip-Slip path sanitization.

pub mod detector;
pub mod normalizer;
pub mod remediator;
pub mod transcoder;

pub use detector::{
    detect_encoding, detect_encoding_with_confidence, CandidateEncoding, ConfidenceLevel,
    DetectionResult, TTZipEncodingDetector,
};
pub use normalizer::{
    clean_control_characters, normalize_and_sanitize_path, normalize_nfc, normalize_nfd,
    normalize_nfkc, normalize_nfkd, sanitize_path as sanitize_text_path, PathSanitizeError,
    UnicodeNormalizer,
};
pub use remediator::{
    remediate_filename_bytes, remediate_text, GarbledTextRemediator, RemediationConfidence,
    RemediationResult,
};
pub use transcoder::{
    decode_to_utf8, decode_to_utf8_lossless, decode_to_utf8_lossy, encode_from_utf8,
    encode_from_utf8_lossless, lookup_encoding_by_name, TTZipTextTranscoder, TextTranscodeError,
    TranscodeOptions, TranscodeStats, DEFAULT_TRANSCODE_BUFFER_SIZE,
};
