// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe RAII wrapper for `liblzma` (XZ Utils) LZMA1 / LZMA-Alone decoder.

use crate::types::TTZipStatus;

#[repr(C)]
struct LzmaStream {
    next_in: *const u8,
    avail_in: libc::size_t,
    total_in: u64,
    next_out: *mut u8,
    avail_out: libc::size_t,
    total_out: u64,
    allocator: *const libc::c_void,
    internal: *mut libc::c_void,
    reserved_ptr1: *mut libc::c_void,
    reserved_ptr2: *mut libc::c_void,
    reserved_ptr3: *mut libc::c_void,
    reserved_ptr4: *mut libc::c_void,
    reserved_seek: u64,
    reserved_int1: u64,
    reserved_int2: libc::size_t,
    reserved_int3: libc::size_t,
    reserved_enum1: libc::c_int,
    reserved_enum2: libc::c_int,
}

impl Default for LzmaStream {
    fn default() -> Self {
        Self {
            next_in: std::ptr::null(),
            avail_in: 0,
            total_in: 0,
            next_out: std::ptr::null_mut(),
            avail_out: 0,
            total_out: 0,
            allocator: std::ptr::null(),
            internal: std::ptr::null_mut(),
            reserved_ptr1: std::ptr::null_mut(),
            reserved_ptr2: std::ptr::null_mut(),
            reserved_ptr3: std::ptr::null_mut(),
            reserved_ptr4: std::ptr::null_mut(),
            reserved_seek: 0,
            reserved_int1: 0,
            reserved_int2: 0,
            reserved_int3: 0,
            reserved_enum1: 0,
            reserved_enum2: 0,
        }
    }
}

pub const LZMA_OK: libc::c_int = 0;
pub const LZMA_STREAM_END: libc::c_int = 1;
pub const LZMA_RUN: libc::c_int = 0;
pub const LZMA_FINISH: libc::c_int = 3;
pub const LZMA_MEM_ERROR: libc::c_int = 5;
pub const LZMA_MEMLIMIT_ERROR: libc::c_int = 6;
pub const LZMA_FORMAT_ERROR: libc::c_int = 7;
pub const LZMA_OPTIONS_ERROR: libc::c_int = 8;
pub const LZMA_DATA_ERROR: libc::c_int = 9;
pub const LZMA_BUF_ERROR: libc::c_int = 10;
pub const LZMA_PROG_ERROR: libc::c_int = 11;

/// Maximum allowable dictionary size for single-pass LZMA1 extraction (64 MiB).
pub const MAX_LZMA_DICT_SIZE: u32 = 64 * 1024 * 1024;

extern "C" {
    fn lzma_alone_decoder(strm: *mut LzmaStream, memlimit: u64) -> libc::c_int;
    fn lzma_code(strm: *mut LzmaStream, action: libc::c_int) -> libc::c_int;
    fn lzma_end(strm: *mut LzmaStream);
}

/// Safe RAII streaming LZMA Alone decoder.
pub struct LzmaAloneDecoder {
    strm: LzmaStream,
    initialized: bool,
}

unsafe impl Send for LzmaAloneDecoder {}

impl LzmaAloneDecoder {
    /// Creates a new LZMA Alone decoder with default 64 MiB memory budget.
    pub fn new() -> Result<Self, TTZipStatus> {
        Self::new_with_memlimit(MAX_LZMA_DICT_SIZE as u64)
    }

    /// Creates a new LZMA Alone decoder with custom memory budget limit.
    pub fn new_with_memlimit(memlimit: u64) -> Result<Self, TTZipStatus> {
        let mut decoder = Self {
            strm: LzmaStream::default(),
            initialized: false,
        };
        let res = unsafe { lzma_alone_decoder(&mut decoder.strm, memlimit) };
        if res == LZMA_MEM_ERROR || res == LZMA_MEMLIMIT_ERROR {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        if res != LZMA_OK {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        decoder.initialized = true;
        Ok(decoder)
    }

    /// Decompresses a chunk of data.
    pub fn decompress_chunk(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        finish: bool,
    ) -> Result<(usize, usize, bool), TTZipStatus> {
        self.strm.next_in = if input.is_empty() {
            std::ptr::null()
        } else {
            input.as_ptr()
        };
        self.strm.avail_in = input.len();
        self.strm.next_out = if output.is_empty() {
            std::ptr::null_mut()
        } else {
            output.as_mut_ptr()
        };
        self.strm.avail_out = output.len();

        let action = if finish { LZMA_FINISH } else { LZMA_RUN };
        let ret = unsafe { lzma_code(&mut self.strm, action) };

        let in_consumed = input.len().saturating_sub(self.strm.avail_in);
        let out_produced = output.len().saturating_sub(self.strm.avail_out);

        if ret == LZMA_STREAM_END {
            Ok((in_consumed, out_produced, true))
        } else if ret == LZMA_OK {
            Ok((in_consumed, out_produced, false))
        } else if ret == LZMA_MEM_ERROR || ret == LZMA_MEMLIMIT_ERROR {
            Err(TTZipStatus::ErrOutOfMemory)
        } else if ret == LZMA_DATA_ERROR || ret == LZMA_FORMAT_ERROR || ret == LZMA_OPTIONS_ERROR {
            Err(TTZipStatus::ErrCorruptHeader)
        } else {
            Err(TTZipStatus::ErrExtractionFailed)
        }
    }
}

impl Drop for LzmaAloneDecoder {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                lzma_end(&mut self.strm);
            }
        }
    }
}

