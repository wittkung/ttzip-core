// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Secure Password Vault engine with Zeroize Compiler Fence and AES-256-GCM.
//!
//! Complies with NIST SP 800-38D (Galois/Counter Mode) and provides
//! Dead-Store Elimination immune memory sanitization with atomic compiler fences.

use crate::aes256::Aes256Context;
use ttzip_types::TTZipStatus;
use std::sync::atomic::{compiler_fence, Ordering};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Forward S-Box for constant-time AES substitution.
static SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76, 0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15, 0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84, 0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8, 0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73, 0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79, 0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a, 0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf, 0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

/// Branchless, constant-time xtime computation over GF(2^8).
#[inline(always)]
fn xtime(b: u8) -> u8 {
    let mask = 0u8.wrapping_sub(b >> 7);
    (b << 1) ^ (0x1b & mask)
}

/// Constant-time SubBytes across a 16-byte AES block to eliminate cache-timing side channels.
///
/// Scans the entire 256-byte S-Box linearly with constant-time bitwise multiplexing,
/// ensuring static memory access patterns and uniform cycle counts regardless of key or data.
#[inline]
pub fn ct_sub_bytes_16(state: &[u8; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (i, &s) in SBOX.iter().enumerate() {
        let target = i as u8;
        for j in 0..16 {
            let diff = state[j] ^ target;
            let mask = 0u8.wrapping_sub(((diff as u16).wrapping_sub(1) >> 15) as u8);
            out[j] |= s & mask;
        }
    }
    out
}

/// Constant-time software AES-256 block encryption.
#[inline]
pub fn aes256_encrypt_block_ct(ctx: &Aes256Context, input: &[u8; 16], output: &mut [u8; 16]) {
    let mut state = *input;
    for i in 0..16 {
        state[i] ^= ctx.round_keys_enc[0][i];
    }
    for r in 1..14 {
        let s = ct_sub_bytes_16(&state);
        let mut next = [0u8; 16];
        for &c in &[0, 4, 8, 12] {
            let (s0, s1, s2, s3) = (s[c], s[(c + 5) % 16], s[(c + 10) % 16], s[(c + 15) % 16]);
            next[c] = xtime(s0 ^ s1) ^ s1 ^ s2 ^ s3 ^ ctx.round_keys_enc[r][c];
            next[c + 1] = xtime(s1 ^ s2) ^ s2 ^ s3 ^ s0 ^ ctx.round_keys_enc[r][c + 1];
            next[c + 2] = xtime(s2 ^ s3) ^ s3 ^ s0 ^ s1 ^ ctx.round_keys_enc[r][c + 2];
            next[c + 3] = xtime(s3 ^ s0) ^ s0 ^ s1 ^ s2 ^ ctx.round_keys_enc[r][c + 3];
        }
        state = next;
    }
    let s = ct_sub_bytes_16(&state);
    for &c in &[0, 4, 8, 12] {
        output[c] = s[c] ^ ctx.round_keys_enc[14][c];
        output[c + 1] = s[(c + 5) % 16] ^ ctx.round_keys_enc[14][c + 1];
        output[c + 2] = s[(c + 10) % 16] ^ ctx.round_keys_enc[14][c + 2];
        output[c + 3] = s[(c + 15) % 16] ^ ctx.round_keys_enc[14][c + 3];
    }
}

#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn aes256_encrypt_block_aarch64(ctx: &Aes256Context, input: &[u8; 16], output: &mut [u8; 16]) {
    unsafe {
        use core::arch::aarch64::*;
        let mut b = vld1q_u8(input.as_ptr());
        for r in 0..13 {
            b = vaesmcq_u8(vaeseq_u8(b, vld1q_u8(ctx.round_keys_enc[r].as_ptr())));
        }
        let rk13 = vld1q_u8(ctx.round_keys_enc[13].as_ptr());
        let rk14 = vld1q_u8(ctx.round_keys_enc[14].as_ptr());
        b = veorq_u8(vaeseq_u8(b, rk13), rk14);
        vst1q_u8(output.as_mut_ptr(), b);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "aes")]
