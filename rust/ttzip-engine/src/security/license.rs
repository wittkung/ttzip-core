// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Sub-millisecond Offline Ed25519 License Verification Engine (RFC 8032).
//!
//! Provides cross-platform deterministic license token validation without platform-specific dependencies.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};

/// Default TTZip embedded Ed25519 public key (32 bytes in Base64).
pub const DEFAULT_PUBLIC_KEY_BASE64: &str = "pOkv5VfIP3WVbXalJnc+OkkLGo1MazH4m0TMPw8dZrs=";

/// Structured payload contained within an Ed25519 signed license key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct UniFFILicensePayload {
    pub version: i32,
    pub email: String,
    pub tier: String,
    pub issued_at: String,
    pub order_id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct InternalLicensePayload {
    #[serde(alias = "version", default = "default_version")]
    pub v: i32,
    pub email: String,
    pub tier: String,
    pub issued_at: String,
    pub order_id: String,
}

fn default_version() -> i32 {
    1
}

/// Verification result enumeration exposed to UniFFI.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum UniFFILicenseResult {
    Valid { payload: UniFFILicensePayload },
    InvalidSignature,
    MalformedKey { reason: String },
}

/// Verifies a license key against embedded or custom Ed25519 public key.
#[uniffi::export]
pub fn verify_license_key(
    license_key: String,
    public_key_base64: Option<String>,
) -> UniFFILicenseResult {
    let trimmed = license_key.trim();
    if !trimmed.starts_with("TTZIP1-") {
        return UniFFILicenseResult::MalformedKey {
            reason: "Missing TTZIP1- protocol prefix".to_string(),
        };
    }

    let token = &trimmed["TTZIP1-".len()..];
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 2 {
        return UniFFILicenseResult::MalformedKey {
            reason: "Invalid token format, expected <payload>.<signature>".to_string(),
        };
    }

    let payload_b64 = parts[0];
    let sig_b64 = parts[1];

    let payload_bytes = match decode_base64(payload_b64) {
        Ok(b) if !b.is_empty() => b,
        _ => {
            return UniFFILicenseResult::MalformedKey {
                reason: "Failed to decode base64 payload".to_string(),
            }
        }
    };

    let sig_bytes = match decode_base64(sig_b64) {
        Ok(b) if b.len() == 64 => b,
        _ => {
            return UniFFILicenseResult::MalformedKey {
                reason: "Failed to decode base64 signature".to_string(),
            }
        }
    };

    let pub_key_str = public_key_base64.as_deref().unwrap_or(DEFAULT_PUBLIC_KEY_BASE64);
    let pub_key_bytes = match decode_base64(pub_key_str) {
        Ok(b) if b.len() == 32 => b,
        Ok(_) => {
            return UniFFILicenseResult::MalformedKey {
                reason: "Failed to initialize Ed25519 public key from raw bytes".to_string(),
            }
        }
        Err(_) => {
            return UniFFILicenseResult::MalformedKey {
                reason: "Invalid base64 public key representation".to_string(),
            }
        }
    };

    let mut pk_arr = [0u8; 32];
    pk_arr.copy_from_slice(&pub_key_bytes);

    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);

    if !ed25519_verify_signature(&pk_arr, &payload_bytes, &sig_arr) {
        return UniFFILicenseResult::InvalidSignature;
    }

    let json_payload: InternalLicensePayload = match serde_json::from_slice(&payload_bytes) {
        Ok(p) => p,
        Err(_) => {
            return UniFFILicenseResult::MalformedKey {
                reason: "Failed to decode LicensePayload JSON".to_string(),
            }
        }
    };

    UniFFILicenseResult::Valid {
        payload: UniFFILicensePayload {
            version: json_payload.v,
            email: json_payload.email,
            tier: json_payload.tier,
            issued_at: json_payload.issued_at,
            order_id: json_payload.order_id,
        },
    }
}

