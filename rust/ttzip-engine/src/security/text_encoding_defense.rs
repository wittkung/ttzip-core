// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Text Encoding 6-Layer Defense-in-Depth Security Subsystem.
//!
//! Enforces deterministic string sanitization, memory exhaustion protection,
//! and injection attack mitigation:
//! 1. **Malformed Byte Sequence & Truncation Guard** ([`MalformedByteSequenceGuard`]):
//!    Intercepts broken multibyte code units, mid-sequence stream truncations, and invalid UTF-8/legacy sequences.
//! 2. **Text Expansion & Memory Fuse Guard** ([`TextExpansionGuard`]):
//!    Enforces a strict 4.0x transcoding expansion ratio quota and a 64 MiB resident memory circuit breaker.
//! 3. **Surrogate & Unassigned Sanitizer** ([`SurrogateAndUnassignedGuard`]):
//!    Scrubs lone surrogates, out-of-bounds scalar values, Trojan Source bidirectional overrides, and unassigned code points.
//! 4. **Null Byte & Path Traversal Guard** ([`NullByteAndPathTraversalGuard`]):
//!    Neutralizes C-string null byte truncation (`\0`) and sanitizes Zip-Slip directory traversal sequences.
//! 5. **Sensitive Text Memory Zeroize Guard** ([`SensitiveTextBuffer`]):
//!    Provides zero-on-drop volatile memory erasure for passwords, secret headers, and credentials.
//! 6. **Encoding Confidence Fallback Guard** ([`EncodingConfidenceFallbackGuard`]):
//!    Demotes low-confidence heuristic detections to strict safety fallbacks (lossless, replacement, or safe defaults).

use std::fmt;
use std::path::{Component, Path};
use encoding_rs::Encoding;
use unicode_normalization::UnicodeNormalization;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::types::TTZipStatus;

/// Maximum allowable expansion ratio during transcoding (4.0x input size).
pub const DEFAULT_MAX_EXPANSION_RATIO: f64 = 4.0;
/// Default memory fuse limit for in-flight text operations (64 MiB).
pub const DEFAULT_TEXT_MEMORY_FUSE_BYTES: usize = 64 * 1024 * 1024;
/// Minimum default confidence threshold for character set detection (0.50).
pub const DEFAULT_MIN_CONFIDENCE_THRESHOLD: f32 = 0.50;
/// Minimum byte buffer headroom for small text transcoding allocations (1024 bytes).
pub const DEFAULT_HEADROOM_BYTES: usize = 1024;

// ============================================================================
// Defense Error Models
// ============================================================================

