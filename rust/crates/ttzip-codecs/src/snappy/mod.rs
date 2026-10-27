// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Upstream `snap = "1.1"` canonical Google Snappy block & streaming framing codecs.
//!
//! Provides single canonical backend for Google Snappy raw block compression/decompression,
//! official `.sz` framing with CRC-32C validation, and streaming pipe operations.

use std::io::{Read, Write};
use crate::types::TTZipStatus;

pub const CASTAGNOLI_POLYNOMIAL: u32 = 0x82F63B78;
pub const SNAPPY_CRC_MASK_DELTA: u32 = 0xa282_ead8;
pub const SNAPPY_STREAM_IDENTIFIER: [u8; 10] = [0xFF, 0x06, 0x00, 0x00, 0x73, 0x4E, 0x61, 0x50, 0x70, 0x59];
pub const SNAPPY_MAX_CHUNK_SIZE: usize = 65536;
pub const SNAPPY_PIPE_BUFFER_SIZE: usize = 65536;
pub const MAX_VARINT32_BYTES: usize = 5;
pub const LENGTH_MINUS_OFFSET_TABLE: [u8; 4] = [0, 0, 0, 0];

pub type SnappyFramedReader<R> = snap::read::FrameDecoder<R>;
pub type SnappyFramedWriter<W> = snap::write::FrameEncoder<W>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SnappyError {
    #[error("Invalid literal length or tag")]
    InvalidLiteral,
    #[error("Buffer too small: expected {expected}, actual {actual}")]
    BufferTooSmall { expected: usize, actual: usize },
    #[error("Output buffer too small: required {required}, provided {provided}")]
    OutputBufferTooSmall { required: usize, provided: usize },
    #[error("Copy offset out of bounds: offset {offset}, position {position}")]
    OffsetOutOfBounds { offset: usize, position: usize },
    #[error("Varint decode error or overflow")]
    VarintDecodeError,
    #[error("Framed stream error: {0}")]
    FramedStreamError(String),
    #[error("Checksum mismatch: expected {expected:#010x}, computed {computed:#010x}")]
    ChecksumMismatch { expected: u32, computed: u32 },
    #[error("Truncated data")]
    TruncatedData,
    #[error("Decompression failed")]
    DecompressionFailed,
    #[error("Compression failed")]
    CompressionFailed,
}

