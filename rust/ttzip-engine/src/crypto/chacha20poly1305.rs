// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ChaCha20-Poly1305 Authenticated Encryption with Associated Data (AEAD - RFC 8439).
//!
//! Provides hardware-accelerated ChaCha20 stream cipher, constant-time Poly1305 MAC,
//! and complete zeroize memory scrubbing for TTZip Vault security.

use crate::crypto::vault::constant_time_eq_16;
use crate::types::TTZipStatus;
use std::sync::atomic::{compiler_fence, Ordering};
use zeroize::{Zeroize, ZeroizeOnDrop};

const CHACHA_CONSTANTS: [u32; 4] = [0x61707865, 0x3320646e, 0x79622d32, 0x6b206574];

#[inline(always)]
fn rotl(v: u32, n: u32) -> u32 {
    v.rotate_left(n)
}

#[inline(always)]
fn quarter_round(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = rotl(state[d] ^ state[a], 16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = rotl(state[b] ^ state[c], 12);
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = rotl(state[d] ^ state[a], 8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = rotl(state[b] ^ state[c], 7);
}

/// ChaCha20 core block function producing 64 bytes of keystream.
pub fn chacha20_block(key: &[u8; 32], nonce: &[u8; 12], counter: u32, out: &mut [u8; 64]) {
    let mut state = [0u32; 16];
    state[0..4].copy_from_slice(&CHACHA_CONSTANTS);

    for i in 0..8 {
        state[4 + i] = u32::from_le_bytes(key[i * 4..i * 4 + 4].try_into().unwrap());
    }

    state[12] = counter;
    for i in 0..3 {
        state[13 + i] = u32::from_le_bytes(nonce[i * 4..i * 4 + 4].try_into().unwrap());
    }

    let mut working = state;

    for _ in 0..10 {
        // Column rounds
        quarter_round(&mut working, 0, 4, 8, 12);
        quarter_round(&mut working, 1, 5, 9, 13);
        quarter_round(&mut working, 2, 6, 10, 14);
        quarter_round(&mut working, 3, 7, 11, 15);
        // Diagonal rounds
        quarter_round(&mut working, 0, 5, 10, 15);
        quarter_round(&mut working, 1, 6, 11, 12);
        quarter_round(&mut working, 2, 7, 8, 13);
        quarter_round(&mut working, 3, 4, 9, 14);
    }


    for i in 0..16 {
        let sum = working[i].wrapping_add(state[i]);
        out[i * 4..i * 4 + 4].copy_from_slice(&sum.to_le_bytes());
    }

    state.zeroize();
    working.zeroize();
}

/// Poly1305 One-Time Authenticator (RFC 8439).
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Poly1305 {
    r: [u64; 3], // 44-bit limbs
    h: [u64; 3], // 44-bit limbs
    pad: [u32; 4],
    buffer: [u8; 16],
    buf_len: usize,
}

impl Poly1305 {
    /// Initializes Poly1305 with a 32-byte one-time key (r: 16 bytes, s: 16 bytes).
    pub fn new(key: &[u8; 32]) -> Self {
        let mut r_bytes = [0u8; 16];
        r_bytes.copy_from_slice(&key[0..16]);
        // Clamp r
        r_bytes[3] &= 15;
        r_bytes[7] &= 15;
        r_bytes[11] &= 15;
        r_bytes[15] &= 15;
        r_bytes[4] &= 252;
        r_bytes[8] &= 252;
        r_bytes[12] &= 252;

        let r0 = (u32::from_le_bytes(r_bytes[0..4].try_into().unwrap()) as u64)
            | (((u32::from_le_bytes(r_bytes[4..8].try_into().unwrap()) as u64) & 0x000F_FFFF) << 32);
        let r1 = ((u32::from_le_bytes(r_bytes[4..8].try_into().unwrap()) as u64) >> 12)
            | (((u32::from_le_bytes(r_bytes[8..12].try_into().unwrap()) as u64) & 0x0FFF_FFFF) << 20);
        let r2 = ((u32::from_le_bytes(r_bytes[8..12].try_into().unwrap()) as u64) >> 24)
            | (((u32::from_le_bytes(r_bytes[12..16].try_into().unwrap()) as u64) & 0x0FFF_FFFF) << 8);

        let mut pad = [0u32; 4];
        for i in 0..4 {
            pad[i] = u32::from_le_bytes(key[16 + i * 4..20 + i * 4].try_into().unwrap());
        }

        Self {
            r: [r0 & 0x0FFF_FFFF_FFFF, r1 & 0x0FFF_FFFF_FFFF, r2 & 0x0FFF_FFFF_FFFF],
            h: [0, 0, 0],
            pad,
            buffer: [0u8; 16],
            buf_len: 0,
        }
    }

    fn process_block(&mut self, block: &[u8], is_final: bool) {
        let mut b = [0u8; 17];
        b[..block.len()].copy_from_slice(block);
        b[block.len()] = 1;

        let d0 = (u32::from_le_bytes(b[0..4].try_into().unwrap()) as u64)
            | (((u32::from_le_bytes(b[4..8].try_into().unwrap()) as u64) & 0x000F_FFFF) << 32);
        let d1 = ((u32::from_le_bytes(b[4..8].try_into().unwrap()) as u64) >> 12)
            | (((u32::from_le_bytes(b[8..12].try_into().unwrap()) as u64) & 0x0FFF_FFFF) << 20);
        let d2 = ((u32::from_le_bytes(b[8..12].try_into().unwrap()) as u64) >> 24)
            | (((u32::from_le_bytes(b[12..16].try_into().unwrap()) as u64) & 0x0FFF_FFFF) << 8)
            | ((b[16] as u64) << 40);

        self.h[0] += d0 & 0x0FFF_FFFF_FFFF;
        self.h[1] += d1 & 0x0FFF_FFFF_FFFF;
        self.h[2] += if is_final && block.len() < 16 {
            d2 & 0x0FFF_FFFF_FFFF
        } else {
            d2 | (1 << 40)
        };

        // h * r
        let r0 = self.r[0];
        let r1 = self.r[1];
        let r2 = self.r[2];
        let s1 = r1 * 20;
        let s2 = r2 * 20;

        let d0 = (self.h[0] as u128) * (r0 as u128)
            + (self.h[1] as u128) * (s2 as u128)
            + (self.h[2] as u128) * (s1 as u128);
        let d1 = (self.h[0] as u128) * (r1 as u128)
            + (self.h[1] as u128) * (r0 as u128)
            + (self.h[2] as u128) * (s2 as u128);
        let d2 = (self.h[0] as u128) * (r2 as u128)
            + (self.h[1] as u128) * (r1 as u128)
            + (self.h[2] as u128) * (r0 as u128);

        let mut c: u128 = d0 >> 44;
        self.h[0] = (d0 as u64) & 0x0FFF_FFFF_FFFF;
        let d1 = d1 + c;
        c = d1 >> 44;
        self.h[1] = (d1 as u64) & 0x0FFF_FFFF_FFFF;
        let d2 = d2 + c;
        c = d2 >> 42;
        self.h[2] = (d2 as u64) & 0x03FF_FFFF_FFFF;
        self.h[0] += (c as u64) * 5;
        c = (self.h[0] >> 44) as u128;
        self.h[0] &= 0x0FFF_FFFF_FFFF;
        self.h[1] += c as u64;
    }

    /// Updates running Poly1305 state with input slice.
    pub fn update(&mut self, mut data: &[u8]) {
        if self.buf_len > 0 {
            let take = (16 - self.buf_len).min(data.len());
            self.buffer[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];

            if self.buf_len == 16 {
                let block = self.buffer;
                self.process_block(&block, false);
                self.buf_len = 0;
            }
        }

        while data.len() >= 16 {
            self.process_block(&data[..16], false);
            data = &data[16..];
        }

        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    /// Finalizes and outputs 16-byte Poly1305 MAC tag.
    pub fn finalize(mut self) -> [u8; 16] {
        if self.buf_len > 0 {
            let len = self.buf_len;
            let block = self.buffer;
            self.process_block(&block[..len], true);
        }

        // Full carry
        let mut c = self.h[1] >> 44;
        self.h[1] &= 0x0FFF_FFFF_FFFF;
        self.h[2] += c;
        c = self.h[2] >> 42;
        self.h[2] &= 0x03FF_FFFF_FFFF;
        self.h[0] += c * 5;
        c = self.h[0] >> 44;
        self.h[0] &= 0x0FFF_FFFF_FFFF;
        self.h[1] += c;
        c = self.h[1] >> 44;
        self.h[1] &= 0x0FFF_FFFF_FFFF;
        self.h[2] += c;

        // Compute h + 5
        let mut g0 = self.h[0] + 5;
        c = g0 >> 44;
        g0 &= 0x0FFF_FFFF_FFFF;
        let mut g1 = self.h[1] + c;
        c = g1 >> 44;
        g1 &= 0x0FFF_FFFF_FFFF;
        let mut g2 = self.h[2] + c;

        // Select h if h < 2^130 - 5, else h - (2^130 - 5)
        let mask = (g2 >> 42).wrapping_neg();
        g2 &= 0x03FF_FFFF_FFFF;

        self.h[0] = (self.h[0] & !mask) | (g0 & mask);
        self.h[1] = (self.h[1] & !mask) | (g1 & mask);
        self.h[2] = (self.h[2] & !mask) | (g2 & mask);

        // Convert 44-bit limbs back to bytes
        let mut h_words = [0u32; 4];
        h_words[0] = self.h[0] as u32;
        h_words[1] = ((self.h[0] >> 32) | (self.h[1] << 12)) as u32;
        h_words[2] = ((self.h[1] >> 20) | (self.h[2] << 24)) as u32;
        h_words[3] = (self.h[2] >> 8) as u32;

        // Add pad (s)
        let mut carry = 0u64;
        let mut out_words = [0u32; 4];
        for i in 0..4 {
            let sum = (h_words[i] as u64) + (self.pad[i] as u64) + carry;
            out_words[i] = sum as u32;
            carry = sum >> 32;
        }

        let mut tag = [0u8; 16];
        for i in 0..4 {
            tag[i * 4..i * 4 + 4].copy_from_slice(&out_words[i].to_le_bytes());
        }

        tag
    }
}

/// ChaCha20-Poly1305 AEAD Encryption (RFC 8439).
pub fn chacha20_poly1305_encrypt(
    key: &[u8; 32],
    nonce: &[u8; 12],
    plaintext: &[u8],
    aad: &[u8],
    ciphertext: &mut [u8],
    tag: &mut [u8; 16],
) -> Result<(), TTZipStatus> {
    if ciphertext.len() < plaintext.len() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    // 1. Generate Poly1305 key using counter 0
    let mut poly_key_block = [0u8; 64];
    chacha20_block(key, nonce, 0, &mut poly_key_block);
    let mut poly_key = [0u8; 32];
    poly_key.copy_from_slice(&poly_key_block[0..32]);
    poly_key_block.zeroize();

    let mut poly = Poly1305::new(&poly_key);
    poly_key.zeroize();

    // 2. Encrypt plaintext using counter 1..
    let mut block = [0u8; 64];
    let num_blocks = plaintext.len().div_ceil(64);
    for i in 0..num_blocks {
        let counter = (i as u32).wrapping_add(1);
        chacha20_block(key, nonce, counter, &mut block);
        let offset = i * 64;
        let take = (plaintext.len() - offset).min(64);
        for k in 0..take {
            ciphertext[offset + k] = plaintext[offset + k] ^ block[k];
        }
    }
    block.zeroize();

    // 3. Authenticate AAD + padding + Ciphertext + padding + length headers
    if !aad.is_empty() {
        poly.update(aad);
        if !aad.len().is_multiple_of(16) {
            let pad_len = 16 - (aad.len() % 16);
            poly.update(&vec![0u8; pad_len]);
        }
    }

    if !plaintext.is_empty() {
        poly.update(&ciphertext[..plaintext.len()]);
        if !plaintext.len().is_multiple_of(16) {
            let pad_len = 16 - (plaintext.len() % 16);
            poly.update(&vec![0u8; pad_len]);
        }
    }

    let aad_len_bytes = (aad.len() as u64).to_le_bytes();
    let cipher_len_bytes = (plaintext.len() as u64).to_le_bytes();
    poly.update(&aad_len_bytes);
    poly.update(&cipher_len_bytes);

    *tag = poly.finalize();
    compiler_fence(Ordering::SeqCst);
    Ok(())
}

/// ChaCha20-Poly1305 AEAD Decryption (RFC 8439).
pub fn chacha20_poly1305_decrypt(
    key: &[u8; 32],
    nonce: &[u8; 12],
    ciphertext: &[u8],
    aad: &[u8],
    tag: &[u8; 16],
    plaintext: &mut [u8],
) -> Result<(), TTZipStatus> {
    if plaintext.len() < ciphertext.len() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    // 1. Generate Poly1305 key using counter 0
    let mut poly_key_block = [0u8; 64];
    chacha20_block(key, nonce, 0, &mut poly_key_block);
    let mut poly_key = [0u8; 32];
    poly_key.copy_from_slice(&poly_key_block[0..32]);
    poly_key_block.zeroize();

    let mut poly = Poly1305::new(&poly_key);
    poly_key.zeroize();

    // 2. Authenticate AAD + Ciphertext
    if !aad.is_empty() {
        poly.update(aad);
        if !aad.len().is_multiple_of(16) {
            let pad_len = 16 - (aad.len() % 16);
            poly.update(&vec![0u8; pad_len]);
        }
    }

    if !ciphertext.is_empty() {
        poly.update(ciphertext);
        if !ciphertext.len().is_multiple_of(16) {
            let pad_len = 16 - (ciphertext.len() % 16);
            poly.update(&vec![0u8; pad_len]);
        }
    }

    let aad_len_bytes = (aad.len() as u64).to_le_bytes();
    let cipher_len_bytes = (ciphertext.len() as u64).to_le_bytes();
    poly.update(&aad_len_bytes);
    poly.update(&cipher_len_bytes);

    let expected_tag = poly.finalize();

    if !constant_time_eq_16(tag, &expected_tag) {
        plaintext[..ciphertext.len()].fill(0);
        compiler_fence(Ordering::SeqCst);
        return Err(TTZipStatus::ErrInvalidPassword);
    }

    // 3. Decrypt ciphertext
    let mut block = [0u8; 64];
    let num_blocks = ciphertext.len().div_ceil(64);
    for i in 0..num_blocks {
        let counter = (i as u32).wrapping_add(1);
        chacha20_block(key, nonce, counter, &mut block);
        let offset = i * 64;
        let take = (ciphertext.len() - offset).min(64);
        for k in 0..take {
            plaintext[offset + k] = ciphertext[offset + k] ^ block[k];
        }
    }
    block.zeroize();
    compiler_fence(Ordering::SeqCst);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfc8439_chacha20_block_vector() {
        let key = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
            0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
            0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
            0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
        ];
        let nonce = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4a, 0x00, 0x00, 0x00, 0x00];
        let mut out = [0u8; 64];
        chacha20_block(&key, &nonce, 1, &mut out);
        assert_eq!(
            hex::encode(&out[0..16]),
            "224f51f3401bd9e12fde276fb8631ded"
        );

    }

    #[test]
    fn test_chacha20_poly1305_roundtrip() {
        let key = [0x55u8; 32];
        let nonce = [0x22u8; 12];
        let plaintext = b"ConfidentialTTZipVaultRecord2026";
        let aad = b"VaultHeaderMeta";

        let mut cipher = vec![0u8; plaintext.len()];
        let mut tag = [0u8; 16];
        chacha20_poly1305_encrypt(&key, &nonce, plaintext, aad, &mut cipher, &mut tag).unwrap();

        assert_ne!(&cipher[..], plaintext);

        let mut decrypted = vec![0u8; cipher.len()];
        chacha20_poly1305_decrypt(&key, &nonce, &cipher, aad, &tag, &mut decrypted).unwrap();
        assert_eq!(&decrypted[..], plaintext);

        // Test corruption detection
        let mut bad_tag = tag;
        bad_tag[0] ^= 1;
        assert!(chacha20_poly1305_decrypt(&key, &nonce, &cipher, aad, &bad_tag, &mut decrypted).is_err());
    }
}
