// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Streaming UUDecode filter for stripping ASCII headers and 6-bit binary decoding.

use std::io::{self, Read};

use crate::archive::unified::filter_pipeline::kinds::FilterKind;
use crate::archive::unified::filter_pipeline::lookahead::SlidingLookaheadReader;
use crate::archive::unified::filter_pipeline::traits::StreamFilter;

/// Streaming UUDecode filter that strips `begin <mode> <filename>` headers
/// and decodes lines to raw binary payload.
pub struct UuencodeFilter<R: Read + Send> {
    reader: SlidingLookaheadReader<R>,
    line_buf: Vec<u8>,
    out_buf: Vec<u8>,
    out_pos: usize,
    started: bool,
    ended: bool,
    bytes_consumed: u64,
    bytes_produced: u64,
}

impl<R: Read + Send> UuencodeFilter<R> {
    /// Creates a new UUDecode streaming filter.
    pub fn new(reader: R) -> Self {
        Self {
            reader: SlidingLookaheadReader::new(reader),
            line_buf: Vec::with_capacity(256),
            out_buf: Vec::with_capacity(1024),
            out_pos: 0,
            started: false,
            ended: false,
            bytes_consumed: 0,
            bytes_produced: 0,
        }
    }

    fn refill_out_buf(&mut self) -> io::Result<()> {
        self.out_buf.clear();
        self.out_pos = 0;

        while self.out_buf.is_empty() && !self.ended {
            if !self.read_next_line()? {
                self.ended = true;
                break;
            }

            let line_slice = &self.line_buf;
            if !self.started {
                if line_slice.starts_with(b"begin ") || line_slice.starts_with(b"begin-base64 ") {
                    self.started = true;
                }
                continue;
            }

            if line_slice == b"end" || line_slice.is_empty() || line_slice == b"`" {
                if line_slice == b"end" {
                    self.ended = true;
                }
                continue;
            }

            decode_uu_line(line_slice, &mut self.out_buf);
        }

        Ok(())
    }

    fn read_next_line(&mut self) -> io::Result<bool> {
        self.line_buf.clear();
        let mut read_any = false;
        loop {
            let mut b = [0u8; 1];
            let n = self.reader.read(&mut b)?;
            if n == 0 {
                break;
            }
            read_any = true;
            self.bytes_consumed += 1;
            let ch = b[0];
            if ch == b'\n' {
                break;
            }
            if ch != b'\r' {
                self.line_buf.push(ch);
            }
        }
        Ok(read_any || !self.line_buf.is_empty())
    }
}

fn decode_uu_line(line_slice: &[u8], out_buf: &mut Vec<u8>) {
    let len_char = line_slice[0];
    let line_len = (len_char.wrapping_sub(b' ') & 0x3F) as usize;
    if line_len == 0 {
        return;
    }

    let encoded_chunks = &line_slice[1..];
    let mut i = 0;
    while i < encoded_chunks.len() && out_buf.len() < line_len {
        let c0 = encoded_chunks.get(i).copied().unwrap_or(b' ');
        let c1 = encoded_chunks.get(i + 1).copied().unwrap_or(b' ');
        let c2 = encoded_chunks.get(i + 2).copied().unwrap_or(b' ');
        let c3 = encoded_chunks.get(i + 3).copied().unwrap_or(b' ');
        i += 4;

        let b0 = c0.wrapping_sub(b' ') & 0x3F;
        let b1 = c1.wrapping_sub(b' ') & 0x3F;
        let b2 = c2.wrapping_sub(b' ') & 0x3F;
        let b3 = c3.wrapping_sub(b' ') & 0x3F;

        out_buf.push((b0 << 2) | (b1 >> 4));
        if out_buf.len() < line_len {
            out_buf.push(((b1 & 0x0F) << 4) | (b2 >> 2));
        }
        if out_buf.len() < line_len {
            out_buf.push(((b2 & 0x03) << 6) | b3);
        }
    }
}

impl<R: Read + Send> Read for UuencodeFilter<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        if self.out_pos >= self.out_buf.len() {
            self.refill_out_buf()?;
        }

        if self.out_pos >= self.out_buf.len() {
            return Ok(0);
        }

        let avail = self.out_buf.len() - self.out_pos;
        let to_copy = avail.min(buf.len());
        buf[..to_copy].copy_from_slice(&self.out_buf[self.out_pos..self.out_pos + to_copy]);
        self.out_pos += to_copy;
        self.bytes_produced += to_copy as u64;
        Ok(to_copy)
    }
}

impl<R: Read + Send> StreamFilter for UuencodeFilter<R> {
    fn filter_kind(&self) -> FilterKind {
        FilterKind::Uuencode
    }
    fn bytes_consumed(&self) -> u64 {
        self.bytes_consumed
    }
    fn bytes_produced(&self) -> u64 {
        self.bytes_produced
    }
}