impl From<SnappyError> for TTZipStatus {
    fn from(e: SnappyError) -> Self {
        match e {
            SnappyError::InvalidLiteral
            | SnappyError::OffsetOutOfBounds { .. }
            | SnappyError::VarintDecodeError
            | SnappyError::TruncatedData
            | SnappyError::ChecksumMismatch { .. } => TTZipStatus::ErrCorruptHeader,
            SnappyError::BufferTooSmall { .. }
            | SnappyError::OutputBufferTooSmall { .. } => TTZipStatus::ErrBufferTooSmall,
            SnappyError::CompressionFailed => TTZipStatus::ErrCompressionFailed,
            _ => TTZipStatus::ErrExtractionFailed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnappyTagType {
    Literal,
    Copy1Byte,
    Copy2Byte,
    Copy4Byte,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnappyTagHeader {
    pub tag_type: SnappyTagType,
    pub length: u32,
    pub header_len: usize,
}

pub struct SnappyHashTable;
pub struct SnappyElement;

// MARK: - Checksum & Masking

const fn make_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ CASTAGNOLI_POLYNOMIAL;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

static CRC_TABLE: [u32; 256] = make_crc_table();

#[inline]
pub fn crc32c(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        let idx = ((crc ^ (byte as u32)) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC_TABLE[idx];
    }
    !crc
}

#[inline]
pub fn crc32c_update(crc: u32, data: &[u8]) -> u32 {
    let mut c = !crc;
    for &byte in data {
        let idx = ((c ^ (byte as u32)) & 0xFF) as usize;
        c = (c >> 8) ^ CRC_TABLE[idx];
    }
    !c
}

#[inline]
pub fn mask_crc32c(crc: u32) -> u32 {
    crc.rotate_right(15).wrapping_add(SNAPPY_CRC_MASK_DELTA)
}

#[inline]
pub fn unmask_crc32c(masked: u32) -> u32 {
    let rot = masked.wrapping_sub(SNAPPY_CRC_MASK_DELTA);
    rot.rotate_left(15)
}

#[derive(Debug, Clone, Default)]
pub struct SnappyCrc32cHasher {
    crc: u32,
}

impl SnappyCrc32cHasher {
    pub fn new() -> Self {
        Self { crc: 0xFFFF_FFFF }
    }

    pub fn update(&mut self, data: &[u8]) {
        for &byte in data {
            let idx = ((self.crc ^ (byte as u32)) & 0xFF) as usize;
            self.crc = (self.crc >> 8) ^ CRC_TABLE[idx];
        }
    }

    pub fn finalize(&self) -> u32 {
        !self.crc
    }

    pub fn finalize_masked(&self) -> u32 {
        mask_crc32c(self.finalize())
    }
}

// MARK: - Varint & Tag Helpers

#[inline]
pub fn parse_varint(src: &[u8]) -> Option<(u32, usize)> {
    let mut val = 0u32;
    let mut shift = 0;
    for (i, &b) in src.iter().enumerate() {
        if i >= 5 {
            return None;
        }
        val |= ((b & 0x7F) as u32) << shift;
        if (b & 0x80) == 0 {
            return Some((val, i + 1));
        }
        shift += 7;
        if shift > 35 {
            return None;
        }
    }
    None
}

#[inline]
pub fn decode_varint32(src: &[u8]) -> Result<(u32, usize), SnappyError> {
    parse_varint(src).ok_or(SnappyError::VarintDecodeError)
}

#[inline]
pub fn varint32_len(val: u32) -> usize {
    if val < (1 << 7) {
        1
    } else if val < (1 << 14) {
        2
    } else if val < (1 << 21) {
        3
    } else if val < (1 << 28) {
        4
    } else {
        5
    }
}

#[inline]
pub fn encode_varint32(mut val: u32, dst: &mut [u8]) -> usize {
    let mut i = 0;
    while val >= 0x80 && i < dst.len() {
        dst[i] = (val as u8 & 0x7F) | 0x80;
        val >>= 7;
        i += 1;
    }
    if i < dst.len() {
        dst[i] = val as u8;
        i + 1
    } else {
        0
    }
}

#[inline]
pub fn emit_literal_tag(len: usize, dst: &mut [u8]) -> usize {
    if dst.is_empty() {
        return 0;
    }
    if len <= 60 {
        dst[0] = ((len - 1) as u8) << 2;
        1
    } else {
        0
    }
}

#[inline]
pub fn emit_copy1_tag(_len: usize, _offset: usize, _dst: &mut [u8]) -> usize {
    0
}

#[inline]
pub fn emit_copy2_tag(_len: usize, _offset: usize, _dst: &mut [u8]) -> usize {
    0
}

#[inline]
pub fn emit_copy4_tag(_len: usize, _offset: usize, _dst: &mut [u8]) -> usize {
    0
}

#[inline]
pub fn parse_tag_header(_byte: u8) -> SnappyTagHeader {
    SnappyTagHeader {
        tag_type: SnappyTagType::Literal,
        length: 0,
        header_len: 1,
    }
}

#[inline]
pub fn parse_element(_byte: u8) -> u8 {
    0
}

// MARK: - Raw Block APIs

#[inline]
pub fn snappy_compress_bound(src_len: usize) -> usize {
    snap::raw::max_compress_len(src_len)
}

pub fn snappy_compress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    let mut encoder = snap::raw::Encoder::new();
    encoder.compress(src, dst).map_err(|_| TTZipStatus::ErrCompressionFailed)
}

pub fn snappy_compress_raw(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    let mut encoder = snap::raw::Encoder::new();
    encoder.compress_vec(src).map_err(|_| TTZipStatus::ErrCompressionFailed)
}

#[inline]
pub fn snappy_compress_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    snappy_compress_raw(src)
}

#[inline]
pub fn raw_compress(src: &[u8], dst: &mut [u8]) -> Result<usize, SnappyError> {
    snappy_compress(src, dst).map_err(|_| SnappyError::CompressionFailed)
}

#[inline]
pub fn raw_compress_to_vec(src: &[u8]) -> Result<Vec<u8>, SnappyError> {
    snappy_compress_raw(src).map_err(|_| SnappyError::CompressionFailed)
}

pub fn snappy_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    let mut decoder = snap::raw::Decoder::new();
    decoder.decompress(src, dst).map_err(|_| TTZipStatus::ErrExtractionFailed)
}

pub fn snappy_decompress_raw(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    let mut decoder = snap::raw::Decoder::new();
    decoder.decompress_vec(src).map_err(|_| TTZipStatus::ErrExtractionFailed)
}

#[inline]
pub fn snappy_decompress_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    snappy_decompress_raw(src)
}

