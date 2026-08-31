// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Multi-layer cascaded streaming filter pipeline engine.
//!
//! Architecture:
//! - `kinds`: `FilterKind` enumeration covering Gzip, Bzip2, Xz, Zstd, Lz4, Lzip, Lzop, Compress, Uuencode, Rpm.
//! - `traits`: `StreamFilter` common trait for zero-copy streaming decoding.
//! - `lookahead`: `SlidingLookaheadReader` non-destructive signature sniffing stream wrapper.
//! - `filters`: Concrete stream filter wrappers for individual decompression formats.
//! - `scheduler`: `FilterPipeline` orchestrator with `MAX_FILTER_CHAIN_DEPTH = 25` anti-DoS limit.

pub mod filters;
pub mod kinds;
pub mod lookahead;
pub mod scheduler;
pub mod traits;

pub use filters::{
    BrotliFilter, Bzip2Filter, CompressFilter, GzipFilter, RpmLeadFilter, SnappyFilter,
    UuencodeFilter, XzFilter, ZstdFilter,
};
pub use kinds::FilterKind;
pub use lookahead::SlidingLookaheadReader;
pub use scheduler::{
    FilterPipeline, FilterPipelineError, FilterPipelineResult, MAX_FILTER_CHAIN_DEPTH,
};
pub use traits::StreamFilter;
