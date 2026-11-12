// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Safe, high-throughput XZ Single-Stream and Multi-Block decompression state machine,
//! Filter Chain inverse transformation engine, and streaming decoder.
//!
//! Complies strictly with the .xz File Format Specification (v1.1.0).

use std::collections::VecDeque;
use std::io::{self, Read};

use crate::xz::block::{pad_to_4, XzBlockError, XzBlockHeader};
use crate::xz::checksum::XzChecksumError;
use crate::xz::header::{XzStreamFlags, XzStreamFooter, XzStreamHeader};
use crate::xz::index::{XzRecord, XzStreamIndex};
use crate::xz::payload::decompress_block_payload;
use crate::xz::types::{XzCheckType, XzError, XZ_HEADER_MAGIC};
use crate::xz::vli::XzVliError;

/// Default physical memory limit for XZ stream decoding (256 MiB).
pub const DEFAULT_XZ_MEMLIMIT: u64 = 256 * 1024 * 1024;

/// Strongly-typed error variants occurring during XZ stream decoding.
#[derive(Debug, thiserror::Error)]
pub enum XzDecodeError {
    /// Bitstream or compressed payload data is corrupted.
    #[error("Corrupted XZ data: {0}")]
    CorruptedData(String),

    /// Block or stream checksum validation failed.
    #[error("XZ checksum mismatch for {check_type:?}: expected {expected}, computed {actual}")]
    ChecksumMismatch {
        check_type: XzCheckType,
        expected: String,
        actual: String,
    },

    /// Memory budget limit exceeded.
    #[error("XZ memory limit exceeded: required {needed} bytes, limit is {limit} bytes")]
    MemlimitExceeded { limit: u64, needed: u64 },

    /// An unsupported or unrecognized filter ID was encountered.
    #[error("Unsupported XZ filter ID: 0x{filter_id:02X}")]
    UnsupportedFilter { filter_id: u64 },

    /// Stream truncated before expected data boundary.
    #[error("Truncated XZ data: expected at least {expected} bytes, found {actual}")]
    TruncatedData { expected: usize, actual: usize },