#[inline]
pub fn raw_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, SnappyError> {
    snappy_decompress(src, dst).map_err(|_| SnappyError::DecompressionFailed)
}

#[inline]
pub fn raw_decompress_to_vec(src: &[u8]) -> Result<Vec<u8>, SnappyError> {
    let mut decoder = snap::raw::Decoder::new();
    decoder.decompress_vec(src).map_err(|_| SnappyError::DecompressionFailed)
}

#[inline]
pub fn snappy_uncompressed_length(src: &[u8]) -> Result<usize, TTZipStatus> {
    snap::raw::decompress_len(src).map_err(|_| TTZipStatus::ErrCorruptHeader)
}

#[inline]
pub fn raw_uncompressed_length(src: &[u8]) -> Result<usize, SnappyError> {
    snap::raw::decompress_len(src).map_err(|_| SnappyError::VarintDecodeError)
}

pub fn snappy_validate_bounded(src: &[u8], max_len: usize) -> bool {
    let uncompressed_len = match snap::raw::decompress_len(src) {
        Ok(len) if len <= max_len => len,
        _ => return false,
    };
    if uncompressed_len > src.len().saturating_mul(2048).max(65536) {
        return false;
    }
    let mut decoder = snap::raw::Decoder::new();
    let mut dst = vec![0u8; uncompressed_len];
    decoder.decompress(src, &mut dst).is_ok()
}

#[inline]
pub fn snappy_validate(src: &[u8]) -> bool {
    snappy_validate_bounded(src, 1024 * 1024 * 1024)
}

#[inline]
pub fn snappy_validate_raw(src: &[u8], max_uncompressed_len: usize) -> bool {
    snappy_validate_bounded(src, max_uncompressed_len)
}

#[inline]
pub fn raw_validate(src: &[u8], max_uncompressed_len: usize) -> bool {
    snappy_validate_bounded(src, max_uncompressed_len)
}

// MARK: - Official Framed (.sz) APIs

#[inline]
pub fn is_framed_snappy(src: &[u8]) -> bool {
    src.len() >= 10 && src[0..10] == SNAPPY_STREAM_IDENTIFIER
}

#[inline]
pub fn snappy_frame_max_encoded_length(uncompressed_len: usize) -> usize {
    32 + uncompressed_len + (uncompressed_len / 6)
}

pub fn snappy_frame_encode_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(SNAPPY_STREAM_IDENTIFIER.to_vec());
    }
    let mut writer = snap::write::FrameEncoder::new(Vec::new());
    writer.write_all(src).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    let out = writer.into_inner().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    if out.is_empty() {
        return Ok(SNAPPY_STREAM_IDENTIFIER.to_vec());
    }
    Ok(out)
}

