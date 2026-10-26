// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Differential fuzzing and fuzz verification for SIMD NEON vs Scalar 128-dimensional INT8 dot products.

use ttzip_types::simd_dot::{dot_product_128, scalar_dot_product_128};

#[cfg(target_arch = "aarch64")]
use ttzip_types::simd_dot::neon_dot_product_128;

/// Fast pseudo-random number generator (Xorshift64star) for reproducible differential fuzzing.
struct XorShift64Star {
    state: u64,
}

impl XorShift64Star {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x853c49e6748fea9b } else { seed },
        }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn fill_i8_128(&mut self, buf: &mut [i8; 128]) {
        for chunk in buf.chunks_exact_mut(8) {
            let val = self.next_u64();
            let bytes = val.to_le_bytes();
            for i in 0..8 {
                chunk[i] = bytes[i] as i8;
            }
        }
    }
}

#[test]
fn test_simd_vs_scalar_10000_random_vectors() {
    let mut rng = XorShift64Star::new(0xDEADBEEF_CAFEF00D);
    let mut a = [0i8; 128];
    let mut b = [0i8; 128];

    // Theoretical maximum absolute value: 128 * 128 * 128 = 2,097,152
    const MAX_ABS_DOT_PRODUCT: i32 = 128 * 128 * 128;

    for i in 0..10_000 {
        rng.fill_i8_128(&mut a);
        rng.fill_i8_128(&mut b);

        let scalar_result = scalar_dot_product_128(&a, &b);
        let dispatched_result = dot_product_128(&a, &b);

        assert!(
            scalar_result.abs() <= MAX_ABS_DOT_PRODUCT,
            "Scalar dot product overflowed theoretical bound at iteration {}: {}",
            i,
            scalar_result
        );

        assert_eq!(
            dispatched_result, scalar_result,
            "Dispatched dot product mismatch at iteration {}: expected {}, got {}",
            i, scalar_result, dispatched_result
        );

        #[cfg(target_arch = "aarch64")]
        {
            let neon_result = unsafe { neon_dot_product_128(&a, &b) };
            assert_eq!(
                neon_result, scalar_result,
                "NEON dot product mismatch at iteration {}: expected {}, got {}",
                i, scalar_result, neon_result
            );
        }
    }
}
