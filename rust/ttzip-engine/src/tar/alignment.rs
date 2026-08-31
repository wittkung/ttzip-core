// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 512-Byte Sector Alignment & 2x512B Zero EOF Detector for TAR Archives.
//!
//! Provides mathematically exact 512-byte sector padding calculations,
//! hardware/SIMD-accelerated 64-bit word zero-block detection, and a streaming
//! End-of-Archive (2x512B) state machine with standard and concatenated archive support.

use std::io::{self, Write};

/// Standard TAR sector/block size in bytes (512 bytes).
pub const TAR_SECTOR_SIZE: usize = 512;

/// Standard TAR sector size as `u64` constant.
pub const TAR_SECTOR_SIZE_U64: u64 = 512;

/// Standard TAR End-of-Archive marker size (2 consecutive 512-byte zero blocks = 1024 bytes).
pub const TAR_EOF_SIZE: usize = TAR_SECTOR_SIZE * 2;

/// Standard TAR End-of-Archive size as `u64` constant.
pub const TAR_EOF_SIZE_U64: u64 = TAR_EOF_SIZE as u64;

/// Computes the trailing zero padding byte count required to align `size` to a 512-byte sector boundary.
///
/// Mathematical formula: `(512 - (size % 512)) % 512`.
///
/// # Examples
/// ```
/// use ttzip_engine::tar::pad_to_512;
///
/// assert_eq!(pad_to_512(0), 0);
/// assert_eq!(pad_to_512(1), 511);
/// assert_eq!(pad_to_512(511), 1);
/// assert_eq!(pad_to_512(512), 0);
/// assert_eq!(pad_to_512(513), 511);
/// assert_eq!(pad_to_512(1024), 0);
/// assert_eq!(pad_to_512(4096), 0);
/// ```
#[inline]
pub const fn pad_to_512(size: u64) -> u64 {
    (TAR_SECTOR_SIZE_U64 - (size % TAR_SECTOR_SIZE_U64)) % TAR_SECTOR_SIZE_U64
}

/// Computes the total physical size (including 512-byte alignment padding) occupied by a payload.
///
/// Returns `size + pad_to_512(size)` with overflow saturation.
///
/// # Examples
/// ```
/// use ttzip_engine::tar::aligned_size_512;
///
/// assert_eq!(aligned_size_512(0), 0);
/// assert_eq!(aligned_size_512(1), 512);
/// assert_eq!(aligned_size_512(511), 512);
/// assert_eq!(aligned_size_512(512), 512);
/// assert_eq!(aligned_size_512(513), 1024);
/// assert_eq!(aligned_size_512(1024), 1024);
/// assert_eq!(aligned_size_512(4096), 4096);
/// ```
#[inline]
pub const fn aligned_size_512(size: u64) -> u64 {
    let pad = pad_to_512(size);
    match size.checked_add(pad) {
        Some(v) => v,
        None => u64::MAX,
    }
}

/// Checks whether an entire 512-byte TAR block consists solely of zero bytes.
///
/// Implemented using 64-bit wide `u64` word comparisons (64 iterations of 8 bytes = 512 bytes)
/// with bitwise OR accumulation, allowing LLVM to emit auto-vectorized SIMD instructions (AVX2/NEON)
/// and execute zero branches in the inner loop.
#[inline]
pub fn is_all_zeros(block: &[u8; TAR_SECTOR_SIZE]) -> bool {
    let mut acc: u64 = 0;
    for chunk in block.chunks_exact(8) {
        // Guaranteed to be exact 8-byte chunk; optimized away by compiler
        let word = u64::from_ne_bytes(match chunk.try_into() {
            Ok(arr) => arr,
            Err(_) => return false,
        });
        acc |= word;
    }
    acc == 0
}

/// Checks whether an arbitrary byte slice consists solely of zero bytes.
///
/// Processes 8-byte aligned words followed by trailing single-byte checks.
#[inline]
pub fn is_slice_all_zeros(slice: &[u8]) -> bool {
    let mut chunks = slice.chunks_exact(8);
    let mut acc: u64 = 0;
    for chunk in &mut chunks {
        let word = u64::from_ne_bytes(match chunk.try_into() {
            Ok(arr) => arr,
            Err(_) => return false,
        });
        acc |= word;
    }
    if acc != 0 {
        return false;
    }
    for &b in chunks.remainder() {
        if b != 0 {
            return false;
        }
    }
    true
}

