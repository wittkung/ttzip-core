// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-Allocation and Streaming Text Transcoding Engine.
//!
//! Provides high-throughput conversion between 40+ WHATWG legacy character encodings
//! and standard UTF-8, supporting both lossless strict validation and replacement character (U+FFFD) modes.

use std::borrow::Cow;
use std::io::{Read, Write};
use encoding_rs::{CoderResult, DecoderResult, Encoding, EncoderResult};
use thiserror::Error;

/// Default buffer capacity for streaming transcoding operations (8 KiB).
pub const DEFAULT_TRANSCODE_BUFFER_SIZE: usize = 8 * 1024;

/// Errors arising during text transcoding operations.
#[derive(Debug, Error)]
pub enum TextTranscodeError {
    /// Input data contains malformed byte sequences for the specified encoding in lossless mode.
    #[error("Malformed byte sequence for encoding '{encoding_name}' at byte offset {byte_offset}")]
    InvalidEncodingData {
        /// Canonical name of the target encoding.
        encoding_name: &'static str,
        /// Approximate byte offset where the error occurred.
        byte_offset: usize,
    },

    /// Character cannot be represented in the destination legacy encoding.
    #[error("Unmappable character for encoding '{encoding_name}' at byte offset {byte_offset}")]
    UnmappableCharacter {
        /// Canonical name of the target encoding.
        encoding_name: &'static str,
        /// Approximate byte offset where unmappable char was encountered.
        byte_offset: usize,
    },

    /// I/O error occurred during streaming transcoding.
    #[error("I/O error during text transcode: {0}")]
    IoError(#[from] std::io::Error),
}

/// Options configuring the transcoding pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscodeOptions {
    /// If true, fail immediately with `TextTranscodeError` on any invalid/unmappable bytes.
    pub lossless: bool,
    /// Memory buffer size for streaming operations in bytes.
    pub buffer_size: usize,
}

impl Default for TranscodeOptions {
    fn default() -> Self {
        Self {
            lossless: false,
            buffer_size: DEFAULT_TRANSCODE_BUFFER_SIZE,
        }
    }
}

/// Statistical summary of a streaming transcoding run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TranscodeStats {
    /// Total raw bytes consumed from input.
    pub bytes_read: usize,
    /// Total UTF-8 or destination bytes emitted.
    pub bytes_written: usize,
    /// Whether any replacement characters (U+FFFD or '?') were inserted.
    pub had_replacements: bool,
    /// Total count of invalid byte sequences encountered.
    pub errors_count: usize,
}

/// Core high-performance text transcoder.
pub struct TTZipTextTranscoder;

