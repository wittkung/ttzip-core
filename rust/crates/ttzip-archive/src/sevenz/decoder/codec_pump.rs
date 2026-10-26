// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 7-Zip Codec Bitstream Pump Helpers.
//!
//! Thin streaming wrappers around the LZMA2 and LZMA1 codecs that push decoded bytes into a
//! caller-supplied sink while enforcing the folder's expected unpack size.

use super::payload::map_codec_bitstream_err;
use crate::types::TTZipStatus;

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

/// Pumps an LZMA2 `Fl2DStream`, continuing to drain internal dictionary/MT buffers even after
/// `plain_slice` is fully consumed. Returns `Ok(true)` when `CHUNK_FINAL` (`remaining == 0`) is
/// reached, or `Ok(false)` when more compressed input is required from the caller.
pub(crate) fn pump_lzma2_stream<F>(
    dstream: &mut crate::codecs::lzma2::Fl2DStream,
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
    let mut in_buf = crate::codecs::lzma2::Fl2InBuffer {
        src: plain_slice.as_ptr() as *const libc::c_void,
        size: plain_slice.len(),
        pos: 0,
    };

    loop {
        let prev_in_pos = in_buf.pos;
        let mut out_buf = crate::codecs::lzma2::Fl2OutBuffer {
            dst: out_chunk.as_mut_ptr() as *mut libc::c_void,
            size: out_chunk.len(),
            pos: 0,
        };

        let remaining = dstream
            .decompress_stream(&mut in_buf, &mut out_buf)
            .map_err(map_codec_bitstream_err)?;
        let produced = out_buf.pos;
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

        let consumed = in_buf.pos.saturating_sub(prev_in_pos);
        if produced == 0 {
            if in_buf.pos >= in_buf.size && !is_final_input {
                return Ok(false);
            }
            if consumed == 0 || is_final_input {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
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
