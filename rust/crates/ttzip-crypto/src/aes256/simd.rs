// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! ARM64 NEON Hardware Pipelined AES-256 Implementations.

#[cfg(target_arch = "aarch64")]
use super::Aes256Context;
#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;
#[cfg(target_arch = "aarch64")]
use zeroize::Zeroize;

#[cfg(target_arch = "aarch64")]
#[inline]
#[target_feature(enable = "aes")]
unsafe fn aes256_encrypt_8blocks_neon(b: &mut [uint8x16_t; 8], rk: &[uint8x16_t; 15]) {
    for &round_key in rk.iter().take(13) {
        for lane in b.iter_mut() {
            *lane = vaesmcq_u8(vaeseq_u8(*lane, round_key));
        }
    }
    for lane in b.iter_mut() {
        *lane = veorq_u8(vaeseq_u8(*lane, rk[13]), rk[14]);
    }
}

#[cfg(target_arch = "aarch64")]
#[inline]
#[target_feature(enable = "aes")]
unsafe fn apply_ctr_8blocks_neon(
    b: &[uint8x16_t; 8],
    src: *const u8,
    dst: *mut u8,
    block_offset: usize,
    rem: usize,
) {
    unsafe {
        if rem == 128 {
            for lane in 0..8 {
                let off = block_offset + lane * 16;
                let s = vld1q_u8(src.add(off));
                vst1q_u8(dst.add(off), veorq_u8(s, b[lane]));
            }
        } else {
            let mut ks = [0u8; 128];
            for lane in 0..8 {
                vst1q_u8(ks.as_mut_ptr().add(lane * 16), b[lane]);
            }
            for (k, &keystream_byte) in ks.iter().take(rem).enumerate() {
                *dst.add(block_offset + k) = *src.add(block_offset + k) ^ keystream_byte;
            }
            ks.zeroize();
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        }
    }
}

/// Encrypts or decrypts bytes in CTR mode using ARM NEON AES vector instructions.
///
/// # Safety
/// Caller must ensure `src` has at least `len` readable bytes, `dst` has at least `len` writable bytes,
/// and CPU supports ARMv8 Cryptography Extensions (FEAT_AES).
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "aes")]
pub unsafe fn aes256_ctr_crypt_neon(
    ctx: &Aes256Context,
    initial_counter: u64,
    src: *const u8,
    len: usize,
    dst: *mut u8,
) {
    unsafe {
        let mut rk = [vdupq_n_u8(0); 15];
        for (r, key) in rk.iter_mut().zip(ctx.round_keys_enc.iter()) {
            *r = vld1q_u8(key.as_ptr());
        }

        let num_blocks = len.div_ceil(16);
        let mut i = 0;

        let zero = vdup_n_u64(0);
        let mut ctr_vec = vcombine_u64(vcreate_u64(initial_counter), zero);
        let step1 = vcombine_u64(vcreate_u64(1), zero);
        let step8 = vcombine_u64(vcreate_u64(8), zero);

        while i + 8 <= num_blocks {
            let mut b: [uint8x16_t; 8] = core::array::from_fn(|lane| {
                let step = vcombine_u64(vcreate_u64(lane as u64), zero);
                vreinterpretq_u8_u64(vaddq_u64(ctr_vec, step))
            });
            ctr_vec = vaddq_u64(ctr_vec, step8);

            aes256_encrypt_8blocks_neon(&mut b, &rk);

            let block_offset = i * 16;
            let rem = (len - block_offset).min(128);
            apply_ctr_8blocks_neon(&b, src, dst, block_offset, rem);

            i += 8;
        }

        while i < num_blocks {
            let mut b0 = vreinterpretq_u8_u64(ctr_vec);
            ctr_vec = vaddq_u64(ctr_vec, step1);

            for &round_key in rk.iter().take(13) {
                b0 = vaesmcq_u8(vaeseq_u8(b0, round_key));
            }
            b0 = veorq_u8(vaeseq_u8(b0, rk[13]), rk[14]);

            let block_offset = i * 16;
            let rem = (len - block_offset).min(16);

            if rem == 16 {
                let s0 = vld1q_u8(src.add(block_offset));
                vst1q_u8(dst.add(block_offset), veorq_u8(s0, b0));
            } else {
                let mut ks = [0u8; 16];
                vst1q_u8(ks.as_mut_ptr(), b0);
                for (k, &keystream_byte) in ks.iter().take(rem).enumerate() {
                    *dst.add(block_offset + k) = *src.add(block_offset + k) ^ keystream_byte;
                }
                ks.zeroize();
                core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
            }

            i += 1;
        }
    }
}

