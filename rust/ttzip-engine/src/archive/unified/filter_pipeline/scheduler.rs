// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! FilterPipeline orchestrator and dynamic cascading state machine.

use std::io::{self, Read};

use super::filters::{
    BrotliFilter, Bzip2Filter, CompressFilter, GzipFilter, RpmLeadFilter, SnappyFilter,
    UuencodeFilter, XzFilter, ZstdFilter,
};
use super::kinds::FilterKind;
use super::lookahead::SlidingLookaheadReader;
use crate::archive::unified::format_sniffer::formats::ArchiveFormat;
use crate::archive::unified::format_sniffer::FormatSniffer;
use crate::types::TTZipStatus;

/// Maximum allowable filter cascade recursion depth before triggering DoS circuit breaker.
pub const MAX_FILTER_CHAIN_DEPTH: usize = 25;

/// Pipeline resolution error taxonomy.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FilterPipelineError {
    /// Cascaded filter recursion depth exceeded the maximum allowed threshold (25).
    #[error("Maximum filter chain depth exceeded (depth: {depth}, limit: {limit})")]
    ErrTooManyFilters { depth: usize, limit: usize },

    /// Corrupted or truncated filter stream header.
    #[error("Corrupt or invalid filter stream header: {0}")]
    CorruptStream(String),

    /// Standard I/O error during filter pipeline processing.
    #[error("I/O error during filter pipeline execution: {0}")]
    Io(String),

    /// Unsupported filter format.
    #[error("Unsupported filter format: {0}")]
    UnsupportedFormat(String),
}

impl From<io::Error> for FilterPipelineError {
    fn from(err: io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<FilterPipelineError> for TTZipStatus {
    fn from(err: FilterPipelineError) -> Self {
        match err {
            FilterPipelineError::ErrTooManyFilters { .. } => TTZipStatus::ErrSecurityViolation,
            FilterPipelineError::CorruptStream(_) => TTZipStatus::ErrCorruptHeader,
            FilterPipelineError::Io(_) => TTZipStatus::ErrExtractionFailed,
            FilterPipelineError::UnsupportedFormat(_) => TTZipStatus::ErrUnsupportedFeature,
        }
    }
}

/// Pipeline execution result containing the unwrapped reader stream,
/// the ordered list of applied filters, and the terminal identified archive format.
pub struct FilterPipelineResult<R> {
    /// Streaming reader yielding the fully unwrapped, decompressed payload.
    pub reader: R,
    /// Ordered vector of all filters stripped from outer to inner.
    pub filters: Vec<FilterKind>,
    /// Terminal identified container or raw archive format.
    pub terminal_format: ArchiveFormat,
}

impl<R> std::fmt::Debug for FilterPipelineResult<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilterPipelineResult")
            .field("filters", &self.filters)
            .field("terminal_format", &self.terminal_format)
            .finish()
    }
}

/// Multi-layer cascaded filter pipeline scheduler.
pub struct FilterPipeline;

impl FilterPipeline {
    /// Maximum recursive filter cascade depth allowed before triggering DoS protection.
    pub const MAX_FILTER_CHAIN_DEPTH: usize = MAX_FILTER_CHAIN_DEPTH;

    /// Automatically unwraps nested filter layers until a container or plain raw data is discovered.
    pub fn unwrap_stream<R: Read + Send + 'static>(
        reader: R,
    ) -> Result<FilterPipelineResult<Box<dyn Read + Send>>, FilterPipelineError> {
        let mut current_reader: Box<dyn Read + Send> = Box::new(reader);
        let mut filters = Vec::new();

        loop {
            // Wrap in lookahead reader to non-destructively peek header
            let mut lookahead = SlidingLookaheadReader::new(current_reader);
            let peek_bytes = lookahead.peek(65536).map_err(|e| FilterPipelineError::Io(e.to_string()))?;

            if peek_bytes.is_empty() {
                return Ok(FilterPipelineResult {
                    reader: Box::new(lookahead),
                    filters,
                    terminal_format: ArchiveFormat::Empty,
                });
            }

            // Sniff for single-stream filter wrapper
            if let Some(kind) = FilterKind::sniff(peek_bytes) {
                if filters.len() >= MAX_FILTER_CHAIN_DEPTH {
                    return Err(FilterPipelineError::ErrTooManyFilters {
                        depth: filters.len() + 1,
                        limit: MAX_FILTER_CHAIN_DEPTH,
                    });
                }

                filters.push(kind);

                // Instantiate corresponding filter
                let next_reader: Box<dyn Read + Send> = match kind {
                    FilterKind::Uuencode => Box::new(UuencodeFilter::new(lookahead)),
                    FilterKind::Rpm => Box::new(RpmLeadFilter::new(lookahead)),
                    FilterKind::Compress => Box::new(CompressFilter::new(lookahead)),
                    FilterKind::Gzip => Box::new(GzipFilter::new(lookahead)),
                    FilterKind::Bzip2 => Box::new(Bzip2Filter::new(lookahead)?),
                    FilterKind::Xz => Box::new(XzFilter::new(lookahead)?),
                    FilterKind::Zstd => Box::new(ZstdFilter::new(lookahead)?),
                    FilterKind::Snappy => Box::new(SnappyFilter::new(lookahead)),
                    FilterKind::Brotli => Box::new(BrotliFilter::new(lookahead)),
                    _ => {
                        return Err(FilterPipelineError::UnsupportedFormat(
                            kind.as_str().to_string(),
                        ));
                    }
                };

                current_reader = next_reader;
                continue;
            }

            // No more single-stream filters detected: determine terminal format
            let sniff_res = FormatSniffer::sniff(peek_bytes);
            let terminal_format = if sniff_res.is_yes() {
                sniff_res.format()
            } else {
                ArchiveFormat::Raw
            };

            return Ok(FilterPipelineResult {
                reader: Box::new(lookahead),
                filters,
                terminal_format,
            });
        }
    }
}
