// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, nanosecond-latency LZ4 partial decompressor and VFS header sniffer.
//!
//! Provides zero-allocation partial block decompression with instruction-level truncation
//! control and dual-layer safe loops, enabling instant (<100ns) VFS format probing and
//! header inspection (e.g. `.tar.lz4`, `.iso.lz4`) without uncompressing entire multi-megabyte streams:
//! - **Instruction-Level Truncation**: Strictly bounded destination capacity `dst_cap = min(target, dst.len())`.
//! - **Literal Stage Early-Stop**: Immediate byte slice truncation and return when literal run satisfies target.
//! - **Match Stage Early-Stop**: Seamless transition from SIMD Wildcopy to exact boundary copy, early-stopping
//!   at `op == oend`.
//! - **External Sliding Dictionary Support**: High-performance cross-block history lookup (`using_dict`).
//! - **`Lz4HeaderSniffer`**: Fast VFS metadata extractor supporting TAR header (512B) and magic sniff (64B).

use crate::archive::unified::format_sniffer::{ArchiveFormat, FormatSniffer, SniffResult};
use crate::codecs::lz4::constants::{
    is_lz4_frame_magic, is_lz4_legacy_magic, is_lz4_skippable_magic, FrameDescriptor,
};
use crate::codecs::lz4::decompress::{
    copy_small_offset_ptr, wild_copy_16, LZ4_FAST_LOOP_MARGIN, LZ4_MIN_MATCH,
};
use crate::types::TTZipStatus;

// MARK: - Safe Partial Decompression APIs

/// Decompresses an LZ4 compressed block into `dst`, stopping immediately upon decoding `target_output_size` bytes.
///
/// Returns the exact number of bytes written to `dst` (which is `<= target_output_size` and `<= dst.len()`),
/// or an explicit `TTZipStatus` error on corrupt or malformed payload.
///
/// # Guarantees
/// - Zero writes beyond `min(target_output_size, dst.len())`.
/// - Nanosecond-level early exit when `target_output_size` is satisfied.
/// - 100% bit-exact parity with full decompression prefix and canonical `LZ4_decompress_safe_partial`.
pub fn lz4_decompress_safe_partial(
    src: &[u8],
    dst: &mut [u8],
    target_output_size: usize,
) -> Result<usize, TTZipStatus> {
    lz4_decompress_safe_partial_core(src, dst, target_output_size, &[])
}

/// Decompresses an LZ4 compressed block using an external history dictionary, stopping immediately
/// upon decoding `target_output_size` bytes.
///
/// # Guarantees
/// - Supports references into preceding dictionary window (`dict`).
/// - Strict offset validation preventing out-of-bounds dictionary or destination reads.
pub fn lz4_decompress_safe_partial_using_dict(
    src: &[u8],
    dst: &mut [u8],
    target_output_size: usize,
    dict: &[u8],
) -> Result<usize, TTZipStatus> {
    lz4_decompress_safe_partial_core(src, dst, target_output_size, dict)
}

// MARK: - Core Partial Decompression Engine

