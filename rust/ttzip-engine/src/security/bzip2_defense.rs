// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Bzip2 6-layer defense guard and decompression bomb mitigation engine.

use crate::types::TTZipStatus;

/// Security limits for Bzip2 decompression.
#[derive(Debug, Clone, Copy)]
pub struct Bzip2SecurityLimits {
    /// Maximum allowed uncompressed output bytes (default 1GB).
    pub max_output_size: usize,
    /// Maximum decompression expansion ratio (e.g., 200:1).
    pub max_expansion_ratio: usize,
    /// Maximum allowed block size (900KB).
    pub max_block_size: usize,
    /// Maximum Huffman code depth (20 bits).
    pub max_huffman_depth: usize,
}

impl Default for Bzip2SecurityLimits {
    fn default() -> Self {
        Self {
            max_output_size: 1024 * 1024 * 1024, // 1 GB
            max_expansion_ratio: 250,            // 250:1
            max_block_size: 900_000,             // 900 KB
            max_huffman_depth: 20,
        }
    }
}

/// 6-Layer security guard and quota tracker for Bzip2 decompression.
#[derive(Debug)]
pub struct Bzip2DefenseGuard {
    limits: Bzip2SecurityLimits,
    total_bytes_produced: usize,
    compressed_bytes_consumed: usize,
}

impl Bzip2DefenseGuard {
    /// Creates a new defense guard with custom or default limits.
    pub fn new(limits: Bzip2SecurityLimits) -> Self {
        Self {
            limits,
            total_bytes_produced: 0,
            compressed_bytes_consumed: 0,
        }
    }

    /// Verifies 4-byte stream magic 'BZh1'..'BZh9'.
    #[inline]
    pub fn verify_stream_header(&self, header: &[u8]) -> Result<u8, TTZipStatus> {
        if header.len() < 4 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        if header[0] != b'B' || header[1] != b'Z' || header[2] != b'h' {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let level = header[3];
        if !(b'1'..=b'9').contains(&level) {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        Ok(level - b'0')
    }

    /// Validates BWT block invariants before matrix reconstruction.
    #[inline]
    pub fn validate_bwt_invariants(&self, orig_ptr: usize, nblock: usize) -> Result<(), TTZipStatus> {
        if nblock == 0 {
            return Ok(());
        }
        if orig_ptr >= nblock {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        if nblock > self.limits.max_block_size {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        Ok(())
    }

    /// Records produced bytes and checks against output limits and bomb thresholds.
    pub fn track_output(&mut self, bytes_produced: usize) -> Result<(), TTZipStatus> {
        self.total_bytes_produced = self
            .total_bytes_produced
            .checked_add(bytes_produced)
            .ok_or(TTZipStatus::ErrSecurityViolation)?;

        if self.total_bytes_produced > self.limits.max_output_size {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        if self.compressed_bytes_consumed > 1024 {
            let ratio = self.total_bytes_produced / self.compressed_bytes_consumed;
            if ratio > self.limits.max_expansion_ratio {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }

        Ok(())
    }

    /// Records consumed compressed input bytes.
    #[inline]
    pub fn track_input(&mut self, bytes_consumed: usize) {
        self.compressed_bytes_consumed = self.compressed_bytes_consumed.saturating_add(bytes_consumed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bzip2_defense_header() {
        let guard = Bzip2DefenseGuard::new(Bzip2SecurityLimits::default());
        assert_eq!(guard.verify_stream_header(b"BZh9").unwrap(), 9);
        assert!(guard.verify_stream_header(b"BZh0").is_err());
        assert!(guard.verify_stream_header(b"PK\x03\x04").is_err());
    }

    #[test]
    fn test_bzip2_defense_bomb_trigger() {
        let limits = Bzip2SecurityLimits {
            max_output_size: 10_000,
            max_expansion_ratio: 10,
            ..Default::default()
        };
        let mut guard = Bzip2DefenseGuard::new(limits);
        guard.track_input(2000);
        assert!(guard.track_output(50_000).is_err());
    }
}