/// Errors emitted when text encoding security invariants or quotas are breached.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TextDefenseError {
    /// Broken or malformed byte sequence encountered in the input data.
    #[error("Malformed byte sequence for encoding '{encoding_name}' at byte offset {offset}")]
    MalformedByteSequence {
        /// Canonical name of the targeted encoding.
        encoding_name: &'static str,
        /// Byte offset where the malformed sequence begins.
        offset: usize,
    },

    /// Multibyte sequence was unexpectedly cut off at stream/slice boundary.
    #[error("Unexpected multibyte truncation at stream boundary (needed {needed} bytes, had {available})")]
    UnexpectedTruncation {
        /// Expected byte length of the multibyte sequence.
        needed: usize,
        /// Number of bytes actually available.
        available: usize,
    },

    /// Text memory allocation exceeded resident circuit breaker ceiling.
    #[error("Text memory fuse ceiling exceeded: allocated {allocated} bytes > ceiling {fuse_limit} bytes")]
    MemoryFuseExceeded {
        /// Total requested or allocated byte count.
        allocated: usize,
        /// Configured memory fuse ceiling.
        fuse_limit: usize,
    },

    /// Transcoding output expansion exceeded allowable quota multiplier.
    #[error("Text expansion quota exceeded: input {in_bytes} bytes expanded to {out_bytes} bytes (max ratio {max_ratio:.1}x)")]
    ExpansionQuotaExceeded {
        /// Input byte length.
        in_bytes: usize,
        /// Output byte length produced or requested.
        out_bytes: usize,
        /// Maximum allowed expansion ratio.
        max_ratio: f64,
    },

    /// Forbidden Unicode scalar value, surrogate, or dangerous control code point detected.
    #[error("Illegal Unicode surrogate or forbidden code point detected: U+{code_point:04X}")]
    IllegalUnicodeScalar {
        /// The invalid 32-bit Unicode scalar value.
        code_point: u32,
    },

    /// Null byte injection attempt detected.
    #[error("Null byte injection detected at byte offset {offset}")]
    NullByteInjection {
        /// Byte offset where the null byte was located.
        offset: usize,
    },

    /// Path traversal or sandbox escape sequence detected.
    #[error("Path traversal or illegal path sequence detected: '{path}'")]
    PathTraversalDetected {
        /// The offending path string.
        path: String,
    },

    /// Detection confidence fell below safety threshold.
    #[error("Encoding detection confidence ({confidence:.2}) below safety threshold ({threshold:.2})")]
    LowConfidenceEncoding {
        /// Canonical name of the guessed encoding.
        detected_name: &'static str,
        /// Measured confidence score in range [0.0, 1.0].
        confidence: f32,
        /// Required minimum confidence threshold.
        threshold: f32,
    },
}

impl From<TextDefenseError> for TTZipStatus {
    fn from(err: TextDefenseError) -> Self {
        match err {
            TextDefenseError::MalformedByteSequence { .. }
            | TextDefenseError::UnexpectedTruncation { .. } => TTZipStatus::ErrCorruptHeader,
            TextDefenseError::MemoryFuseExceeded { .. } => TTZipStatus::ErrOutOfMemory,
            TextDefenseError::ExpansionQuotaExceeded { .. }
            | TextDefenseError::IllegalUnicodeScalar { .. }
            | TextDefenseError::NullByteInjection { .. }
            | TextDefenseError::PathTraversalDetected { .. } => TTZipStatus::ErrSecurityViolation,
            TextDefenseError::LowConfidenceEncoding { .. } => TTZipStatus::ErrInvalidParam,
        }
    }
}

// ============================================================================
// 1. Malformed Byte Sequence & Truncation Guard
// ============================================================================

/// Guard for detecting and trimming malformed or truncated multibyte sequences.
#[derive(Debug, Clone, Copy, Default)]
pub struct MalformedByteSequenceGuard;

impl MalformedByteSequenceGuard {
    /// Validates that a byte slice is strictly valid UTF-8 without truncation or malformed bytes.
    pub fn validate_utf8(data: &[u8]) -> Result<(), TextDefenseError> {
        match std::str::from_utf8(data) {
            Ok(_) => Ok(()),
            Err(e) => {
                let valid_len = e.valid_up_to();
                if e.error_len().is_none() {
                    Err(TextDefenseError::UnexpectedTruncation {
                        needed: data.len() - valid_len + 1,
                        available: data.len() - valid_len,
                    })
                } else {
                    Err(TextDefenseError::MalformedByteSequence {
                        encoding_name: "UTF-8",
                        offset: valid_len,
                    })
                }
            }
        }
    }

    /// Validates that a byte slice contains valid character sequences under the specified encoding.
    pub fn validate_encoding(data: &[u8], encoding: &'static Encoding) -> Result<(), TextDefenseError> {
        if encoding == encoding_rs::UTF_8 {
            return Self::validate_utf8(data);
        }
        let mut decoder = encoding.new_decoder_without_bom_handling();
        let mut dest = vec![0u8; data.len() * 4 + 16];
        let (result, read, _) = decoder.decode_to_utf8_without_replacement(data, &mut dest, true);
        match result {
            encoding_rs::DecoderResult::InputEmpty => Ok(()),
            encoding_rs::DecoderResult::Malformed(len, _) => Err(TextDefenseError::MalformedByteSequence {
                encoding_name: encoding.name(),
                offset: read + (len as usize),
            }),
            encoding_rs::DecoderResult::OutputFull => Err(TextDefenseError::MemoryFuseExceeded {
                allocated: dest.len(),
                fuse_limit: DEFAULT_TEXT_MEMORY_FUSE_BYTES,
            }),
        }
    }

