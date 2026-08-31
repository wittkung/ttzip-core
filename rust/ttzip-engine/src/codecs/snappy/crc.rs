// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Hardware-accelerated Castagnoli CRC-32C (RFC 3720) and Snappy Masked CRC-32C engine.
//!
//! Provides ARMv8 CRC-32C hardware acceleration, x86 SSE4.2 acceleration,
//! and a Slice-by-8 scalar fallback, together with reversible Snappy checksum masking.

/// Castagnoli CRC-32C reversed polynomial (0x82F63B78).
pub const CASTAGNOLI_POLYNOMIAL: u32 = 0x82F63B78;

/// Snappy framing format checksum masking additive delta constant.
pub const SNAPPY_CRC_MASK_DELTA: u32 = 0xa282_ead8;

// ============================================================================
// 1. ARMv8 CRC-32C Hardware Acceleration
// ============================================================================
#[cfg(target_arch = "aarch64")]
mod arm64 {
    use core::arch::aarch64::*;

    #[inline]
    #[target_feature(enable = "crc")]
    pub(crate) unsafe fn crc32c_arm64_unrolled(mut crc: u32, mut p: *const u8, mut len: usize) -> u32 {
        while len >= 64 {
            crc = __crc32cd(crc, (p as *const u64).read_unaligned());
            crc = __crc32cd(crc, (p.add(8) as *const u64).read_unaligned());
            crc = __crc32cd(crc, (p.add(16) as *const u64).read_unaligned());
            crc = __crc32cd(crc, (p.add(24) as *const u64).read_unaligned());
            crc = __crc32cd(crc, (p.add(32) as *const u64).read_unaligned());
            crc = __crc32cd(crc, (p.add(40) as *const u64).read_unaligned());
            crc = __crc32cd(crc, (p.add(48) as *const u64).read_unaligned());
            crc = __crc32cd(crc, (p.add(56) as *const u64).read_unaligned());
            p = p.add(64);
            len -= 64;
        }

        if len >= 32 {
            crc = __crc32cd(crc, (p as *const u64).read_unaligned());
            crc = __crc32cd(crc, (p.add(8) as *const u64).read_unaligned());
            crc = __crc32cd(crc, (p.add(16) as *const u64).read_unaligned());
            crc = __crc32cd(crc, (p.add(24) as *const u64).read_unaligned());
            p = p.add(32);
            len -= 32;
        }

        if len >= 16 {
            crc = __crc32cd(crc, (p as *const u64).read_unaligned());
            crc = __crc32cd(crc, (p.add(8) as *const u64).read_unaligned());
            p = p.add(16);
            len -= 16;
        }

        if len >= 8 {
            crc = __crc32cd(crc, (p as *const u64).read_unaligned());
            p = p.add(8);
            len -= 8;
        }

        if len >= 4 {
            crc = __crc32cw(crc, (p as *const u32).read_unaligned());
            p = p.add(4);
            len -= 4;
        }

        if len >= 2 {
            crc = __crc32ch(crc, (p as *const u16).read_unaligned());
            p = p.add(2);
            len -= 2;
        }

        if len == 1 {
            crc = __crc32cb(crc, *p);
        }

        crc
    }
}

// ============================================================================
// 2. x86_64 SSE4.2 Hardware Acceleration
// ============================================================================
#[cfg(target_arch = "x86_64")]
mod x86_64 {
    use core::arch::x86_64::*;

