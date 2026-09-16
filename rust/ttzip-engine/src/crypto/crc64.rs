// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-performance CRC-64 (ECMA-182) computation engine.
//!
//! Provides zero-allocation, precomputed lookup table and slicing-by-8 acceleration.

const POLY: u64 = 0x42F0_E1EB_A9EA_3693;

/// Precomputed CRC64-ECMA Slicing-by-8 table.
static CRC64_TABLE8: [[u64; 256]; 8] = {
    let mut tables = [[0u64; 256]; 8];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u64;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ POLY;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        tables[0][i] = crc;
        i += 1;
    }

    let mut k = 1;
    while k < 8 {
        let mut i = 0;
        while i < 256 {
            let prev = tables[k - 1][i];
            tables[k][i] = tables[0][(prev as u8) as usize] ^ (prev >> 8);
            i += 1;
        }
        k += 1;
    }
    tables
};

/// Computes CRC-64 ECMA checksum with initial seed using Slicing-by-8.
#[inline]
pub fn crc64(data: &[u8], seed: u64) -> u64 {
    let mut crc = !seed;
    let chunks = data.chunks_exact(8);
    let remainder = chunks.remainder();

    for chunk in chunks {
        let word = u64::from_le_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3],
            chunk[4], chunk[5], chunk[6], chunk[7],
        ]);
        let term = crc ^ word;
        crc = CRC64_TABLE8[7][(term & 0xFF) as usize]
            ^ CRC64_TABLE8[6][((term >> 8) & 0xFF) as usize]
            ^ CRC64_TABLE8[5][((term >> 16) & 0xFF) as usize]
            ^ CRC64_TABLE8[4][((term >> 24) & 0xFF) as usize]
            ^ CRC64_TABLE8[3][((term >> 32) & 0xFF) as usize]
            ^ CRC64_TABLE8[2][((term >> 40) & 0xFF) as usize]
            ^ CRC64_TABLE8[1][((term >> 48) & 0xFF) as usize]
            ^ CRC64_TABLE8[0][(term >> 56) as usize];
    }

    for &byte in remainder {
        let idx = ((crc as u8) ^ byte) as usize;
        crc = CRC64_TABLE8[0][idx] ^ (crc >> 8);
    }
    !crc
}

/// Fast alias for CRC-64 computation.
#[inline]
pub fn crc64_fast(data: &[u8]) -> u64 {
    crc64(data, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc64_empty() {
        assert_eq!(crc64(b"", 0), 0);
    }

    #[test]
    fn test_crc64_known_vector() {
        let val = crc64_fast(b"123456789");
        assert_eq!(val, 13288015728624077471u64);
    }
}
