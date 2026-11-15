// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 7-Zip Codec Bitstream Pump Helpers.
//!
//! Thin streaming wrappers around the LZMA2 and LZMA1 codecs that push decoded bytes into a
//! caller-supplied sink while enforcing the folder's expected unpack size.

use super::payload::map_codec_bitstream_err;
use crate::types::TTZipStatus;
use std::cell::RefCell;

const MIN_STREAM_CHUNK_BYTES: usize = 16 * 1024;
const MAX_RETAINED_SCRATCH_BYTES: usize = 1024 * 1024;

thread_local! {
    static DECODE_SCRATCH_BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static REUSABLE_FL2_DSTREAM: RefCell<Option<crate::codecs::lzma2::Fl2DStream>> = const { RefCell::new(None) };
}

#[inline]
pub(crate) fn resolve_decoder_threads(threads: u32) -> u32 {
    if threads == 0 {
        std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1)
    } else {
        threads
    }
}

#[inline]
pub(crate) fn bounded_out_chunk_len(unpack_limit: u64, max_chunk: usize) -> usize {
    let floor = MIN_STREAM_CHUNK_BYTES.min(max_chunk.max(1));
    if unpack_limit > 0 {
        (unpack_limit as usize).clamp(floor, max_chunk.max(floor))
    } else {
        max_chunk.max(floor)
    }
}

pub(crate) fn with_decode_scratch_buf<R, F>(len: usize, f: F) -> R
where
    F: FnOnce(&mut [u8]) -> R,
{
    DECODE_SCRATCH_BUF.with(|cell| {
        if let Ok(mut buf) = cell.try_borrow_mut() {
            if buf.len() < len {
                buf.resize(len, 0);
            }
            let res = f(&mut buf[..len]);
            if buf.capacity() > MAX_RETAINED_SCRATCH_BYTES {
                buf.truncate(MAX_RETAINED_SCRATCH_BYTES);
                buf.shrink_to(MAX_RETAINED_SCRATCH_BYTES);
            }
            res
        } else {
            let mut fallback = vec![0u8; len];
            f(&mut fallback)
        }
    })
}

pub(crate) fn lzma2_dict_size_from_prop(prop: u8) -> Option<u64> {
    if prop > 40 {
        return None;
    }
    if prop == 40 {
        return Some(0xFFFF_FFFF);
    }
    let base = 2u64 | ((prop as u64) & 1);
    Some(base << ((prop as u32) / 2 + 11))
}

pub(crate) fn effective_lzma2_dict_prop(prop: Option<u8>, expected_unpack_size: u64) -> Option<u8> {
    let p = prop?;
    if expected_unpack_size == 0 || p > 40 {
        return Some(p);
    }
    let declared = lzma2_dict_size_from_prop(p)?;
    let needed = expected_unpack_size.max(64 * 1024);
    if declared <= needed {
        return Some(p);
    }
    for cand in 0..=p {
        if let Some(sz) = lzma2_dict_size_from_prop(cand) {
            if sz >= needed {
                return Some(cand);
            }
        }
    }
    Some(p)
}

pub(crate) fn with_reusable_fl2_dstream<R, F>(
    threads: u32,
    prop: Option<u8>,
    expected_unpack_size: u64,
    f: F,
) -> Result<R, TTZipStatus>
where
    F: FnOnce(&mut crate::codecs::lzma2::Fl2DStream) -> Result<R, TTZipStatus>,
{
    let eff_prop = effective_lzma2_dict_prop(prop, expected_unpack_size);
    if threads <= 1 {
        return REUSABLE_FL2_DSTREAM.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut() {
                let ds = match slot.as_mut() {
                    Some(ds) => ds,
                    None => slot.insert(crate::codecs::lzma2::Fl2DStream::new(1)?),
                };
                ds.init(eff_prop)?;
                let res = f(ds);
                if eff_prop.unwrap_or(0) > 24 {
                    *slot = None;
                }
                return res;
            }
            let mut ds = crate::codecs::lzma2::Fl2DStream::new(1)?;
            ds.init(eff_prop)?;
            f(&mut ds)
        });
    }
    let mut ds = crate::codecs::lzma2::Fl2DStream::new(resolve_decoder_threads(threads))?;
    ds.init(eff_prop)?;
    f(&mut ds)
}