fn decode_base64_char(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}

fn decode_base64(input: &str) -> Result<Vec<u8>, ()> {
    let clean: Vec<u8> = input
        .bytes()
        .filter(|&b| !b.is_ascii_whitespace())
        .collect();
    if clean.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::with_capacity((clean.len() * 3) / 4);
    let mut buf = 0u32;
    let mut bits = 0;

    for &b in &clean {
        if b == b'=' {
            break;
        }
        let val = decode_base64_char(b).ok_or(())?;
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

// ==============================================================================
// Pure Safe Rust Ed25519 Curve25519 Arithmetic (RFC 8032)
// ==============================================================================

const P_WORDS: [u64; 4] = [
    0xffffffffffffffed,
    0xffffffffffffffff,
    0xffffffffffffffff,
    0x7fffffffffffffff,
];

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct FieldElement([u64; 4]);

#[allow(dead_code)]
impl FieldElement {
    const ZERO: Self = FieldElement([0, 0, 0, 0]);
    const ONE: Self = FieldElement([1, 0, 0, 0]);
    const D: Self = FieldElement([
        0x75eb4dca135978a3,
        0x00700a4d4141d8ab,
        0x8cc740797779e898,
        0x52036cee2b6ffe73,
    ]);
    const D2: Self = FieldElement([
        0xebd69b9426b2f159,
        0x00e0149a8283b156,
        0x198e80f2eef3d130,
        0x2406d9dc56dffce7,
    ]);

    fn sqrt_m1() -> Self {
        const P_MINUS_1_DIV_4: [u64; 4] = [
            0xfffffffffffffffb,
            0xffffffffffffffff,
            0xffffffffffffffff,
            0x1fffffffffffffff,
        ];
        FieldElement([2, 0, 0, 0]).pow(&P_MINUS_1_DIV_4)
    }

    fn from_bytes(bytes: &[u8; 32]) -> Self {
        let mut w = [0u64; 4];
        for i in 0..4 {
            let chunk: [u8; 8] = bytes[i * 8..(i + 1) * 8].try_into().unwrap();
            w[i] = u64::from_le_bytes(chunk);
        }
        w[3] &= 0x7fffffffffffffff;
        FieldElement(w)
    }

    #[cfg(test)]
    fn to_bytes(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        for i in 0..4 {
            out[i * 8..(i + 1) * 8].copy_from_slice(&self.0[i].to_le_bytes());
        }
        out
    }

    fn add(&self, rhs: &Self) -> Self {
        let mut sum = [0u64; 4];
        let mut carry = 0u128;
        for i in 0..4 {
            let s = (self.0[i] as u128) + (rhs.0[i] as u128) + carry;
            sum[i] = s as u64;
            carry = s >> 64;
        }
        reduce_256(sum, carry as u64)
    }

    fn sub(&self, rhs: &Self) -> Self {
        let mut diff = [0u64; 4];
        let mut borrow = 0u128;
        for i in 0..4 {
            let d = (self.0[i] as u128)
                .wrapping_sub(rhs.0[i] as u128)
                .wrapping_sub(borrow);
            diff[i] = d as u64;
            borrow = if d > u64::MAX as u128 { 1 } else { 0 };
        }
        if borrow != 0 {
            let mut carry = 0u128;
            for i in 0..4 {
                let s = (diff[i] as u128) + (P_WORDS[i] as u128) + carry;
                diff[i] = s as u64;
                carry = s >> 64;
            }
        }
        diff_reduce(diff)
    }

    fn mul(&self, rhs: &Self) -> Self {
        let mut prod = [0u64; 8];
        for i in 0..4 {
            let mut carry = 0u128;
            for j in 0..4 {
                let cur = prod[i + j] as u128 + (self.0[i] as u128) * (rhs.0[j] as u128) + carry;
                prod[i + j] = cur as u64;
                carry = cur >> 64;
            }
            prod[i + 4] = carry as u64;
        }
        reduce_512(&prod)
    }

    fn square(&self) -> Self {
        self.mul(self)
    }

    fn pow(&self, exp: &[u64; 4]) -> Self {
        let mut res = FieldElement::ONE;
        let mut cur = *self;
        for word in exp {
            let mut w = *word;
            for _ in 0..64 {
                if (w & 1) == 1 {
                    res = res.mul(&cur);
                }
                cur = cur.square();
                w >>= 1;
            }
        }
        res
    }

    #[cfg(test)]
    fn invert(&self) -> Self {
        const P_MINUS_2: [u64; 4] = [
            0xffffffffffffffeb,
            0xffffffffffffffff,
            0xffffffffffffffff,
            0x7fffffffffffffff,
        ];
        self.pow(&P_MINUS_2)
    }

    fn is_odd(&self) -> bool {
        (self.0[0] & 1) == 1
    }
}

fn reduce_256(mut w: [u64; 4], carry: u64) -> FieldElement {
    let top = (w[3] >> 63) + (carry << 1);
    w[3] &= 0x7fffffffffffffff;

    let mut c = (top as u128) * 19;
    for slot in &mut w {
        let s = (*slot as u128) + c;
        *slot = s as u64;
        c = s >> 64;
    }
    diff_reduce(w)
}

fn diff_reduce(mut w: [u64; 4]) -> FieldElement {
    if w[3] >> 63 != 0 {
        let top = w[3] >> 63;
        w[3] &= 0x7fffffffffffffff;
        let mut c = (top as u128) * 19;
        for slot in &mut w {
            let s = (*slot as u128) + c;
            *slot = s as u64;
            c = s >> 64;
        }
    }
    if field_ge(&w, &P_WORDS) {
        let mut borrow = 0u128;
        for i in 0..4 {
            let d = (w[i] as u128)
                .wrapping_sub(P_WORDS[i] as u128)
                .wrapping_sub(borrow);
            w[i] = d as u64;
            borrow = if d > u64::MAX as u128 { 1 } else { 0 };
        }
    }
    FieldElement(w)
}

fn field_ge(a: &[u64; 4], b: &[u64; 4]) -> bool {
    for i in (0..4).rev() {
        if a[i] > b[i] {
            return true;
        }
        if a[i] < b[i] {
            return false;
        }
    }
    true
}

fn reduce_512(prod: &[u64; 8]) -> FieldElement {
    let w0 = prod[0];
    let w1 = prod[1];
    let w2 = prod[2];
    let w3 = prod[3] & 0x7fffffffffffffff;

    let h0 = (prod[3] >> 63) | (prod[4] << 1);
    let h1 = (prod[4] >> 63) | (prod[5] << 1);
    let h2 = (prod[5] >> 63) | (prod[6] << 1);
    let h3 = (prod[6] >> 63) | (prod[7] << 1);
    let h4 = prod[7] >> 63;

    let mut r = [w0, w1, w2, w3];
    let h = [h0, h1, h2, h3];
    let mut carry = 0u128;
    for i in 0..4 {
        let cur = (r[i] as u128) + (h[i] as u128) * 19 + carry;
        r[i] = cur as u64;
        carry = cur >> 64;
    }
    let top_carry = (h4 as u128) * 19 + carry;
    reduce_256(r, top_carry as u64)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct ExtendedPoint {
    x: FieldElement,
    y: FieldElement,
    z: FieldElement,
    t: FieldElement,
}

impl ExtendedPoint {
    const IDENTITY: Self = ExtendedPoint {
        x: FieldElement::ZERO,
        y: FieldElement::ONE,
        z: FieldElement::ONE,
        t: FieldElement::ZERO,
    };

    fn base() -> Self {
        ExtendedPoint {
            x: FieldElement([
                0xc9562d608f25d51a,
                0x692cc7609525a7b2,
                0xc0a4e231fdd6dc5c,
                0x216936d3cd6e53fe,
            ]),
            y: FieldElement([
                0x6666666666666658,
                0x6666666666666666,
                0x6666666666666666,
                0x6666666666666666,
            ]),
            z: FieldElement::ONE,
            t: FieldElement([
                0xc9562d608f25d51a,
                0x692cc7609525a7b2,
                0xc0a4e231fdd6dc5c,
                0x216936d3cd6e53fe,
            ])
            .mul(&FieldElement([
                0x6666666666666658,
                0x6666666666666666,
                0x6666666666666666,
                0x6666666666666666,
            ])),
        }
    }

    fn from_bytes(bytes: &[u8; 32]) -> Option<Self> {
        let sign_bit = (bytes[31] >> 7) & 1;
        let y = FieldElement::from_bytes(bytes);
        let y2 = y.square();
        let u = y2.sub(&FieldElement::ONE);
        let v = FieldElement::D.mul(&y2).add(&FieldElement::ONE);

        // x = sqrt(u / v) via candidate x = u * v^3 * (u * v^7)^((p-5)/8)
        let v3 = v.square().mul(&v);
        let v7 = v3.square().mul(&v);
        let uv7 = u.mul(&v7);

        const P_MINUS_5_DIV_8: [u64; 4] = [
            0xfffffffffffffffd,
            0xffffffffffffffff,
            0xffffffffffffffff,
            0x0fffffffffffffff,
        ];

        let mut x = u.mul(&v3).mul(&uv7.pow(&P_MINUS_5_DIV_8));
        let vx2 = v.mul(&x.square());

        if vx2 != u {
            let neg_u = FieldElement::ZERO.sub(&u);
            if vx2 == neg_u {
                x = x.mul(&FieldElement::sqrt_m1());
            } else {
                return None;
            }
        }

        if (x.is_odd() as u8) != sign_bit {
            x = FieldElement::ZERO.sub(&x);
        }

        let t = x.mul(&y);
        Some(ExtendedPoint {
            x,
            y,
            z: FieldElement::ONE,
            t,
        })
    }

    fn add(&self, rhs: &Self) -> Self {
        let a = (self.y.sub(&self.x)).mul(&rhs.y.sub(&rhs.x));
        let b = (self.y.add(&self.x)).mul(&rhs.y.add(&rhs.x));
        let c = self.t.mul(&rhs.t).mul(&FieldElement::D2);
        let d = self.z.mul(&rhs.z).mul(&FieldElement([2, 0, 0, 0]));
        let e = b.sub(&a);
        let f = d.sub(&c);
        let g = d.add(&c);
        let h = b.add(&a);

        ExtendedPoint {
            x: e.mul(&f),
            y: g.mul(&h),
            z: f.mul(&g),
            t: e.mul(&h),
        }
    }

    fn double(&self) -> Self {
        let a = self.x.square();
        let b = self.y.square();
        let c = self.z.square().mul(&FieldElement([2, 0, 0, 0]));
        let d = FieldElement::ZERO.sub(&a);
        let e = (self.x.add(&self.y)).square().sub(&a).sub(&b);
        let g = d.add(&b);
        let f = g.sub(&c);
        let h = d.sub(&b);

        ExtendedPoint {
            x: e.mul(&f),
            y: g.mul(&h),
            z: f.mul(&g),
            t: e.mul(&h),
        }
    }

    fn scalar_mul(&self, scalar: &[u64; 4]) -> Self {
        let mut res = Self::IDENTITY;
        let mut cur = *self;
        for word in scalar {
            let mut w = *word;
            for _ in 0..64 {
                if (w & 1) == 1 {
                    res = res.add(&cur);
                }
                cur = cur.double();
                w >>= 1;
            }
        }
        res
    }

    fn equals(&self, rhs: &Self) -> bool {
        let x1z2 = self.x.mul(&rhs.z);
        let x2z1 = rhs.x.mul(&self.z);
        let y1z2 = self.y.mul(&rhs.z);
        let y2z1 = rhs.y.mul(&self.z);
        x1z2 == x2z1 && y1z2 == y2z1
    }
}

// Group order L = 2^252 + 27742317777372353535851937790883648493
const ORDER_L: [u64; 4] = [
    0x5812631a5cf5d3ed,
    0x14def9dea2f79cd6,
    0x0000000000000000,
    0x1000000000000000,
];

fn reduce_scalar_512(hash_bytes: &[u8; 64]) -> [u64; 4] {
    let mut rem = [0u64; 4];
    for bit_idx in (0..512).rev() {
        let byte_pos = bit_idx / 8;
        let bit_pos = bit_idx % 8;
        let bit = ((hash_bytes[byte_pos] >> bit_pos) & 1) as u64;

        let mut carry = bit;
        for w in &mut rem {
            let next_carry = *w >> 63;
            *w = (*w << 1) | carry;
            carry = next_carry;
        }

        if carry > 0 || scalar_ge(&rem, &ORDER_L) {
            let mut borrow = 0u128;
            for i in 0..4 {
                let diff = (rem[i] as u128)
                    .wrapping_sub(ORDER_L[i] as u128)
                    .wrapping_sub(borrow);
                rem[i] = diff as u64;
                borrow = if diff > u64::MAX as u128 { 1 } else { 0 };
            }
        }
    }
    rem
}

fn scalar_ge(a: &[u64; 4], b: &[u64; 4]) -> bool {
    for i in (0..4).rev() {
        if a[i] > b[i] {
            return true;
        }
        if a[i] < b[i] {
            return false;
        }
    }
    true
}

fn ed25519_verify_signature(
    public_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> bool {
    let a_point = match ExtendedPoint::from_bytes(public_key) {
        Some(pt) => pt,
        None => return false,
    };

    let mut r_bytes = [0u8; 32];
    r_bytes.copy_from_slice(&signature[0..32]);
    let r_point = match ExtendedPoint::from_bytes(&r_bytes) {
        Some(pt) => pt,
        None => return false,
    };

    let mut s_words = [0u64; 4];
    for i in 0..4 {
        let chunk: [u8; 8] = signature[32 + i * 8..32 + (i + 1) * 8].try_into().unwrap();
        s_words[i] = u64::from_le_bytes(chunk);
    }
    if scalar_ge(&s_words, &ORDER_L) {
        return false;
    }

    let mut hasher = Sha512::new();
    hasher.update(&signature[0..32]);
    hasher.update(public_key);
    hasher.update(message);
    let hash_result = hasher.finalize();

    let mut hash_arr = [0u8; 64];
    hash_arr.copy_from_slice(&hash_result);
    let k_words = reduce_scalar_512(&hash_arr);

    // Verify S * B == R + k * A
    let sb = ExtendedPoint::base().scalar_mul(&s_words);
    let ka = a_point.scalar_mul(&k_words);
    let r_plus_ka = r_point.add(&ka);

    sb.equals(&r_plus_ka)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_decode() {
        assert_eq!(decode_base64("").unwrap(), b"");
        assert_eq!(decode_base64("AQID").unwrap(), vec![1, 2, 3]);
        assert_eq!(decode_base64("SGVsbG8=").unwrap(), b"Hello");
    }

    #[test]
    fn test_field_arithmetic_sqrt_m1() {
        let i = FieldElement::sqrt_m1();
        let i2 = i.square();
        let neg_one = FieldElement::ZERO.sub(&FieldElement::ONE);
        assert_eq!(i2, neg_one);
    }

    #[test]
    fn test_verify_license_key_malformed() {
        let res = verify_license_key("INVALID-KEY".to_string(), None);
        assert!(matches!(res, UniFFILicenseResult::MalformedKey { .. }));
    }
}
