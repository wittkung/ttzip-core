// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Apple `LZVN` ultra-high-throughput bytecode decompressor and 8-byte Wild Copy virtual machine.
//!
//! # Overview
//!
//! LZVN is Apple's lightweight byte-aligned LZ77 compression format used extensively across
//! macOS and iOS for kernel caches, boot images, disk images (UDIF/DMG), and embedded payloads.
//! It uses a 256-entry opcode table encoding literals (L), matches (M), and match distances (D),
//! terminating with an 8-byte end-of-stream (`0x06`) marker.
//!
//! # Architecture & Performance Invariants
//!
//! 1. **256-Element Opcode Dispatch**: Single-byte opcode classification into 11 functional categories:
//!    (`sml_d`, `med_d`, `lrg_d`, `pre_d`, `sml_m`, `lrg_m`, `sml_l`, `lrg_l`, `nop`, `eos`, `udef`).
//! 2. **64-bit (8-byte) Wild Copy VM**: When remaining destination capacity $\ge M + 7$ and $D \ge 8$,
//!    emits non-overlapping 8-byte words to saturate processor memory bandwidth.
//! 3. **Run-Length & Pattern Overlap Acceleration**:
//!    - $D = 1$: Single-byte broadcast via `slice::fill` (RLE Splat).
//!    - $D = 2$: 16-bit word repeat broadcast via 64-bit quadrupled writes.
//!    - $D = 4$: 32-bit dword repeat broadcast via 64-bit doubled writes.
//!    - General $D < 8$: Sequential in-order byte expansion.
//! 4. **Defensive Invariants**:
//!    - Distance underflow/zero check ($D == 0 \lor D > \text{dst\_pos}$) strictly returns error.
//!    - Truncated source or undefined opcodes return [`TTZipStatus::ErrCorruptHeader`].
//!    - Zero dynamic allocations and zero panics under malformed inputs.

use crate::types::TTZipStatus;

// MARK: - Opcode Classification

/// Classification of LZVN instruction opcodes in the 256-element jump table.
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LzvnOpcodeKind {
    /// Small distance match with literal (`sml_d`).
    SmlD = 0,
    /// Medium distance match with literal (`med_d`).
    MedD = 1,
    /// Large distance match with literal (`lrg_d`).
    LrgD = 2,
    /// Previous distance match with literal (`pre_d`).
    PreD = 3,
    /// Small match only without literal (`sml_m`).
    SmlM = 4,
    /// Large match only without literal (`lrg_m`).
    LrgM = 5,
    /// Small literal only without match (`sml_l`).
    SmlL = 6,
    /// Large literal only without match (`lrg_l`).
    LrgL = 7,
    /// No-operation instruction (`nop`).
    Nop = 8,
    /// End-of-stream marker instruction (`eos`).
    Eos = 9,
    /// Undefined or invalid opcode (`udef`).
    Udef = 10,
}