pub(crate) fn emit_checked_chunk<F>(
    chunk: &[u8],
    total_decompressed: &mut u64,
    expected_unpack_size: u64,
    sink: &mut F,
) -> Result<(), TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    if chunk.is_empty() {
        return Ok(());
    }
    let next_total = total_decompressed
        .checked_add(chunk.len() as u64)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    if expected_unpack_size > 0 && next_total > expected_unpack_size {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    *total_decompressed = next_total;
    sink(chunk)
}

pub(crate) enum Lzma2Work<'a, 'token> {
    Archive(u64),
    Preview(&'a mut PreviewWork<'token>),
}

pub(crate) struct PreviewWork<'a> {
    pub(crate) end: u64,
    pub(crate) accepted: u64,
    pub(crate) cancellation: &'a dyn Fn() -> bool,
}

impl PreviewWork<'_> {
    pub(crate) fn check_cancelled(&self) -> Result<(), TTZipStatus> {
        if (self.cancellation)() {
            return Err(TTZipStatus::Cancelled);
        }
        Ok(())
    }

    fn after_step(&mut self, accepted: usize, decoded: u64) -> Result<(), TTZipStatus> {
        self.check_cancelled()?;
        self.accepted = self.accepted.checked_add(accepted as u64)
            .ok_or(TTZipStatus::ErrCorruptHeader)?;
        // Fast-LZMA2 may report up to 40 bytes retained in its overlap buffer as consumed.
        let consumed = self.accepted.saturating_sub(40);
        if decoded > (1024 * 1024).max(consumed.saturating_mul(64)) {
            return Err(TTZipStatus::ErrSolidBudgetExceeded);
        }
        Ok(())
    }
}

impl Lzma2Work<'_, '_> {
    fn output_cap(&self, proposed: usize, decoded: u64) -> Result<usize, TTZipStatus> {
        match self {
            Self::Archive(_) => Ok(proposed),
            Self::Preview(preview) => {
                preview.check_cancelled()?;
                let remaining = preview.end.saturating_sub(decoded);
                Ok(proposed.min(remaining as usize))
            }
        }
    }

    fn after_step(&mut self, consumed: usize, produced: usize, decoded: u64) -> Result<(), TTZipStatus> {
        if let Self::Preview(preview) = self {
            let decoded = decoded.checked_add(produced as u64)
                .ok_or(TTZipStatus::ErrCorruptHeader)?;
            preview.after_step(consumed, decoded)?;
        }
        Ok(())
    }
}

fn lzma2_needs_input(
    input: &crate::codecs::lzma2::Fl2InBuffer,
    consumed: usize,
    is_final_input: bool,
) -> Result<bool, TTZipStatus> {
    if input.pos >= input.size && !is_final_input {
        return Ok(true);
    }
    if consumed == 0 || is_final_input {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    Ok(false)
}

/// Pumps an LZMA2 `Fl2DStream`, continuing to drain internal dictionary/MT buffers even after
/// `plain_slice` is fully consumed. Returns `Ok(true)` when `CHUNK_FINAL` (`remaining == 0`) is
/// reached, or `Ok(false)` when more compressed input is required from the caller.
pub(crate) fn pump_lzma2_stream<F>(
    dstream: &mut crate::codecs::lzma2::Fl2DStream,
    plain_slice: &[u8],
    out_chunk: &mut [u8],
    is_final_input: bool,
    total_decompressed: &mut u64,
    mut work: Lzma2Work<'_, '_>,
    sink: &mut F,
) -> Result<bool, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let expected_unpack_size = match &work {
        Lzma2Work::Archive(size) => *size,
        Lzma2Work::Preview(preview) => preview.end,
    };
    let mut in_buf = crate::codecs::lzma2::Fl2InBuffer {
        src: plain_slice.as_ptr() as *const libc::c_void,
        size: plain_slice.len(),
        pos: 0,
    };
    let mut step_cap = if *total_decompressed == 0 {
        MIN_STREAM_CHUNK_BYTES.min(out_chunk.len().max(1))
    } else {
        out_chunk.len()
    };

    loop {
        let prev_in_pos = in_buf.pos;
        let rem_cap = if expected_unpack_size > 0 {
            (expected_unpack_size.saturating_sub(*total_decompressed) as usize)
                .max(MIN_STREAM_CHUNK_BYTES)
        } else {
            out_chunk.len()
        };
        let proposed_cap = step_cap.min(rem_cap).min(out_chunk.len());
        let cur_cap = work.output_cap(proposed_cap, *total_decompressed)?;
        step_cap = step_cap.saturating_mul(2).min(out_chunk.len());
        let mut out_buf = crate::codecs::lzma2::Fl2OutBuffer {
            dst: out_chunk.as_mut_ptr() as *mut libc::c_void,
            size: cur_cap,
            pos: 0,
        };

        let remaining = dstream
            .decompress_stream(&mut in_buf, &mut out_buf)
            .map_err(map_codec_bitstream_err)?;
        let produced = out_buf.pos;
        let consumed = in_buf.pos.saturating_sub(prev_in_pos);
        work.after_step(consumed, produced, *total_decompressed)?;
        if produced > 0 {
            emit_checked_chunk(
                &out_chunk[..produced],
                total_decompressed,
                expected_unpack_size,
                sink,
            )?;
        }

        if remaining == 0 {
            return Ok(true);
        }

        if produced == 0 && lzma2_needs_input(&in_buf, consumed, is_final_input)? {
            return Ok(false);
        }
    }
}

