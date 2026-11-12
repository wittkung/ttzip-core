// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! RISC-V (RV32I / RV64I) BCJ Hardware Instruction Filter (Filter ID `0x0B`).
//!
//! Complies with Section 5.3 of the XZ specification and liblzma 5.6.0+ RV32/RV64 filter design.
//! Filters `JAL` instructions targeting `x1` (ra) or `x5` (t0) with big-endian immediate normalization,
//! and `AUIPC + inst2` pairs with bijective reconstruction and fake decode mechanisms to guarantee
//! absolute reversible mathematical equivalence across arbitrary binary streams.

use super::{BranchFilter, FILTER_ID_RISCV};

/// Checks whether `auipc` and `inst2` form a valid AUIPC+inst2 instruction pair.
#[inline(always)]
fn not_auipc_pair(auipc: u32, inst2: u32) -> bool {
    (((auipc << 8) ^ (inst2.wrapping_sub(3))) & 0xF8003) != 0
}

/// Checks whether `auipc` matches the special transformed format used during encoding.
#[inline(always)]
fn not_special_auipc(auipc: u32, inst2_rs1: u32) -> bool {
    ((auipc.wrapping_sub(0x3117)) << 18) >= (inst2_rs1 & 0x1D)
}

/// RISC-V branch conversion filter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BcjRiscv;

impl BcjRiscv {
    /// Creates a new `BcjRiscv` filter.
    pub fn new() -> Self {
        Self
    }

    /// Encodes RISC-V JAL and AUIPC+inst2 instructions to normalized absolute forms.
    pub fn encode_buffer(&self, buffer: &mut [u8], now_pos: u32) -> usize {
        if buffer.len() < 8 {
            return 0;
        }

        let limit = buffer.len() - 8;
        let mut i = 0;

        while i <= limit {
            let inst0 = buffer[i];
            let pc = now_pos.wrapping_add(i as u32);
            if inst0 == 0xEF {
                i += Self::encode_jal(&mut buffer[i..i + 4], pc);
            } else if (inst0 & 0x7F) == 0x17 {
                i += Self::encode_auipc_pair(&mut buffer[i..i + 8], pc);
            } else {
                i += 2;
            }
        }

        i
    }

    #[inline(always)]
    fn encode_jal(slot: &mut [u8], pc: u32) -> usize {
        let b1 = slot[1] as u32;
        if (b1 & 0x0D) != 0 {
            return 2;
        }

        let b2 = slot[2] as u32;
        let b3 = slot[3] as u32;
        let addr = (((b1 & 0xF0) << 8)
            | ((b2 & 0x0F) << 16)
            | ((b2 & 0x10) << 7)
            | ((b2 & 0xE0) >> 4)
            | ((b3 & 0x7F) << 4)
            | ((b3 & 0x80) << 13))
            .wrapping_add(pc);

        slot[1] = (b1 as u8 & 0x0F) | ((addr >> 13) as u8 & 0xF0);
        slot[2] = (addr >> 9) as u8;
        slot[3] = (addr >> 1) as u8;
        4
    }

    #[inline(always)]
    fn encode_auipc_pair(slot: &mut [u8], pc: u32) -> usize {
        let mut inst = u32::from_le_bytes([slot[0], slot[1], slot[2], slot[3]]);
        let inst2 = u32::from_le_bytes([slot[4], slot[5], slot[6], slot[7]]);

        if (inst & 0xE80) != 0 {
            if not_auipc_pair(inst, inst2) {
                return 6;
            }
            let addr = (inst & 0xFFFF_F000)
                .wrapping_add(inst2 >> 20)
                .wrapping_sub((inst2 >> 19) & 0x1000)
                .wrapping_add(pc);

            inst = 0x17 | (2 << 7) | (inst2 << 12);
            slot[..4].copy_from_slice(&inst.to_le_bytes());
            slot[4..8].copy_from_slice(&addr.to_be_bytes());
            8
        } else {
            let fake_rs1 = inst >> 27;
            if not_special_auipc(inst, fake_rs1) {
                return 4;
            }
            let fake_inst2 = (inst >> 12) | (inst2 << 20);
            inst = 0x17 | (fake_rs1 << 7) | (inst2 & 0xFFFF_F000);
            slot[..4].copy_from_slice(&inst.to_le_bytes());
            slot[4..8].copy_from_slice(&fake_inst2.to_le_bytes());
            8
        }
    }