/// 256-element static opcode lookup table for $O(1)$ instruction classification.
#[rustfmt::skip]
pub const LZVN_OPCODE_TABLE: [LzvnOpcodeKind; 256] = [
    // 0x00..0x07 (0..7)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::Eos,  LzvnOpcodeKind::LrgD,
    // 0x08..0x0F (8..15)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::Nop,  LzvnOpcodeKind::LrgD,
    // 0x10..0x17 (16..23)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::Nop,  LzvnOpcodeKind::LrgD,
    // 0x18..0x1F (24..31)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::Udef, LzvnOpcodeKind::LrgD,
    // 0x20..0x27 (32..39)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::Udef, LzvnOpcodeKind::LrgD,
    // 0x28..0x2F (40..47)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::Udef, LzvnOpcodeKind::LrgD,
    // 0x30..0x37 (48..55)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::Udef, LzvnOpcodeKind::LrgD,
    // 0x38..0x3F (56..63)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::Udef, LzvnOpcodeKind::LrgD,
    // 0x40..0x47 (64..71)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::PreD, LzvnOpcodeKind::LrgD,
    // 0x48..0x4F (72..79)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::PreD, LzvnOpcodeKind::LrgD,
    // 0x50..0x57 (80..87)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::PreD, LzvnOpcodeKind::LrgD,
    // 0x58..0x5F (88..95)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::PreD, LzvnOpcodeKind::LrgD,
    // 0x60..0x67 (96..103)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::PreD, LzvnOpcodeKind::LrgD,
    // 0x68..0x6F (104..111)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::PreD, LzvnOpcodeKind::LrgD,
    // 0x70..0x77 (112..119)
    LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef,
    LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef,
    // 0x78..0x7F (120..127)
    LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef,
    LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef,
    // 0x80..0x87 (128..135)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::PreD, LzvnOpcodeKind::LrgD,
    // 0x88..0x8F (136..143)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::PreD, LzvnOpcodeKind::LrgD,
    // 0x90..0x97 (144..151)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::PreD, LzvnOpcodeKind::LrgD,
    // 0x98..0x9F (152..159)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::PreD, LzvnOpcodeKind::LrgD,
    // 0xA0..0xA7 (160..167)
    LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD,
    LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD,
    // 0xA8..0xAF (168..175)
    LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD,
    LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD,
    // 0xB0..0xB7 (176..183)
    LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD,
    LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD,
    // 0xB8..0xBF (184..191)
    LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD,
    LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD, LzvnOpcodeKind::MedD,
    // 0xC0..0xC7 (192..199)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::PreD, LzvnOpcodeKind::LrgD,
    // 0xC8..0xCF (200..207)
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD,
    LzvnOpcodeKind::SmlD, LzvnOpcodeKind::SmlD, LzvnOpcodeKind::PreD, LzvnOpcodeKind::LrgD,
    // 0xD0..0xD7 (208..215)
    LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef,
    LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef,
    // 0xD8..0xDF (216..223)
    LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef,
    LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef, LzvnOpcodeKind::Udef,
    // 0xE0..0xE7 (224..231)
    LzvnOpcodeKind::LrgL, LzvnOpcodeKind::SmlL, LzvnOpcodeKind::SmlL, LzvnOpcodeKind::SmlL,
    LzvnOpcodeKind::SmlL, LzvnOpcodeKind::SmlL, LzvnOpcodeKind::SmlL, LzvnOpcodeKind::SmlL,
    // 0xE8..0xEF (232..239)
    LzvnOpcodeKind::SmlL, LzvnOpcodeKind::SmlL, LzvnOpcodeKind::SmlL, LzvnOpcodeKind::SmlL,
    LzvnOpcodeKind::SmlL, LzvnOpcodeKind::SmlL, LzvnOpcodeKind::SmlL, LzvnOpcodeKind::SmlL,
    // 0xF0..0xF7 (240..247)
    LzvnOpcodeKind::LrgM, LzvnOpcodeKind::SmlM, LzvnOpcodeKind::SmlM, LzvnOpcodeKind::SmlM,
    LzvnOpcodeKind::SmlM, LzvnOpcodeKind::SmlM, LzvnOpcodeKind::SmlM, LzvnOpcodeKind::SmlM,
    // 0xF8..0xFF (248..255)
    LzvnOpcodeKind::SmlM, LzvnOpcodeKind::SmlM, LzvnOpcodeKind::SmlM, LzvnOpcodeKind::SmlM,
    LzvnOpcodeKind::SmlM, LzvnOpcodeKind::SmlM, LzvnOpcodeKind::SmlM, LzvnOpcodeKind::SmlM,
];

// MARK: - LZVN Virtual Machine Decoder

/// LZVN streaming and one-shot bytecode decoder virtual machine.
#[derive(Debug, Clone, Default)]
pub struct LzvnDecoder {
    /// Match distance from the previous instruction ($D_{\text{prev}}$).
    pub d_prev: usize,
    /// Whether the end-of-stream (EOS) token has been successfully decoded.
    pub end_of_stream: bool,
}

