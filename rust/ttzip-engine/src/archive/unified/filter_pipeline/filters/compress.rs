// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Streaming Unix compress (`.Z`, `\x1F\x9D`) LZW filter.

use std::io::{self, Read};

use crate::archive::unified::filter_pipeline::kinds::FilterKind;
use crate::archive::unified::filter_pipeline::lookahead::SlidingLookaheadReader;
use crate::archive::unified::filter_pipeline::traits::StreamFilter;

/// Streaming Unix compress (`.Z`, `\x1F\x9D`) LZW filter.
pub struct CompressFilter<R: Read + Send> {
    reader: SlidingLookaheadReader<R>,
    out_buf: Vec<u8>,
    out_pos: usize,
    table_prefix: Vec<u16>,
    table_suffix: Vec<u8>,
    stack: Vec<u8>,
    max_bits: u8,
    block_mode: bool,
    curr_bits: u8,
    free_code: usize,
    old_code: usize,
    fin_char: u8,
    bit_buf: u32,
    bits_in_buf: u8,
    header_parsed: bool,
    eof_reached: bool,
    bytes_consumed: u64,
    bytes_produced: u64,
}

impl<R: Read + Send> CompressFilter<R> {
    /// Creates a new Unix Compress (.Z) LZW streaming filter.
    pub fn new(reader: R) -> Self {
        Self {
            reader: SlidingLookaheadReader::new(reader),
            out_buf: Vec::with_capacity(4096),
            out_pos: 0,
            table_prefix: vec![0; 65536],
            table_suffix: vec![0; 65536],
            stack: Vec::with_capacity(65536),
            max_bits: 16,
            block_mode: true,
            curr_bits: 9,
            free_code: 257,
            old_code: 0,
            fin_char: 0,
            bit_buf: 0,
            bits_in_buf: 0,
            header_parsed: false,
            eof_reached: false,
            bytes_consumed: 0,
            bytes_produced: 0,
        }
    }

    fn init_table(&mut self) {
        self.curr_bits = 9;
        self.free_code = if self.block_mode { 257 } else { 256 };
    }

    fn read_bits(&mut self) -> io::Result<Option<usize>> {
        while self.bits_in_buf < self.curr_bits {
            let mut b = [0u8; 1];
            let n = self.reader.read(&mut b)?;
            if n == 0 {
                if self.bits_in_buf == 0 {
                    return Ok(None);
                }
                return Ok(None);
            }
            self.bytes_consumed += 1;
            self.bit_buf |= (b[0] as u32) << self.bits_in_buf;
            self.bits_in_buf += 8;
        }

        let mask = (1u32 << self.curr_bits) - 1;
        let code = (self.bit_buf & mask) as usize;
        self.bit_buf >>= self.curr_bits;
        self.bits_in_buf -= self.curr_bits;
        Ok(Some(code))
    }

    fn fill_output(&mut self) -> io::Result<()> {
        self.out_buf.clear();
        self.out_pos = 0;

        if !self.header_parsed {
            let mut hdr = [0u8; 3];
            let n = self.reader.read(&mut hdr)?;
            if n < 3 || hdr[0] != 0x1F || hdr[1] != 0x9D {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid compress (.Z) header"));
            }
            self.bytes_consumed += 3;
            self.max_bits = hdr[2] & 0x1F;
            self.block_mode = (hdr[2] & 0x80) != 0;
            if self.max_bits < 9 || self.max_bits > 16 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid max_bits in compress header"));
            }
            self.init_table();

            // First code
            if let Some(first_code) = self.read_bits()? {
                self.fin_char = first_code as u8;
                self.old_code = first_code;
                self.out_buf.push(self.fin_char);
            } else {
                self.eof_reached = true;
                return Ok(());
            }
            self.header_parsed = true;
        }

        while self.out_buf.len() < 4096 && !self.eof_reached {
            let raw_code = match self.read_bits()? {
                Some(c) => c,
                None => {
                    self.eof_reached = true;
                    break;
                }
            };

            if raw_code == 256 && self.block_mode {
                self.init_table();
                match self.read_bits()? {
                    Some(c) => {
                        self.fin_char = c as u8;
                        self.old_code = c;
                        self.out_buf.push(self.fin_char);
                        continue;
                    }
                    None => {
                        self.eof_reached = true;
                        break;
                    }
                }
            }

            let mut code = raw_code;
            self.stack.clear();

            if code >= self.free_code {
                if code > self.free_code {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "corrupted compress stream"));
                }
                self.stack.push(self.fin_char);
                code = self.old_code;
            }

            while code >= 256 {
                self.stack.push(self.table_suffix[code]);
                code = self.table_prefix[code] as usize;
            }
            self.fin_char = code as u8;
            self.stack.push(self.fin_char);

            while let Some(ch) = self.stack.pop() {
                self.out_buf.push(ch);
            }

            // Insert new code into table
            if self.free_code < (1 << self.max_bits) {
                self.table_prefix[self.free_code] = self.old_code as u16;
                self.table_suffix[self.free_code] = self.fin_char;
                self.free_code += 1;
                if self.free_code >= (1 << self.curr_bits) && self.curr_bits < self.max_bits {
                    self.curr_bits += 1;
                }
            }

            self.old_code = raw_code;
        }

        Ok(())
    }
}

impl<R: Read + Send> Read for CompressFilter<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        if self.out_pos >= self.out_buf.len() {
            self.fill_output()?;
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

impl<R: Read + Send> StreamFilter for CompressFilter<R> {
    fn filter_kind(&self) -> FilterKind {
        FilterKind::Compress
    }
    fn bytes_consumed(&self) -> u64 {
        self.bytes_consumed
    }
    fn bytes_produced(&self) -> u64 {
        self.bytes_produced
    }
}
