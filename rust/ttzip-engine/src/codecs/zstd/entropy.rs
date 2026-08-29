// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-throughput FSE (Finite State Entropy / tANS) and Huff0 (4-Stream Huffman) entropy codecs.

use super::types::*;
use crate::types::TTZipStatus;

// MARK: - Constants

#[allow(dead_code)]
const FSE_MAX_TABLELOG: u32 = 12;
#[allow(dead_code)]
const FSE_MAX_SYMBOL_VALUE: u32 = 255;
#[allow(dead_code)]
const HUF_MAX_TABLELOG: u32 = 11;
#[allow(dead_code)]
const HUF_MAX_SYMBOL_VALUE: u32 = 255;
#[allow(dead_code)]
const HUF_WORKSPACE_SIZE_BYTES: usize = 9 * 1024;

// MARK: - FSE (Finite State Entropy / tANS)

/// Computes upper bound on compressed bytes for FSE encoding.
#[inline]
pub fn fse_compress_bound(src_size: usize) -> usize {
    unsafe { FSE_compressBound(src_size) + 512 }
}

fn fse_compress_raw(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    if dst.len() < 16 {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    let mut count = [0u32; 256];
    let mut max_symbol_value: u32 = 255;
    let mut hist_wksp = [0u32; 1024];

    let hist_res = unsafe {
        HIST_count_wksp(
            count.as_mut_ptr(),
            &mut max_symbol_value,
            src.as_ptr() as *const libc::c_void,
            src.len(),
            hist_wksp.as_mut_ptr() as *mut libc::c_void,
            std::mem::size_of_val(&hist_wksp),
        )
    };
    if unsafe { FSE_isError(hist_res) } != 0 {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    let table_log = unsafe {
        FSE_optimalTableLog(FSE_MAX_TABLELOG, src.len(), max_symbol_value)
    };
    if table_log == 0 {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    let mut normalized_counter = [0i16; 256];
    let norm_res = unsafe {
        FSE_normalizeCount(
            normalized_counter.as_mut_ptr(),
            table_log,
            count.as_ptr(),
            src.len(),
            max_symbol_value,
            1,
        )
    };
    if unsafe { FSE_isError(norm_res) } != 0 {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    let header_size = unsafe {
        FSE_writeNCount(
            dst.as_mut_ptr() as *mut libc::c_void,
            dst.len(),
            normalized_counter.as_ptr(),
            max_symbol_value,
            table_log,
        )
    };
    if unsafe { FSE_isError(header_size) } != 0 || header_size >= dst.len() {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    let mut ctable = vec![0u32; 4096];
    let mut wksp = vec![0u8; 16384];
    let build_res = unsafe {
        FSE_buildCTable_wksp(
            ctable.as_mut_ptr(),
            normalized_counter.as_ptr(),
            max_symbol_value,
            table_log,
            wksp.as_mut_ptr() as *mut libc::c_void,
            wksp.len(),
        )
    };
    if unsafe { FSE_isError(build_res) } != 0 {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    let compressed_body_size = unsafe {
        FSE_compress_usingCTable(
            dst[header_size..].as_mut_ptr() as *mut libc::c_void,
            dst.len() - header_size,
            src.as_ptr() as *const libc::c_void,
            src.len(),
            ctable.as_ptr(),
        )
    };
    if unsafe { FSE_isError(compressed_body_size) } != 0 || compressed_body_size == 0 {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    Ok(header_size + compressed_body_size)
}

fn fse_decompress_raw(c_src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if c_src.is_empty() {
        return Ok(0);
    }
    if dst.is_empty() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let mut wksp = [0u32; 8192];
    let decomp_size = unsafe {
        FSE_decompress_wksp_bmi2(
            dst.as_mut_ptr() as *mut libc::c_void,
            dst.len(),
            c_src.as_ptr() as *const libc::c_void,
            c_src.len(),
            FSE_MAX_TABLELOG,
            wksp.as_mut_ptr() as *mut libc::c_void,
            std::mem::size_of_val(&wksp),
            0,
        )
    };
    if unsafe { FSE_isError(decomp_size) } != 0 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    Ok(decomp_size)
}

/// Compresses a block using Finite State Entropy (tANS) with automatic uncompressed fallback.
pub fn fse_compress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    if dst.len() < src.len() + 1 {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    if let Ok(c_len) = fse_compress_raw(src, &mut dst[1..]) {
        if c_len < src.len() {
            dst[0] = 0x01; // Mode 1: Compressed FSE
            return Ok(c_len + 1);
        }
    }

    // Fallback: Mode 0: Store raw
    dst[0] = 0x00;
    dst[1..1 + src.len()].copy_from_slice(src);
    Ok(1 + src.len())
}

/// Decompresses an FSE encoded block into destination buffer.
pub fn fse_decompress(c_src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if c_src.is_empty() {
        return Ok(0);
    }
    if dst.is_empty() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    match c_src[0] {
        0x00 => {
            let raw_len = c_src.len() - 1;
            if dst.len() < raw_len {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            dst[..raw_len].copy_from_slice(&c_src[1..]);
            Ok(raw_len)
        }
        0x01 => fse_decompress_raw(&c_src[1..], dst),
        _ => Err(TTZipStatus::ErrCorruptHeader),
    }
}

// MARK: - Huff0 (4-Stream Parallel Huffman)

/// Computes upper bound on compressed bytes for Huff0 encoding.
#[inline]
pub fn huf0_compress_bound(src_size: usize) -> usize {
    unsafe { HUF_compressBound(src_size) + 512 }
}

fn huf0_compress4x_raw(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.len() < 32 || dst.len() < 16 {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    let mut wksp = [0u64; 2048];
    let res = unsafe {
        HUF_compress4X_repeat(
            dst.as_mut_ptr() as *mut libc::c_void,
            dst.len(),
            src.as_ptr() as *const libc::c_void,
            src.len(),
            HUF_MAX_SYMBOL_VALUE,
            HUF_MAX_TABLELOG,
            wksp.as_mut_ptr() as *mut libc::c_void,
            std::mem::size_of_val(&wksp),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };

    if unsafe { HUF_isError(res) } != 0 || res == 0 {
        Err(TTZipStatus::ErrCompressionFailed)
    } else {
        Ok(res)
    }
}

fn huf0_compress1x_raw(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() || dst.len() < 16 {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    let mut wksp = [0u64; 2048];
    let res = unsafe {
        HUF_compress1X_repeat(
            dst.as_mut_ptr() as *mut libc::c_void,
            dst.len(),
            src.as_ptr() as *const libc::c_void,
            src.len(),
            HUF_MAX_SYMBOL_VALUE,
            HUF_MAX_TABLELOG,
            wksp.as_mut_ptr() as *mut libc::c_void,
            std::mem::size_of_val(&wksp),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };

    if unsafe { HUF_isError(res) } != 0 || res == 0 {
        Err(TTZipStatus::ErrCompressionFailed)
    } else {
        Ok(res)
    }
}

fn huf0_decompress4x_raw(c_src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if c_src.is_empty() || dst.is_empty() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let mut dtable = [0u32; 1 << (HUF_MAX_TABLELOG + 1)];
    dtable[0] = HUF_MAX_TABLELOG * 0x0100_0001;
    let mut wksp = [0u64; 1024];

    let header_size = unsafe {
        HUF_readDTableX2_wksp(
            dtable.as_mut_ptr(),
            c_src.as_ptr() as *const libc::c_void,
            c_src.len(),
            wksp.as_mut_ptr() as *mut libc::c_void,
            std::mem::size_of_val(&wksp),
            0,
        )
    };
    if unsafe { HUF_isError(header_size) } != 0 || header_size >= c_src.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let decomp_size = unsafe {
        HUF_decompress4X_usingDTable(
            dst.as_mut_ptr() as *mut libc::c_void,
            dst.len(),
            c_src[header_size..].as_ptr() as *const libc::c_void,
            c_src.len() - header_size,
            dtable.as_ptr(),
            0,
        )
    };

    if unsafe { HUF_isError(decomp_size) } != 0 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    Ok(decomp_size)
}

fn huf0_decompress1x_raw(c_src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if c_src.is_empty() || dst.is_empty() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let mut dtable = [0u32; 1 << (HUF_MAX_TABLELOG + 1)];
    dtable[0] = (HUF_MAX_TABLELOG - 1) * 0x0100_0001;
    let mut wksp = [0u64; 1024];

    let header_size = unsafe {
        HUF_readDTableX1_wksp(
            dtable.as_mut_ptr(),
            c_src.as_ptr() as *const libc::c_void,
            c_src.len(),
            wksp.as_mut_ptr() as *mut libc::c_void,
            std::mem::size_of_val(&wksp),
            0,
        )
    };
    if unsafe { HUF_isError(header_size) } != 0 || header_size >= c_src.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let decomp_size = unsafe {
        HUF_decompress1X_usingDTable(
            dst.as_mut_ptr() as *mut libc::c_void,
            dst.len(),
            c_src[header_size..].as_ptr() as *const libc::c_void,
            c_src.len() - header_size,
            dtable.as_ptr(),
            0,
        )
    };

    if unsafe { HUF_isError(decomp_size) } != 0 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    Ok(decomp_size)
}

/// Compresses a buffer using Huff0 4-Stream parallel Huffman encoding with store mode fallback.
pub fn huf0_compress4x(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    if dst.len() < src.len() + 1 {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    if let Ok(c_len) = huf0_compress4x_raw(src, &mut dst[1..]) {
        if c_len < src.len() {
            dst[0] = 0x04; // Mode 4: Huff0 4-Stream
            return Ok(c_len + 1);
        }
    }

    // Try 1-Stream if 4-Stream cannot divide
    if let Ok(c_len) = huf0_compress1x_raw(src, &mut dst[1..]) {
        if c_len < src.len() {
            dst[0] = 0x01; // Mode 1: Huff0 1-Stream
            return Ok(c_len + 1);
        }
    }

    // Fallback: Mode 0: Store raw
    dst[0] = 0x00;
    dst[1..1 + src.len()].copy_from_slice(src);
    Ok(1 + src.len())
}

/// Compresses a buffer using Huff0 1-Stream sequential Huffman encoding with store mode fallback.
pub fn huf0_compress1x(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    if dst.len() < src.len() + 1 {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    if let Ok(c_len) = huf0_compress1x_raw(src, &mut dst[1..]) {
        if c_len < src.len() {
            dst[0] = 0x01; // Mode 1: Huff0 1-Stream
            return Ok(c_len + 1);
        }
    }

    // Fallback: Mode 0: Store raw
    dst[0] = 0x00;
    dst[1..1 + src.len()].copy_from_slice(src);
    Ok(1 + src.len())
}

/// Decompresses a Huff0 4-Stream or framed block into destination buffer.
pub fn huf0_decompress4x(c_src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if c_src.is_empty() {
        return Ok(0);
    }
    if dst.is_empty() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    match c_src[0] {
        0x00 => {
            let raw_len = c_src.len() - 1;
            if dst.len() < raw_len {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            dst[..raw_len].copy_from_slice(&c_src[1..]);
            Ok(raw_len)
        }
        0x01 => huf0_decompress1x_raw(&c_src[1..], dst),
        0x04 => huf0_decompress4x_raw(&c_src[1..], dst),
        _ => Err(TTZipStatus::ErrCorruptHeader),
    }
}

/// Decompresses a Huff0 1-Stream or framed block into destination buffer.
pub fn huf0_decompress1x(c_src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    huf0_decompress4x(c_src, dst)
}




