// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Uncompressed data pass-through and metadata block payload streaming.
//!
//! Compliant with RFC 7932 Section 9.2 (Uncompressed Meta-Blocks) and Section 9.3 (Metadata Meta-Blocks).

use std::io::Read;

use super::decoder::BrotliStreamDecoder;
use super::error::BrotliError;

impl<R: Read> BrotliStreamDecoder<R> {
    /// Streams raw uncompressed meta-block bytes into the sliding ring buffer.
    pub(crate) fn read_uncompressed_chunk(&mut self) -> Result<(), BrotliError> {
        while self.bit_pos >= 8 && self.meta_block_remaining_len > 0 {
            let b = (self.bit_val & 0xFF) as u8;
            self.drop_bits(8);
            self.ring_buffer.write_byte(b);
            self.meta_block_remaining_len -= 1;
        }

        while self.meta_block_remaining_len > 0 {
            if self.buf_pos < self.buf_len {
                let avail = self.buf_len - self.buf_pos;
                let take = avail.min(self.meta_block_remaining_len);
                self.ring_buffer
                    .copy_slice(&self.buffer[self.buf_pos..self.buf_pos + take]);
                self.buf_pos += take;
                self.meta_block_remaining_len -= take;
            } else {
                if self.eof_reached {
                    return Err(BrotliError::UnexpectedEof);
                }
                match self.reader.read(&mut self.buffer) {
                    Ok(0) => {
                        self.eof_reached = true;
                        return Err(BrotliError::UnexpectedEof);
                    }
                    Ok(n) => {
                        self.buf_pos = 0;
                        self.buf_len = n;
                    }
                    Err(e) => return Err(BrotliError::DecompressionFailed(e.to_string())),
                }
            }
            if self.ring_buffer.available_data() >= 16384 {
                break;
            }
        }

        Ok(())
    }

    /// Skips metadata block payload bytes from the stream without copying to output ring buffer.
    pub(crate) fn skip_metadata_chunk(&mut self) -> Result<(), BrotliError> {
        while self.bit_pos >= 8 && self.meta_block_remaining_len > 0 {
            self.drop_bits(8);
            self.meta_block_remaining_len -= 1;
        }

        while self.meta_block_remaining_len > 0 {
            if self.buf_pos < self.buf_len {
                let avail = self.buf_len - self.buf_pos;
                let take = avail.min(self.meta_block_remaining_len);
                self.buf_pos += take;
                self.meta_block_remaining_len -= take;
            } else {
                if self.eof_reached {
                    return Err(BrotliError::UnexpectedEof);
                }
                match self.reader.read(&mut self.buffer) {
                    Ok(0) => {
                        self.eof_reached = true;
                        return Err(BrotliError::UnexpectedEof);
                    }
                    Ok(n) => {
                        self.buf_pos = 0;
                        self.buf_len = n;
                    }
                    Err(e) => return Err(BrotliError::DecompressionFailed(e.to_string())),
                }
            }
        }
        Ok(())
    }
}