/// Decompresses raw LZMA1 payload using 5-byte coder properties and expected uncompressed size.
pub fn lzma1_decompress(
    raw_payload: &[u8],
    coder_props: &[u8],
    uncompressed_size: u64,
    dst: &mut [u8],
) -> Result<usize, TTZipStatus> {
    if coder_props.len() < 5 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let dict_size = u32::from_le_bytes([coder_props[1], coder_props[2], coder_props[3], coder_props[4]]);
    if dict_size > MAX_LZMA_DICT_SIZE {
        return Err(TTZipStatus::ErrOutOfMemory);
    }

    // 1. Stack-allocated 13-byte Alone header (0 heap allocations)
    let mut header = [0u8; 13];
    header[..5].copy_from_slice(&coder_props[..5]);
    header[5..13].copy_from_slice(&uncompressed_size.to_le_bytes());

    let mut decoder = LzmaAloneDecoder::new()?;

    // 2. Feed 13-byte header to initialize LZMA model parameters
    let mut header_slice = &header[..];
    let mut out_offset = 0;
    while !header_slice.is_empty() {
        let (consumed, produced, is_end) = decoder.decompress_chunk(header_slice, &mut dst[out_offset..], false)?;
        header_slice = &header_slice[consumed..];
        out_offset += produced;
        if is_end {
            return Ok(out_offset);
        }
    }

    // 3. Zero-copy stream raw payload directly without intermediate Vec allocation
    let mut payload_slice = raw_payload;
    while !payload_slice.is_empty() {
        let (consumed, produced, is_end) = decoder.decompress_chunk(payload_slice, &mut dst[out_offset..], true)?;
        payload_slice = &payload_slice[consumed..];
        out_offset += produced;
        if is_end || (consumed == 0 && produced == 0) {
            break;
        }
    }

    Ok(out_offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(C)]
    struct LzmaOptionsLzma {
        dict_size: u32,
        preset_dict: *const u8,
        preset_dict_size: u32,
        lc: u32,
        lp: u32,
        pb: u32,
        mode: libc::c_int,
        nice_len: u32,
        mf: libc::c_int,
        depth: u32,
        reserved_int1: u32,
        reserved_int2: u32,
        reserved_int3: u32,
        reserved_int4: u32,
        reserved_ptr1: *mut libc::c_void,
        reserved_ptr2: *mut libc::c_void,
    }

    extern "C" {
        fn lzma_lzma_preset(options: *mut LzmaOptionsLzma, preset: u32) -> bool;
        fn lzma_alone_encoder(strm: *mut LzmaStream, options: *const LzmaOptionsLzma) -> libc::c_int;
    }

    #[test]
    fn test_lzma1_alone_roundtrip() {
        let input = b"TTZip High-Performance LZMA1 liblzma Engine test string payload with repetition 1234567890 1234567890";

        let mut opt = std::mem::MaybeUninit::<LzmaOptionsLzma>::uninit();
        let preset_err = unsafe { lzma_lzma_preset(opt.as_mut_ptr(), 6) };
        assert!(!preset_err);
        let opt = unsafe { opt.assume_init() };

        let mut strm = LzmaStream::default();
        let enc_res = unsafe { lzma_alone_encoder(&mut strm, &opt) };
        assert_eq!(enc_res, LZMA_OK);

        let mut comp_buf = vec![0u8; 1024];
        strm.next_in = input.as_ptr();
        strm.avail_in = input.len();
        strm.next_out = comp_buf.as_mut_ptr();
        strm.avail_out = comp_buf.len();

        let code_res = unsafe { lzma_code(&mut strm, LZMA_FINISH) };
        assert_eq!(code_res, LZMA_STREAM_END);
        let total_comp = strm.total_out as usize;
        unsafe { lzma_end(&mut strm) };

        assert!(total_comp >= 13);
        let coder_props = &comp_buf[0..5];
        let raw_payload = &comp_buf[13..total_comp];

        let mut decomp = vec![0u8; input.len()];
        let out_len = lzma1_decompress(raw_payload, coder_props, input.len() as u64, &mut decomp)
            .expect("lzma1_decompress failed");
        assert_eq!(out_len, input.len());
        assert_eq!(&decomp, input);
    }
}