/// Initializes an `LzmaAloneDecoder` with the synthesized 13-byte LZMA_Alone header.
pub(crate) fn init_lzma1_alone_decoder<F>(
    coder_props: &[u8],
    expected_unpack_size: u64,
    out_chunk: &mut [u8],
    total_decompressed: &mut u64,
    sink: &mut F,
) -> Result<(crate::codecs::lzma::LzmaAloneDecoder, bool), TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    if coder_props.len() < 5 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let dict_size = u32::from_le_bytes([
        coder_props[1],
        coder_props[2],
        coder_props[3],
        coder_props[4],
    ]);
    if dict_size > crate::codecs::lzma::alone::MAX_LZMA_DICT_SIZE {
        return Err(TTZipStatus::ErrOutOfMemory);
    }

    let mut header = [0u8; 13];
    header[..5].copy_from_slice(&coder_props[..5]);
    let alone_size = if expected_unpack_size > 0 {
        expected_unpack_size
    } else {
        u64::MAX
    };
    header[5..13].copy_from_slice(&alone_size.to_le_bytes());

    let mut decoder =
        crate::codecs::lzma::LzmaAloneDecoder::new().map_err(map_codec_bitstream_err)?;
    let mut header_slice = &header[..];
    while !header_slice.is_empty() {
        let (consumed, produced, is_end) = decoder
            .decompress_chunk(header_slice, out_chunk, false)
            .map_err(map_codec_bitstream_err)?;
        header_slice = &header_slice[consumed..];
        if produced > 0 {
            emit_checked_chunk(
                &out_chunk[..produced],
                total_decompressed,
                expected_unpack_size,
                sink,
            )?;
        }
        if is_end {
            return Ok((decoder, true));
        }
        if consumed == 0 && produced == 0 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
    }
    Ok((decoder, false))
}

/// Pumps an `LzmaAloneDecoder`, draining internal output even after `plain_slice` is exhausted.
pub(crate) fn pump_lzma1_stream<F>(
    decoder: &mut crate::codecs::lzma::LzmaAloneDecoder,
    plain_slice: &[u8],
    out_chunk: &mut [u8],
    is_final_input: bool,
    total_decompressed: &mut u64,
    expected_unpack_size: u64,
    sink: &mut F,
) -> Result<bool, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let mut input_slice = plain_slice;
    loop {
        let (consumed, produced, is_end) = decoder
            .decompress_chunk(input_slice, out_chunk, false)
            .map_err(map_codec_bitstream_err)?;
        input_slice = &input_slice[consumed..];
        if produced > 0 {
            emit_checked_chunk(
                &out_chunk[..produced],
                total_decompressed,
                expected_unpack_size,
                sink,
            )?;
        }
        if is_end {
            return Ok(true);
        }
        if produced == 0 {
            if input_slice.is_empty() && !is_final_input {
                return Ok(false);
            }
            if consumed == 0 || is_final_input {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
        }
    }
}