#[cfg(target_arch = "aarch64")]
#[inline]
#[target_feature(enable = "aes")]
unsafe fn aes256_decrypt_8blocks_neon(b: &mut [uint8x16_t; 8], rk_dec: &[uint8x16_t; 15]) {
    for &round_key in rk_dec.iter().take(13) {
        for lane in b.iter_mut() {
            *lane = vaesimcq_u8(vaesdq_u8(*lane, round_key));
        }
    }
    for lane in b.iter_mut() {
        *lane = veorq_u8(vaesdq_u8(*lane, rk_dec[13]), rk_dec[14]);
    }
}

/// Decrypts bytes in CBC mode using ARM NEON AES vector instructions.
///
/// # Safety
/// Caller must ensure `src` has at least `len` readable bytes, `dst` has at least `len` writable bytes,
/// and CPU supports ARMv8 Cryptography Extensions (FEAT_AES).
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "aes")]
pub unsafe fn aes256_cbc_decrypt_neon(
    ctx: &Aes256Context,
    iv: &[u8; 16],
    src: *const u8,
    len: usize,
    dst: *mut u8,
) {
    unsafe {
        let mut rk_dec = [vdupq_n_u8(0); 15];
        for (r, key) in rk_dec.iter_mut().zip(ctx.round_keys_dec.iter()) {
            *r = vld1q_u8(key.as_ptr());
        }

        let num_blocks = len / 16;
        let mut current_iv = vld1q_u8(iv.as_ptr());
        let mut i = 0;

        while i + 8 <= num_blocks {
            let offset = i * 16;
            let c: [uint8x16_t; 8] = core::array::from_fn(|lane| vld1q_u8(src.add(offset + lane * 16)));
            let mut b = c;

            aes256_decrypt_8blocks_neon(&mut b, &rk_dec);

            vst1q_u8(dst.add(offset), veorq_u8(b[0], current_iv));
            for lane in 1..8 {
                vst1q_u8(dst.add(offset + lane * 16), veorq_u8(b[lane], c[lane - 1]));
            }
            current_iv = c[7];

            i += 8;
        }

        while i < num_blocks {
            let offset = i * 16;
            let c = vld1q_u8(src.add(offset));
            let mut b = c;
            for &round_key in rk_dec.iter().take(13) {
                b = vaesimcq_u8(vaesdq_u8(b, round_key));
            }
            b = veorq_u8(vaesdq_u8(b, rk_dec[13]), rk_dec[14]);
            let p = veorq_u8(b, current_iv);
            current_iv = c;
            vst1q_u8(dst.add(offset), p);
            i += 1;
        }
    }
}

/// Encrypts bytes in CBC mode using ARM NEON AES vector instructions.
///
/// # Safety
/// Caller must ensure `src` has at least `len` readable bytes, `dst` has at least `len` writable bytes,
/// and CPU supports ARMv8 Cryptography Extensions (FEAT_AES).
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "aes")]
pub unsafe fn aes256_cbc_encrypt_neon(
    ctx: &Aes256Context,
    iv: &[u8; 16],
    src: *const u8,
    len: usize,
    dst: *mut u8,
) {
    unsafe {
        let mut rk_enc = [vdupq_n_u8(0); 15];
        for (r, key) in rk_enc.iter_mut().zip(ctx.round_keys_enc.iter()) {
            *r = vld1q_u8(key.as_ptr());
        }

        let num_blocks = len / 16;
        let mut current_iv = vld1q_u8(iv.as_ptr());

        for i in 0..num_blocks {
            let offset = i * 16;
            let p = vld1q_u8(src.add(offset));
            let mut b = veorq_u8(p, current_iv);
            for &round_key in rk_enc.iter().take(13) {
                b = vaesmcq_u8(vaeseq_u8(b, round_key));
            }
            b = veorq_u8(vaeseq_u8(b, rk_enc[13]), rk_enc[14]);
            current_iv = b;
            vst1q_u8(dst.add(offset), b);
        }
    }
}
