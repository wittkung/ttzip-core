// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Microkernel SIMD Vector Dot Product Engine for 128-dimensional INT8 embeddings.
//!
//! # Microarchitecture Optimization Details
//! - Hardware acceleration: ARM NEON `sdot` (`vdotq_s32`) (ARMv8.2-A / Apple Silicon M1-M4).
//! - Dual-accumulator unrolled loop maximizing instruction-level parallelism (ILP)
//!   and avoiding execution pipeline stalls across execution ports.
//! - Portable scalar reference implementation for non-aarch64 architectures and differential verification.
//! - Strict bounds-free register streaming with zero dynamic memory allocation.

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn vdotq_s32_stable(acc: int32x4_t, a: int8x16_t, b: int8x16_t) -> int32x4_t {
    let mut res = acc;
    core::arch::asm!(
        "sdot {res:v}.4s, {a:v}.16b, {b:v}.16b",
        res = inout(vreg) res,
        a = in(vreg) a,
        b = in(vreg) b,
        options(pure, nomem, nostack)
    );
    res
}

/// Computes the dot product of two 128-dimensional INT8 vectors using ARM NEON SIMD intrinsics.
///
/// # Safety
/// Requires `aarch64` target with NEON and Dot Product (`dotprod`) extension.
#[cfg(target_arch = "aarch64")]
#[inline]
pub unsafe fn neon_dot_product_128(a: &[i8; 128], b: &[i8; 128]) -> i32 {
    let a_ptr = a.as_ptr();
    let b_ptr = b.as_ptr();

    // Dual accumulators to break dependency chains across execution ports
    let mut acc0 = vdupq_n_s32(0);
    let mut acc1 = vdupq_n_s32(0);

    // Iteration 0: bytes 0..32
    let va0 = vld1q_s8(a_ptr);
    let vb0 = vld1q_s8(b_ptr);
    acc0 = vdotq_s32_stable(acc0, va0, vb0);

    let va1 = vld1q_s8(a_ptr.add(16));
    let vb1 = vld1q_s8(b_ptr.add(16));
    acc1 = vdotq_s32_stable(acc1, va1, vb1);

    // Iteration 1: bytes 32..64
    let va2 = vld1q_s8(a_ptr.add(32));
    let vb2 = vld1q_s8(b_ptr.add(32));
    acc0 = vdotq_s32_stable(acc0, va2, vb2);

    let va3 = vld1q_s8(a_ptr.add(48));
    let vb3 = vld1q_s8(b_ptr.add(48));
    acc1 = vdotq_s32_stable(acc1, va3, vb3);

    // Iteration 2: bytes 64..96
    let va4 = vld1q_s8(a_ptr.add(64));
    let vb4 = vld1q_s8(b_ptr.add(64));
    acc0 = vdotq_s32_stable(acc0, va4, vb4);

    let va5 = vld1q_s8(a_ptr.add(80));
    let vb5 = vld1q_s8(b_ptr.add(80));
    acc1 = vdotq_s32_stable(acc1, va5, vb5);

    // Iteration 3: bytes 96..128
    let va6 = vld1q_s8(a_ptr.add(96));
    let vb6 = vld1q_s8(b_ptr.add(96));
    acc0 = vdotq_s32_stable(acc0, va6, vb6);

    let va7 = vld1q_s8(a_ptr.add(112));
    let vb7 = vld1q_s8(b_ptr.add(112));
    acc1 = vdotq_s32_stable(acc1, va7, vb7);

    // Combine accumulators and reduce vector sum to 32-bit scalar
    let total_acc = vaddq_s32(acc0, acc1);
    vaddvq_s32(total_acc)
}

/// Reference scalar dot product over 128 signed 8-bit integers.
#[inline]
pub fn scalar_dot_product_128(a: &[i8; 128], b: &[i8; 128]) -> i32 {
    let mut sum: i32 = 0;
    for i in 0..128 {
        sum += (a[i] as i32) * (b[i] as i32);
    }
    sum
}

/// Computes the dot product of two 128-dimensional INT8 vectors, dispatching
/// to ARM NEON SIMD on aarch64 or falling back to portable scalar arithmetic.
#[inline(always)]
pub fn dot_product_128(a: &[i8; 128], b: &[i8; 128]) -> i32 {
    #[cfg(target_arch = "aarch64")]
    {
        unsafe { neon_dot_product_128(a, b) }
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        scalar_dot_product_128(a, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_vectors() {
        let a = [0i8; 128];
        let b = [0i8; 128];
        assert_eq!(dot_product_128(&a, &b), 0);
        assert_eq!(scalar_dot_product_128(&a, &b), 0);
        #[cfg(target_arch = "aarch64")]
        unsafe {
            assert_eq!(neon_dot_product_128(&a, &b), 0);
        }
    }

    #[test]
    fn test_max_positive_vectors() {
        let a = [127i8; 128];
        let b = [127i8; 128];
        // 128 * (127 * 127) = 128 * 16129 = 2,064,512
        let expected = 128 * 127 * 127;
        assert_eq!(scalar_dot_product_128(&a, &b), expected);
        assert_eq!(dot_product_128(&a, &b), expected);
        #[cfg(target_arch = "aarch64")]
        unsafe {
            assert_eq!(neon_dot_product_128(&a, &b), expected);
        }
    }

    #[test]
    fn test_max_negative_vectors() {
        let a = [-128i8; 128];
        let b = [-128i8; 128];
        // 128 * (-128 * -128) = 128 * 16384 = 2,097,152
        let expected = 128 * 128 * 128;
        assert_eq!(scalar_dot_product_128(&a, &b), expected);
        assert_eq!(dot_product_128(&a, &b), expected);
        #[cfg(target_arch = "aarch64")]
        unsafe {
            assert_eq!(neon_dot_product_128(&a, &b), expected);
        }
    }

    #[test]
    fn test_orthogonal_vectors() {
        let mut a = [0i8; 128];
        let mut b = [0i8; 128];
        for i in 0..64 {
            a[i] = 50;
            b[i + 64] = 50;
        }
        assert_eq!(scalar_dot_product_128(&a, &b), 0);
        assert_eq!(dot_product_128(&a, &b), 0);
        #[cfg(target_arch = "aarch64")]
        unsafe {
            assert_eq!(neon_dot_product_128(&a, &b), 0);
        }
    }

    #[test]
    fn test_alternating_signs() {
        let mut a = [0i8; 128];
        let mut b = [0i8; 128];
        for i in 0..128 {
            a[i] = if i % 2 == 0 { 10 } else { -10 };
            b[i] = if i % 2 == 0 { -10 } else { 10 };
        }
        // Each element product is -100, 128 * -100 = -12800
        let expected = -12800;
        assert_eq!(scalar_dot_product_128(&a, &b), expected);
        assert_eq!(dot_product_128(&a, &b), expected);
        #[cfg(target_arch = "aarch64")]
        unsafe {
            assert_eq!(neon_dot_product_128(&a, &b), expected);
        }
    }
}