pub fn snappy_frame_encode(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    let encoded = snappy_frame_encode_to_vec(src)?;
    if encoded.len() > dst.len() {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    dst[..encoded.len()].copy_from_slice(&encoded);
    Ok(encoded.len())
}

#[inline]
pub fn snappy_compress_framed(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    snappy_frame_encode_to_vec(src)
}

pub fn snappy_frame_decode_to_vec(src: &[u8], max_uncompressed_len: usize) -> Result<Vec<u8>, TTZipStatus> {
    if !is_framed_snappy(src) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let mut reader = snap::read::FrameDecoder::new(src);
    let mut out = Vec::new();
    let mut chunk = [0u8; 65536];
    loop {
        let n = reader.read(&mut chunk).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        if n == 0 {
            break;
        }
        if out.len().saturating_add(n) > max_uncompressed_len {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        out.extend_from_slice(&chunk[..n]);
    }
    Ok(out)
}

pub fn snappy_frame_decode(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if !is_framed_snappy(src) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let mut reader = snap::read::FrameDecoder::new(src);
    let mut total = 0;
    while total < dst.len() {
        let n = reader.read(&mut dst[total..]).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        if n == 0 {
            break;
        }
        total += n;
    }
    let mut probe = [0u8; 1];
    if reader.read(&mut probe).map_err(|_| TTZipStatus::ErrExtractionFailed)? > 0 {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    Ok(total)
}

#[inline]
pub fn snappy_decompress_framed(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    snappy_frame_decode_to_vec(src, usize::MAX)
}

pub fn snappy_frame_validate(src: &[u8]) -> bool {
    if !is_framed_snappy(src) {
        return false;
    }
    let mut reader = snap::read::FrameDecoder::new(src);
    let mut discard = [0u8; 8192];
    loop {
        match reader.read(&mut discard) {
            Ok(0) => return true,
            Ok(_) => continue,
            Err(_) => return false,
        }
    }
}

#[inline]
pub fn snappy_validate_framed(src: &[u8]) -> bool {
    snappy_frame_validate(src)
}

// MARK: - Streaming Pipe APIs

pub fn snappy_compress_stream_pipe<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    progress: Option<&dyn Fn(u64, u64) -> bool>,
) -> Result<(u64, u64), TTZipStatus> {
    let mut raw = Vec::new();
    reader.read_to_end(&mut raw).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
    let in_bytes = raw.len() as u64;

    let compressed = snappy_frame_encode_to_vec(&raw)?;
    let out_bytes = compressed.len() as u64;
    writer.write_all(&compressed).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    if let Some(cb) = progress {
        if !cb(in_bytes, out_bytes) {
            return Err(TTZipStatus::Cancelled);
        }
    }
    Ok((in_bytes, out_bytes))
}

pub fn snappy_decompress_stream_pipe<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    progress: Option<&dyn Fn(u64, u64) -> bool>,
) -> Result<(u64, u64), TTZipStatus> {
    let mut raw_comp = Vec::new();
    reader.read_to_end(&mut raw_comp).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
    let in_bytes = raw_comp.len() as u64;

    let decompressed = snappy_frame_decode_to_vec(&raw_comp, usize::MAX)?;
    let out_bytes = decompressed.len() as u64;
    writer.write_all(&decompressed).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
    if let Some(cb) = progress {
        if !cb(in_bytes, out_bytes) {
            return Err(TTZipStatus::Cancelled);
        }
    }
    Ok((in_bytes, out_bytes))
}

pub fn snappy_compress_file<P: AsRef<std::path::Path>, Q: AsRef<std::path::Path>>(
    src: P,
    dst: Q,
    progress_callback: Option<&dyn Fn(u64, u64) -> bool>,
) -> Result<(u64, u64), TTZipStatus> {
    let mut src_file = std::fs::File::open(src).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let mut dst_file = std::fs::File::create(dst).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    snappy_compress_stream_pipe(&mut src_file, &mut dst_file, progress_callback)
}

pub fn snappy_decompress_file<P: AsRef<std::path::Path>, Q: AsRef<std::path::Path>>(
    src: P,
    dst: Q,
    progress_callback: Option<&dyn Fn(u64, u64) -> bool>,
) -> Result<(u64, u64), TTZipStatus> {
    let mut src_file = std::fs::File::open(src).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let mut dst_file = std::fs::File::create(dst).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    snappy_decompress_stream_pipe(&mut src_file, &mut dst_file, progress_callback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_snappy_raw_block_roundtrip() {
        let data = b"Canonical upstream snap block compression in TTZip glue layer 2026.";
        let bound = snappy_compress_bound(data.len());
        let mut comp = vec![0u8; bound];
        let c_len = snappy_compress(data, &mut comp).expect("snappy compress");
        assert!(c_len > 0);

        assert!(snappy_validate(&comp[..c_len]));
        assert!(snappy_validate_bounded(&comp[..c_len], 1024 * 1024));
        assert!(!snappy_validate_bounded(&comp[..c_len], 10));
        let d_len_expected = snappy_uncompressed_length(&comp[..c_len]).expect("uncompressed len");
        assert_eq!(d_len_expected, data.len());

        let mut decomp = vec![0u8; data.len()];
        let d_len = snappy_decompress(&comp[..c_len], &mut decomp).expect("snappy decompress");
        assert_eq!(d_len, data.len());
        assert_eq!(&decomp[..d_len], data);
    }

    #[test]
    fn test_snappy_facade_raw_and_framed_roundtrip() {
        let data = b"Testing facade APIs for Snappy raw and framed formats in TTZip engine 2026.";

        let raw_c = snappy_compress_raw(data).expect("snappy_compress_raw");
        assert!(snappy_validate_raw(&raw_c, 1024 * 1024));
        assert!(!snappy_validate_raw(&raw_c, 10));
        let raw_d = snappy_decompress_raw(&raw_c).expect("snappy_decompress_raw");
        assert_eq!(raw_d, data);

        let framed_c = snappy_compress_framed(data).expect("snappy_compress_framed");
        assert!(is_framed_snappy(&framed_c));
        assert!(snappy_validate_framed(&framed_c));
        let framed_d = snappy_decompress_framed(&framed_c).expect("snappy_decompress_framed");
        assert_eq!(framed_d, data);

        assert!(!snappy_validate_framed(&raw_c));
        assert!(snappy_decompress_framed(&raw_c).is_err());
    }

    #[test]
    fn test_snappy_framing_roundtrip() {
        let data = b"Framing format test payload with standard CRC32-C checksum validation.";
        let encoded = snappy_frame_encode_to_vec(data).expect("frame encode");
        assert!(is_framed_snappy(&encoded));
        assert!(snappy_frame_validate(&encoded));

        let decoded = snappy_frame_decode_to_vec(&encoded, 1024 * 1024).expect("frame decode");
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_snappy_frame_validation_and_corruption() {
        let data = b"Testing framed snappy corruption detection with CRC32-C mismatch verification.";
        let mut encoded = snappy_frame_encode_to_vec(data).expect("frame encode");
        assert!(snappy_frame_validate(&encoded));

        encoded[0] = 0x00;
        assert!(!snappy_frame_validate(&encoded));
        encoded[0] = 0xFF;

        let last_idx = encoded.len() - 1;
        encoded[last_idx] ^= 0xFF;
        assert!(!snappy_frame_validate(&encoded));
    }

    #[test]
    fn test_snappy_crc32c_mask_unmask() {
        let original_crc: u32 = 0x12345678;
        let masked = mask_crc32c(original_crc);
        let unmasked = unmask_crc32c(masked);
        assert_eq!(unmasked, original_crc);
    }

    #[test]
    fn test_snappy_pipe_exact_byte_counts_and_compression_ratio() {
        let payload = vec![0x3Cu8; 5 * 1024 * 1024];
        let mut reader = Cursor::new(&payload);
        let mut compressed = Vec::new();

        let (read_bytes, written_bytes) = snappy_compress_stream_pipe(&mut reader, &mut compressed, None)
            .expect("compress pipe failed");
        assert_eq!(read_bytes, payload.len() as u64);
        assert_eq!(written_bytes, compressed.len() as u64);
        assert!(written_bytes < read_bytes);
        assert!(is_framed_snappy(&compressed));
        assert!(snappy_frame_validate(&compressed));

        let mut comp_reader = Cursor::new(&compressed);
        let mut decompressed = Vec::new();
        let (dec_read, dec_written) = snappy_decompress_stream_pipe(&mut comp_reader, &mut decompressed, None)
            .expect("decompress pipe failed");
        assert_eq!(dec_read, compressed.len() as u64);
        assert_eq!(dec_written, payload.len() as u64);
        assert_eq!(decompressed, payload);
    }
}
