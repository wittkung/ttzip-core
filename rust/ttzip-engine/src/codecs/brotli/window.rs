// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Brotli sliding window bits (WBITS) parser and maximum distance calculator (RFC 7932 & RFC 9841).

use super::bit_reader::BrotliBitReader;
use super::error::BrotliError;

/// Distance subtraction gap defined in RFC 7932 Section 9.1 (`1 << WBITS - 16`).
pub const BROTLI_WINDOW_GAP: usize = 16;

/// Minimum valid sliding window bits exponent (10 = 1 KiB - 16 B).
pub const BROTLI_MIN_WINDOW_BITS: u8 = 10;

/// Maximum sliding window bits exponent in standard RFC 7932 (24 = 16 MiB - 16 B).
pub const BROTLI_MAX_WINDOW_BITS: u8 = 24;

/// Maximum sliding window bits exponent in Large Window extension (30 = 1 GiB - 16 B).
pub const BROTLI_LARGE_MAX_WINDOW_BITS: u8 = 30;

/// Brotli sliding window configuration and maximum reference distance geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrotliWindow {
    /// Sliding window exponent (10..=24 standard RFC 7932, up to 30 in Large Window extension).
    pub window_bits: u8,
    /// Maximum backward reference distance `(1 << window_bits) - BROTLI_WINDOW_GAP`.
    pub max_distance: usize,
}

impl BrotliWindow {
    /// Constructs a `BrotliWindow` from explicit `window_bits` exponent.
    ///
    /// # Errors
    /// Returns `BrotliError::InvalidWindowBits` if `window_bits` is out of allowable range.
    pub fn new(window_bits: u8, allow_large_window: bool) -> Result<Self, BrotliError> {
        let max_bits = if allow_large_window {
            BROTLI_LARGE_MAX_WINDOW_BITS
        } else {
            BROTLI_MAX_WINDOW_BITS
        };

        if !(BROTLI_MIN_WINDOW_BITS..=max_bits).contains(&window_bits) {
            return Err(BrotliError::InvalidWindowBits(window_bits));
        }

        let max_distance = (1usize << window_bits) - BROTLI_WINDOW_GAP;
        Ok(Self {
            window_bits,
            max_distance,
        })
    }

    /// Parses the variable-length WBITS prefix from a `BrotliBitReader` per RFC 7932 Section 9.1.
    ///
    /// Variable-length bitstream encodings:
    /// - 1-bit `0` $\to$ WBITS = 16
    /// - 4-bit `1` + `n (1..=7)` $\to$ WBITS = `17 + n` (18..=24)
    /// - 7-bit `1` + `000` + `n (0..=7)`:
    ///   - `n = 0` $\to$ WBITS = 17
    ///   - `n = 1` $\to$ Large Window extension (`00010001` pattern, 14-bit total):
    ///     - Next 1-bit must be `0` (extra bit)
    ///     - Next 6-bit `wbits` in range 10..=30
    ///   - `n >= 2` $\to$ WBITS = `8 + n` (10..=15)
    pub fn parse_window_bits(
        br: &mut BrotliBitReader<'_>,
        allow_large_window: bool,
    ) -> Result<Self, BrotliError> {
        let bit0 = br.read_bits(1)?;
        if bit0 == 0 {
            return Self::new(16, allow_large_window);
        }

        let n = br.read_bits(3)?;
        if n > 0 {
            let window_bits = 17 + n as u8;
            return Self::new(window_bits, allow_large_window);
        }

        // n == 0: bit0 was 1, next 3 bits were 000
        let m = br.read_bits(3)?;
        if m == 0 {
            return Self::new(17, allow_large_window);
        }

        if m == 1 {
            // Large Window extension pattern (RFC 9841)
            if !allow_large_window {
                return Err(BrotliError::InvalidWindowBits(1));
            }
            let extra_bit = br.read_bits(1)?;
            if extra_bit != 0 {
                return Err(BrotliError::InvalidWindowBits(0));
            }
            let wbits = br.read_bits(6)? as u8;
            if !(BROTLI_MIN_WINDOW_BITS..=BROTLI_LARGE_MAX_WINDOW_BITS).contains(&wbits) {
                return Err(BrotliError::InvalidWindowBits(wbits));
            }
            return Self::new(wbits, true);
        }

        // m in 2..=7 maps to window_bits 10..=15
        let window_bits = 8 + m as u8;
        Self::new(window_bits, allow_large_window)
    }
}