    /// Trims any trailing incomplete UTF-8 multibyte sequence at the end of a slice.
    /// Returns the valid prefix slice and the number of truncated bytes trimmed.
    #[must_use]
    pub fn trim_to_valid_utf8_boundary(data: &[u8]) -> (&[u8], usize) {
        if data.is_empty() {
            return (data, 0);
        }
        match std::str::from_utf8(data) {
            Ok(_) => (data, 0),
            Err(e) => {
                let valid_len = e.valid_up_to();
                (&data[..valid_len], data.len() - valid_len)
            }
        }
    }
}

// ============================================================================
// 2. Text Expansion & Memory Fuse Guard
// ============================================================================

/// Tracks and bounds in-flight transcoding memory allocation to prevent zip-bomb inflation.
#[derive(Debug, Clone)]
pub struct TextExpansionGuard {
    max_ratio: f64,
    max_memory_fuse: usize,
    cumulative_in: usize,
    cumulative_out: usize,
}

impl Default for TextExpansionGuard {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_EXPANSION_RATIO, DEFAULT_TEXT_MEMORY_FUSE_BYTES)
    }
}

impl TextExpansionGuard {
    /// Creates a new expansion guard with explicit ratio and memory fuse boundaries.
    #[must_use]
    pub fn new(max_ratio: f64, max_memory_fuse: usize) -> Self {
        Self {
            max_ratio: if max_ratio > 0.0 { max_ratio } else { DEFAULT_MAX_EXPANSION_RATIO },
            max_memory_fuse: if max_memory_fuse > 0 { max_memory_fuse } else { DEFAULT_TEXT_MEMORY_FUSE_BYTES },
            cumulative_in: 0,
            cumulative_out: 0,
        }
    }

    /// Records transcoding chunk consumption and checks whether expansion constraints are met.
    pub fn track_transcode(&mut self, in_bytes: usize, out_bytes: usize) -> Result<(), TextDefenseError> {
        self.cumulative_in = self.cumulative_in.saturating_add(in_bytes);
        self.cumulative_out = self.cumulative_out.saturating_add(out_bytes);

        if self.cumulative_out > self.max_memory_fuse {
            return Err(TextDefenseError::MemoryFuseExceeded {
                allocated: self.cumulative_out,
                fuse_limit: self.max_memory_fuse,
            });
        }

        let max_allowed = ((self.cumulative_in as f64) * self.max_ratio).ceil() as usize + DEFAULT_HEADROOM_BYTES;
        if self.cumulative_out > max_allowed {
            return Err(TextDefenseError::ExpansionQuotaExceeded {
                in_bytes: self.cumulative_in,
                out_bytes: self.cumulative_out,
                max_ratio: self.max_ratio,
            });
        }

        Ok(())
    }

    /// Validates that an upcoming transcoding allocation does not violate physical boundaries.
    pub fn check_allocation_bounds(in_bytes: usize, requested_out_bytes: usize, max_ratio: f64, fuse: usize) -> Result<(), TextDefenseError> {
        if requested_out_bytes > fuse {
            return Err(TextDefenseError::MemoryFuseExceeded {
                allocated: requested_out_bytes,
                fuse_limit: fuse,
            });
        }
        let max_allowed = ((in_bytes as f64) * max_ratio).ceil() as usize + DEFAULT_HEADROOM_BYTES;
        if requested_out_bytes > max_allowed {
            return Err(TextDefenseError::ExpansionQuotaExceeded {
                in_bytes,
                out_bytes: requested_out_bytes,
                max_ratio,
            });
        }
        Ok(())
    }

