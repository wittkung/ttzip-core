// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe RAII wrapper and streaming context for Facebook `Zstandard` (zstd).
//!
//! Supports multi-threaded parallel compression (`nb_workers`), Long Distance Matching (LDM),
//! custom window/overlap logs, zero-copy in-memory buffer operations, and 4MB In / 4MB Out bounded pipes.

pub mod cctx;
pub mod dctx;
pub mod dict;
mod entropy;
pub mod pipe;
pub mod stream;
pub mod types;

#[cfg(test)]
mod tests;

pub use cctx::{
    with_thread_local_zstd_cctx, zstd_compress, zstd_compress_advanced, zstd_compress_bound,
    zstd_compress_ldm, ZstdCCtx,
};
pub use dctx::{
    with_thread_local_zstd_dctx, zstd_decompress, zstd_get_decompressed_size, ZstdDCtx,
};
pub use dict::*;
pub use entropy::{
    fse_compress, fse_compress_bound, fse_decompress, huf0_compress1x, huf0_compress4x,
    huf0_compress_bound, huf0_decompress1x, huf0_decompress4x,
};
pub use pipe::{zstd_compress_stream_pipe, zstd_decompress_stream_pipe, ZSTD_PIPE_BUFFER_SIZE};
pub use stream::{ZstdStreamReader, ZstdStreamWriter, ZSTD_STREAM_BUFFER_SIZE};
pub use types::{
    ZstdCParameter, ZstdConfig, ZstdDParameter, ZstdEndDirective, ZstdInBuffer, ZstdOutBuffer,
};
pub use crate::codecs::zstd_seekable::{
    SeekFrameInfo, SeekTableDecoder, SeekTableEncoder, SeekTableEntry, SeekableError,
    ZstdSeekableReader, ZstdSeekableWriter, DEFAULT_SEEKABLE_FRAME_SIZE, SEEKABLE_FOOTER_SIZE,
    SEEKABLE_MAGIC_NUMBER, SEEK_TABLE_FLAG_CHECKSUM, SKIPPABLE_HEADER_SIZE,
    SKIPPABLE_MAGIC_NUMBER,
};