impl TTZipTextTranscoder {
    /// Fast look up of an encoding by name, MIME label, or alias.
    #[must_use]
    pub fn lookup_encoding(name: &str) -> Option<&'static Encoding> {
        Encoding::for_label(name.as_bytes())
    }

    /// Decodes a byte slice to UTF-8 string with zero allocation when input is already UTF-8.
    ///
    /// Returns `(Cow<str>, had_replacements)`.
    #[must_use]
    pub fn decode_to_utf8<'a>(data: &'a [u8], encoding: &'static Encoding) -> (Cow<'a, str>, bool) {
        if encoding == encoding_rs::UTF_8 {
            match std::str::from_utf8(data) {
                Ok(valid_str) => (Cow::Borrowed(valid_str), false),
                Err(_) => {
                    let (cow, _, had_errors) = encoding.decode(data);
                    (cow, had_errors)
                }
            }
        } else {
            let (cow, _, had_errors) = encoding.decode(data);
            (cow, had_errors)
        }
    }

    /// Decodes a byte slice to UTF-8 strictly in lossless mode.
    ///
    /// Returns `Err(TextTranscodeError::InvalidEncodingData)` if malformed bytes are encountered.
    pub fn decode_to_utf8_lossless<'a>(
        data: &'a [u8],
        encoding: &'static Encoding,
    ) -> Result<Cow<'a, str>, TextTranscodeError> {
        if encoding == encoding_rs::UTF_8 {
            std::str::from_utf8(data)
                .map(Cow::Borrowed)
                .map_err(|e| TextTranscodeError::InvalidEncodingData {
                    encoding_name: encoding.name(),
                    byte_offset: e.valid_up_to(),
                })
        } else {
            let mut decoder = encoding.new_decoder_without_bom_handling();
            // Estimate required buffer capacity
            let max_len = decoder
                .max_utf8_buffer_length_without_replacement(data.len())
                .unwrap_or(data.len() * 3);
            let mut output_str = String::with_capacity(max_len);

            let mut total_read = 0;
            let mut dst_buf = vec![0u8; 4096];

            loop {
                let (res, read, written) = decoder.decode_to_utf8_without_replacement(
                    &data[total_read..],
                    &mut dst_buf,
                    true,
                );
                total_read = total_read.saturating_add(read);

                let valid_chunk = match std::str::from_utf8(&dst_buf[..written]) {
                    Ok(s) => s,
                    Err(e) => {
                        return Err(TextTranscodeError::InvalidEncodingData {
                            encoding_name: encoding.name(),
                            byte_offset: total_read.saturating_add(e.valid_up_to()),
                        });
                    }
                };
                output_str.push_str(valid_chunk);

                match res {
                    DecoderResult::InputEmpty => break,
                    DecoderResult::OutputFull => continue,
                    DecoderResult::Malformed(_, _) => {
                        return Err(TextTranscodeError::InvalidEncodingData {
                            encoding_name: encoding.name(),
                            byte_offset: total_read,
                        });
                    }
                }
            }
            Ok(Cow::Owned(output_str))
        }
    }

    /// Decodes a byte slice using lossy replacement character `U+FFFD`.
    #[must_use]
    pub fn decode_to_utf8_lossy<'a>(data: &'a [u8], encoding: &'static Encoding) -> (Cow<'a, str>, bool) {
        Self::decode_to_utf8(data, encoding)
    }

    /// Encodes a UTF-8 string into target legacy encoding with zero allocation when possible.
    ///
    /// Returns `(Cow<[u8]>, had_unmappable_chars)`.
    #[must_use]
    pub fn encode_from_utf8<'a>(text: &'a str, encoding: &'static Encoding) -> (Cow<'a, [u8]>, bool) {
        if encoding == encoding_rs::UTF_8 {
            (Cow::Borrowed(text.as_bytes()), false)
        } else {
            let (cow, _, had_errors) = encoding.encode(text);
            (cow, had_errors)
        }
    }

    /// Encodes a UTF-8 string into target legacy encoding strictly in lossless mode.
    pub fn encode_from_utf8_lossless<'a>(
        text: &'a str,
        encoding: &'static Encoding,
    ) -> Result<Cow<'a, [u8]>, TextTranscodeError> {
        if encoding == encoding_rs::UTF_8 {
            Ok(Cow::Borrowed(text.as_bytes()))
        } else {
            let mut encoder = encoding.new_encoder();
            let max_len = encoder
                .max_buffer_length_from_utf8_without_replacement(text.len())
                .unwrap_or(text.len() * 2);
            let mut output_bytes = Vec::with_capacity(max_len);

            let mut total_read = 0;
            let mut dst_buf = vec![0u8; 4096];

            loop {
                let (res, read, written) = encoder.encode_from_utf8_without_replacement(
                    &text[total_read..],
                    &mut dst_buf,
                    true,
                );
                total_read = total_read.saturating_add(read);
                output_bytes.extend_from_slice(&dst_buf[..written]);

                match res {
                    EncoderResult::InputEmpty => break,
                    EncoderResult::OutputFull => continue,
                    EncoderResult::Unmappable(_) => {
                        return Err(TextTranscodeError::UnmappableCharacter {
                            encoding_name: encoding.name(),
                            byte_offset: total_read,
                        });
                    }
                }
            }
            Ok(Cow::Owned(output_bytes))
        }
    }

    /// Transcodes a streaming input from arbitrary encoding to UTF-8 output.
    pub fn decode_streaming<R: Read, W: Write>(
        reader: &mut R,
        writer: &mut W,
        encoding: &'static Encoding,
        options: &TranscodeOptions,
    ) -> Result<TranscodeStats, TextTranscodeError> {
        let mut stats = TranscodeStats::default();
        let buf_size = options.buffer_size.max(512);

        let mut read_buf = vec![0u8; buf_size];
        let mut write_buf = vec![0u8; buf_size.saturating_mul(3)];

        let mut decoder = encoding.new_decoder();

        loop {
            let bytes_read = reader.read(&mut read_buf)?;
            if bytes_read == 0 {
                // Flush remaining bytes
                if options.lossless {
                    let (res, _, written) =
                        decoder.decode_to_utf8_without_replacement(&[], &mut write_buf, true);
                    if written > 0 {
                        writer.write_all(&write_buf[..written])?;
                        stats.bytes_written = stats.bytes_written.saturating_add(written);
                    }
                    if let DecoderResult::Malformed(_, _) = res {
                        return Err(TextTranscodeError::InvalidEncodingData {
                            encoding_name: encoding.name(),
                            byte_offset: stats.bytes_read,
                        });
                    }
                } else {
                    let (_, _, written, had_replacements) =
                        decoder.decode_to_utf8(&[], &mut write_buf, true);
                    if written > 0 {
                        writer.write_all(&write_buf[..written])?;
                        stats.bytes_written = stats.bytes_written.saturating_add(written);
                    }
                    if had_replacements {
                        stats.had_replacements = true;
                        stats.errors_count = stats.errors_count.saturating_add(1);
                    }
                }
                break;
            }

            stats.bytes_read = stats.bytes_read.saturating_add(bytes_read);
            let mut src_offset = 0;

            while src_offset < bytes_read {
                if options.lossless {
                    let (res, read, written) = decoder.decode_to_utf8_without_replacement(
                        &read_buf[src_offset..bytes_read],
                        &mut write_buf,
                        false,
                    );
                    src_offset = src_offset.saturating_add(read);
                    if written > 0 {
                        writer.write_all(&write_buf[..written])?;
                        stats.bytes_written = stats.bytes_written.saturating_add(written);
                    }

                    match res {
                        DecoderResult::InputEmpty => break,
                        DecoderResult::OutputFull => continue,
                        DecoderResult::Malformed(_, _) => {
                            return Err(TextTranscodeError::InvalidEncodingData {
                                encoding_name: encoding.name(),
                                byte_offset: stats.bytes_read.saturating_sub(bytes_read).saturating_add(src_offset),
                            });
                        }
                    }
                } else {
                    let (res, read, written, had_replacements) = decoder.decode_to_utf8(
                        &read_buf[src_offset..bytes_read],
                        &mut write_buf,
                        false,
                    );
                    src_offset = src_offset.saturating_add(read);
                    if written > 0 {
                        writer.write_all(&write_buf[..written])?;
                        stats.bytes_written = stats.bytes_written.saturating_add(written);
                    }
                    if had_replacements {
                        stats.had_replacements = true;
                        stats.errors_count = stats.errors_count.saturating_add(1);
                    }

                    if res == CoderResult::InputEmpty {
                        break;
                    }
                }
            }
        }

        Ok(stats)
    }
}