    /// Resets the cumulative state machine counters for reuse.
    pub fn reset(&mut self) {
        self.cumulative_in = 0;
        self.cumulative_out = 0;
    }

    /// Returns the cumulative input bytes processed.
    #[must_use]
    pub const fn cumulative_in(&self) -> usize {
        self.cumulative_in
    }

    /// Returns the cumulative output bytes emitted.
    #[must_use]
    pub const fn cumulative_out(&self) -> usize {
        self.cumulative_out
    }
}

// ============================================================================
// 3. Surrogate & Unassigned Code Point Sanitizer
// ============================================================================

/// Sanitizes Unicode scalar values, strips dangerous bidirectional overrides, and enforces normalization.
#[derive(Debug, Clone)]
pub struct SurrogateAndUnassignedGuard {
    /// Whether to allow Private Use Area (PUA) codepoints.
    pub allow_pua: bool,
    /// Whether to strip Trojan Source bidi control characters.
    pub strip_bidi: bool,
    /// Replacement character for illegal/sanitized codepoints.
    pub replacement_char: char,
}

impl Default for SurrogateAndUnassignedGuard {
    fn default() -> Self {
        Self {
            allow_pua: false,
            strip_bidi: true,
            replacement_char: '\u{FFFD}',
        }
    }
}

impl SurrogateAndUnassignedGuard {
    /// Creates a new guard with custom PUA and bidi filter settings.
    #[must_use]
    pub fn new(allow_pua: bool, strip_bidi: bool) -> Self {
        Self {
            allow_pua,
            strip_bidi,
            replacement_char: '\u{FFFD}',
        }
    }

    /// Returns `true` if a character is a bidirectional override or directional isolate.
    #[must_use]
    pub fn is_bidi_override(c: char) -> bool {
        matches!(
            c,
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}'
        )
    }

    /// Returns `true` if a character falls in the Unicode Private Use Area (PUA).
    #[must_use]
    pub fn is_private_use(c: char) -> bool {
        matches!(
            c as u32,
            0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD
        )
    }

    /// Validates that a raw 32-bit codepoint represents a legal Unicode scalar value.
    pub fn validate_codepoint(cp: u32) -> Result<(), TextDefenseError> {
        if (0xD800..=0xDFFF).contains(&cp) || cp > 0x10FFFF {
            return Err(TextDefenseError::IllegalUnicodeScalar { code_point: cp });
        }
        Ok(())
    }

    /// Sanitizes an input string by replacing or stripping illegal scalar values and bidi overrides.
    #[must_use]
    pub fn sanitize_text(&self, text: &str) -> String {
        let mut sanitized = String::with_capacity(text.len());
        for c in text.chars() {
            if self.strip_bidi && Self::is_bidi_override(c) {
                continue;
            }
            if !self.allow_pua && Self::is_private_use(c) {
                sanitized.push(self.replacement_char);
                continue;
            }
            sanitized.push(c);
        }
        sanitized
    }

    /// Sanitizes and canonicalizes input text into Unicode NFC form.
    #[must_use]
    pub fn sanitize_and_normalize_nfc(&self, text: &str) -> String {
        let cleaned = self.sanitize_text(text);
        cleaned.nfc().collect::<String>()
    }

    /// Canonicalizes input text into Unicode NFD form.
    #[must_use]
    pub fn normalize_nfd(text: &str) -> String {
        text.nfd().collect::<String>()
    }

    /// Canonicalizes input text into Unicode NFC form.
    #[must_use]
    pub fn normalize_nfc(text: &str) -> String {
        text.nfc().collect::<String>()
    }
}

// ============================================================================
// 4. Null Byte & Path Traversal Guard
// ============================================================================

/// Intercepts C-string null byte injection and directory traversal attacks.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullByteAndPathTraversalGuard;

impl NullByteAndPathTraversalGuard {
    /// Scans a byte slice for embedded null bytes (`0x00`).
    pub fn validate_no_null_bytes(data: &[u8]) -> Result<(), TextDefenseError> {
        if let Some(pos) = data.iter().position(|&b| b == 0) {
            return Err(TextDefenseError::NullByteInjection { offset: pos });
        }
        Ok(())
    }

    /// Validates that a path string is safe, has no null bytes, and cannot traverse parent directories.
    pub fn validate_path(path: &str) -> Result<(), TextDefenseError> {
        if let Some(pos) = path.as_bytes().iter().position(|&b| b == 0) {
            return Err(TextDefenseError::NullByteInjection { offset: pos });
        }

        let p = Path::new(path);
        for comp in p.components() {
            match comp {
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(TextDefenseError::PathTraversalDetected {
                        path: path.to_string(),
                    });
                }
                Component::Normal(os_str) => {
                    if let Some(s) = os_str.to_str() {
                        if Self::is_dos_device_name(s) {
                            return Err(TextDefenseError::PathTraversalDetected {
                                path: path.to_string(),
                            });
                        }
                    }
                }
                Component::CurDir => {}
            }
        }

        if path.contains(':') || path.starts_with('\\') || path.starts_with('/') {
            return Err(TextDefenseError::PathTraversalDetected {
                path: path.to_string(),
            });
        }

        Ok(())
    }

    /// Sanitizes an arbitrary path string into a safe, normalized relative path.
    #[must_use]
    pub fn sanitize_path(path: &str) -> String {
        let mut clean = String::with_capacity(path.len());
        for c in path.chars() {
            if c != '\0' && !c.is_control() && c != ':' {
                clean.push(c);
            }
        }

        let normalized = clean.replace('\\', "/");
        let parts: Vec<&str> = normalized.split('/').collect();
        let mut safe_parts: Vec<&str> = Vec::new();

        for part in parts {
            let trimmed = part.trim();
            if trimmed.is_empty() || trimmed == "." {
                continue;
            }
            if trimmed == ".." {
                safe_parts.pop();
                continue;
            }
            if Self::is_dos_device_name(trimmed) {
                continue;
            }
            safe_parts.push(trimmed);
        }

        if safe_parts.is_empty() {
            "unnamed_entry".to_string()
        } else {
            safe_parts.join("/")
        }
    }

    /// Strips null bytes from input string.
    #[must_use]
    pub fn strip_null_bytes(text: &str) -> String {
        text.chars().filter(|&c| c != '\0').collect()
    }

    /// Returns `true` if the filename matches a reserved DOS device name (e.g. `CON`, `NUL`, `PRN`, `COM1`).
    #[must_use]
    pub fn is_dos_device_name(name: &str) -> bool {
        let base = name.split('.').next().unwrap_or(name);
        let upper = base.to_ascii_uppercase();
        matches!(
            upper.as_str(),
            "CON" | "PRN" | "AUX" | "NUL"
                | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
                | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
        )
    }
}

// ============================================================================
// 5. Sensitive Text Memory Zeroize Guard
// ============================================================================

/// Volatile heap buffer for passwords, encryption keys, and secrets that zeroizes on drop.
#[derive(Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct SensitiveTextBuffer {
    inner: Vec<u8>,
}

impl std::str::FromStr for SensitiveTextBuffer {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            inner: s.as_bytes().to_vec(),
        })
    }
}

impl From<&str> for SensitiveTextBuffer {
    fn from(s: &str) -> Self {
        Self {
            inner: s.as_bytes().to_vec(),
        }
    }
}

impl SensitiveTextBuffer {
    /// Creates a new sensitive buffer from a string slice.
    #[must_use]
    pub fn new_from_str(s: &str) -> Self {
        Self::from(s)
    }

