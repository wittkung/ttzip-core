// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Strongly-typed parameters and tier-based quality levels for Google Brotli compression.
//!
//! Brotli compression quality spans from 0 to 11 and is architecturally partitioned
//! into three distinct optimization tiers (RFC 7932 / Google Brotli encoder):
//! - **Fast 1-Pass / 2-Pass (`Fast1Pass`, Q0..=1)**: Ultra-fast greedy LZ77 matching, minimal
//!   state machine overhead, and streaming uncompressed/lightweight block output.
//! - **Balanced (`Balanced`, Q2..=9)**: Standard multi-level hash-chain matching, dynamic
//!   entropy modeling, and command/literal block splitting.
//! - **Optimal (`Optimal`, Q10..=11)**: Full-graph backward reference parsing (Zopfli-style
//!   cost modeling) and high-density Huffman tree construction.

use std::fmt;

use super::error::BrotliError;

/// Graded compression quality tier for Google Brotli.
///
/// Ensures compile-time and runtime validation across the entire 0..=11 spectrum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BrotliQuality {
    /// Ultra-fast 1-pass / 2-pass compression (levels 0..=1).
    Fast1Pass(u32),
    /// Standard balanced multi-level hash-chain compression (levels 2..=9).
    Balanced(u32),
    /// High-quality (HQ) optimal parsing and entropy modeling (levels 10..=11).
    Optimal(u32),
}

impl BrotliQuality {
    /// Minimum allowed Brotli quality level.
    pub const MIN: u32 = 0;
    /// Maximum allowed Brotli quality level.
    pub const MAX: u32 = 11;

    /// Creates a new `BrotliQuality` instance, returning an error if out of bounds (0..=11).
    pub fn new(quality: u32) -> Result<Self, BrotliError> {
        match quality {
            0..=1 => Ok(Self::Fast1Pass(quality)),
            2..=9 => Ok(Self::Balanced(quality)),
            10..=11 => Ok(Self::Optimal(quality)),
            _ => Err(BrotliError::InvalidQuality(quality)),
        }
    }

    /// Creates a `BrotliQuality` instance from raw integer, clamping to `0..=11`.
    #[inline]
    pub fn from_raw(quality: u32) -> Self {
        let clamped = quality.min(Self::MAX);
        match clamped {
            0..=1 => Self::Fast1Pass(clamped),
            2..=9 => Self::Balanced(clamped),
            _ => Self::Optimal(clamped),
        }
    }

    /// Returns the underlying integer quality level (0..=11).
    #[inline]
    pub fn value(&self) -> u32 {
        match *self {
            Self::Fast1Pass(q) | Self::Balanced(q) | Self::Optimal(q) => q,
        }
    }

    /// Returns `true` if this quality falls in the fast 1-pass / 2-pass tier (0..=1).
    #[inline]
    pub fn is_fast(&self) -> bool {
        matches!(self, Self::Fast1Pass(_))
    }

    /// Returns `true` if this quality falls in the balanced tier (2..=9).
    #[inline]
    pub fn is_balanced(&self) -> bool {
        matches!(self, Self::Balanced(_))
    }

    /// Returns `true` if this quality falls in the optimal HQ tier (10..=11).
    #[inline]
    pub fn is_optimal(&self) -> bool {
        matches!(self, Self::Optimal(_))
    }

    /// Returns the human-readable tier identifier name.
    #[inline]
    pub fn tier_name(&self) -> &'static str {
        match self {
            Self::Fast1Pass(_) => "Fast1Pass",
            Self::Balanced(_) => "Balanced",
            Self::Optimal(_) => "Optimal",
        }
    }

    /// Recommended default sliding window size (log2) for this quality level.
    #[inline]
    pub fn default_lgwin(&self) -> u32 {
        match self {
            Self::Fast1Pass(_) => 20,
            Self::Balanced(_) => 22,
            Self::Optimal(_) => 24,
        }
    }

    /// Recommended default block size (log2) for this quality level (0 = auto).
    #[inline]
    pub fn default_lgblock(&self) -> u32 {
        0
    }

    /// Recommended default streaming buffer size in bytes for this quality level.
    #[inline]
    pub fn default_buffer_size(&self) -> usize {
        match self {
            Self::Fast1Pass(_) => 32 * 1024,
            Self::Balanced(_) => 64 * 1024,
            Self::Optimal(_) => 128 * 1024,
        }
    }
}