unsafe fn aes256_encrypt_block_aesni(ctx: &Aes256Context, input: &[u8; 16], output: &mut [u8; 16]) {
    // SAFETY: input and output are valid 16-byte arrays; round_keys_enc contains 15 valid round keys.
    // The caller guarantees the `aes` target feature is available.
    unsafe {
        use core::arch::x86_64::*;
        let in_vec = _mm_loadu_si128(input.as_ptr() as *const __m128i);
        let mut b = _mm_xor_si128(in_vec, _mm_loadu_si128(ctx.round_keys_enc[0].as_ptr() as *const __m128i));
        for r in 1..14 {
            b = _mm_aesenc_si128(b, _mm_loadu_si128(ctx.round_keys_enc[r].as_ptr() as *const __m128i));
        }
        b = _mm_aesenclast_si128(b, _mm_loadu_si128(ctx.round_keys_enc[14].as_ptr() as *const __m128i));
        _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, b);
    }
}

/// Encrypts a single 16-byte block with AES-256 in constant time.
#[inline]
pub fn aes256_encrypt_block(ctx: &Aes256Context, input: &[u8; 16], output: &mut [u8; 16]) {
    #[cfg(target_arch = "aarch64")]
    // SAFETY: input and output are valid 16-byte arrays; round_keys_enc contains 15 valid round keys.
    unsafe {
        aes256_encrypt_block_aarch64(ctx, input, output);
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("aes") {
            // SAFETY: AES-NI instruction set availability is verified dynamically via CPUID.
            unsafe {
                aes256_encrypt_block_aesni(ctx, input, output);
            }
            return;
        }
        aes256_encrypt_block_ct(ctx, input, output);
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        aes256_encrypt_block_ct(ctx, input, output);
    }
}


/// GHASH authenticator over GF(2^128).
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct GHash {
    h: [u8; 16],
    state: [u8; 16],
}

impl GHash {
    pub fn new(h: &[u8; 16]) -> Self {
        Self {
            h: *h,
            state: [0u8; 16],
        }
    }

    /// Constant-Time GF(2^128) multiplication with irreducible polynomial x^128 + x^7 + x^2 + x + 1.
    #[cfg(target_arch = "aarch64")]
    pub fn mul_h(x: &[u8; 16], h: &[u8; 16]) -> [u8; 16] {
        unsafe {
            use core::arch::aarch64::*;

            // Load and bit-reverse byte bits into vector registers
            let x_vec = vrbitq_u8(vld1q_u8(x.as_ptr()));
            let h_vec = vrbitq_u8(vld1q_u8(h.as_ptr()));

            let a = vreinterpretq_u64_u8(x_vec);
            let b = vreinterpretq_u64_u8(h_vec);

            let a_lo = vgetq_lane_u64::<0>(a);
            let a_hi = vgetq_lane_u64::<1>(a);
            let b_lo = vgetq_lane_u64::<0>(b);
            let b_hi = vgetq_lane_u64::<1>(b);

            // Karatsuba 3-way polynomial multiplication:
            // d0 = a_lo * b_lo
            // d2 = a_hi * b_hi (using vmull_high_p64)
            // dm = (a_lo ^ a_hi) * (b_lo ^ b_hi)
            let d0 = vmull_p64(a_lo, b_lo) as u128;
            let d2 = vmull_high_p64(vreinterpretq_p64_u8(x_vec), vreinterpretq_p64_u8(h_vec)) as u128;
            let dm = vmull_p64(a_lo ^ a_hi, b_lo ^ b_hi) as u128;

            let (d0_lo, d0_hi) = (d0 as u64, (d0 >> 64) as u64);
            let (d2_lo, d2_hi) = (d2 as u64, (d2 >> 64) as u64);
            let (dm_lo, dm_hi) = (dm as u64, (dm >> 64) as u64);
            let (d1_lo, d1_hi) = (dm_lo ^ d0_lo ^ d2_lo, dm_hi ^ d0_hi ^ d2_hi);

            let (lo_lo, lo_hi) = (d0_lo, d0_hi ^ d1_lo);
            let (hi_lo, hi_hi) = (d2_lo ^ d1_hi, d2_hi);

            // Barrett reduction modulo P(x) = x^128 + x^7 + x^2 + x + 1 (constant 0x87)
            let r_hi = vmull_p64(hi_hi, 0x87) as u128;
            let r_lo = vmull_p64(hi_lo, 0x87) as u128;

            let (r_lo_lo, r_lo_hi) = (r_lo as u64, (r_lo >> 64) as u64);
            let (r_hi_lo, r_hi_hi) = (r_hi as u64, (r_hi >> 64) as u64);

            // hi_hi * 0x87 has degree <= 70, so r_hi_hi has at most 7 overflow bits past x^127
            let ov = r_hi_hi;
            let corr = ov ^ (ov << 1) ^ (ov << 2) ^ (ov << 7);

            let res_lo = lo_lo ^ r_lo_lo ^ corr;
            let res_hi = lo_hi ^ r_lo_hi ^ r_hi_lo;

            let res = vreinterpretq_u8_u64(vsetq_lane_u64::<1>(res_hi, vsetq_lane_u64::<0>(res_lo, vdupq_n_u64(0))));
            let out_v = vrbitq_u8(res);

            let mut out = [0u8; 16];
            vst1q_u8(out.as_mut_ptr(), out_v);
            out
        }
    }