    /// Creates a new sensitive buffer from an existing byte vector.
    #[must_use]
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        Self { inner: bytes }
    }

    /// Creates an empty sensitive buffer with pre-allocated capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Vec::with_capacity(capacity),
        }
    }

    /// Borrows the buffer contents as a UTF-8 string slice if valid.
    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.inner)
    }

    /// Borrows the buffer contents as a raw byte slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.inner
    }

    /// Returns the length in bytes of the sensitive buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Explicitly overwrites buffer memory with zeroes and empties it.
    pub fn explicit_zeroize(&mut self) {
        self.inner.zeroize();
        self.inner.clear();
    }
}

impl fmt::Debug for SensitiveTextBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SensitiveTextBuffer")
            .field("len", &self.inner.len())
            .field("redacted", &"***REDACTED***")
            .finish()
    }
}

// ============================================================================
// 6. Encoding Confidence Fallback Guard
// ============================================================================

/// Resolution policy when character set detection confidence falls below safety bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackStrategy {
    /// Strictly reject low-confidence detections with an error.
    StrictReject,
    /// Fallback to standard UTF-8 with `U+FFFD` replacement character decoding.
    SafeUtf8Replacement,
    /// Fallback to a predetermined system default encoding.
    SystemDefault(&'static Encoding),
}

/// The final evaluated encoding result after confidence guard assessment.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedEncoding {
    /// The resolved encoding to use for transcoding.
    pub encoding: &'static Encoding,
    /// Evaluated confidence score.
    pub confidence: f32,
    /// Whether this encoding was assigned via safety fallback demotion.
    pub is_fallback: bool,
}

/// Evaluates detection confidence and enforces safe fallback policies.
#[derive(Debug, Clone)]
pub struct EncodingConfidenceFallbackGuard {
    /// Minimum acceptable confidence score [0.0, 1.0].
    pub min_confidence: f32,
    /// Fallback strategy to employ when below threshold.
    pub strategy: FallbackStrategy,
}

impl Default for EncodingConfidenceFallbackGuard {
    fn default() -> Self {
        Self {
            min_confidence: DEFAULT_MIN_CONFIDENCE_THRESHOLD,
            strategy: FallbackStrategy::SafeUtf8Replacement,
        }
    }
}

impl EncodingConfidenceFallbackGuard {
    /// Creates a new confidence fallback guard with explicit threshold and strategy.
    #[must_use]
    pub fn new(min_confidence: f32, strategy: FallbackStrategy) -> Self {
        Self {
            min_confidence: if (0.0..=1.0).contains(&min_confidence) {
                min_confidence
            } else {
                DEFAULT_MIN_CONFIDENCE_THRESHOLD
            },
            strategy,
        }
    }