/// Internal dual-layer partial decompressor implementation.
pub(crate) fn lz4_decompress_safe_partial_core(
    src: &[u8],
    dst: &mut [u8],
    target_output_size: usize,
    dict: &[u8],
) -> Result<usize, TTZipStatus> {
    if target_output_size == 0 || dst.is_empty() || src.is_empty() {
        return Ok(0);
    }
    if src.len() > i32::MAX as usize
        || dst.len() > i32::MAX as usize
        || dict.len() > i32::MAX as usize
    {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let target_cap = std::cmp::min(target_output_size, dst.len());
    let ip_start = src.as_ptr();
    let iend = unsafe { ip_start.add(src.len()) };
    let mut ip = ip_start;

    let op_start = dst.as_mut_ptr();
    let oend = unsafe { op_start.add(target_cap) };
    let mut op = op_start;

    let dict_ptr = dict.as_ptr();
    let dict_len = dict.len();

    unsafe {
        // ─────────────────────────────────────────────────────────────────────
        // Phase 1: Fast SIMD Partial Loop (Safe Head & Body Zone)
        // ─────────────────────────────────────────────────────────────────────
        while (iend as usize).saturating_sub(ip as usize) >= LZ4_FAST_LOOP_MARGIN
            && (oend as usize).saturating_sub(op as usize) >= LZ4_FAST_LOOP_MARGIN
        {
            let token = *ip;
            ip = ip.add(1);
            let lit_len_token = (token >> 4) as usize;
            let match_len_token = (token & 0x0F) as usize;

            // 1. Literal Stage
            if lit_len_token < 15 {
                std::ptr::copy_nonoverlapping(ip, op, 16);
                ip = ip.add(lit_len_token);
                op = op.add(lit_len_token);
            } else {
                let mut lit_len = 15usize;
                loop {
                    if ip >= iend {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    let s = *ip;
                    ip = ip.add(1);
                    lit_len = lit_len
                        .checked_add(s as usize)
                        .ok_or(TTZipStatus::ErrCorruptHeader)?;
                    if s != 255 {
                        break;
                    }
                }

                let avail_dst = oend as usize - op as usize;
                if lit_len > avail_dst {
                    // Truncate literal copy to exact remaining target capacity and exit
                    if (iend as usize - ip as usize) < avail_dst {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    std::ptr::copy_nonoverlapping(ip, op, avail_dst);
                    return Ok(target_cap);
                }

                if (iend as usize - ip as usize) < lit_len {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }

                if (oend as usize - op as usize) >= lit_len + 16
                    && (iend as usize - ip as usize) >= lit_len + 16
                {
                    wild_copy_16(op, ip, op.add(lit_len));
                } else {
                    std::ptr::copy_nonoverlapping(ip, op, lit_len);
                }
                ip = ip.add(lit_len);
                op = op.add(lit_len);
            }

            if op >= oend {
                return Ok(target_cap);
            }
            if ip >= iend {
                if ip == iend {
                    return Ok(op as usize - op_start as usize);
                }
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            // 2. Read 2-byte Match Offset
            if (iend as usize - ip as usize) < 2 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            let offset = u16::from_le_bytes([*ip, *ip.add(1)]) as usize;
            ip = ip.add(2);

            let history_len = op as usize - op_start as usize;
            if offset == 0 || offset > history_len + dict_len {
                return Err(TTZipStatus::ErrInvalidOffset);
            }

            // If match starts in external dictionary
            if offset > history_len {
                let mut match_len = match_len_token + LZ4_MIN_MATCH;
                if match_len_token == 15 {
                    loop {
                        if ip >= iend {
                            return Err(TTZipStatus::ErrCorruptHeader);
                        }
                        let s = *ip;
                        ip = ip.add(1);
                        match_len = match_len
                            .checked_add(s as usize)
                            .ok_or(TTZipStatus::ErrCorruptHeader)?;
                        if s != 255 {
                            break;
                        }
                    }
                }

                let avail_dst = oend as usize - op as usize;
                let copy_len = std::cmp::min(match_len, avail_dst);

                let dict_offset = offset - history_len;
                let dict_match_ptr = dict_ptr.add(dict_len - dict_offset);
                let dict_avail = dict_offset;

                if copy_len <= dict_avail {
                    std::ptr::copy_nonoverlapping(dict_match_ptr, op, copy_len);
                } else {
                    std::ptr::copy_nonoverlapping(dict_match_ptr, op, dict_avail);
                    let rest = copy_len - dict_avail;
                    for i in 0..rest {
                        *op.add(dict_avail + i) = *op_start.add(i);
                    }
                }
                op = op.add(copy_len);
                if op >= oend {
                    return Ok(target_cap);
                }
                continue;
            }

            // Match within destination buffer
            let match_ptr = op.sub(offset);

            if match_len_token < 15 {
                let match_len = match_len_token + LZ4_MIN_MATCH; // 4..=18
                let avail_dst = oend as usize - op as usize;
                if match_len > avail_dst {
                    let copy_len = avail_dst;
                    if offset == 1 {
                        std::ptr::write_bytes(op, *match_ptr, copy_len);
                    } else if offset >= copy_len {
                        std::ptr::copy_nonoverlapping(match_ptr, op, copy_len);
                    } else {
                        for i in 0..copy_len {
                            *op.add(i) = *match_ptr.add(i);
                        }
                    }
                    return Ok(target_cap);
                }

                if offset >= 8 {
                    std::ptr::copy_nonoverlapping(match_ptr, op, 8);
                    std::ptr::copy_nonoverlapping(match_ptr.add(8), op.add(8), 8);
                    std::ptr::copy_nonoverlapping(match_ptr.add(16), op.add(16), 2);
                    op = op.add(match_len);
                } else {
                    copy_small_offset_ptr(op, match_ptr, match_len, offset);
                    op = op.add(match_len);
                }
            } else {
                let mut match_len = 15 + LZ4_MIN_MATCH;
                loop {
                    if ip >= iend {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    let s = *ip;
                    ip = ip.add(1);
                    match_len = match_len
                        .checked_add(s as usize)
                        .ok_or(TTZipStatus::ErrCorruptHeader)?;
                    if s != 255 {
                        break;
                    }
                }

                let avail_dst = oend as usize - op as usize;
                if match_len > avail_dst {
                    let copy_len = avail_dst;
                    if offset == 1 {
                        std::ptr::write_bytes(op, *match_ptr, copy_len);
                    } else if offset >= copy_len {
                        std::ptr::copy_nonoverlapping(match_ptr, op, copy_len);
                    } else {
                        for i in 0..copy_len {
                            *op.add(i) = *match_ptr.add(i);
                        }
                    }
                    return Ok(target_cap);
                }

                if offset >= 16 && (oend as usize - op as usize) >= match_len + 16 {
                    wild_copy_16(op, match_ptr, op.add(match_len));
                    op = op.add(match_len);
                } else if offset < 8 || (oend as usize - op as usize) >= match_len + 8 {
                    copy_small_offset_ptr(op, match_ptr, match_len, offset);
                    op = op.add(match_len);
                } else {
                    for i in 0..match_len {
                        *op.add(i) = *match_ptr.add(i);
                    }
                    op = op.add(match_len);
                }
            }

            if op >= oend {
                return Ok(target_cap);
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Phase 2: Safe Boundary Convergence Partial Loop
        // ─────────────────────────────────────────────────────────────────────
        while ip < iend && op < oend {
            let token = *ip;
            ip = ip.add(1);
            let lit_len_token = (token >> 4) as usize;
            let match_len_token = (token & 0x0F) as usize;

            let mut lit_len = lit_len_token;
            if lit_len == 15 {
                loop {
                    if ip >= iend {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    let s = *ip;
                    ip = ip.add(1);
                    lit_len = lit_len
                        .checked_add(s as usize)
                        .ok_or(TTZipStatus::ErrCorruptHeader)?;
                    if s != 255 {
                        break;
                    }
                }
            }

            let avail_dst = oend as usize - op as usize;
            if lit_len > avail_dst {
                // Exact truncate and immediate exit
                if (iend as usize - ip as usize) < avail_dst {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                std::ptr::copy_nonoverlapping(ip, op, avail_dst);
                return Ok(target_cap);
            }

            if (iend as usize - ip as usize) < lit_len {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            if lit_len > 0 {
                std::ptr::copy_nonoverlapping(ip, op, lit_len);
                ip = ip.add(lit_len);
                op = op.add(lit_len);
            }

            if op >= oend {
                return Ok(target_cap);
            }
            if ip == iend {
                return Ok(op as usize - op_start as usize);
            }

            // Read 2-byte Match Offset
            if (iend as usize - ip as usize) < 2 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            let offset = u16::from_le_bytes([*ip, *ip.add(1)]) as usize;
            ip = ip.add(2);

            let history_len = op as usize - op_start as usize;
            if offset == 0 || offset > history_len + dict_len {
                return Err(TTZipStatus::ErrInvalidOffset);
            }

            let mut match_len = match_len_token + LZ4_MIN_MATCH;
            if match_len_token == 15 {
                loop {
                    if ip >= iend {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    let s = *ip;
                    ip = ip.add(1);
                    match_len = match_len
                        .checked_add(s as usize)
                        .ok_or(TTZipStatus::ErrCorruptHeader)?;
                    if s != 255 {
                        break;
                    }
                }
            }

            let avail_dst = oend as usize - op as usize;
            let copy_len = std::cmp::min(match_len, avail_dst);

            if offset <= history_len {
                let match_ptr = op.sub(offset);
                if offset == 1 {
                    let byte = *match_ptr;
                    std::ptr::write_bytes(op, byte, copy_len);
                } else if offset >= copy_len {
                    std::ptr::copy_nonoverlapping(match_ptr, op, copy_len);
                } else {
                    for i in 0..copy_len {
                        *op.add(i) = *match_ptr.add(i);
                    }
                }
            } else {
                let dict_offset = offset - history_len;
                let dict_match_ptr = dict_ptr.add(dict_len - dict_offset);
                let dict_avail = dict_offset;
                if copy_len <= dict_avail {
                    std::ptr::copy_nonoverlapping(dict_match_ptr, op, copy_len);
                } else {
                    std::ptr::copy_nonoverlapping(dict_match_ptr, op, dict_avail);
                    let rest = copy_len - dict_avail;
                    for i in 0..rest {
                        *op.add(dict_avail + i) = *op_start.add(i);
                    }
                }
            }

            op = op.add(copy_len);
            if op >= oend {
                return Ok(target_cap);
            }
        }
    }

    Ok(op as usize - op_start as usize)
}

// MARK: - VFS Header Sniffer

/// High-speed zero-allocation VFS header sniffer for LZ4 compressed streams and frames.
///
/// Enables instant (<100ns) inspection of `.tar.lz4`, `.iso.lz4`, `.cpio.lz4`, and
/// embedded archive metadata without decompressing the remaining payload.
pub struct Lz4HeaderSniffer;

impl Lz4HeaderSniffer {
    /// Sniffs and decompresses up to `target_len` bytes from either an LZ4 Frame or raw LZ4 Block.
    ///
    /// Returns a `Vec<u8>` containing the decoded prefix bytes.
    pub fn sniff_payload(src: &[u8], target_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        if target_len == 0 || src.is_empty() {
            return Ok(Vec::new());
        }
        let mut dst = vec![0u8; target_len];
        let written = Self::sniff_payload_into(src, &mut dst, target_len)?;
        dst.truncate(written);
        Ok(dst)
    }

    /// Sniffs and decompresses up to `target_len` bytes into caller-provided destination slice.
    pub fn sniff_payload_into(
        src: &[u8],
        dst: &mut [u8],
        target_len: usize,
    ) -> Result<usize, TTZipStatus> {
        if target_len == 0 || dst.is_empty() || src.is_empty() {
            return Ok(0);
        }

        let mut cursor = 0;

        // Skip any leading skippable frames
        while cursor + 8 <= src.len() {
            let magic = u32::from_le_bytes([
                src[cursor],
                src[cursor + 1],
                src[cursor + 2],
                src[cursor + 3],
            ]);
            if is_lz4_skippable_magic(magic) {
                let skip_len = u32::from_le_bytes([
                    src[cursor + 4],
                    src[cursor + 5],
                    src[cursor + 6],
                    src[cursor + 7],
                ]) as usize;
                cursor = cursor.saturating_add(8).saturating_add(skip_len);
            } else {
                break;
            }
        }

        if cursor >= src.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let remaining = &src[cursor..];

        // Check if stream is standard LZ4 Frame
        if remaining.len() >= 4 {
            let magic = u32::from_le_bytes([
                remaining[0],
                remaining[1],
                remaining[2],
                remaining[3],
            ]);

            if is_lz4_frame_magic(magic) {
                let (_desc, desc_consumed) = FrameDescriptor::parse(&remaining[4..])?;
                let mut block_pos = 4 + desc_consumed;

                if block_pos + 4 > remaining.len() {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }

                let block_size_raw = u32::from_le_bytes([
                    remaining[block_pos],
                    remaining[block_pos + 1],
                    remaining[block_pos + 2],
                    remaining[block_pos + 3],
                ]);
                block_pos += 4;

                if block_size_raw == 0 {
                    // Empty frame / EndMark
                    return Ok(0);
                }

                let is_uncompressed = (block_size_raw & 0x8000_0000) != 0;
                let block_size = (block_size_raw & 0x7FFF_FFFF) as usize;

                if block_pos > remaining.len() {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }

                let block_end = std::cmp::min(block_pos + block_size, remaining.len());
                let block_data = &remaining[block_pos..block_end];

                if is_uncompressed {
                    let copy_len = std::cmp::min(
                        target_len,
                        std::cmp::min(dst.len(), block_data.len()),
                    );
                    dst[..copy_len].copy_from_slice(&block_data[..copy_len]);
                    return Ok(copy_len);
                } else {
                    return lz4_decompress_safe_partial(block_data, dst, target_len);
                }
            } else if is_lz4_legacy_magic(magic) {
                // Legacy frame: 4-byte magic, followed by 4-byte block size and raw LZ4 block
                if remaining.len() < 8 {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                let block_size = u32::from_le_bytes([
                    remaining[4],
                    remaining[5],
                    remaining[6],
                    remaining[7],
                ]) as usize;
                let block_end = std::cmp::min(8 + block_size, remaining.len());
                let block_data = &remaining[8..block_end];
                return lz4_decompress_safe_partial(block_data, dst, target_len);
            }
        }

        // Fallback: Treat as raw LZ4 compressed block
        lz4_decompress_safe_partial(remaining, dst, target_len)
    }

    /// Sniffs the first 512 uncompressed bytes from an LZ4 payload (Tar Header).
    pub fn sniff_tar_header(src: &[u8]) -> Result<[u8; 512], TTZipStatus> {
        let mut header = [0u8; 512];
        let n = Self::sniff_payload_into(src, &mut header, 512)?;
        if n < 512 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        Ok(header)
    }

    /// Sniffs the first 64 uncompressed bytes from an LZ4 payload for magic number detection.
    pub fn sniff_magic_64(src: &[u8]) -> Result<[u8; 64], TTZipStatus> {
        let mut magic = [0u8; 64];
        let n = Self::sniff_payload_into(src, &mut magic, 64)?;
        if n < 64 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        Ok(magic)
    }

    /// Determines if the provided LZ4 stream encapsulates a valid TAR archive.
    pub fn is_tar_lz4(src: &[u8]) -> bool {
        if let Ok(header) = Self::sniff_tar_header(src) {
            // Check POSIX ustar ("ustar\0" or "ustar  \0") at offset 257
            if header[257..263] == *b"ustar\0" || header[257..265] == *b"ustar  \0" {
                return true;
            }
            // Or validate 512-byte tar header checksum
            Self::validate_tar_header_checksum(&header)
        } else {
            false
        }
    }

    /// Validates the octal checksum field of a standard 512-byte TAR header block.
    pub fn validate_tar_header_checksum(header: &[u8; 512]) -> bool {
        let chksum_field = &header[148..156];
        let chksum_str = match std::str::from_utf8(chksum_field) {
            Ok(s) => s.trim().trim_matches('\0'),
            Err(_) => return false,
        };
        if chksum_str.is_empty() {
            return false;
        }
        let expected_chksum = match u32::from_str_radix(chksum_str, 8) {
            Ok(v) => v,
            Err(_) => return false,
        };

        let mut unsigned_sum = 0u32;
        let mut signed_sum = 0i32;

        for (i, &byte) in header.iter().enumerate() {
            let val = if (148..156).contains(&i) {
                0x20u8
            } else {
                byte
            };
            unsigned_sum += val as u32;
            signed_sum += (val as i8) as i32;
        }

        expected_chksum == unsigned_sum || expected_chksum as i32 == signed_sum
    }

    /// Sniffs the inner archive format encapsulated within the LZ4 stream.
    pub fn sniff_inner_format(src: &[u8]) -> Result<ArchiveFormat, TTZipStatus> {
        let peek = Self::sniff_payload(src, 512)?;
        match FormatSniffer::sniff(&peek) {
            SniffResult::Yes { format, .. } => Ok(format),
            _ => {
                if peek.len() >= 512
                    && Self::validate_tar_header_checksum(
                        (&peek[..512]).try_into().unwrap_or(&[0u8; 512]),
                    )
                {
                    Ok(ArchiveFormat::Tar)
                } else {
                    Ok(ArchiveFormat::Unknown)
                }
            }
        }
    }
}