impl Default for BrotliQuality {
    #[inline]
    fn default() -> Self {
        Self::Balanced(6)
    }
}

impl TryFrom<u32> for BrotliQuality {
    type Error = BrotliError;

    #[inline]
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<BrotliQuality> for u32 {
    #[inline]
    fn from(quality: BrotliQuality) -> Self {
        quality.value()
    }
}

impl fmt::Display for BrotliQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(Q{})", self.tier_name(), self.value())
    }
}

/// Content-aware compression mode hints for Brotli.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum BrotliEncoderMode {
    /// Generic uncompressed/binary payload without domain-specific assumptions (default).
    #[default]
    Generic = 0,
    /// UTF-8 or ASCII text data, optimizing literal context modeling.
    Text = 1,
    /// WOFF 2.0 / SFNT font data.
    Font = 2,
}

impl BrotliEncoderMode {
    /// Maps to lower-level `brotli::enc::backward_references::BrotliEncoderMode`.
    #[inline]
    pub fn to_brotli_mode(self) -> ::brotli::enc::backward_references::BrotliEncoderMode {
        match self {
            Self::Generic => ::brotli::enc::backward_references::BrotliEncoderMode::BROTLI_MODE_GENERIC,
            Self::Text => ::brotli::enc::backward_references::BrotliEncoderMode::BROTLI_MODE_TEXT,
            Self::Font => ::brotli::enc::backward_references::BrotliEncoderMode::BROTLI_MODE_FONT,
        }
    }

    /// Converts from lower-level `brotli::enc::backward_references::BrotliEncoderMode`.
    #[inline]
    pub fn from_brotli_mode(mode: ::brotli::enc::backward_references::BrotliEncoderMode) -> Self {
        match mode {
            ::brotli::enc::backward_references::BrotliEncoderMode::BROTLI_MODE_TEXT => Self::Text,
            ::brotli::enc::backward_references::BrotliEncoderMode::BROTLI_MODE_FONT => Self::Font,
            _ => Self::Generic,
        }
    }
}

impl From<BrotliEncoderMode> for ::brotli::enc::backward_references::BrotliEncoderMode {
    #[inline]
    fn from(mode: BrotliEncoderMode) -> Self {
        mode.to_brotli_mode()
    }
}

/// Complete configuration parameter set for Google Brotli stream compression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrotliEncoderParams {
    /// Compression quality level (0..=11).
    pub quality: u32,
    /// Sliding window size log2 (10..=24, or 0 for auto/default 22).
    pub lgwin: u32,
    /// Block size log2 (16..=24, or 0 for auto).
    pub lgblock: u32,
    /// Content domain mode hint.
    pub mode: BrotliEncoderMode,
    /// Estimated uncompressed stream size hint in bytes (0 if unknown).
    pub size_hint: usize,
    /// Disables UTF-8 2-byte literal context modeling when true.
    pub disable_literal_context_modeling: bool,
    /// Enables large window extension (lgwin up to 30).
    pub large_window: bool,
    /// Internal I/O chunk buffer size in bytes (default 65536).
    pub buffer_size: usize,
}

impl BrotliEncoderParams {
    /// Creates a new parameter configuration with custom basic settings.
    pub fn new(quality: u32, lgwin: u32, lgblock: u32, mode: BrotliEncoderMode) -> Self {
        Self {
            quality: quality.min(11),
            lgwin: if lgwin == 0 { 22 } else { lgwin.clamp(10, 24) },
            lgblock,
            mode,
            size_hint: 0,
            disable_literal_context_modeling: false,
            large_window: false,
            buffer_size: 65536,
        }
    }

