// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe Rust streaming Bzip2 reader implementing `std::io::Read`.

use std::io::{self, Read};
use super::block::decode_bzip2_block;
use super::crc::Bzip2CombinedCrc;
use super::huffman::BitReader;

/// Streaming Bzip2 decompressor wrapping any `Read` source.
pub struct Bzip2Reader<R: Read> {
    _inner: std::marker::PhantomData<R>,
    decompressed_buf: Vec<u8>,
    read_pos: usize,
}

impl<R: Read> Bzip2Reader<R> {
    /// Creates a new streaming Bzip2 reader.
    pub fn new(mut inner: R) -> io::Result<Self> {
        let mut raw_compressed = Vec::new();
        inner.read_to_end(&mut raw_compressed)?;

        let mut decompressed_buf = Vec::new();
        if !raw_compressed.is_empty() {
            if raw_compressed.len() < 4
                || raw_compressed[0] != b'B'
                || raw_compressed[1] != b'Z'
                || raw_compressed[2] != b'h'
                || !(b'1'..=b'9').contains(&raw_compressed[3])
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid Bzip2 stream header",
                ));
            }

            let mut reader = BitReader::new(&raw_compressed[4..]);
            let mut combined_crc = Bzip2CombinedCrc::new();

            loop {
                match decode_bzip2_block(&mut reader, &mut decompressed_buf, &mut combined_crc) {
                    Ok(true) => continue, // More blocks
                    Ok(false) => break,  // EOS reached
                    Err(e) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Bzip2 decode error: {:?}", e),
                        ))
                    }
                }
            }
        }

        Ok(Self {
            _inner: std::marker::PhantomData,
            decompressed_buf,
            read_pos: 0,
        })
    }
}

impl<R: Read> Read for Bzip2Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.read_pos >= self.decompressed_buf.len() {
            return Ok(0);
        }

        let available = self.decompressed_buf.len() - self.read_pos;
        let to_copy = available.min(buf.len());
        buf[..to_copy].copy_from_slice(&self.decompressed_buf[self.read_pos..self.read_pos + to_copy]);
        self.read_pos += to_copy;
        Ok(to_copy)
    }
}