    /// Stream Header / Footer structural or parity error.
    #[error("XZ header error: {0}")]
    Header(#[from] XzError),

    /// Block Header parsing or validation error.
    #[error("XZ block header error: {0}")]
    BlockHeader(#[from] XzBlockError),

    /// VLI integer decoding or encoding error.
    #[error("XZ VLI error: {0}")]
    Vli(#[from] XzVliError),

    /// Checksum engine error.
    #[error("XZ checksum error: {0}")]
    Checksum(#[from] XzChecksumError),

    /// Index structural or reconciliation mismatch.
    #[error("XZ index reconciliation failure: {0}")]
    InvalidIndex(String),

    /// Invalid seek parameter.
    #[error("Invalid seek parameter: {0}")]
    InvalidSeek(String),

    /// Underlying standard I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

impl From<XzDecodeError> for io::Error {
    fn from(err: XzDecodeError) -> Self {
        match err {
            XzDecodeError::Io(e) => e,
            err @ (XzDecodeError::CorruptedData(_)
            | XzDecodeError::ChecksumMismatch { .. }
            | XzDecodeError::MemlimitExceeded { .. }
            | XzDecodeError::UnsupportedFilter { .. }
            | XzDecodeError::TruncatedData { .. }
            | XzDecodeError::Header(_)
            | XzDecodeError::BlockHeader(_)
            | XzDecodeError::Vli(_)
            | XzDecodeError::Checksum(_)
            | XzDecodeError::InvalidIndex(_)
            | XzDecodeError::InvalidSeek(_)) => io::Error::new(io::ErrorKind::InvalidData, err),
        }
    }
}

/// Operational state sequence of the XZ stream decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XzDecoderState {
    /// Consuming 12-byte Stream Header.
    StreamHeader,
    /// Consuming Block Header or detecting Index indicator (`0x00`).
    BlockHeader,
    /// Consuming and decompressing Block Payload.
    BlockData,
    /// Consuming 0..=3 bytes of zeroed Block Padding.
    BlockPadding,
    /// Consuming Block Checksum and verifying digest.
    BlockCheck,
    /// Consuming and reconciling Stream Index.
    Index,
    /// Consuming 12-byte Stream Footer.
    StreamFooter,
    /// Consuming optional 4-byte multiples of Stream Padding.
    StreamPadding,
    /// Stream decoding completed (EOF reached).
    End,
}

/// Pure streaming, single-pass XZ decompressor implementing `std::io::Read`.
pub struct XzStreamDecoder<R: Read> {
    inner: R,
    state: XzDecoderState,
    stream_flags: Option<XzStreamFlags>,
    current_block_header: Option<XzBlockHeader>,
    current_block_compressed: Vec<u8>,
    cumulative_records: Vec<XzRecord>,
    output_fifo: VecDeque<u8>,
    total_uncompressed_bytes: u64,
    total_stream_bytes_read: u64,
    memlimit: u64,
    allow_multiple_streams: bool,
    pending_first_byte: Option<u8>,
}

impl<R: Read> XzStreamDecoder<R> {
    /// Creates a new `XzStreamDecoder` wrapping `inner` with default 256 MiB memory limit.
    pub fn new(inner: R) -> Self {
        Self::with_memlimit(inner, DEFAULT_XZ_MEMLIMIT)
    }

    /// Creates an `XzStreamDecoder` with custom memory limit.
    pub fn with_memlimit(inner: R, memlimit: u64) -> Self {
        Self {
            inner,
            state: XzDecoderState::StreamHeader,
            stream_flags: None,
            current_block_header: None,
            current_block_compressed: Vec::new(),
            cumulative_records: Vec::new(),
            output_fifo: VecDeque::with_capacity(64 * 1024),
            total_uncompressed_bytes: 0,
            total_stream_bytes_read: 0,
            memlimit,
            allow_multiple_streams: true,
            pending_first_byte: None,
        }
    }

    /// Configures whether chained multi-stream XZ archives should be transparently decoded.
    pub fn set_allow_multiple_streams(&mut self, allow: bool) {
        self.allow_multiple_streams = allow;
    }

    /// Returns current state machine phase.
    #[inline]
    pub fn current_state(&self) -> XzDecoderState {
        self.state
    }

    /// Returns total uncompressed bytes produced so far.
    #[inline]
    pub fn total_uncompressed_bytes(&self) -> u64 {
        self.total_uncompressed_bytes
    }

    /// Returns total physical stream bytes read.
    #[inline]
    pub fn total_stream_bytes_read(&self) -> u64 {
        self.total_stream_bytes_read
    }

    /// Returns the parsed Stream Flags if Stream Header has been processed.
    #[inline]
    pub fn stream_flags(&self) -> Option<XzStreamFlags> {
        self.stream_flags
    }

    /// Returns the accumulated list of decoded block records.
    #[inline]
    pub fn cumulative_records(&self) -> &[XzRecord] {
        &self.cumulative_records
    }

    /// Advances the state machine by executing one structural decoding step.
    pub fn step(&mut self) -> Result<(), XzDecodeError> {
        match self.state {
            XzDecoderState::StreamHeader => self.step_stream_header(),
            XzDecoderState::BlockHeader => self.step_block_header(),
            XzDecoderState::BlockData => self.step_block_data(),
            XzDecoderState::BlockPadding => self.step_block_padding(),
            XzDecoderState::BlockCheck => self.step_block_check(),
            XzDecoderState::Index => self.step_index(),
            XzDecoderState::StreamFooter => self.step_stream_footer(),
            XzDecoderState::StreamPadding => self.step_stream_padding(),
            XzDecoderState::End => Ok(()),
        }
    }

    fn step_stream_header(&mut self) -> Result<(), XzDecodeError> {
        let mut header_buf = [0u8; 12];
        let mut read_bytes = 0;

        if let Some(first) = self.pending_first_byte.take() {
            header_buf[0] = first;
            read_bytes = 1;
        }

        while read_bytes < 12 {
            let n = self.inner.read(&mut header_buf[read_bytes..])?;
            if n == 0 {
                if read_bytes == 0 && self.total_stream_bytes_read > 0 {
                    self.state = XzDecoderState::End;
                    return Ok(());
                }
                return Err(XzDecodeError::TruncatedData {
                    expected: 12,
                    actual: read_bytes,
                });
            }
            read_bytes += n;
        }
        self.total_stream_bytes_read += 12;

        let header = XzStreamHeader::parse(&header_buf)?;
        self.stream_flags = Some(header.flags);
        self.cumulative_records.clear();
        self.state = XzDecoderState::BlockHeader;
        Ok(())
    }

    fn step_block_header(&mut self) -> Result<(), XzDecodeError> {
        let mut byte0_buf = [0u8; 1];
        let n = self.inner.read(&mut byte0_buf)?;
        if n == 0 {
            return Err(XzDecodeError::TruncatedData {
                expected: 1,
                actual: 0,
            });
        }
        self.total_stream_bytes_read += 1;
        let b0 = byte0_buf[0];

        if b0 == 0x00 {
            self.state = XzDecoderState::Index;
            return Ok(());
        }

        let header_size = (b0 as usize + 1) * 4;
        let mut full_header_buf = vec![0u8; header_size];
        full_header_buf[0] = b0;
        self.inner.read_exact(&mut full_header_buf[1..])?;
        self.total_stream_bytes_read += (header_size - 1) as u64;

        let flags = self
            .stream_flags
            .ok_or_else(|| XzDecodeError::CorruptedData("Missing stream flags".to_string()))?;
        let block_header = XzBlockHeader::parse(&full_header_buf, flags.check_type)?;
        self.current_block_header = Some(block_header);
        self.current_block_compressed.clear();
        self.state = XzDecoderState::BlockData;
        Ok(())
    }

    fn step_block_data(&mut self) -> Result<(), XzDecodeError> {
        let block_hdr = self.current_block_header.clone().ok_or_else(|| {
            XzDecodeError::CorruptedData("Missing active block header".to_string())
        })?;

        if let Some(comp_size) = block_hdr.compressed_size {
            if comp_size > self.memlimit {
                return Err(XzDecodeError::MemlimitExceeded {
                    limit: self.memlimit,
                    needed: comp_size,
                });
            }
            let mut comp_buf = vec![0u8; comp_size as usize];
            self.inner.read_exact(&mut comp_buf)?;
            self.total_stream_bytes_read += comp_size;
            self.current_block_compressed = comp_buf;
        } else {
            self.current_block_compressed = self.read_unbounded_lzma2_chunks()?;
        }

        self.state = XzDecoderState::BlockPadding;
        Ok(())
    }

    fn read_unbounded_lzma2_chunks(&mut self) -> Result<Vec<u8>, XzDecodeError> {
        let mut compressed_data = Vec::new();
        let mut total_block_bytes = 0u64;
        loop {
            let mut ctrl_buf = [0u8; 1];
            self.inner.read_exact(&mut ctrl_buf)?;
            self.total_stream_bytes_read += 1;
            total_block_bytes += 1;
            let ctrl = ctrl_buf[0];
            compressed_data.push(ctrl);

            if total_block_bytes > self.memlimit {
                return Err(XzDecodeError::MemlimitExceeded {
                    limit: self.memlimit,
                    needed: total_block_bytes,
                });
            }

            if ctrl == 0x00 {
                break;
            }
            self.read_lzma2_chunk_body(ctrl, &mut compressed_data, &mut total_block_bytes)?;
        }
        Ok(compressed_data)
    }

    fn read_lzma2_chunk_body(
        &mut self,
        ctrl: u8,
        compressed_data: &mut Vec<u8>,
        total_block_bytes: &mut u64,
    ) -> Result<(), XzDecodeError> {
        if ctrl == 0x01 || ctrl == 0x02 {
            let mut size_buf = [0u8; 2];
            self.inner.read_exact(&mut size_buf)?;
            self.total_stream_bytes_read += 2;
            *total_block_bytes += 2;
            let unpack_size = (((size_buf[0] as usize) << 8) | (size_buf[1] as usize)) + 1;
            compressed_data.extend_from_slice(&size_buf);
            self.read_chunk_payload(unpack_size, compressed_data, total_block_bytes)
        } else if ctrl >= 0x80 {
            let mode = (ctrl >> 5) & 0x03;
            let extra_hdr_len = if mode >= 2 { 5 } else { 4 };
            let mut hdr_tail = [0u8; 5];
            self.inner.read_exact(&mut hdr_tail[..extra_hdr_len])?;
            self.total_stream_bytes_read += extra_hdr_len as u64;
            *total_block_bytes += extra_hdr_len as u64;
            let pack_size = (((hdr_tail[2] as usize) << 8) | (hdr_tail[3] as usize)) + 1;
            compressed_data.extend_from_slice(&hdr_tail[..extra_hdr_len]);
            self.read_chunk_payload(pack_size, compressed_data, total_block_bytes)
        } else {
            Err(XzDecodeError::CorruptedData(format!(
                "Invalid LZMA2 control byte 0x{ctrl:02X}"
            )))
        }
    }

    fn read_chunk_payload(
        &mut self,
        payload_len: usize,
        compressed_data: &mut Vec<u8>,
        total_block_bytes: &mut u64,
    ) -> Result<(), XzDecodeError> {
        let start = compressed_data.len();
        compressed_data.resize(start + payload_len, 0);
        if let Err(e) = self.inner.read_exact(&mut compressed_data[start..]) {
            compressed_data.truncate(start);
            return Err(e.into());
        }
        self.total_stream_bytes_read += payload_len as u64;
        *total_block_bytes += payload_len as u64;
        Ok(())
    }

    fn step_block_padding(&mut self) -> Result<(), XzDecodeError> {
        let pad_len = pad_to_4(self.current_block_compressed.len() as u64);
        if pad_len > 0 {
            let mut pad_buf = vec![0u8; pad_len];
            self.inner.read_exact(&mut pad_buf)?;
            self.total_stream_bytes_read += pad_len as u64;
            if pad_buf.iter().any(|&b| b != 0x00) {
                return Err(XzDecodeError::CorruptedData(
                    "Non-zero block padding byte detected".to_string(),
                ));
            }
        }
        self.state = XzDecoderState::BlockCheck;
        Ok(())
    }

    fn step_block_check(&mut self) -> Result<(), XzDecodeError> {
        let block_hdr = self.current_block_header.take().ok_or_else(|| {
            XzDecodeError::CorruptedData("Missing active block header".to_string())
        })?;

        let check_type = block_hdr.check_type;
        let check_size = check_type.check_size();
        let mut check_buf = vec![0u8; check_size];
        if check_size > 0 {
            self.inner.read_exact(&mut check_buf)?;
            self.total_stream_bytes_read += check_size as u64;
        }

        let uncompressed = decompress_block_payload(
            &self.current_block_compressed,
            &block_hdr,
            check_type,
            &check_buf,
            self.memlimit,
        )?;

        let unpadded_size =
            block_hdr.header_size as u64 + self.current_block_compressed.len() as u64 + check_size as u64;
        let uncompressed_size = uncompressed.len() as u64;

        self.cumulative_records
            .push(XzRecord::new(unpadded_size, uncompressed_size));
        self.output_fifo.extend(uncompressed);
        self.state = XzDecoderState::BlockHeader;
        Ok(())
    }

    fn step_index(&mut self) -> Result<(), XzDecodeError> {
        let mut chain = io::Cursor::new([0x00u8]).chain(&mut self.inner);
        let (index, consumed) = XzStreamIndex::parse_stream(&mut chain)?;
        self.total_stream_bytes_read += (consumed.saturating_sub(1)) as u64;

        if index.records.len() != self.cumulative_records.len() {
            return Err(XzDecodeError::InvalidIndex(format!(
                "Index record count mismatch: expected {}, decoded {}",
                index.records.len(),
                self.cumulative_records.len()
            )));
        }

        for (idx, (expected, actual)) in
            index.records.iter().zip(self.cumulative_records.iter()).enumerate()
        {
            if expected != actual {
                return Err(XzDecodeError::InvalidIndex(format!(
                    "Index record #{idx} mismatch: expected {expected:?}, decoded {actual:?}"
                )));
            }
        }

        self.state = XzDecoderState::StreamFooter;
        Ok(())
    }

    fn step_stream_footer(&mut self) -> Result<(), XzDecodeError> {
        let mut footer_buf = [0u8; 12];
        self.inner.read_exact(&mut footer_buf)?;
        self.total_stream_bytes_read += 12;

        let flags = self
            .stream_flags
            .ok_or_else(|| XzDecodeError::CorruptedData("Missing stream flags".to_string()))?;
        let footer = XzStreamFooter::parse_and_verify_header(&footer_buf, &flags)?;

        let mut index = XzStreamIndex::with_capacity(self.cumulative_records.len());
        for r in &self.cumulative_records {
            index.append(r.unpadded_size, r.uncompressed_size)?;
        }

        let expected_backward_size = index.index_size()? as u64;
        if footer.backward_size != expected_backward_size {
            return Err(XzDecodeError::InvalidIndex(format!(
                "Footer backward size {} does not match computed index size {}",
                footer.backward_size, expected_backward_size
            )));
        }

        self.state = XzDecoderState::StreamPadding;
        Ok(())
    }

    fn step_stream_padding(&mut self) -> Result<(), XzDecodeError> {
        let mut pad_word = [0u8; 4];
        loop {
            let n = self.inner.read(&mut pad_word[..1])?;
            if n == 0 {
                self.state = XzDecoderState::End;
                return Ok(());
            }
            if pad_word[0] == 0x00 {
                self.inner.read_exact(&mut pad_word[1..4])?;
                self.total_stream_bytes_read += 4;
                if pad_word != [0, 0, 0, 0] {
                    return Err(XzDecodeError::CorruptedData(
                        "Non-zero byte detected in Stream Padding".to_string(),
                    ));
                }
            } else if self.allow_multiple_streams && pad_word[0] == XZ_HEADER_MAGIC[0] {
                self.pending_first_byte = Some(pad_word[0]);
                self.state = XzDecoderState::StreamHeader;
                return Ok(());
            } else {
                return Err(XzDecodeError::CorruptedData(
                    "Unexpected non-padding trailing byte".to_string(),
                ));
            }
        }
    }
}

impl<R: Read> Read for XzStreamDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        while self.output_fifo.is_empty() && self.state != XzDecoderState::End {
            self.step().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        }

        if self.output_fifo.is_empty() {
            return Ok(0);
        }

        let to_copy = buf.len().min(self.output_fifo.len());
        let (front, back) = self.output_fifo.as_slices();
        let front_copy = to_copy.min(front.len());
        buf[..front_copy].copy_from_slice(&front[..front_copy]);
        if front_copy < to_copy {
            let back_copy = to_copy - front_copy;
            buf[front_copy..to_copy].copy_from_slice(&back[..back_copy]);
        }
        self.output_fifo.drain(..to_copy);
        self.total_uncompressed_bytes += to_copy as u64;
        Ok(to_copy)
    }
}

/// Convenience standalone function to decompress an entire XZ container from a byte slice.
pub fn xz_decompress(input: &[u8]) -> Result<Vec<u8>, XzDecodeError> {
    let mut decoder = XzStreamDecoder::new(io::Cursor::new(input));
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    Ok(out)
}

/// Convenience type alias for `XzStreamDecoder`.
pub type XzStreamReader<R> = XzStreamDecoder<R>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xz_decoder_slice_drain_and_wraparound() {
        let mut dec = XzStreamDecoder::new(io::Cursor::new(Vec::<u8>::new()));
        dec.state = XzDecoderState::End;
        dec.output_fifo = VecDeque::with_capacity(8);
        for b in 0..6u8 {
            dec.output_fifo.push_back(b);
        }
        let mut head = [0u8; 4];
        assert_eq!(dec.read(&mut head).expect("read head"), 4);
        assert_eq!(head, [0, 1, 2, 3]);
        for b in 6..12u8 {
            dec.output_fifo.push_back(b);
        }
        let (front, back) = dec.output_fifo.as_slices();
        assert!(!front.is_empty() && !back.is_empty());

        let mut out = [0u8; 8];
        let n = dec.read(&mut out).expect("read wrapped");
        assert_eq!(n, 8);
        assert_eq!(out, [4, 5, 6, 7, 8, 9, 10, 11]);
        assert!(dec.output_fifo.is_empty());
    }
}
