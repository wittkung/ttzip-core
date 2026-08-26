// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! RFC 1321 compliant MD5 Message-Digest Algorithm.
//!
//! Provides stack-allocated, zero-heap streaming MD5 hashing and one-shot digest calculation.

const S11: u32 = 7;
const S12: u32 = 12;
const S13: u32 = 17;
const S14: u32 = 22;
const S21: u32 = 5;
const S22: u32 = 9;
const S23: u32 = 14;
const S24: u32 = 20;
const S31: u32 = 4;
const S32: u32 = 11;
const S33: u32 = 16;
const S34: u32 = 23;
const S41: u32 = 6;
const S42: u32 = 10;
const S43: u32 = 15;
const S44: u32 = 21;

#[inline(always)]
fn f(x: u32, y: u32, z: u32) -> u32 {
    (x & y) | (!x & z)
}

#[inline(always)]
fn g(x: u32, y: u32, z: u32) -> u32 {
    (x & z) | (y & !z)
}

#[inline(always)]
fn h(x: u32, y: u32, z: u32) -> u32 {
    x ^ y ^ z
}

#[inline(always)]
fn i_func(x: u32, y: u32, z: u32) -> u32 {
    y ^ (x | !z)
}

#[inline(always)]
fn ff(a: &mut u32, b: u32, c: u32, d: u32, x: u32, s: u32, ac: u32) {
    *a = a.wrapping_add(f(b, c, d)).wrapping_add(x).wrapping_add(ac);
    *a = a.rotate_left(s).wrapping_add(b);
}

#[inline(always)]
fn gg(a: &mut u32, b: u32, c: u32, d: u32, x: u32, s: u32, ac: u32) {
    *a = a.wrapping_add(g(b, c, d)).wrapping_add(x).wrapping_add(ac);
    *a = a.rotate_left(s).wrapping_add(b);
}

#[inline(always)]
fn hh(a: &mut u32, b: u32, c: u32, d: u32, x: u32, s: u32, ac: u32) {
    *a = a.wrapping_add(h(b, c, d)).wrapping_add(x).wrapping_add(ac);
    *a = a.rotate_left(s).wrapping_add(b);
}

#[inline(always)]
fn ii(a: &mut u32, b: u32, c: u32, d: u32, x: u32, s: u32, ac: u32) {
    *a = a.wrapping_add(i_func(b, c, d)).wrapping_add(x).wrapping_add(ac);
    *a = a.rotate_left(s).wrapping_add(b);
}

fn md5_transform(state: &mut [u32; 4], block: &[u8; 64]) {
    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];

    let mut x = [0u32; 16];
    for j in 0..16 {
        x[j] = u32::from_le_bytes(block[j * 4..j * 4 + 4].try_into().unwrap());
    }

    // Round 1
    ff(&mut a, b, c, d, x[0], S11, 0xd76aa478);
    ff(&mut d, a, b, c, x[1], S12, 0xe8c7b756);
    ff(&mut c, d, a, b, x[2], S13, 0x242070db);
    ff(&mut b, c, d, a, x[3], S14, 0xc1bdceee);
    ff(&mut a, b, c, d, x[4], S11, 0xf57c0faf);
    ff(&mut d, a, b, c, x[5], S12, 0x4787c62a);
    ff(&mut c, d, a, b, x[6], S13, 0xa8304613);
    ff(&mut b, c, d, a, x[7], S14, 0xfd469501);
    ff(&mut a, b, c, d, x[8], S11, 0x698098d8);
    ff(&mut d, a, b, c, x[9], S12, 0x8b44f7af);
    ff(&mut c, d, a, b, x[10], S13, 0xffff5bb1);
    ff(&mut b, c, d, a, x[11], S14, 0x895cd7be);
    ff(&mut a, b, c, d, x[12], S11, 0x6b901122);
    ff(&mut d, a, b, c, x[13], S12, 0xfd987193);
    ff(&mut c, d, a, b, x[14], S13, 0xa679438e);
    ff(&mut b, c, d, a, x[15], S14, 0x49b40821);

    // Round 2
    gg(&mut a, b, c, d, x[1], S21, 0xf61e2562);
    gg(&mut d, a, b, c, x[6], S22, 0xc040b340);
    gg(&mut c, d, a, b, x[11], S23, 0x265e5a51);
    gg(&mut b, c, d, a, x[0], S24, 0xe9b6c7aa);
    gg(&mut a, b, c, d, x[5], S21, 0xd62f105d);
    gg(&mut d, a, b, c, x[10], S22, 0x02441453);
    gg(&mut c, d, a, b, x[15], S23, 0xd8a1e681);
    gg(&mut b, c, d, a, x[4], S24, 0xe7d3fbc8);
    gg(&mut a, b, c, d, x[9], S21, 0x21e1cde6);
    gg(&mut d, a, b, c, x[14], S22, 0xc33707d6);
    gg(&mut c, d, a, b, x[3], S23, 0xf4d50d87);
    gg(&mut b, c, d, a, x[8], S24, 0x455a14ed);
    gg(&mut a, b, c, d, x[13], S21, 0xa9e3e905);
    gg(&mut d, a, b, c, x[2], S22, 0xfcefa3f8);
    gg(&mut c, d, a, b, x[7], S23, 0x676f02d9);
    gg(&mut b, c, d, a, x[12], S24, 0x8d2a4c8a);

    // Round 3
    hh(&mut a, b, c, d, x[5], S31, 0xfffa3942);
    hh(&mut d, a, b, c, x[8], S32, 0x8771f681);
    hh(&mut c, d, a, b, x[11], S33, 0x6d9d6122);
    hh(&mut b, c, d, a, x[14], S34, 0xfde5380c);
    hh(&mut a, b, c, d, x[1], S31, 0xa4beea44);
    hh(&mut d, a, b, c, x[4], S32, 0x4bdecfa9);
    hh(&mut c, d, a, b, x[7], S33, 0xf6bb4b60);
    hh(&mut b, c, d, a, x[10], S34, 0xbebfbc70);
    hh(&mut a, b, c, d, x[13], S31, 0x289b7ec6);
    hh(&mut d, a, b, c, x[0], S32, 0xeaa127fa);
    hh(&mut c, d, a, b, x[3], S33, 0xd4ef3085);
    hh(&mut b, c, d, a, x[6], S34, 0x04881d05);
    hh(&mut a, b, c, d, x[9], S31, 0xd9d4d039);
    hh(&mut d, a, b, c, x[12], S32, 0xe6db99e5);
    hh(&mut c, d, a, b, x[15], S33, 0x1fa27cf8);
    hh(&mut b, c, d, a, x[2], S34, 0xc4ac5665);

    // Round 4
    ii(&mut a, b, c, d, x[0], S41, 0xf4292244);
    ii(&mut d, a, b, c, x[7], S42, 0x432aff97);
    ii(&mut c, d, a, b, x[14], S43, 0xab9423a7);
    ii(&mut b, c, d, a, x[5], S44, 0xfc93a039);
    ii(&mut a, b, c, d, x[12], S41, 0x655b59c3);
    ii(&mut d, a, b, c, x[3], S42, 0x8f0ccc92);
    ii(&mut c, d, a, b, x[10], S43, 0xffeff47d);
    ii(&mut b, c, d, a, x[1], S44, 0x85845dd1);
    ii(&mut a, b, c, d, x[8], S41, 0x6fa87e4f);
    ii(&mut d, a, b, c, x[15], S42, 0xfe2ce6e0);
    ii(&mut c, d, a, b, x[6], S43, 0xa3014314);
    ii(&mut b, c, d, a, x[13], S44, 0x4e0811a1);
    ii(&mut a, b, c, d, x[4], S41, 0xf7537e82);
    ii(&mut d, a, b, c, x[11], S42, 0xbd3af235);
    ii(&mut c, d, a, b, x[2], S43, 0x2ad7d2bb);
    ii(&mut b, c, d, a, x[9], S44, 0xeb86d391);

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
}

