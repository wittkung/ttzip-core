// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Ultra-fast hardware-accelerated and L1D-optimized Adler-32 and CRC-32 checksum engine.
//!
//! Features:
//! - **Adler-32 (RFC 1950)**:
//!   - 5552-byte chunk deferred modulo arithmetic (`ADLER32_MAX_CHUNK = 5552`).
//!   - 4-byte unrolled instruction-level parallelism (ILP) accumulation.
//!   - Hardware vector acceleration (ARM64 NEON dotprod / universal vector pipeline).
//!   - Constant-time block combination (`combine_adler32`).
//! - **CRC-32 (IEEE 802.3 / RFC 1952 / Gzip)**:
//!   - Slice-by-8 8x256 precomputed lookup table (8KB, 100% L1D cache-friendly).
//!   - 8-byte / 4-byte aligned unrolled memory loads.
//!   - Hardware vector polynomial folding (ARM64 PMULL / ACLE).
//!   - Logarithmic GF(2) matrix exponentiation combination (`combine_crc32`).

use crate::crypto::{adler32_fast, crc32_fast};

/// The Adler-32 divisor ("base") constant (largest prime smaller than 65536).
pub const ADLER32_DIVISOR: u32 = 65521;

/// The maximum number of bytes that can be processed without the possibility
/// of `s2` overflowing when accumulated in an unsigned 32-bit integer.
pub const ADLER32_MAX_CHUNK: usize = 5552;

// ============================================================================
// 1. Adler-32 Checksum Algorithm (RFC 1950)
// ============================================================================

/// Computes the pure scalar 4-byte unrolled Adler-32 checksum with 5552-byte deferred modulo.
///
/// This provides maximum portable performance with high instruction-level parallelism.
pub fn adler32_scalar(adler: u32, data: &[u8]) -> u32 {
    if data.is_empty() {
        return adler;
    }

    let mut s1 = adler & 0xFFFF;
    let mut s2 = adler >> 16;
    let mut p = data.as_ptr();
    let mut len = data.len();

    while len > 0 {
        let n = len.min(ADLER32_MAX_CHUNK & !3);
        len -= n;
        let mut cur_n = n;

        if cur_n >= 4 {
            let mut s1_sum = 0u32;
            let mut b0_sum = 0u32;
            let mut b1_sum = 0u32;
            let mut b2_sum = 0u32;
            let mut b3_sum = 0u32;

            while cur_n >= 4 {
                s1_sum = s1_sum.wrapping_add(s1);
                let p0 = unsafe { *p as u32 };
                let p1 = unsafe { *p.add(1) as u32 };
                let p2 = unsafe { *p.add(2) as u32 };
                let p3 = unsafe { *p.add(3) as u32 };

                s1 = s1.wrapping_add(p0 + p1 + p2 + p3);
                b0_sum = b0_sum.wrapping_add(p0);
                b1_sum = b1_sum.wrapping_add(p1);
                b2_sum = b2_sum.wrapping_add(p2);
                b3_sum = b3_sum.wrapping_add(p3);

                unsafe {
                    p = p.add(4);
                }
                cur_n -= 4;
            }

            s2 = s2.wrapping_add(
                4 * (s1_sum.wrapping_add(b0_sum))
                    + 3 * b1_sum
                    + 2 * b2_sum
                    + b3_sum,
            );
        }

        while cur_n > 0 {
            let b = unsafe { *p as u32 };
            s1 = s1.wrapping_add(b);
            s2 = s2.wrapping_add(s1);
            unsafe {
                p = p.add(1);
            }
            cur_n -= 1;
        }

        s1 %= ADLER32_DIVISOR;
        s2 %= ADLER32_DIVISOR;
    }

    (s2 << 16) | s1
}

/// Incrementally updates an Adler-32 checksum with hardware acceleration where available.
///
/// If `data` is empty, returns `adler` unmodified.
#[inline]
pub fn adler32_update(adler: u32, data: &[u8]) -> u32 {
    if data.is_empty() {
        return adler;
    }
    adler32_fast(adler, data)
}