    /// Decodes normalized RISC-V instructions back to relative offsets.
    pub fn decode_buffer(&self, buffer: &mut [u8], now_pos: u32) -> usize {
        if buffer.len() < 8 {
            return 0;
        }

        let limit = buffer.len() - 8;
        let mut i = 0;

        while i <= limit {
            let inst0 = buffer[i];
            let pc = now_pos.wrapping_add(i as u32);
            if inst0 == 0xEF {
                i += Self::decode_jal(&mut buffer[i..i + 4], pc);
            } else if (inst0 & 0x7F) == 0x17 {
                i += Self::decode_auipc_pair(&mut buffer[i..i + 8], pc);
            } else {
                i += 2;
            }
        }

        i
    }

    #[inline(always)]
    fn decode_jal(slot: &mut [u8], pc: u32) -> usize {
        let b1 = slot[1] as u32;
        if (b1 & 0x0D) != 0 {
            return 2;
        }

        let b2 = slot[2] as u32;
        let b3 = slot[3] as u32;
        let addr = (((b1 & 0xF0) << 13) | (b2 << 9) | (b3 << 1)).wrapping_sub(pc);

        slot[1] = (b1 as u8 & 0x0F) | ((addr >> 8) as u8 & 0xF0);
        slot[2] = ((addr >> 16) as u8 & 0x0F)
            | ((addr >> 7) as u8 & 0x10)
            | ((addr << 4) as u8 & 0xE0);
        slot[3] = ((addr >> 4) as u8 & 0x7F) | ((addr >> 13) as u8 & 0x80);
        4
    }

    #[inline(always)]
    fn decode_auipc_pair(slot: &mut [u8], pc: u32) -> usize {
        let mut inst = u32::from_le_bytes([slot[0], slot[1], slot[2], slot[3]]);
        if (inst & 0xE80) != 0 {
            let inst2 = u32::from_le_bytes([slot[4], slot[5], slot[6], slot[7]]);
            if not_auipc_pair(inst, inst2) {
                return 6;
            }
            let inst2_out = (inst & 0xFFFF_F000).wrapping_add(inst2 >> 20);
            inst = 0x17 | (2 << 7) | (inst2 << 12);
            slot[..4].copy_from_slice(&inst.to_le_bytes());
            slot[4..8].copy_from_slice(&inst2_out.to_le_bytes());
            8
        } else {
            let inst2_rs1 = inst >> 27;
            if not_special_auipc(inst, inst2_rs1) {
                return 4;
            }
            let addr = u32::from_be_bytes([slot[4], slot[5], slot[6], slot[7]]).wrapping_sub(pc);
            let inst2 = (inst >> 12) | (addr << 20);
            inst = 0x17 | (inst2_rs1 << 7) | (addr.wrapping_add(0x800) & 0xFFFF_F000);
            slot[..4].copy_from_slice(&inst.to_le_bytes());
            slot[4..8].copy_from_slice(&inst2.to_le_bytes());
            8
        }
    }
}

impl BranchFilter for BcjRiscv {
    #[inline]
    fn filter_id(&self) -> u64 {
        FILTER_ID_RISCV
    }

    #[inline]
    fn alignment(&self) -> usize {
        2
    }

    #[inline]
    fn unfiltered_max(&self) -> usize {
        8
    }

    #[inline]
    fn encode(&mut self, buf: &mut [u8], now_pos: u32) -> usize {
        self.encode_buffer(buf, now_pos)
    }

    #[inline]
    fn decode(&mut self, buf: &mut [u8], now_pos: u32) -> usize {
        self.decode_buffer(buf, now_pos)
    }

    #[inline]
    fn reset(&mut self) {
        // Stateless filter
    }
}
