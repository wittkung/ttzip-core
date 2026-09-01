// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! PDF Stream Expansion Quota Guard.
//!
//! Enforces memory fuses and decompression ratio limits to protect against
//! Flate/LZW decompression bombs and infinite stream expansion attacks in PDF payloads.

use std::io::Read;
use flate2::read::ZlibDecoder;

use super::{
    PdfDefenseError, DEFAULT_MAX_SINGLE_STREAM_BYTES, DEFAULT_MAX_STREAM_EXPANSION_RATIO,
    DEFAULT_MAX_TOTAL_STREAM_BYTES,
};

/// Summary report of stream inspection across a PDF document.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamInspectionResult {
    /// Number of stream objects inspected.
    pub stream_count: usize,
    /// Maximum uncompressed stream size encountered in bytes.
    pub max_stream_size: usize,
    /// Cumulative uncompressed payload size across all streams in bytes.
    pub total_uncompressed_bytes: usize,
    /// Highest expansion ratio observed across all compressed streams.
    pub max_expansion_ratio: f64,
}

/// Guard enforcing memory ceilings and expansion quotas during PDF stream decompression.
#[derive(Debug, Clone)]
pub struct StreamExpansionQuotaGuard {
    /// Maximum allowable uncompressed size for a single stream (default: 32 MiB).
    max_single_stream_bytes: usize,
    /// Maximum allowable expansion ratio (default: 200.0x).
    max_expansion_ratio: f64,
    /// Maximum cumulative uncompressed payload size across the document (default: 128 MiB).
    max_total_stream_bytes: usize,
    /// Minimum compressed size threshold before enforcing ratio checks (to avoid false positives on tiny 10-byte streams).
    ratio_enforcement_threshold: usize,
    /// Cumulative uncompressed stream bytes tracked in this session.
    cumulative_bytes: usize,
}

impl Default for StreamExpansionQuotaGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamExpansionQuotaGuard {
    /// Creates a new guard with default security thresholds (single <= 32MB, ratio <= 200x, total <= 128MB).
    pub fn new() -> Self {
        Self {
            max_single_stream_bytes: DEFAULT_MAX_SINGLE_STREAM_BYTES,
            max_expansion_ratio: DEFAULT_MAX_STREAM_EXPANSION_RATIO,
            max_total_stream_bytes: DEFAULT_MAX_TOTAL_STREAM_BYTES,
            ratio_enforcement_threshold: 64,
            cumulative_bytes: 0,
        }
    }

    /// Creates a new guard with custom quotas.
    pub fn with_limits(
        max_single_stream_bytes: usize,
        max_expansion_ratio: f64,
        max_total_stream_bytes: usize,
    ) -> Self {
        Self {
            max_single_stream_bytes,
            max_expansion_ratio,
            max_total_stream_bytes,
            ratio_enforcement_threshold: 64,
            cumulative_bytes: 0,
        }
    }

    /// Resets cumulative byte tracking for a new document inspection.
    pub fn reset(&mut self) {
        self.cumulative_bytes = 0;
    }

    /// Returns the maximum allowed single stream size.
    pub fn max_single_stream_bytes(&self) -> usize {
        self.max_single_stream_bytes
    }

    /// Returns the maximum allowed expansion ratio.
    pub fn max_expansion_ratio(&self) -> f64 {
        self.max_expansion_ratio
    }

    /// Validates raw stream metadata before attempting decompression.
    pub fn validate_metadata(
        &self,
        compressed_len: usize,
        declared_uncompressed_len: Option<usize>,
    ) -> Result<(), PdfDefenseError> {
        if let Some(uncompressed) = declared_uncompressed_len {
            if uncompressed > self.max_single_stream_bytes {
                return Err(PdfDefenseError::StreamSizeExceeded {
                    size: uncompressed,
                    max_size: self.max_single_stream_bytes,
                });
            }

            if compressed_len >= self.ratio_enforcement_threshold {
                let ratio = uncompressed as f64 / compressed_len.max(1) as f64;
                if ratio > self.max_expansion_ratio {
                    return Err(PdfDefenseError::StreamExpansionRatioExceeded {
                        ratio,
                        max_ratio: self.max_expansion_ratio,
                        compressed: compressed_len,
                        uncompressed,
                    });
                }
            }
        }
        Ok(())
    }