    /// Constant-Time GF(2^128) scalar reference multiplication.
    #[cfg(not(target_arch = "aarch64"))]
    pub fn mul_h(x: &[u8; 16], h: &[u8; 16]) -> [u8; 16] {
        Self::mul_h_scalar(x, h)
    }

    /// Constant-Time GF(2^128) scalar multiplication reference implementation.
    pub fn mul_h_scalar(x: &[u8; 16], h: &[u8; 16]) -> [u8; 16] {
        let mut z0 = 0u64;
        let mut z1 = 0u64;
        let mut v0 = u64::from_be_bytes(h[0..8].try_into().unwrap_or([0u8; 8]));
        let mut v1 = u64::from_be_bytes(h[8..16].try_into().unwrap_or([0u8; 8]));

        for i in 0..128 {
            let byte_idx = i / 8;
            let bit_idx = 7 - (i % 8);
            // Constant-time mask: 0xFFFFFFFFFFFFFFFF if bit is 1, 0x0 otherwise
            let bit = ((x[byte_idx] >> bit_idx) & 1) as u64;
            let mask = 0u64.wrapping_sub(bit);

            z0 ^= v0 & mask;
            z1 ^= v1 & mask;

            let lsb = v1 & 1;
            let lsb_mask = 0u64.wrapping_sub(lsb);

            v1 = (v1 >> 1) | (v0 << 63);
            v0 >>= 1;
            v0 ^= 0xe100_0000_0000_0000 & lsb_mask;
        }

        let mut out = [0u8; 16];
        out[0..8].copy_from_slice(&z0.to_be_bytes());
        out[8..16].copy_from_slice(&z1.to_be_bytes());
        out
    }

    pub fn update_block(&mut self, block: &[u8; 16]) {
        let mut x = [0u8; 16];
        for i in 0..16 {
            x[i] = self.state[i] ^ block[i];
        }
        self.state = Self::mul_h(&x, &self.h);
    }

    pub fn update(&mut self, mut data: &[u8]) {
        while data.len() >= 16 {
            let Ok(block) = <&[u8; 16]>::try_from(&data[..16]) else {
                break;
            };
            self.update_block(block);
            data = &data[16..];
        }
        if !data.is_empty() {
            let mut block = [0u8; 16];
            block[..data.len()].copy_from_slice(data);
            self.update_block(&block);
        }
    }

    pub fn finalize(self) -> [u8; 16] {
        self.state
    }
}