/// Computes the initial Adler-32 checksum of `data` starting from the standard initial value `1`.
#[inline]
pub fn adler32_compute(data: &[u8]) -> u32 {
    adler32_update(1, data)
}

/// Combines two Adler-32 checksums into the checksum of their concatenated data in $O(1)$ time.
///
/// `adler1` is the checksum of the first block, `adler2` is the checksum of the second block,
/// and `len2` is the byte length of the second block.
pub fn combine_adler32(adler1: u32, adler2: u32, len2: usize) -> u32 {
    if len2 == 0 {
        return adler1;
    }

    let rem = (len2 % (ADLER32_DIVISOR as usize)) as u32;
    let mut s1 = adler1 & 0xFFFF;
    let mut s2 = (rem * s1) % ADLER32_DIVISOR;
    s1 = (s1 + (adler2 & 0xFFFF) + ADLER32_DIVISOR - 1) % ADLER32_DIVISOR;
    s2 = (s2 + (adler1 >> 16) + (adler2 >> 16) + ADLER32_DIVISOR - rem) % ADLER32_DIVISOR;

    (s2 << 16) | s1
}

// ============================================================================
// 2. CRC-32 (IEEE 802.3) Slice-by-8 Engine
// ============================================================================

/// Compile-time generation of the 8x256 (8KB) Slice-by-8 lookup table for IEEE 802.3 CRC-32.
const fn make_slice8_tables() -> [[u32; 256]; 8] {
    let mut tables = [[0u32; 256]; 8];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            crc = if (crc & 1) != 0 {
                (crc >> 1) ^ 0xEDB88320
            } else {
                crc >> 1
            };
            j += 1;
        }
        tables[0][i] = crc;
        i += 1;
    }

    let mut slice = 1;
    while slice < 8 {
        let mut idx = 0;
        while idx < 256 {
            let prev = tables[slice - 1][idx];
            tables[slice][idx] = (prev >> 8) ^ tables[0][(prev & 0xFF) as usize];
            idx += 1;
        }
        slice += 1;
    }
    tables
}

/// Precomputed 8KB Slice-by-8 lookup tables (8 x 256 entries).
pub static CRC32_SLICE8_TABLE: [[u32; 256]; 8] = make_slice8_tables();

/// Computes CRC-32 using the portable Slice-by-8 algorithm with 4-byte/8-byte aligned fast loads.
pub fn crc32_slice8(mut crc: u32, data: &[u8]) -> u32 {
    if data.is_empty() {
        return crc;
    }

    crc = !crc;
    let mut p = data.as_ptr();
    let mut len = data.len();

    // Fast alignment to 8-byte boundary
    while ((p as usize) & 7) != 0 && len > 0 {
        let b = unsafe { *p };
        crc = (crc >> 8) ^ CRC32_SLICE8_TABLE[0][((crc ^ (b as u32)) & 0xFF) as usize];
        unsafe {
            p = p.add(1);
        }
        len -= 1;
    }

    // Main 8-byte chunk loop using slice-by-8
    while len >= 8 {
        let one = crc ^ unsafe { (p as *const u32).read_unaligned() };
        let two = unsafe { (p.add(4) as *const u32).read_unaligned() };

        let b0 = (one & 0xFF) as usize;
        let b1 = ((one >> 8) & 0xFF) as usize;
        let b2 = ((one >> 16) & 0xFF) as usize;
        let b3 = ((one >> 24) & 0xFF) as usize;

        let b4 = (two & 0xFF) as usize;
        let b5 = ((two >> 8) & 0xFF) as usize;
        let b6 = ((two >> 16) & 0xFF) as usize;
        let b7 = ((two >> 24) & 0xFF) as usize;

        crc = CRC32_SLICE8_TABLE[7][b0]
            ^ CRC32_SLICE8_TABLE[6][b1]
            ^ CRC32_SLICE8_TABLE[5][b2]
            ^ CRC32_SLICE8_TABLE[4][b3]
            ^ CRC32_SLICE8_TABLE[3][b4]
            ^ CRC32_SLICE8_TABLE[2][b5]
            ^ CRC32_SLICE8_TABLE[1][b6]
            ^ CRC32_SLICE8_TABLE[0][b7];

        unsafe {
            p = p.add(8);
        }
        len -= 8;
    }

    // Trailing 4-byte fast step if remaining length >= 4
    if len >= 4 {
        let one = crc ^ unsafe { (p as *const u32).read_unaligned() };
        let b0 = (one & 0xFF) as usize;
        let b1 = ((one >> 8) & 0xFF) as usize;
        let b2 = ((one >> 16) & 0xFF) as usize;
        let b3 = ((one >> 24) & 0xFF) as usize;

        crc = CRC32_SLICE8_TABLE[3][b0]
            ^ CRC32_SLICE8_TABLE[2][b1]
            ^ CRC32_SLICE8_TABLE[1][b2]
            ^ CRC32_SLICE8_TABLE[0][b3];

        unsafe {
            p = p.add(4);
        }
        len -= 4;
    }

    // Trailing 1..3 bytes
    while len > 0 {
        let b = unsafe { *p };
        crc = (crc >> 8) ^ CRC32_SLICE8_TABLE[0][((crc ^ (b as u32)) & 0xFF) as usize];
        unsafe {
            p = p.add(1);
        }
        len -= 1;
    }

    !crc
}

