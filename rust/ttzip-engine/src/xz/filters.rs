// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! XZ Filter Chain inverse transformations (BCJ architecture filters and Delta decoding).
//!
//! Complies strictly with Section 5.3 of the .xz File Format Specification.

use crate::codecs::branch::x86_decode;
use crate::xz::bcj::{BcjArm64, BranchFilter};
use crate::xz::block::{
    XzFilterConfig, FILTER_ID_ARM, FILTER_ID_ARM64, FILTER_ID_ARMTHUMB, FILTER_ID_DELTA,
    FILTER_ID_IA64, FILTER_ID_POWERPC, FILTER_ID_RISCV, FILTER_ID_SPARC, FILTER_ID_X86,
};
use crate::xz::types::XzError;

/// In-place Delta filter decoding algorithm (§5.3.3).
pub fn delta_decode(data: &mut [u8], distance: usize) {
    if distance == 0 || data.is_empty() {
        return;
    }
    let mut state = [0u8; 256];
    let mut state_pos = 0;
    for byte in data.iter_mut() {
        let decoded = byte.wrapping_add(state[state_pos]);
        state[state_pos] = decoded;
        state_pos = (state_pos + 1) % distance;
        *byte = decoded;
    }
}

/// In-place ARM BCJ filter decoding algorithm (§5.3.4).
pub fn arm_decode(data: &mut [u8], start_ip: u32) -> usize {
    if data.len() < 8 {
        return 0;
    }
    let len = data.len() - 8;
    let mut i = 0;
    while i <= len {
        if data[i + 3] == 0xEB {
            let mut src = (data[i] as u32)
                | ((data[i + 1] as u32) << 8)
                | ((data[i + 2] as u32) << 16);
            src <<= 2;
            if (src & 0x0200_0000) != 0 {
                src |= 0xFC00_0000;
            }
            let ip = start_ip.wrapping_add(i as u32).wrapping_add(8);
            let dest = src.wrapping_sub(ip) >> 2;
            data[i] = dest as u8;
            data[i + 1] = (dest >> 8) as u8;
            data[i + 2] = (dest >> 16) as u8;
        }
        i += 4;
    }
    i
}

/// In-place ARM-Thumb BCJ filter decoding algorithm (§5.3.5).
pub fn arm_thumb_decode(data: &mut [u8], start_ip: u32) -> usize {
    if data.len() < 4 {
        return 0;
    }
    let len = data.len() - 4;
    let mut i = 0;
    while i <= len {
        let b1 = data[i + 1];
        let b3 = data[i + 3];
        if (b1 & 0xF8) == 0xF0 && (b3 & 0xF8) == 0xF8 {
            let b0 = data[i];
            let b2 = data[i + 2];
            let mut src = (((b1 as u32 & 0x07) << 19)
                | ((b0 as u32) << 11)
                | ((b3 as u32 & 0x07) << 8)
                | (b2 as u32))
                << 1;
            if (src & 0x0080_0000) != 0 {
                src |= 0xFF00_0000;
            }
            let ip = start_ip.wrapping_add(i as u32).wrapping_add(4);
            let dest = src.wrapping_sub(ip) >> 1;
            data[i + 1] = (0xF0 | ((dest >> 19) & 0x07)) as u8;
            data[i] = (dest >> 11) as u8;
            data[i + 3] = (0xF8 | ((dest >> 8) & 0x07)) as u8;
            data[i + 2] = dest as u8;
            i += 2;
        }
        i += 2;
    }
    i
}

/// In-place PowerPC BCJ filter decoding algorithm (§5.3.6).
pub fn powerpc_decode(data: &mut [u8], start_ip: u32) -> usize {
    if data.len() < 4 {
        return 0;
    }
    let len = data.len() - 4;
    let mut i = 0;
    while i <= len {
        let b0 = data[i];
        if (b0 & 0xFC) == 0x48 && (data[i + 3] & 3) == 1 {
            let mut src = ((b0 as u32 & 0x03) << 24)
                | ((data[i + 1] as u32) << 16)
                | ((data[i + 2] as u32) << 8)
                | (data[i + 3] as u32 & 0xFC);
            if (src & 0x0200_0000) != 0 {
                src |= 0xFC00_0000;
            }
            let ip = start_ip.wrapping_add(i as u32);
            let dest = src.wrapping_sub(ip);
            data[i] = (0x48 | ((dest >> 24) & 0x03)) as u8;
            data[i + 1] = (dest >> 16) as u8;
            data[i + 2] = (dest >> 8) as u8;
            data[i + 3] = (data[i + 3] & 0x03) | (dest as u8 & 0xFC);
        }
        i += 4;
    }
    i
}