/// Constant-time 16-byte slice comparison to prevent timing side-channel leaks.
#[inline]
pub fn constant_time_eq_16(a: &[u8; 16], b: &[u8; 16]) -> bool {
    a.ct_eq(b).into()
}

/// Dead-Store Elimination immune memory sanitization with SeqCst compiler fence.
#[inline]
pub fn secure_wipe(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    unsafe {
        let mut p = ptr;
        let mut remaining = len;
        while remaining >= core::mem::size_of::<usize>() {
            std::ptr::write_volatile(p as *mut usize, 0);
            p = p.add(core::mem::size_of::<usize>());
            remaining -= core::mem::size_of::<usize>();
        }
        while remaining > 0 {
            std::ptr::write_volatile(p, 0);
            p = p.add(1);
            remaining -= 1;
        }
        compiler_fence(Ordering::SeqCst);
    }
}

/// Dead-Store Elimination immune slice wiper.
#[inline]
pub fn secure_wipe_slice(slice: &mut [u8]) {
    secure_wipe(slice.as_mut_ptr(), slice.len());
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn gcm_ctr_8blocks_aarch64(
    rk: &[core::arch::aarch64::uint8x16_t; 15],
    ctr_base: &[u8; 16],
    block_idx: usize,
    src: &[u8],
    dst: &mut [u8],
) {
    unsafe {
        use core::arch::aarch64::*;
        let offset = block_idx * 16;
        let counter_start = (block_idx as u32).wrapping_add(2);

        let mut b: [uint8x16_t; 8] = core::array::from_fn(|lane| {
            let mut c = *ctr_base;
            c[12..16].copy_from_slice(&counter_start.wrapping_add(lane as u32).to_be_bytes());
            vld1q_u8(c.as_ptr())
        });

        for &round_key in rk.iter().take(13) {
            for lane in b.iter_mut() {
                *lane = vaesmcq_u8(vaeseq_u8(*lane, round_key));
            }
        }
        for lane in b.iter_mut() {
            *lane = veorq_u8(vaeseq_u8(*lane, rk[13]), rk[14]);
        }

        let s_ptr = src.as_ptr().add(offset);
        let d_ptr = dst.as_mut_ptr().add(offset);
        for lane in 0..8 {
            let off = lane * 16;
            vst1q_u8(d_ptr.add(off), veorq_u8(vld1q_u8(s_ptr.add(off)), b[lane]));
        }
    }
}

#[cfg(target_arch = "aarch64")]
pub fn gcm_ctr_crypt(ctx: &Aes256Context, iv: &[u8; 12], src: &[u8], dst: &mut [u8]) {
    unsafe {
        use core::arch::aarch64::*;

        let rk: [uint8x16_t; 15] =
            core::array::from_fn(|r| vld1q_u8(ctx.round_keys_enc[r].as_ptr()));

        let mut ctr_base = [0u8; 16];
        ctr_base[..12].copy_from_slice(iv);

        let total_blocks = src.len().div_ceil(16);
        let mut block_idx = 0usize;

        while block_idx + 8 <= total_blocks {
            gcm_ctr_8blocks_aarch64(&rk, &ctr_base, block_idx, src, dst);
            block_idx += 8;
        }

        while block_idx < total_blocks {
            let offset = block_idx * 16;
            let counter_val = (block_idx as u32).wrapping_add(2);
            let mut c = ctr_base;
            c[12..16].copy_from_slice(&counter_val.to_be_bytes());

            let mut keystream = [0u8; 16];
            aes256_encrypt_block(ctx, &c, &mut keystream);

            let chunk_len = (src.len() - offset).min(16);
            for k in 0..chunk_len {
                dst[offset + k] = src[offset + k] ^ keystream[k];
            }

            keystream.zeroize();
            c.zeroize();
            block_idx += 1;
        }

        ctr_base.zeroize();
        compiler_fence(Ordering::SeqCst);
    }
}

#[cfg(not(target_arch = "aarch64"))]
pub fn gcm_ctr_crypt(ctx: &Aes256Context, iv: &[u8; 12], src: &[u8], dst: &mut [u8]) {
    let mut ctr_block = [0u8; 16];
    ctr_block[..12].copy_from_slice(iv);

    let num_blocks = src.len().div_ceil(16);
    for i in 0..num_blocks {
        let counter_val = (i as u32).wrapping_add(2);
        ctr_block[12..16].copy_from_slice(&counter_val.to_be_bytes());

        let mut keystream = [0u8; 16];
        aes256_encrypt_block(ctx, &ctr_block, &mut keystream);

        let block_offset = i * 16;
        let block_len = (src.len() - block_offset).min(16);
        for k in 0..block_len {
            dst[block_offset + k] = src[block_offset + k] ^ keystream[k];
        }
        keystream.zeroize();
    }
    ctr_block.zeroize();
}

fn gcm_compute_tag(h: &[u8; 16], tag_mask: &[u8; 16], aad: &[u8], ciphertext: &[u8]) -> [u8; 16] {
    let mut ghash = GHash::new(h);
    if !aad.is_empty() {
        ghash.update(aad);
    }
    if !ciphertext.is_empty() {
        ghash.update(ciphertext);
    }

    let mut len_block = [0u8; 16];
    len_block[0..8].copy_from_slice(&((aad.len() as u64) * 8).to_be_bytes());
    len_block[8..16].copy_from_slice(&((ciphertext.len() as u64) * 8).to_be_bytes());
    ghash.update_block(&len_block);

    let s = ghash.finalize();
    let mut tag = [0u8; 16];
    for i in 0..16 {
        tag[i] = s[i] ^ tag_mask[i];
    }
    len_block.zeroize();
    tag
}

/// AES-256-GCM authenticated encryption (NIST SP 800-38D).
pub fn aes256_gcm_encrypt(
    key: &[u8; 32],
    iv: &[u8; 12],
    plaintext: &[u8],
    aad: &[u8],
    ciphertext: &mut [u8],
    tag: &mut [u8; 16],
) -> Result<(), TTZipStatus> {
    if ciphertext.len() < plaintext.len() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let ctx = Aes256Context::new(key);
    let mut h = [0u8; 16];
    aes256_encrypt_block(&ctx, &[0u8; 16], &mut h);

    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(iv);
    j0[15] = 1;

    let mut tag_mask = [0u8; 16];
    aes256_encrypt_block(&ctx, &j0, &mut tag_mask);

    gcm_ctr_crypt(&ctx, iv, plaintext, &mut ciphertext[..plaintext.len()]);
    *tag = gcm_compute_tag(&h, &tag_mask, aad, &ciphertext[..plaintext.len()]);

    h.zeroize();
    j0.zeroize();
    tag_mask.zeroize();
    compiler_fence(Ordering::SeqCst);

    Ok(())
}

/// AES-256-GCM authenticated decryption (NIST SP 800-38D).
pub fn aes256_gcm_decrypt(
    key: &[u8; 32],
    iv: &[u8; 12],
    ciphertext: &[u8],
    aad: &[u8],
    tag: &[u8; 16],
    plaintext: &mut [u8],
) -> Result<(), TTZipStatus> {
    if plaintext.len() < ciphertext.len() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let ctx = Aes256Context::new(key);
    let mut h = [0u8; 16];
    aes256_encrypt_block(&ctx, &[0u8; 16], &mut h);

    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(iv);
    j0[15] = 1;

    let mut tag_mask = [0u8; 16];
    aes256_encrypt_block(&ctx, &j0, &mut tag_mask);

    let mut expected_tag = gcm_compute_tag(&h, &tag_mask, aad, ciphertext);

    if !constant_time_eq_16(tag, &expected_tag) {
        secure_wipe_slice(plaintext);
        h.zeroize();
        j0.zeroize();
        tag_mask.zeroize();
        expected_tag.zeroize();
        compiler_fence(Ordering::SeqCst);
        return Err(TTZipStatus::ErrInvalidPassword);
    }

    gcm_ctr_crypt(&ctx, iv, ciphertext, &mut plaintext[..ciphertext.len()]);

    h.zeroize();
    j0.zeroize();
    tag_mask.zeroize();
    expected_tag.zeroize();
    compiler_fence(Ordering::SeqCst);

    Ok(())
}

fn init_hmac_sha256_contexts(
    key: &[u8],
) -> (crate::sha256::FastSha256, crate::sha256::FastSha256) {
    let mut k_pad = [0u8; 64];
    if key.len() > 64 {
        let digest = crate::sha256::FastSha256::digest(key);
        k_pad[..32].copy_from_slice(&digest);
    } else {
        k_pad[..key.len()].copy_from_slice(key);
    }

    let mut k_ipad = [0x36u8; 64];
    let mut k_opad = [0x5cu8; 64];
    for i in 0..64 {
        k_ipad[i] ^= k_pad[i];
        k_opad[i] ^= k_pad[i];
    }

    let mut base_inner = crate::sha256::FastSha256::new();
    base_inner.update(&k_ipad);

    let mut base_outer = crate::sha256::FastSha256::new();
    base_outer.update(&k_opad);

    k_pad.zeroize();
    k_ipad.zeroize();
    k_opad.zeroize();
    compiler_fence(Ordering::SeqCst);

    (base_inner, base_outer)
}

/// Standard HMAC-SHA256 (RFC 2104 / RFC 4231).
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let (mut inner, mut outer) = init_hmac_sha256_contexts(key);
    inner.update(data);
    let inner_hash = inner.finalize();
    outer.update(&inner_hash);
    outer.finalize()
}