#[inline(always)]
fn is_bcj_x86_ms_byte(b: u8) -> bool {
    b == 0x00 || b == 0xFF
}

#[inline]
fn compute_x86_branch_dest_decode(mut src: u32, adj: u32, prev_mask: u32) -> u32 {
    const MASK_TO_BIT_NUMBER: [u32; 5] = [0, 1, 2, 2, 3];
    loop {
        let dest = src.wrapping_sub(adj);
        if prev_mask == 0 {
            return dest;
        }
        let i = MASK_TO_BIT_NUMBER[(prev_mask >> 1) as usize];
        let b_check = (dest >> (24 - i * 8)) as u8;
        if !is_bcj_x86_ms_byte(b_check) {
            return dest;
        }
        src = dest ^ ((1u32 << (32 - i * 8)).wrapping_sub(1));
    }
}

fn x86_decode_stateful(
    buffer: &mut [u8],
    now_pos: u32,
    prev_mask: &mut u32,
    prev_pos: &mut u32,
) -> usize {
    if buffer.len() < 5 {
        return 0;
    }
    let limit = buffer.len() - 5;
    let mut buffer_pos = 0usize;
    while buffer_pos <= limit {
        let b0 = buffer[buffer_pos];
        if b0 != 0xE8 && b0 != 0xE9 {
            buffer_pos += 1;
            continue;
        }
        let curr_pos = now_pos.wrapping_add(buffer_pos as u32);
        let offset = curr_pos.wrapping_sub(*prev_pos);
        *prev_pos = curr_pos;
        if offset > 5 {
            *prev_mask = 0;
        } else {
            for _ in 0..offset {
                *prev_mask &= 0x77;
                *prev_mask <<= 1;
            }
        }
        let b = buffer[buffer_pos + 4];
        if is_bcj_x86_ms_byte(b) && (*prev_mask >> 1) <= 4 && (*prev_mask >> 1) != 3 {
            let src = ((b as u32) << 24)
                | ((buffer[buffer_pos + 3] as u32) << 16)
                | ((buffer[buffer_pos + 2] as u32) << 8)
                | (buffer[buffer_pos + 1] as u32);
            let adj = now_pos.wrapping_add(buffer_pos as u32).wrapping_add(5);
            let dest = compute_x86_branch_dest_decode(src, adj, *prev_mask);
            buffer[buffer_pos + 4] = (!(((dest >> 24) & 1).wrapping_sub(1))) as u8;
            buffer[buffer_pos + 3] = (dest >> 16) as u8;
            buffer[buffer_pos + 2] = (dest >> 8) as u8;
            buffer[buffer_pos + 1] = dest as u8;
            buffer_pos += 5;
            *prev_mask = 0;
        } else {
            buffer_pos += 1;
            *prev_mask |= 1;
            if is_bcj_x86_ms_byte(b) {
                *prev_mask |= 0x10;
            }
        }
    }
    buffer_pos
}

pub(crate) struct StreamingFilterState {
    method_id: u64,
    delta_dist: usize,
    delta_state: [u8; 256],
    delta_pos: usize,
    ip: u32,
    bcj_prev_mask: u32,
    bcj_prev_pos: u32,
    tail: [u8; 8],
    tail_len: usize,
    work_buf: Vec<u8>,
}

impl StreamingFilterState {
    pub(crate) fn new(method_id: u64, props: &[u8]) -> Self {
        Self {
            method_id,
            delta_dist: props.first().map(|&d| (d as usize) + 1).unwrap_or(1),
            delta_state: [0u8; 256],
            delta_pos: 0,
            ip: 0,
            bcj_prev_mask: 0,
            bcj_prev_pos: 0u32.wrapping_sub(5),
            tail: [0u8; 8],
            tail_len: 0,
            work_buf: Vec::new(),
        }
    }