impl LzvnDecoder {
    /// Creates a new `LzvnDecoder` instance with default state.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            d_prev: 0,
            end_of_stream: false,
        }
    }

    /// Resets the internal state machine for reuse across independent blocks.
    #[inline]
    pub fn reset(&mut self) {
        self.d_prev = 0;
        self.end_of_stream = false;
    }

    /// Decodes LZVN compressed bytecode from `src` into `dst`.
    ///
    /// # Return Value
    /// Returns `Ok((src_bytes_read, dst_bytes_written))` on successful decompression.
    ///
    /// # Errors
    /// - [`TTZipStatus::ErrCorruptHeader`]: Truncated stream, invalid opcode, or illegal match distance.
    /// - [`TTZipStatus::ErrExtractionFailed`]: Destination buffer capacity exceeded.
    pub fn decode(&mut self, src: &[u8], dst: &mut [u8]) -> Result<(usize, usize), TTZipStatus> {
        if src.is_empty() || dst.is_empty() {
            return Ok((0, 0));
        }

        let mut src_pos: usize = 0;
        let mut dst_pos: usize = 0;

        while src_pos < src.len() && !self.end_of_stream {
            let opc = src[src_pos];
            let kind = LZVN_OPCODE_TABLE[opc as usize];

            match kind {
                LzvnOpcodeKind::Eos => {
                    // EOS command is 8 bytes: 0x06 followed by 7 zero padding bytes
                    if src.len() - src_pos < 8 {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    src_pos += 8;
                    self.end_of_stream = true;
                    break;
                }
                LzvnOpcodeKind::Nop => {
                    if src.len() - src_pos <= 1 {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    src_pos += 1;
                }
                LzvnOpcodeKind::Udef => {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                LzvnOpcodeKind::SmlL => {
                    let l = (opc & 0x0f) as usize;
                    let opc_len = 1;
                    if src.len() - src_pos <= opc_len + l {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    src_pos += opc_len;
                    Self::copy_literal(src, &mut src_pos, dst, &mut dst_pos, l)?;
                }
                LzvnOpcodeKind::LrgL => {
                    let opc_len = 2;
                    if src.len() - src_pos <= opc_len {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    let l = (src[src_pos + 1] as usize) + 16;
                    if src.len() - src_pos <= opc_len + l {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    src_pos += opc_len;
                    Self::copy_literal(src, &mut src_pos, dst, &mut dst_pos, l)?;
                }
                LzvnOpcodeKind::SmlM => {
                    let m = (opc & 0x0f) as usize;
                    let opc_len = 1;
                    if src.len() - src_pos <= opc_len {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    src_pos += opc_len;
                    let d = self.d_prev;
                    Self::copy_match(dst, &mut dst_pos, m, d)?;
                }
                LzvnOpcodeKind::LrgM => {
                    let opc_len = 2;
                    if src.len() - src_pos <= opc_len {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    let m = (src[src_pos + 1] as usize) + 16;
                    src_pos += opc_len;
                    let d = self.d_prev;
                    Self::copy_match(dst, &mut dst_pos, m, d)?;
                }
                LzvnOpcodeKind::SmlD => {
                    let l = ((opc >> 6) & 0x03) as usize;
                    let m = (((opc >> 3) & 0x07) as usize) + 3;
                    let opc_len = 2;
                    if src.len() - src_pos <= opc_len + l {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    let d = (((opc & 0x07) as usize) << 8) | (src[src_pos + 1] as usize);
                    src_pos += opc_len;
                    if l > 0 {
                        Self::copy_literal(src, &mut src_pos, dst, &mut dst_pos, l)?;
                    }
                    self.d_prev = d;
                    Self::copy_match(dst, &mut dst_pos, m, d)?;
                }
                LzvnOpcodeKind::MedD => {
                    let l = ((opc >> 3) & 0x03) as usize;
                    let opc_len = 3;
                    if src.len() - src_pos <= opc_len + l {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    let opc23 = (src[src_pos + 1] as u16) | ((src[src_pos + 2] as u16) << 8);
                    let m = ((((opc & 0x07) as usize) << 2) | ((opc23 & 0x03) as usize)) + 3;
                    let d = ((opc23 >> 2) & 0x3fff) as usize;
                    src_pos += opc_len;
                    if l > 0 {
                        Self::copy_literal(src, &mut src_pos, dst, &mut dst_pos, l)?;
                    }
                    self.d_prev = d;
                    Self::copy_match(dst, &mut dst_pos, m, d)?;
                }
                LzvnOpcodeKind::LrgD => {
                    let l = ((opc >> 6) & 0x03) as usize;
                    let m = (((opc >> 3) & 0x07) as usize) + 3;
                    let opc_len = 3;
                    if src.len() - src_pos <= opc_len + l {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    let d = (src[src_pos + 1] as usize) | ((src[src_pos + 2] as usize) << 8);
                    src_pos += opc_len;
                    if l > 0 {
                        Self::copy_literal(src, &mut src_pos, dst, &mut dst_pos, l)?;
                    }
                    self.d_prev = d;
                    Self::copy_match(dst, &mut dst_pos, m, d)?;
                }
                LzvnOpcodeKind::PreD => {
                    let l = ((opc >> 6) & 0x03) as usize;
                    let m = (((opc >> 3) & 0x07) as usize) + 3;
                    let opc_len = 1;
                    if src.len() - src_pos <= opc_len + l {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    src_pos += opc_len;
                    if l > 0 {
                        Self::copy_literal(src, &mut src_pos, dst, &mut dst_pos, l)?;
                    }
                    let d = self.d_prev;
                    Self::copy_match(dst, &mut dst_pos, m, d)?;
                }
            }
        }

        Ok((src_pos, dst_pos))
    }

    // MARK: - Internal Copy Engines

    /// Copies `l` literal bytes from `src` to `dst` using 64-bit Wild Copy when safe.
    #[inline(always)]
    fn copy_literal(
        src: &[u8],
        src_pos: &mut usize,
        dst: &mut [u8],
        dst_pos: &mut usize,
        l: usize,
    ) -> Result<(), TTZipStatus> {
        if l == 0 {
            return Ok(());
        }

        if dst.len() - *dst_pos < l {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        if src.len() - *src_pos < l {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        // 8-byte Wild Copy Fast Path for literals
        if dst.len() - *dst_pos >= l + 7 && src.len() - *src_pos >= l + 7 {
            let mut off = 0;
            while off < l {
                unsafe {
                    let s_ptr = src.as_ptr().add(*src_pos + off);
                    let d_ptr = dst.as_mut_ptr().add(*dst_pos + off);
                    std::ptr::copy_nonoverlapping(s_ptr, d_ptr, 8);
                }
                off += 8;
            }
        } else {
            dst[*dst_pos..*dst_pos + l].copy_from_slice(&src[*src_pos..*src_pos + l]);
        }

        *src_pos += l;
        *dst_pos += l;
        Ok(())
    }

    /// Copies `m` match bytes from distance `d` into `dst` with 64-bit Wild Copy & overlap expansion.
    #[inline(always)]
    fn copy_match(
        dst: &mut [u8],
        dst_pos: &mut usize,
        m: usize,
        d: usize,
    ) -> Result<(), TTZipStatus> {
        if m == 0 {
            return Ok(());
        }

        // Strict distance verification: D must be non-zero and within emitted output
        if d == 0 || d > *dst_pos {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        // Destination capacity check
        if dst.len() - *dst_pos < m {
            return Err(TTZipStatus::ErrExtractionFailed);
        }

        // Fast Path 1: 64-bit Wild Copy for non-overlapping match (D >= 8 and safety padding >= 7)
        if d >= 8 && dst.len() - *dst_pos >= m + 7 {
            let mut off = 0;
            while off < m {
                unsafe {
                    let r_ptr = dst.as_ptr().add(*dst_pos + off - d);
                    let w_ptr = dst.as_mut_ptr().add(*dst_pos + off);
                    std::ptr::copy_nonoverlapping(r_ptr, w_ptr, 8);
                }
                off += 8;
            }
            *dst_pos += m;
            return Ok(());
        }

        // Fast Path 2: Overlap expansion for D == 1 (RLE Splat)
        if d == 1 {
            let byte = dst[*dst_pos - 1];
            dst[*dst_pos..*dst_pos + m].fill(byte);
            *dst_pos += m;
            return Ok(());
        }

        // Fast Path 3: Overlap expansion for D == 2 (16-bit word repeat)
        if d == 2 {
            let b0 = dst[*dst_pos - 2];
            let b1 = dst[*dst_pos - 1];
            let word16 = u16::from_ne_bytes([b0, b1]);
            let word32 = (word16 as u32) | ((word16 as u32) << 16);
            let word64 = (word32 as u64) | ((word32 as u64) << 32);

            if dst.len() - *dst_pos >= m + 7 {
                let mut off = 0;
                while off < m {
                    unsafe {
                        let w_ptr = dst.as_mut_ptr().add(*dst_pos + off) as *mut u64;
                        w_ptr.write_unaligned(word64);
                    }
                    off += 8;
                }
            } else {
                for i in 0..m {
                    dst[*dst_pos + i] = if (i & 1) == 0 { b0 } else { b1 };
                }
            }
            *dst_pos += m;
            return Ok(());
        }

        // Fast Path 4: Overlap expansion for D == 4 (32-bit dword repeat)
        if d == 4 {
            let b = [
                dst[*dst_pos - 4],
                dst[*dst_pos - 3],
                dst[*dst_pos - 2],
                dst[*dst_pos - 1],
            ];
            let word32 = u32::from_ne_bytes(b);
            let word64 = (word32 as u64) | ((word32 as u64) << 32);

            if dst.len() - *dst_pos >= m + 7 {
                let mut off = 0;
                while off < m {
                    unsafe {
                        let w_ptr = dst.as_mut_ptr().add(*dst_pos + off) as *mut u64;
                        w_ptr.write_unaligned(word64);
                    }
                    off += 8;
                }
            } else {
                for i in 0..m {
                    dst[*dst_pos + i] = b[i % 4];
                }
            }
            *dst_pos += m;
            return Ok(());
        }

        // General fallback: Byte-by-byte in increasing address order (correctly handles any D < M)
        for i in 0..m {
            dst[*dst_pos + i] = dst[*dst_pos + i - d];
        }
        *dst_pos += m;
        Ok(())
    }
}

// MARK: - Public Safe Facades

/// Decompresses an Apple LZVN compressed slice into a destination buffer.
///
/// Returns the number of uncompressed bytes written to `dst`.
#[inline]
pub fn lzvn_decompress_pure_rust(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    let mut decoder = LzvnDecoder::new();
    let (_src_read, written) = decoder.decode(src, dst)?;
    if !decoder.end_of_stream && !src.is_empty() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    Ok(written)
}

/// Decompresses an Apple LZVN compressed slice into a newly allocated `Vec<u8>`.
#[inline]
pub fn lzvn_decompress_to_vec_pure_rust(
    src: &[u8],
    uncompressed_len: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() || uncompressed_len == 0 {
        return Ok(Vec::new());
    }
    let mut out = vec![0u8; uncompressed_len];
    let written = lzvn_decompress_pure_rust(src, &mut out)?;
    if written != uncompressed_len {
        return Err(TTZipStatus::ErrExtractionFailed);
    }
    Ok(out)
}

/// Decompresses an Apple LZVN encoded buffer into newly allocated `Vec<u8>`.
#[inline]
pub fn lzvn_decompress_pure_rust_to_vec(
    src: &[u8],
    uncompressed_len: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    lzvn_decompress_to_vec_pure_rust(src, uncompressed_len)
}

/// Decompresses an Apple LZVN encoded buffer into `dst` slice.
#[inline]
pub fn lzvn_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    lzvn_decompress_pure_rust(src, dst)
}

/// Decompresses an Apple LZVN encoded buffer into newly allocated `Vec<u8>`.
#[inline]
pub fn lzvn_decompress_to_vec(
    src: &[u8],
    uncompressed_len: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    lzvn_decompress_to_vec_pure_rust(src, uncompressed_len)
}

/// Decompresses an Apple LZVN encoded buffer with pre-known length into newly allocated `Vec<u8>`.
#[inline]
pub fn lzvn_decompress_raw(
    src: &[u8],
    uncompressed_len: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    lzvn_decompress_to_vec_pure_rust(src, uncompressed_len)
}

/// Validates LZVN stream integrity by stepping through opcodes without allocating memory.
pub fn lzvn_validate(src: &[u8]) -> bool {
    if src.is_empty() {
        return true;
    }

    let mut src_pos: usize = 0;
    let mut dst_pos: usize = 0;
    let mut d_prev: usize = 0;
    let mut end_of_stream = false;

    while src_pos < src.len() && !end_of_stream {
        let opc = src[src_pos];
        let kind = LZVN_OPCODE_TABLE[opc as usize];

        match kind {
            LzvnOpcodeKind::Eos => {
                if src.len() - src_pos < 8 {
                    return false;
                }
                end_of_stream = true;
                break;
            }
            LzvnOpcodeKind::Nop => {
                src_pos += 1;
            }
            LzvnOpcodeKind::Udef => {
                return false;
            }
            LzvnOpcodeKind::SmlL => {
                let l = (opc & 0x0f) as usize;
                let opc_len = 1;
                if src.len() - src_pos < opc_len + l {
                    return false;
                }
                src_pos += opc_len + l;
                dst_pos += l;
            }
            LzvnOpcodeKind::LrgL => {
                let opc_len = 2;
                if src.len() - src_pos < opc_len {
                    return false;
                }
                let l = (src[src_pos + 1] as usize) + 16;
                if src.len() - src_pos < opc_len + l {
                    return false;
                }
                src_pos += opc_len + l;
                dst_pos += l;
            }
            LzvnOpcodeKind::SmlM => {
                let m = (opc & 0x0f) as usize;
                let opc_len = 1;
                if src.len() - src_pos < opc_len {
                    return false;
                }
                src_pos += opc_len;
                let d = d_prev;
                if d == 0 || d > dst_pos {
                    return false;
                }
                dst_pos += m;
            }
            LzvnOpcodeKind::LrgM => {
                let opc_len = 2;
                if src.len() - src_pos < opc_len {
                    return false;
                }
                let m = (src[src_pos + 1] as usize) + 16;
                src_pos += opc_len;
                let d = d_prev;
                if d == 0 || d > dst_pos {
                    return false;
                }
                dst_pos += m;
            }
            LzvnOpcodeKind::SmlD => {

                let l = ((opc >> 6) & 0x03) as usize;
                let m = (((opc >> 3) & 0x07) as usize) + 3;
                let opc_len = 2;
                if src.len() - src_pos < opc_len + l {
                    return false;
                }
                let d = (((opc & 0x07) as usize) << 8) | (src[src_pos + 1] as usize);
                src_pos += opc_len + l;
                dst_pos += l;
                if d == 0 || d > dst_pos {
                    return false;
                }
                d_prev = d;
                dst_pos += m;
            }
            LzvnOpcodeKind::MedD => {
                let l = ((opc >> 3) & 0x03) as usize;
                let opc_len = 3;
                if src.len() - src_pos < opc_len + l {
                    return false;
                }
                let opc23 = (src[src_pos + 1] as u16) | ((src[src_pos + 2] as u16) << 8);
                let m = ((((opc & 0x07) as usize) << 2) | ((opc23 & 0x03) as usize)) + 3;
                let d = ((opc23 >> 2) & 0x3fff) as usize;
                src_pos += opc_len + l;
                dst_pos += l;
                if d == 0 || d > dst_pos {
                    return false;
                }
                d_prev = d;
                dst_pos += m;
            }
            LzvnOpcodeKind::LrgD => {
                let l = ((opc >> 6) & 0x03) as usize;
                let m = (((opc >> 3) & 0x07) as usize) + 3;
                let opc_len = 3;
                if src.len() - src_pos < opc_len + l {
                    return false;
                }
                let d = (src[src_pos + 1] as usize) | ((src[src_pos + 2] as usize) << 8);
                src_pos += opc_len + l;
                dst_pos += l;
                if d == 0 || d > dst_pos {
                    return false;
                }
                d_prev = d;
                dst_pos += m;
            }
            LzvnOpcodeKind::PreD => {
                let l = ((opc >> 6) & 0x03) as usize;
                let m = (((opc >> 3) & 0x07) as usize) + 3;
                let opc_len = 1;
                if src.len() - src_pos < opc_len + l {
                    return false;
                }
                src_pos += opc_len + l;
                dst_pos += l;
                let d = d_prev;
                if d == 0 || d > dst_pos {
                    return false;
                }
                dst_pos += m;
            }
        }
    }

    end_of_stream
}