    /// Creates standard parameters for a given quality level with validation.
    pub fn with_quality(quality: u32) -> Result<Self, BrotliError> {
        let q = BrotliQuality::new(quality)?;
        Ok(Self {
            quality: q.value(),
            lgwin: q.default_lgwin(),
            lgblock: q.default_lgblock(),
            mode: BrotliEncoderMode::Generic,
            size_hint: 0,
            disable_literal_context_modeling: false,
            large_window: false,
            buffer_size: q.default_buffer_size(),
        })
    }

    /// Fast 1-pass streaming preset (Quality 1, Window 20).
    pub fn fast() -> Self {
        Self {
            quality: 1,
            lgwin: 20,
            lgblock: 0,
            mode: BrotliEncoderMode::Generic,
            size_hint: 0,
            disable_literal_context_modeling: false,
            large_window: false,
            buffer_size: 32 * 1024,
        }
    }

    /// Balanced multi-level hash preset (Quality 6, Window 22).
    pub fn balanced() -> Self {
        Self {
            quality: 6,
            lgwin: 22,
            lgblock: 0,
            mode: BrotliEncoderMode::Generic,
            size_hint: 0,
            disable_literal_context_modeling: false,
            large_window: false,
            buffer_size: 64 * 1024,
        }
    }

    /// Optimal HQ preset (Quality 11, Window 24).
    pub fn optimal() -> Self {
        Self {
            quality: 11,
            lgwin: 24,
            lgblock: 0,
            mode: BrotliEncoderMode::Generic,
            size_hint: 0,
            disable_literal_context_modeling: false,
            large_window: false,
            buffer_size: 128 * 1024,
        }
    }

    /// Validates parameter ranges according to RFC 7932 and Google Brotli specifications.
    pub fn validated(&self) -> Result<Self, BrotliError> {
        if self.quality > 11 {
            return Err(BrotliError::InvalidQuality(self.quality));
        }
        if self.lgwin != 0 && !(10..=30).contains(&self.lgwin) {
            return Err(BrotliError::InvalidWindowBits(self.lgwin as u8));
        }
        if self.lgblock != 0 && !(16..=24).contains(&self.lgblock) {
            return Err(BrotliError::CompressionFailed);
        }
        Ok(*self)
    }

    /// Returns a clamped, safe clone of these parameters.
    pub fn clamped(&self) -> Self {
        Self {
            quality: self.quality.min(11),
            lgwin: if self.lgwin == 0 { 22 } else { self.lgwin.clamp(10, 24) },
            lgblock: if self.lgblock == 0 { 0 } else { self.lgblock.clamp(16, 24) },
            mode: self.mode,
            size_hint: self.size_hint,
            disable_literal_context_modeling: self.disable_literal_context_modeling,
            large_window: self.large_window,
            buffer_size: if self.buffer_size == 0 { 65536 } else { self.buffer_size },
        }
    }

    /// Maps this configuration to the lower-level `brotli::enc::BrotliEncoderParams`.
    pub fn to_brotli_params(&self) -> brotli::enc::BrotliEncoderParams {
        let clamped = self.clamped();
        brotli::enc::BrotliEncoderParams {
            quality: clamped.quality as i32,
            lgwin: clamped.lgwin as i32,
            lgblock: clamped.lgblock as i32,
            mode: clamped.mode.to_brotli_mode(),
            size_hint: clamped.size_hint,
            disable_literal_context_modeling: if clamped.disable_literal_context_modeling {
                1
            } else {
                0
            },
            large_window: clamped.large_window,
            ..Default::default()
        }
    }
}

impl Default for BrotliEncoderParams {
    #[inline]
    fn default() -> Self {
        Self::balanced()
    }
}