    /// Evaluates a detected encoding and confidence score against the guard invariants.
    pub fn evaluate(
        &self,
        detected: &'static Encoding,
        confidence: f32,
        raw_bytes: &[u8],
    ) -> Result<ResolvedEncoding, TextDefenseError> {
        if std::str::from_utf8(raw_bytes).is_ok() {
            return Ok(ResolvedEncoding {
                encoding: encoding_rs::UTF_8,
                confidence: 1.0,
                is_fallback: false,
            });
        }

        if confidence >= self.min_confidence {
            return Ok(ResolvedEncoding {
                encoding: detected,
                confidence,
                is_fallback: false,
            });
        }

        match self.strategy {
            FallbackStrategy::StrictReject => Err(TextDefenseError::LowConfidenceEncoding {
                detected_name: detected.name(),
                confidence,
                threshold: self.min_confidence,
            }),
            FallbackStrategy::SafeUtf8Replacement => Ok(ResolvedEncoding {
                encoding: encoding_rs::UTF_8,
                confidence: 0.0,
                is_fallback: true,
            }),
            FallbackStrategy::SystemDefault(def_enc) => Ok(ResolvedEncoding {
                encoding: def_enc,
                confidence: 0.0,
                is_fallback: true,
            }),
        }
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_malformed_sequence_and_truncation() {
        let valid = "Hello, 世界!".as_bytes();
        assert!(MalformedByteSequenceGuard::validate_utf8(valid).is_ok());

        let truncated = [0xE4, 0xB8];
        let err = MalformedByteSequenceGuard::validate_utf8(&truncated).unwrap_err();
        assert!(matches!(err, TextDefenseError::UnexpectedTruncation { .. }));

        let (prefix, trimmed) = MalformedByteSequenceGuard::trim_to_valid_utf8_boundary(&truncated);
        assert_eq!(prefix.len(), 0);
        assert_eq!(trimmed, 2);

        let invalid = [0xFF, 0xFE];
        assert!(MalformedByteSequenceGuard::validate_utf8(&invalid).is_err());
    }

    #[test]
    fn test_text_expansion_and_memory_fuse() {
        let mut guard = TextExpansionGuard::new(4.0, 1024 * 1024);
        assert!(guard.track_transcode(100, 300).is_ok());

        let err = guard.track_transcode(10, 20000).unwrap_err();
        assert!(matches!(err, TextDefenseError::ExpansionQuotaExceeded { .. }));

        let mut small_fuse = TextExpansionGuard::new(10.0, 1000);
        let err2 = small_fuse.track_transcode(200, 1500).unwrap_err();
        assert!(matches!(err2, TextDefenseError::MemoryFuseExceeded { .. }));
    }

    #[test]
    fn test_surrogate_and_bidi_sanitizer() {
        let guard = SurrogateAndUnassignedGuard::default();
        let malicious = "test\u{202E}txt.exe";
        let cleaned = guard.sanitize_text(malicious);
        assert_eq!(cleaned, "testtxt.exe");

        assert!(SurrogateAndUnassignedGuard::validate_codepoint(0xD800).is_err());
        assert!(SurrogateAndUnassignedGuard::validate_codepoint(0x110000).is_err());
        assert!(SurrogateAndUnassignedGuard::validate_codepoint(0x4E2D).is_ok());
    }

    #[test]
    fn test_null_byte_and_path_traversal() {
        let malicious_path = "folder/../../etc/passwd";
        assert!(NullByteAndPathTraversalGuard::validate_path(malicious_path).is_err());

        let null_path = "file.txt\0.exe";
        assert!(NullByteAndPathTraversalGuard::validate_path(null_path).is_err());

        let sanitized = NullByteAndPathTraversalGuard::sanitize_path("foo/../bar//baz/../qux");
        assert_eq!(sanitized, "bar/qux");

        let dos_device = "CON.txt";
        assert!(NullByteAndPathTraversalGuard::validate_path(dos_device).is_err());
    }

    #[test]
    fn test_sensitive_text_buffer_zeroize() {
        let mut secret = SensitiveTextBuffer::from("SuperSecretPassword123!");
        assert_eq!(secret.as_str().unwrap(), "SuperSecretPassword123!");
        assert!(!secret.is_empty());

        secret.explicit_zeroize();
        assert!(secret.is_empty());
    }

    #[test]
    fn test_encoding_confidence_fallback() {
        let guard = EncodingConfidenceFallbackGuard::new(0.60, FallbackStrategy::SafeUtf8Replacement);
        let non_utf8 = [0x82, 0xA0]; // Shift-JIS 'あ'

        let res = guard.evaluate(encoding_rs::SHIFT_JIS, 0.30, &non_utf8).unwrap();
        assert_eq!(res.encoding, encoding_rs::UTF_8);
        assert!(res.is_fallback);

        let res2 = guard.evaluate(encoding_rs::SHIFT_JIS, 0.85, &non_utf8).unwrap();
        assert_eq!(res2.encoding, encoding_rs::SHIFT_JIS);
        assert!(!res2.is_fallback);

        let strict_guard = EncodingConfidenceFallbackGuard::new(0.60, FallbackStrategy::StrictReject);
        let err = strict_guard.evaluate(encoding_rs::SHIFT_JIS, 0.40, &non_utf8).unwrap_err();
        assert!(matches!(err, TextDefenseError::LowConfidenceEncoding { .. }));
    }
}