/// PBKDF2-HMAC-SHA256 key derivation (RFC 2898 / RFC 6070).
pub fn pbkdf2_hmac_sha256(
    password: &[u8],
    salt: &[u8],
    rounds: u32,
    out_key: &mut [u8],
) -> Result<(), TTZipStatus> {
    if (password.is_empty() && salt.is_empty()) || rounds == 0 || out_key.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let (base_inner, base_outer) = init_hmac_sha256_contexts(password);
    let key_len = out_key.len();
    let blocks_needed = key_len.div_ceil(32);

    let mut u_digest = [0u8; 32];
    let mut t_digest = [0u8; 32];

    for block_idx in 1..=blocks_needed as u32 {
        let be_block = block_idx.to_be_bytes();

        let mut inner = base_inner.clone();
        inner.update(salt);
        inner.update(&be_block);
        let inner_hash = inner.finalize();

        let mut outer = base_outer.clone();
        outer.update(&inner_hash);
        u_digest = outer.finalize();
        t_digest.copy_from_slice(&u_digest);

        for _ in 1..rounds {
            let mut inner = base_inner.clone();
            inner.update(&u_digest);
            let inner_hash = inner.finalize();

            let mut outer = base_outer.clone();
            outer.update(&inner_hash);
            u_digest = outer.finalize();

            for k in 0..32 {
                t_digest[k] ^= u_digest[k];
            }
        }

        let offset = (block_idx as usize - 1) * 32;
        let copy_len = (offset + 32).min(key_len) - offset;
        out_key[offset..offset + copy_len].copy_from_slice(&t_digest[..copy_len]);
    }

    u_digest.zeroize();
    t_digest.zeroize();
    compiler_fence(Ordering::SeqCst);

    Ok(())
}

/// Cryptographically secure random bytes generator.
pub fn get_random_bytes(buf: &mut [u8]) -> Result<(), TTZipStatus> {
    if buf.is_empty() {
        return Ok(());
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    unsafe {
        libc::arc4random_buf(buf.as_mut_ptr() as *mut libc::c_void, buf.len());
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        use std::io::Read;
        let mut f = std::fs::File::open("/dev/urandom").map_err(|_| TTZipStatus::ErrIo)?;
        f.read_exact(buf).map_err(|_| TTZipStatus::ErrIo)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