    /// Decompresses a Flate/Zlib compressed PDF stream with chunked quota enforcement.
    pub fn decompress_flate(&mut self, compressed: &[u8]) -> Result<Vec<u8>, PdfDefenseError> {
        let compressed_len = compressed.len();
        let mut decoder = ZlibDecoder::new(compressed);
        let mut uncompressed_buf = Vec::new();
        let mut chunk = [0u8; 16384];

        loop {
            match decoder.read(&mut chunk) {
                Ok(0) => break,
                Ok(bytes_read) => {
                    let new_total = uncompressed_buf.len() + bytes_read;

                    // 1. Check single stream size limit
                    if new_total > self.max_single_stream_bytes {
                        return Err(PdfDefenseError::StreamSizeExceeded {
                            size: new_total,
                            max_size: self.max_single_stream_bytes,
                        });
                    }

                    // 2. Check expansion ratio if compressed data is non-trivial
                    if compressed_len >= self.ratio_enforcement_threshold {
                        let ratio = new_total as f64 / compressed_len.max(1) as f64;
                        if ratio > self.max_expansion_ratio {
                            return Err(PdfDefenseError::StreamExpansionRatioExceeded {
                                ratio,
                                max_ratio: self.max_expansion_ratio,
                                compressed: compressed_len,
                                uncompressed: new_total,
                            });
                        }
                    }

                    // 3. Check cumulative document-level stream limit
                    let new_cumulative = self.cumulative_bytes + bytes_read;
                    if new_cumulative > self.max_total_stream_bytes {
                        return Err(PdfDefenseError::TotalStreamBytesExceeded {
                            total_bytes: new_cumulative,
                            max_bytes: self.max_total_stream_bytes,
                        });
                    }

                    uncompressed_buf.extend_from_slice(&chunk[..bytes_read]);
                    self.cumulative_bytes += bytes_read;
                }
                Err(err) => {
                    return Err(PdfDefenseError::MalformedPdf {
                        reason: format!("Flate stream decompression error: {err}"),
                        offset: None,
                    });
                }
            }
        }

        Ok(uncompressed_buf)
    }

    /// Inspects and validates all streams in a `lopdf::Document`.
    pub fn inspect_all_streams(
        &mut self,
        doc: &lopdf::Document,
    ) -> Result<StreamInspectionResult, PdfDefenseError> {
        self.reset();
        let mut stream_count = 0;
        let mut max_stream_size = 0;
        let mut max_expansion_ratio = 1.0f64;

        for obj in doc.objects.values() {
            if let lopdf::Object::Stream(stream) = obj {
                stream_count += 1;
                let compressed_len = stream.content.len();

                // Check for filter
                let filter = stream.dict.get(b"Filter").ok();
                let is_flate = match filter {
                    Some(lopdf::Object::Name(name)) => name == b"FlateDecode",
                    Some(lopdf::Object::Array(arr)) => {
                        arr.iter().any(|item| matches!(item, lopdf::Object::Name(n) if n == b"FlateDecode"))
                    }
                    _ => false,
                };

                let uncompressed_len = if is_flate {
                    let decompressed = self.decompress_flate(&stream.content)?;
                    let len = decompressed.len();
                    let ratio = len as f64 / compressed_len.max(1) as f64;
                    if ratio > max_expansion_ratio {
                        max_expansion_ratio = ratio;
                    }
                    len
                } else {
                    if compressed_len > self.max_single_stream_bytes {
                        return Err(PdfDefenseError::StreamSizeExceeded {
                            size: compressed_len,
                            max_size: self.max_single_stream_bytes,
                        });
                    }
                    self.cumulative_bytes += compressed_len;
                    if self.cumulative_bytes > self.max_total_stream_bytes {
                        return Err(PdfDefenseError::TotalStreamBytesExceeded {
                            total_bytes: self.cumulative_bytes,
                            max_bytes: self.max_total_stream_bytes,
                        });
                    }
                    compressed_len
                };

                if uncompressed_len > max_stream_size {
                    max_stream_size = uncompressed_len;
                }
            }
        }

        Ok(StreamInspectionResult {
            stream_count,
            max_stream_size,
            total_uncompressed_bytes: self.cumulative_bytes,
            max_expansion_ratio,
        })
    }
}