    #[inline]
    #[target_feature(enable = "sse4.2")]
    pub(crate) unsafe fn crc32c_x86_unrolled(crc: u32, mut p: *const u8, mut len: usize) -> u32 {
        let mut crc64 = crc as u64;
        while len >= 64 {
            crc64 = _mm_crc32_u64(crc64, (p as *const u64).read_unaligned());
            crc64 = _mm_crc32_u64(crc64, (p.add(8) as *const u64).read_unaligned());
            crc64 = _mm_crc32_u64(crc64, (p.add(16) as *const u64).read_unaligned());
            crc64 = _mm_crc32_u64(crc64, (p.add(24) as *const u64).read_unaligned());
            crc64 = _mm_crc32_u64(crc64, (p.add(32) as *const u64).read_unaligned());
            crc64 = _mm_crc32_u64(crc64, (p.add(40) as *const u64).read_unaligned());
            crc64 = _mm_crc32_u64(crc64, (p.add(48) as *const u64).read_unaligned());
            crc64 = _mm_crc32_u64(crc64, (p.add(56) as *const u64).read_unaligned());
            p = p.add(64);
            len -= 64;
        }

        if len >= 32 {
            crc64 = _mm_crc32_u64(crc64, (p as *const u64).read_unaligned());
            crc64 = _mm_crc32_u64(crc64, (p.add(8) as *const u64).read_unaligned());
            crc64 = _mm_crc32_u64(crc64, (p.add(16) as *const u64).read_unaligned());
            crc64 = _mm_crc32_u64(crc64, (p.add(24) as *const u64).read_unaligned());
            p = p.add(32);
            len -= 32;
        }

        if len >= 16 {
            crc64 = _mm_crc32_u64(crc64, (p as *const u64).read_unaligned());
            crc64 = _mm_crc32_u64(crc64, (p.add(8) as *const u64).read_unaligned());
            p = p.add(16);
            len -= 16;
        }

        if len >= 8 {
            crc64 = _mm_crc32_u64(crc64, (p as *const u64).read_unaligned());
            p = p.add(8);
            len -= 8;
        }

        let mut crc = crc64 as u32;

        if len >= 4 {
            crc = _mm_crc32_u32(crc, (p as *const u32).read_unaligned());
            p = p.add(4);
            len -= 4;
        }

        if len >= 2 {
            crc = _mm_crc32_u16(crc, (p as *const u16).read_unaligned());
            p = p.add(2);
            len -= 2;
        }

        if len == 1 {
            crc = _mm_crc32_u8(crc, *p);
        }

        crc
    }
}

// ============================================================================
// 3. Slice-by-8 Scalar Fallback
// ============================================================================
pub mod scalar {
    use super::CASTAGNOLI_POLYNOMIAL;