/// Convenience helper to decode bytes to UTF-8 string with fallback replacement characters.
#[must_use]
pub fn decode_to_utf8<'a>(data: &'a [u8], encoding: &'static Encoding) -> (Cow<'a, str>, bool) {
    TTZipTextTranscoder::decode_to_utf8(data, encoding)
}

/// Convenience helper to decode bytes to UTF-8 strictly without replacements.
pub fn decode_to_utf8_lossless<'a>(
    data: &'a [u8],
    encoding: &'static Encoding,
) -> Result<Cow<'a, str>, TextTranscodeError> {
    TTZipTextTranscoder::decode_to_utf8_lossless(data, encoding)
}

/// Convenience helper for lossy decoding.
#[must_use]
pub fn decode_to_utf8_lossy<'a>(data: &'a [u8], encoding: &'static Encoding) -> (Cow<'a, str>, bool) {
    TTZipTextTranscoder::decode_to_utf8_lossy(data, encoding)
}

/// Convenience helper to encode UTF-8 text into target encoding bytes.
#[must_use]
pub fn encode_from_utf8<'a>(text: &'a str, encoding: &'static Encoding) -> (Cow<'a, [u8]>, bool) {
    TTZipTextTranscoder::encode_from_utf8(text, encoding)
}

/// Convenience helper to encode UTF-8 text into target encoding strictly without unmappable replacement.
pub fn encode_from_utf8_lossless<'a>(
    text: &'a str,
    encoding: &'static Encoding,
) -> Result<Cow<'a, [u8]>, TextTranscodeError> {
    TTZipTextTranscoder::encode_from_utf8_lossless(text, encoding)
}

/// Convenience helper to lookup encoding by name or label.
#[must_use]
pub fn lookup_encoding_by_name(name: &str) -> Option<&'static Encoding> {
    TTZipTextTranscoder::lookup_encoding(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_alloc_utf8_decode() {
        let text = "TTZip Architecture Standard";
        let (cow, had_errors) = decode_to_utf8(text.as_bytes(), encoding_rs::UTF_8);
        assert!(!had_errors);
        assert!(matches!(cow, Cow::Borrowed(_)));
        assert_eq!(cow, text);
    }

    #[test]
    fn test_gbk_decode_lossless() {
        // "中文测试" in GBK: D6 D0 CE C4 B2 E2 CA D4
        let gbk_bytes = [0xD6, 0xD0, 0xCE, 0xC4, 0xB2, 0xE2, 0xCA, 0xD4];
        let decoded = decode_to_utf8_lossless(&gbk_bytes, encoding_rs::GB18030).unwrap();
        assert_eq!(decoded, "中文测试");
    }

    #[test]
    fn test_shift_jis_streaming() {
        // "テスト" in Shift_JIS: 83 65 83 58 83 67
        let sjis_bytes = [0x83, 0x65, 0x83, 0x58, 0x83, 0x67];
        let mut cursor = std::io::Cursor::new(&sjis_bytes);
        let mut out = Vec::new();

        let stats = TTZipTextTranscoder::decode_streaming(
            &mut cursor,
            &mut out,
            encoding_rs::SHIFT_JIS,
            &TranscodeOptions::default(),
        )
        .unwrap();

        assert_eq!(stats.bytes_read, 6);
        assert_eq!(String::from_utf8(out).unwrap(), "テスト");
    }

    #[test]
    fn test_lossless_error_handling() {
        let malformed_utf8 = [0xFF, 0xFE, 0xFD];
        let res = decode_to_utf8_lossless(&malformed_utf8, encoding_rs::UTF_8);
        assert!(res.is_err());
    }
}
