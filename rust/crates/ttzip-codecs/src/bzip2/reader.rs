// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Safe Rust streaming Bzip2 reader implementing `std::io::Read`.

use std::io::{self, Read};
use super::block::decode_bzip2_block;
use super::crc::Bzip2CombinedCrc;
use super::huffman::BitReader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecoderState {
    Ready,
    Eof,
}

/// Streaming Bzip2 decompressor wrapping any `Read` source with a single-block ring buffer.
pub struct Bzip2Reader<R: Read> {
    header: [u8; 4],
    bit_reader: BitReader<R>,
    block_ring_buf: Vec<u8>,
    ring_cursor: usize,
    combined_crc: Bzip2CombinedCrc,
    state: DecoderState,
}

impl<R: Read> Bzip2Reader<R> {
    /// Creates a new streaming Bzip2 reader, verifying the 4-byte header without decoding blocks.
    pub fn new(mut inner: R) -> io::Result<Self> {
        let mut header = [0u8; 4];
        inner.read_exact(&mut header)?;

        if header[0] != b'B'
            || header[1] != b'Z'
            || header[2] != b'h'
            || !(b'1'..=b'9').contains(&header[3])
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid Bzip2 stream header",
            ));
        }

        let block_size_bytes = (header[3] - b'0') as usize * 100_000;

        Ok(Self {
            header,
            bit_reader: BitReader::new(inner),
            block_ring_buf: Vec::with_capacity(block_size_bytes),
            ring_cursor: 0,
            combined_crc: Bzip2CombinedCrc::new(),
            state: DecoderState::Ready,
        })
    }

    /// Returns the verified 4-byte stream header `BZh[1-9]`.
    #[inline]
    pub fn header(&self) -> &[u8; 4] {
        &self.header
    }
}

impl<R: Read> Read for Bzip2Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        // Refill single block buffer on demand
        while self.ring_cursor >= self.block_ring_buf.len() {
            if self.state == DecoderState::Eof {
                return Ok(0);
            }

            self.block_ring_buf.clear();
            self.ring_cursor = 0;

            match decode_bzip2_block(
                &mut self.bit_reader,
                &mut self.block_ring_buf,
                &mut self.combined_crc,
            ) {
                Ok(true) => {
                    break;
                }
                Ok(false) => {
                    self.state = DecoderState::Eof;
                    return Ok(0);
                }
                Err(e) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Bzip2 decode error: {:?}", e),
                    ));
                }
            }
        }

        let available = self.block_ring_buf.len() - self.ring_cursor;
        let to_copy = available.min(buf.len());
        buf[..to_copy].copy_from_slice(
            &self.block_ring_buf[self.ring_cursor..self.ring_cursor + to_copy],
        );
        self.ring_cursor += to_copy;
        Ok(to_copy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bzip2::bzip2_compress_vec;

    #[test]
    fn test_bzip2_reader_streaming() {
        let sample = b"Bzip2 streaming state machine verification test 2026. Repeat data to test chunks. ".repeat(200);
        let compressed = bzip2_compress_vec(&sample, 1).expect("compression failed");

        let mut reader = Bzip2Reader::new(&compressed[..]).expect("init reader");
        assert_eq!(reader.header(), b"BZh1");

        let mut decompressed = Vec::new();
        let mut buf = [0u8; 128];
        loop {
            let n = reader.read(&mut buf).expect("read chunk");
            if n == 0 {
                break;
            }
            decompressed.extend_from_slice(&buf[..n]);
        }

        assert_eq!(decompressed, sample);
    }
}