    const fn make_crc32c_tables() -> [[u32; 256]; 8] {
        let mut tables = [[0u32; 256]; 8];
        let mut i = 0;
        while i < 256 {
            let mut crc = i as u32;
            let mut j = 0;
            while j < 8 {
                crc = if (crc & 1) != 0 {
                    (crc >> 1) ^ CASTAGNOLI_POLYNOMIAL
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

    static CRC32C_TABLE: [[u32; 256]; 8] = make_crc32c_tables();

    /// Computes Castagnoli CRC-32C using Slice-by-8 algorithm.
    #[inline]
    pub fn crc32c_slice8(crc: u32, data: &[u8]) -> u32 {
        let mut raw_crc = !crc;
        let mut p = data.as_ptr();
        let mut len = data.len();

        while len >= 8 {
            let one = raw_crc ^ unsafe { (p as *const u32).read_unaligned() };
            let two = unsafe { (p.add(4) as *const u32).read_unaligned() };

            let b0 = (one & 0xFF) as usize;
            let b1 = ((one >> 8) & 0xFF) as usize;
            let b2 = ((one >> 16) & 0xFF) as usize;
            let b3 = ((one >> 24) & 0xFF) as usize;

            let b4 = (two & 0xFF) as usize;
            let b5 = ((two >> 8) & 0xFF) as usize;
            let b6 = ((two >> 16) & 0xFF) as usize;
            let b7 = ((two >> 24) & 0xFF) as usize;

            raw_crc = CRC32C_TABLE[7][b0]
                ^ CRC32C_TABLE[6][b1]
                ^ CRC32C_TABLE[5][b2]
                ^ CRC32C_TABLE[4][b3]
                ^ CRC32C_TABLE[3][b4]
                ^ CRC32C_TABLE[2][b5]
                ^ CRC32C_TABLE[1][b6]
                ^ CRC32C_TABLE[0][b7];

            unsafe {
                p = p.add(8);
            }
            len -= 8;
        }

        while len > 0 {
            let b = unsafe { *p };
            raw_crc = (raw_crc >> 8) ^ CRC32C_TABLE[0][((raw_crc ^ (b as u32)) & 0xFF) as usize];
            unsafe {
                p = p.add(1);
            }
            len -= 1;
        }

        !raw_crc
    }
}

// ============================================================================
// 4. Public High-Speed CRC-32C Entrypoints
// ============================================================================

/// Computes or incrementally updates a Castagnoli CRC-32C checksum with hardware acceleration.
///
/// If `data` is empty, returns `crc` unmodified.
#[inline]
pub fn crc32c_update(crc: u32, data: &[u8]) -> u32 {
    if data.is_empty() {
        return crc;
    }

    #[cfg(target_arch = "aarch64")]
    {
        let raw_crc = !crc;
        let updated = unsafe { arm64::crc32c_arm64_unrolled(raw_crc, data.as_ptr(), data.len()) };
        !updated
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("sse4.2") {
            let raw_crc = !crc;
            let updated = unsafe { x86_64::crc32c_x86_unrolled(raw_crc, data.as_ptr(), data.len()) };
            !updated
        } else {
            scalar::crc32c_slice8(crc, data)
        }
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        scalar::crc32c_slice8(crc, data)
    }
}

/// Computes the Castagnoli CRC-32C checksum of `data` starting from seed 0.
#[inline]
pub fn crc32c(data: &[u8]) -> u32 {
    crc32c_update(0, data)
}

/// Masks CRC32C per Snappy framing specification: `((crc >> 15) | (crc << 17)) + 0xa282_ead8`.
#[inline]
pub const fn mask_crc32c(crc: u32) -> u32 {
    crc.rotate_right(15).wrapping_add(SNAPPY_CRC_MASK_DELTA)
}

/// Unmasks CRC32C per Snappy framing specification.
#[inline]
pub const fn unmask_crc32c(masked: u32) -> u32 {
    masked.wrapping_sub(SNAPPY_CRC_MASK_DELTA).rotate_left(15)
}

// ============================================================================
// 5. Streaming Snappy CRC-32C State Machine
// ============================================================================

/// Streaming Castagnoli CRC-32C hasher maintaining incremental state for Snappy framing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnappyCrc32cHasher {
    crc: u32,
}

impl Default for SnappyCrc32cHasher {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl SnappyCrc32cHasher {
    /// Creates a new `SnappyCrc32cHasher` initialized to 0.
    #[inline]
    pub const fn new() -> Self {
        Self { crc: 0 }
    }

    /// Creates a `SnappyCrc32cHasher` with a custom starting seed.
    #[inline]
    pub const fn with_seed(seed: u32) -> Self {
        Self { crc: seed }
    }

    /// Updates the running CRC-32C with new data bytes.
    #[inline]
    pub fn update(&mut self, data: &[u8]) {
        self.crc = crc32c_update(self.crc, data);
    }

    /// Resets the internal state to standard seed 0.
    #[inline]
    pub fn reset(&mut self) {
        self.crc = 0;
    }

    /// Finalizes and returns the unmasked Castagnoli CRC-32C checksum.
    #[inline]
    pub fn finalize(&self) -> u32 {
        self.crc
    }

    /// Finalizes and returns the Snappy-masked CRC-32C checksum.
    #[inline]
    pub fn finalize_masked(&self) -> u32 {
        mask_crc32c(self.crc)
    }
}

impl core::hash::Hasher for SnappyCrc32cHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.crc as u64
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.update(bytes);
    }
}
