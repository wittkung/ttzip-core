// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Zstandard 4MB In / 4MB Out bounded streaming compression and decompression pipelines.
//!
//! Enforces strict <16MB RSS memory bounds during multi-gigabyte compression/decompression,
//! eliminating high-memory consumption and Out-Of-Memory (OOM) failures.

use super::cctx::ZstdCCtx;
use super::dctx::ZstdDCtx;
use super::types::*;
use crate::types::TTZipStatus;
use std::io::{Read, Write};

/// Bounded pipe chunk buffer size: 4MB In / 4MB Out.
pub const ZSTD_PIPE_BUFFER_SIZE: usize = 4 * 1024 * 1024; // 4MB

fn flush_zstd_end<W: Write>(
    cctx: &mut ZstdCCtx,
    writer: &mut W,
    out_buf: &mut [u8],
) -> Result<u64, TTZipStatus> {
    let mut written: u64 = 0;
    let mut in_struct = ZstdInBuffer {
        src: std::ptr::null(),
        size: 0,
        pos: 0,
    };
    loop {
        let mut out_struct = ZstdOutBuffer {
            dst: out_buf.as_mut_ptr() as *mut libc::c_void,
            capacity: out_buf.len(),
            pos: 0,
        };
        let remaining = cctx.compress_stream(&mut in_struct, &mut out_struct, ZstdEndDirective::End)?;
        if out_struct.pos > 0 {
            writer
                .write_all(&out_buf[..out_struct.pos])
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            written += out_struct.pos as u64;
        }
        if remaining == 0 {
            break;
        }
    }
    Ok(written)
}

fn compress_chunk_to_writer<W: Write>(
    cctx: &mut ZstdCCtx,
    writer: &mut W,
    in_slice: &[u8],
    out_buf: &mut [u8],
) -> Result<u64, TTZipStatus> {
    let mut written: u64 = 0;
    let mut in_struct = ZstdInBuffer {
        src: in_slice.as_ptr() as *const libc::c_void,
        size: in_slice.len(),
        pos: 0,
    };
    while in_struct.pos < in_struct.size {
        let mut out_struct = ZstdOutBuffer {
            dst: out_buf.as_mut_ptr() as *mut libc::c_void,
            capacity: out_buf.len(),
            pos: 0,
        };
        let _ = cctx.compress_stream(&mut in_struct, &mut out_struct, ZstdEndDirective::Continue)?;
        if out_struct.pos > 0 {
            writer
                .write_all(&out_buf[..out_struct.pos])
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            written += out_struct.pos as u64;
        }
    }
    Ok(written)
}

/// Streams uncompressed data from `reader`, compresses it via Zstandard with bounded memory,
/// and writes compressed frames to `writer`.
pub fn zstd_compress_stream_pipe<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    config: &ZstdConfig,
    progress_callback: Option<&dyn Fn(u64, u64) -> bool>,
) -> Result<(u64, u64), TTZipStatus> {
    let mut cctx = ZstdCCtx::new()?;
    cctx.apply_config(config)?;

    let mut in_buf = vec![0u8; ZSTD_PIPE_BUFFER_SIZE];
    let mut out_buf = vec![0u8; ZSTD_PIPE_BUFFER_SIZE];

    let mut total_read: u64 = 0;
    let mut total_written: u64 = 0;

    loop {
        let bytes_read = reader.read(&mut in_buf).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        if bytes_read == 0 {
            total_written += flush_zstd_end(&mut cctx, writer, &mut out_buf)?;
            break;
        }

        total_read += bytes_read as u64;
        total_written +=
            compress_chunk_to_writer(&mut cctx, writer, &in_buf[..bytes_read], &mut out_buf)?;

        if let Some(cb) = progress_callback {
            if !cb(total_read, total_written) {
                return Err(TTZipStatus::Cancelled);
            }
        }
    }

    writer.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok((total_read, total_written))
}

fn decompress_chunk_to_writer<W: Write>(
    dctx: &mut ZstdDCtx,
    writer: &mut W,
    in_slice: &[u8],
    out_buf: &mut [u8],
) -> Result<(u64, usize), TTZipStatus> {
    let mut written: u64 = 0;
    let mut last_ret: usize = 0;
    let mut in_struct = ZstdInBuffer {
        src: in_slice.as_ptr() as *const libc::c_void,
        size: in_slice.len(),
        pos: 0,
    };

    while in_struct.pos < in_struct.size {
        let mut out_struct = ZstdOutBuffer {
            dst: out_buf.as_mut_ptr() as *mut libc::c_void,
            capacity: out_buf.len(),
            pos: 0,
        };
        last_ret = dctx.decompress_stream(&mut in_struct, &mut out_struct)?;
        if out_struct.pos > 0 {
            writer
                .write_all(&out_buf[..out_struct.pos])
                .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
            written += out_struct.pos as u64;
        }

        while in_struct.pos == in_struct.size
            && out_struct.pos == out_struct.capacity
            && last_ret > 0
        {
            out_struct.pos = 0;
            last_ret = dctx.decompress_stream(&mut in_struct, &mut out_struct)?;
            if out_struct.pos == 0 {
                break;
            }
            writer
                .write_all(&out_buf[..out_struct.pos])
                .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
            written += out_struct.pos as u64;
        }
    }

    Ok((written, last_ret))
}

/// Streams compressed data from `reader`, decompresses via Zstandard with bounded memory,
/// and writes uncompressed frames to `writer`.
pub fn zstd_decompress_stream_pipe<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    progress_callback: Option<&dyn Fn(u64, u64) -> bool>,
) -> Result<(u64, u64), TTZipStatus> {
    let mut dctx = ZstdDCtx::new()?;

    let mut in_buf = vec![0u8; ZSTD_PIPE_BUFFER_SIZE];
    let mut out_buf = vec![0u8; ZSTD_PIPE_BUFFER_SIZE];

    let mut total_read: u64 = 0;
    let mut total_written: u64 = 0;
    let mut last_ret: usize = 0;

    loop {
        let bytes_read = reader.read(&mut in_buf).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        if bytes_read == 0 {
            break;
        }

        total_read += bytes_read as u64;
        let (chunk_written, ret) =
            decompress_chunk_to_writer(&mut dctx, writer, &in_buf[..bytes_read], &mut out_buf)?;
        total_written += chunk_written;
        last_ret = ret;

        if let Some(cb) = progress_callback {
            if !cb(total_read, total_written) {
                return Err(TTZipStatus::Cancelled);
            }
        }
    }

    if total_read == 0 || last_ret != 0 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    writer.flush().map_err(|_| TTZipStatus::ErrExtractionFailed)?;
    Ok((total_read, total_written))
}