/// Incrementally updates an IEEE 802.3 CRC-32 checksum with hardware acceleration where available.
///
/// If `data` is empty, returns `crc` unmodified.
#[inline]
pub fn crc32_update(crc: u32, data: &[u8]) -> u32 {
    if data.is_empty() {
        return crc;
    }
    crc32_fast(crc, data)
}

/// Computes the initial IEEE 802.3 CRC-32 checksum of `data` starting from `0`.
#[inline]
pub fn crc32_compute(data: &[u8]) -> u32 {
    crc32_update(0, data)
}

// ============================================================================
// 3. CRC-32 Matrix Combination Arithmetic
// ============================================================================

fn gf2_matrix_times(mat: &[u32; 32], mut vec: u32) -> u32 {
    let mut sum = 0u32;
    let mut idx = 0;
    while vec != 0 {
        if (vec & 1) != 0 {
            sum ^= mat[idx];
        }
        vec >>= 1;
        idx += 1;
    }
    sum
}

fn gf2_matrix_square(square: &mut [u32; 32], mat: &[u32; 32]) {
    for n in 0..32 {
        square[n] = gf2_matrix_times(mat, mat[n]);
    }
}

/// Combines two CRC-32 checksums into the checksum of their concatenated data in $O(\log N)$ time.
///
/// `crc1` is the checksum of the first block, `crc2` is the checksum of the second block,
/// and `len2` is the byte length of the second block.
pub fn combine_crc32(crc1: u32, crc2: u32, len2: usize) -> u32 {
    if len2 == 0 {
        return crc1;
    }

    let mut even = [0u32; 32];
    let mut odd = [0u32; 32];

    odd[0] = 0xEDB88320;
    let mut row = 1u32;
    for n in 1..32 {
        odd[n] = row;
        row <<= 1;
    }

    gf2_matrix_square(&mut even, &odd);
    gf2_matrix_square(&mut odd, &even);

    let mut c1 = crc1;
    let mut len = len2 as u64;
    loop {
        gf2_matrix_square(&mut even, &odd);
        if (len & 1) != 0 {
            c1 = gf2_matrix_times(&even, c1);
        }
        len >>= 1;
        if len == 0 {
            break;
        }

        gf2_matrix_square(&mut odd, &even);
        if (len & 1) != 0 {
            c1 = gf2_matrix_times(&odd, c1);
        }
        len >>= 1;
        if len == 0 {
            break;
        }
    }

    c1 ^ crc2
}