/// Status emitted by `EofBlockDetector` when consuming 512-byte blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TarEofStatus {
    /// Active payload/header block consumed, archive continuation expected.
    Continue,
    /// Standard TAR End-of-Archive reached (2 consecutive 512-byte zero blocks / 1024 bytes).
    EndOfArchive,
    /// Non-standard single 512-byte zero block detected before unexpected stream termination.
    TruncatedZero,
    /// Ignored intermediate zero block in multi-archive concatenation stream (`ignore_zeros` mode).
    IgnoredZero,
}

/// Streaming End-of-Archive (EOF) detector state machine for TAR streams.
///
/// Standard TAR archives terminate with at least two consecutive 512-byte blocks of binary zeroes (1024 bytes).
/// `EofBlockDetector` tracks sequential zero blocks and handles:
/// 1. Standard 2x512B End-of-Archive detection (`TarEofStatus::EndOfArchive`).
/// 2. Concatenated TAR archives where intermediate zero blocks should be skipped (`ignore_zeros = true`).
/// 3. Non-standard single-zero-block premature stream truncation (`TarEofStatus::TruncatedZero`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EofBlockDetector {
    consecutive_zero_blocks: usize,
    ignore_zeros: bool,
    eof_reached: bool,
}

impl Default for EofBlockDetector {
    fn default() -> Self {
        Self::new(false)
    }
}

impl EofBlockDetector {
    /// Creates a new `EofBlockDetector`.
    ///
    /// # Arguments
    /// * `ignore_zeros` - When `true`, intermediate zero blocks are treated as `IgnoredZero`
    ///   instead of terminating the stream at 2 consecutive blocks (useful for concatenated archives).
    pub fn new(ignore_zeros: bool) -> Self {
        Self {
            consecutive_zero_blocks: 0,
            ignore_zeros,
            eof_reached: false,
        }
    }

    /// Sets whether to ignore zero blocks for multi-stream concatenation.
    pub fn with_ignore_zeros(mut self, ignore_zeros: bool) -> Self {
        self.ignore_zeros = ignore_zeros;
        self
    }

    /// Feeds a 512-byte sector into the EOF detector and returns the resulting status.
    pub fn feed_block(&mut self, block: &[u8; TAR_SECTOR_SIZE]) -> TarEofStatus {
        if is_all_zeros(block) {
            self.consecutive_zero_blocks += 1;
            if self.ignore_zeros {
                TarEofStatus::IgnoredZero
            } else if self.consecutive_zero_blocks >= 2 {
                self.eof_reached = true;
                TarEofStatus::EndOfArchive
            } else {
                // First zero block encountered; awaiting potential second zero block
                TarEofStatus::Continue
            }
        } else {
            self.consecutive_zero_blocks = 0;
            TarEofStatus::Continue
        }
    }

    /// Evaluates stream status when the underlying physical reader reaches EOF or terminates.
    ///
    /// If the stream ended immediately following a single 512-byte zero block without providing
    /// the second zero block, returns `TarEofStatus::TruncatedZero` for graceful recovery.
    /// If 2 or more zero blocks were consumed, returns `TarEofStatus::EndOfArchive`.
    pub fn on_stream_end(&self) -> TarEofStatus {
        if self.eof_reached || self.consecutive_zero_blocks >= 2 {
            TarEofStatus::EndOfArchive
        } else if self.consecutive_zero_blocks == 1 {
            TarEofStatus::TruncatedZero
        } else {
            TarEofStatus::Continue
        }
    }

    /// Returns the number of consecutive 512-byte zero blocks currently observed.
    #[inline]
    pub fn consecutive_zero_blocks(&self) -> usize {
        self.consecutive_zero_blocks
    }

    /// Returns `true` if standard 2x512B End-of-Archive has been reached.
    #[inline]
    pub fn is_eof(&self) -> bool {
        self.eof_reached
    }

    /// Returns whether `ignore_zeros` mode is active.
    #[inline]
    pub fn ignore_zeros(&self) -> bool {
        self.ignore_zeros
    }

    /// Resets the detector state to clean initial values.
    pub fn reset(&mut self) {
        self.consecutive_zero_blocks = 0;
        self.eof_reached = false;
    }
}

/// Writes 2 consecutive 512-byte zero blocks (total 1024 bytes) marking the standard End-of-Archive.
///
/// Returns the number of bytes written (`1024`) on success.
///
/// # Arguments
/// * `writer` - Target output stream implementing `std::io::Write`.
pub fn write_eof_blocks<W: Write>(writer: &mut W) -> io::Result<usize> {
    static ZERO_EOF_BLOCKS: [u8; TAR_EOF_SIZE] = [0u8; TAR_EOF_SIZE];
    writer.write_all(&ZERO_EOF_BLOCKS)?;
    Ok(TAR_EOF_SIZE)
}