/// In-place SPARC BCJ filter decoding algorithm (§5.3.7).
pub fn sparc_decode(data: &mut [u8], start_ip: u32) -> usize {
    if data.len() < 4 {
        return 0;
    }
    let len = data.len() - 4;
    let mut i = 0;
    while i <= len {
        let b0 = data[i];
        let b1 = data[i + 1];
        if (b0 == 0x40 && (b1 & 0xC0) == 0x00) || (b0 == 0x7F && (b1 & 0xC0) == 0xC0) {
            let src = (((b0 as u32) << 24)
                | ((b1 as u32) << 16)
                | ((data[i + 2] as u32) << 8)
                | (data[i + 3] as u32))
                << 2;

            let ip = start_ip.wrapping_add(i as u32);
            let dest = src.wrapping_sub(ip) >> 2;
            let high = if b0 == 0x40 { 0x40 } else { 0x7F };
            data[i] = (high | ((dest >> 24) & 0x3F)) as u8;
            data[i + 1] = (dest >> 16) as u8;
            data[i + 2] = (dest >> 8) as u8;
            data[i + 3] = dest as u8;
        }
        i += 4;
    }
    i
}

/// In-place IA-64 BCJ filter decoding algorithm (§5.3.8).
pub fn ia64_decode(data: &mut [u8], start_ip: u32) -> usize {
    if data.len() < 16 {
        return 0;
    }
    let len = data.len() - 16;
    let mut i = 0;
    while i <= len {
        let mask = match data[i] & 0x1F {
            0x12 | 0x13 | 0x16 | 0x17 => 7,
            0x18 | 0x19 => 6,
            _ => 0,
        };
        if mask != 0 {
            for slot in 0..3 {
                if (mask & (1 << slot)) != 0 {
                    let bit_pos = 5 + slot * 41;
                    let byte_pos = bit_pos / 8;
                    let bit_offset = bit_pos % 8;
                    let mut val = 0u64;
                    for k in 0..6 {
                        if i + byte_pos + k < data.len() {
                            val |= (data[i + byte_pos + k] as u64) << (k * 8);
                        }
                    }
                    let inst = (val >> bit_offset) & 0x1FF_FFFF_FFFF;
                    if ((inst >> 37) & 0x0F) == 0x05 {
                        let mut target = ((inst >> 13) & 0xFFFFF) | (((inst >> 36) & 1) << 20);
                        target <<= 4;
                        let ip = start_ip.wrapping_add(i as u32);
                        target = target.wrapping_sub(ip as u64);
                        target >>= 4;
                        let new_inst = (inst & !((0xFFFFF << 13) | (1 << 36)))
                            | ((target & 0xFFFFF) << 13)
                            | (((target >> 20) & 1) << 36);
                        val = (val & !(0x1FF_FFFF_FFFF << bit_offset)) | (new_inst << bit_offset);
                        for k in 0..6 {
                            if i + byte_pos + k < data.len() {
                                data[i + byte_pos + k] = (val >> (k * 8)) as u8;
                            }
                        }
                    }
                }
            }
        }
        i += 16;
    }
    i
}

/// Applies inverse filters to decoded byte buffer in reverse order.
pub fn apply_filters_decode(
    filters: &[XzFilterConfig],
    data: &mut [u8],
) -> Result<(), XzError> {
    if filters.is_empty() {
        return Ok(());
    }

    let pre_filter_count = filters.len().saturating_sub(1);
    for idx in (0..pre_filter_count).rev() {
        let filter = &filters[idx];
        let start_ip = if filter.properties.len() >= 4 {
            u32::from_le_bytes(filter.properties[0..4].try_into().unwrap())
        } else {
            0
        };

        match filter.filter_id {
            FILTER_ID_X86 => {
                x86_decode(data, start_ip);
            }
            FILTER_ID_ARM64 => {
                BcjArm64::new().decode(data, start_ip);
            }
            FILTER_ID_ARM => {
                arm_decode(data, start_ip);
            }
            FILTER_ID_ARMTHUMB => {
                arm_thumb_decode(data, start_ip);
            }
            FILTER_ID_POWERPC => {
                powerpc_decode(data, start_ip);
            }
            FILTER_ID_IA64 => {
                ia64_decode(data, start_ip);
            }
            FILTER_ID_SPARC => {
                sparc_decode(data, start_ip);
            }
            FILTER_ID_RISCV => {
                crate::xz::bcj::BcjRiscv::new().decode(data, start_ip);
            }
            FILTER_ID_DELTA => {
                let dist = (filter.properties.first().copied().unwrap_or(0) as usize) + 1;
                delta_decode(data, dist);
            }
            other => return Err(XzError::UnsupportedFilter(other)),
        }
    }
    Ok(())
}
