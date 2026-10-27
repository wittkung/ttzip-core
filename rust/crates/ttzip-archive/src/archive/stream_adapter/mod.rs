// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Stream adapter bridging `std::io::{Read, Write, Seek}` to `libarchive` C callbacks.
//!
//! Enforces:
//! 1. Memory bound: $\le 64\text{MB}$ RSS resident memory limit with micro-buffering (64KB - 2MB).
//! 2. Error safety: Strict deterministic Result and C-ABI code return without unwind landing pads.
//! 3. Pinned callback state (`Pin<Box<StreamReaderState<R>>>`) ensuring stable raw pointers.

mod container_bridge;
pub mod read;
pub mod seek;
mod sevenz_selected;
mod single_stream;
pub mod write;
mod xar_wim_bridge;

#[cfg(test)]
mod tests;

pub use container_bridge::*;
pub use read::*;
pub use seek::*;
pub use single_stream::*;
pub use write::*;

/// Libarchive status constants.
pub const ARCHIVE_OK: libc::c_int = 0;
pub const ARCHIVE_EOF: libc::c_int = 1;
pub const ARCHIVE_RETRY: libc::c_int = -10;
pub const ARCHIVE_WARN: libc::c_int = -20;
pub const ARCHIVE_FAILED: libc::c_int = -25;
pub const ARCHIVE_FATAL: libc::c_int = -30;

/// Default micro-buffer size: 64 KB.
pub const DEFAULT_STREAM_BUFFER_SIZE: usize = 64 * 1024;
/// Maximum allowable buffer capacity per stream: 2 MB.
pub const MAX_STREAM_BUFFER_SIZE: usize = 2 * 1024 * 1024;
/// Task hard limit for resident memory allocation: 64 MB.
pub const MAX_RESIDENT_MEMORY_MB: usize = 64;

/// Stream pipeline operating mode conforming to `contracts/ttzip_stream_contract.json`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipStreamMode {
    ReadSequential,
    ReadSeekable,
    WriteSequential,
    WriteChunkedArena,
}

/// State snapshot for stream progress monitoring and contract compliance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamStateSnapshot {
    pub bytes_consumed: u64,
    pub bytes_written: u64,
    pub is_eof: bool,
    pub has_error: bool,
    pub last_error_msg: Option<String>,
}