/// Fast stack-allocated streaming MD5 hasher.
#[derive(Clone)]
pub struct FastMd5 {
    state: [u32; 4],
    buffer: [u8; 64],
    buf_len: usize,
    total_len: u64,
}

impl Default for FastMd5 {
    fn default() -> Self {
        Self::new()
    }
}

impl FastMd5 {
    pub const fn new() -> Self {
        Self {
            state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476],
            buffer: [0u8; 64],
            buf_len: 0,
            total_len: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total_len += data.len() as u64;

        if self.buf_len > 0 {
            let take = (64 - self.buf_len).min(data.len());
            self.buffer[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];

            if self.buf_len == 64 {
                let block = self.buffer;
                md5_transform(&mut self.state, &block);
                self.buf_len = 0;
            }
        }

        while data.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[..64]);
            md5_transform(&mut self.state, &block);
            data = &data[64..];
        }

        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    pub fn finalize(mut self) -> [u8; 16] {
        let bit_len = self.total_len * 8;
        self.buffer[self.buf_len] = 0x80;
        self.buf_len += 1;

        if self.buf_len > 56 {
            self.buffer[self.buf_len..64].fill(0);
            let block = self.buffer;
            md5_transform(&mut self.state, &block);
            self.buf_len = 0;
        }

        self.buffer[self.buf_len..56].fill(0);
        self.buffer[56..64].copy_from_slice(&bit_len.to_le_bytes());
        let block = self.buffer;
        md5_transform(&mut self.state, &block);

        let mut out = [0u8; 16];
        for i in 0..4 {
            out[i * 4..i * 4 + 4].copy_from_slice(&self.state[i].to_le_bytes());
        }
        out
    }
}

/// Computes MD5 hash for a slice of bytes.
#[inline]
pub fn md5(data: &[u8]) -> [u8; 16] {
    let mut hasher = FastMd5::new();
    hasher.update(data);
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn test_md5_rfc1321_vectors() {
        assert_eq!(to_hex(&md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(to_hex(&md5(b"a")), "0cc175b9c0f1b6a831c399e269772661");
        assert_eq!(to_hex(&md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            to_hex(&md5(b"message digest")),
            "f96b697d7cb7938d525a2f31aaf161d0"
        );
        assert_eq!(
            to_hex(&md5(b"abcdefghijklmnopqrstuvwxyz")),
            "c3fcd3d76192e4007dfb496cca67e13b"
        );
        assert_eq!(
            to_hex(&md5(b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789")),
            "d174ab98d277d9f5a5611c2c9f419d9f"
        );
        assert_eq!(
            to_hex(&md5(b"12345678901234567890123456789012345678901234567890123456789012345678901234567890")),
            "57edf4a22be3c955ac49da2e2107b67a"
        );
    }
}