    fn apply_filter_work_buf(&mut self) -> usize {
        use crate::sevenz::format::{METHOD_ARM64, METHOD_ARM64_ALT, METHOD_BCJ_X86, METHOD_DELTA};
        if self.method_id == METHOD_DELTA {
            for b in self.work_buf.iter_mut() {
                let out = b.wrapping_add(self.delta_state[self.delta_pos]);
                self.delta_state[self.delta_pos] = out;
                *b = out;
                self.delta_pos = (self.delta_pos + 1) % self.delta_dist;
            }
            self.work_buf.len()
        } else if self.method_id == METHOD_ARM64 || self.method_id == METHOD_ARM64_ALT {
            let aligned = self.work_buf.len() & !3;
            if aligned > 0 {
                crate::codecs::branch::bcj_arm64::arm64_decode(&mut self.work_buf[..aligned], self.ip);
            }
            aligned
        } else if self.method_id == METHOD_BCJ_X86 {
            x86_decode_stateful(
                &mut self.work_buf,
                self.ip,
                &mut self.bcj_prev_mask,
                &mut self.bcj_prev_pos,
            )
        } else {
            self.work_buf.len()
        }
    }

    pub(crate) fn push_chunk<F>(&mut self, chunk: &[u8], sink: &mut F) -> Result<(), TTZipStatus>
    where
        F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
    {
        if chunk.is_empty() {
            return Ok(());
        }
        const FILTER_WINDOW_BYTES: usize = 64 * 1024;
        for sub in chunk.chunks(FILTER_WINDOW_BYTES) {
            let needed = self.tail_len + sub.len();
            self.work_buf.clear();
            if self.work_buf.capacity() < needed {
                self.work_buf.reserve(needed - self.work_buf.capacity());
            }
            self.work_buf.extend_from_slice(&self.tail[..self.tail_len]);
            self.work_buf.extend_from_slice(sub);
            let consumed = self.apply_filter_work_buf();
            if consumed > 0 {
                self.ip = self.ip.wrapping_add(consumed as u32);
                sink(&self.work_buf[..consumed])?;
            }
            self.tail_len = self.work_buf.len() - consumed;
            self.tail[..self.tail_len].copy_from_slice(&self.work_buf[consumed..]);
        }
        Ok(())
    }

    pub(crate) fn finish<F>(&mut self, sink: &mut F) -> Result<(), TTZipStatus>
    where
        F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
    {
        if self.tail_len > 0 {
            let len = std::mem::take(&mut self.tail_len);
            sink(&self.tail[..len])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sevenz::format::METHOD_BCJ_X86;

    #[test]
    fn test_streaming_bcj_x86_preserves_prev_mask_across_64k_window_boundary() {
        let mut raw = vec![0x90u8; 130 * 1024];
        // At offset 65531 (last byte before the first 64 KiB window limit of 65532),
        // place an E8 whose MSB at 65535 is 0x01 (!0x00 && !0xFF), so prev_pos = 65531 and prev_mask = 1.
        // At offset 65532 (start of the second window's tail), place another E8 with MSB 0x00 at 65536
        // such that after subtracting (ip + 5) = 65537, byte 3 of v ((v >> 24) as u8, checked when
        // prev_mask == 1 because K_MASK_TO_BIT_NUMBER[1] == 1) equals 0xFF.
        raw[65531] = 0xE8;
        raw[65532] = 0xE8;
        raw[65533] = 0x01;
        raw[65534] = 0x00;
        raw[65535] = 0x01;
        raw[65536] = 0x00;

        let mut expected = raw.clone();
        let consumed = crate::codecs::branch::bcj_x86::x86_decode(&mut expected, 0);
        assert!(consumed > 65540);

        let mut filter = StreamingFilterState::new(METHOD_BCJ_X86, &[]);
        let mut streamed = Vec::with_capacity(raw.len());
        filter
            .push_chunk(&raw, &mut |chunk| {
                streamed.extend_from_slice(chunk);
                Ok(())
            })
            .expect("push_chunk should succeed");
        filter
            .finish(&mut |chunk| {
                streamed.extend_from_slice(chunk);
                Ok(())
            })
            .expect("finish should succeed");

        assert_eq!(streamed.len(), expected.len());
        assert_eq!(
            &streamed[65528..65544],
            &expected[65528..65544],
            "StreamingFilterState must preserve BCJ x86 prev_mask and prev_pos across 64 KiB windows"
        );
    }
}


