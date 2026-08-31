// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Streaming LZ4 Frame block compressor implementing `std::io::Write`.
//!
//! Conforms to the LZ4 Framing Format specification with optional block checksums,
//! uncompressed fallbacks, content checksum verification, and sliding dictionary state.

use crate::checksum::{xxh32, Xxh32Hasher};
use crate::codecs::lz4::block::{lz4_compress_bound, lz4_compress_fast};
use crate::codecs::lz4::constants::FrameDescriptor;
use std::hash::Hasher;
use std::io::{self, Write};

// MARK: - Native C LZ4 Stream Compression Bindings

extern "C" {
    fn LZ4_createStream() -> *mut libc::c_void;
    fn LZ4_freeStream(stream_ptr: *mut libc::c_void) -> libc::c_int;
    fn LZ4_loadDict(
        stream_ptr: *mut libc::c_void,
        dictionary: *const libc::c_char,
        dict_size: libc::c_int,
    ) -> libc::c_int;
    fn LZ4_compress_fast_continue(
        stream_ptr: *mut libc::c_void,
        src: *const libc::c_char,
        dst: *mut libc::c_char,
        src_size: libc::c_int,
        dst_capacity: libc::c_int,
        acceleration: libc::c_int,
    ) -> libc::c_int;
}

// MARK: - Lz4FrameEncoder

/// Streaming LZ4 Frame compressor implementing `std::io::Write`.
pub struct Lz4FrameEncoder<W: Write> {
    writer: Option<W>,
    descriptor: FrameDescriptor,
    compression_level: i32,
    content_hasher: Option<Xxh32Hasher>,
    in_buf: Vec<u8>,
    dict_history: Vec<u8>,
    cstream: Option<*mut libc::c_void>,
    finished: bool,
}

unsafe impl<W: Write + Send> Send for Lz4FrameEncoder<W> {}

impl<W: Write> Lz4FrameEncoder<W> {
    /// Creates a new `Lz4FrameEncoder` with default preferences (Independent, 64KB blocks, acceleration = 1).
    pub fn new(writer: W) -> io::Result<Self> {
        Self::with_options(writer, FrameDescriptor::default(), 1)
    }

    /// Creates a new `Lz4FrameEncoder` with explicit descriptor options and acceleration / level factor.
    pub fn with_options(
        mut writer: W,
        descriptor: FrameDescriptor,
        compression_level: i32,
    ) -> io::Result<Self> {
        let mut header_buf = vec![0u8; 4 + descriptor.total_header_size()];
        descriptor.emit_with_magic(&mut header_buf).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("descriptor emit error: {:?}", e),
            )
        })?;
        writer.write_all(&header_buf)?;

        let content_hasher = if descriptor.content_checksum {
            Some(Xxh32Hasher::new())
        } else {
            None
        };

        let cstream = if !descriptor.block_independence.is_independent() {
            let ptr = unsafe { LZ4_createStream() };
            if ptr.is_null() {
                return Err(io::Error::new(
                    io::ErrorKind::OutOfMemory,
                    "failed to allocate LZ4 stream",
                ));
            }
            Some(ptr)
        } else {
            None
        };

        let max_block = descriptor.block_max_size.max_bytes();
        Ok(Self {
            writer: Some(writer),
            descriptor,
            compression_level,
            content_hasher,
            in_buf: Vec::with_capacity(max_block),
            dict_history: Vec::new(),
            cstream,
            finished: false,
        })
    }

    /// Compresses and writes out the active block.
    fn flush_block(&mut self) -> io::Result<()> {
        if self.in_buf.is_empty() {
            return Ok(());
        }
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "writer closed"))?;

        if let Some(hasher) = &mut self.content_hasher {
            hasher.write(&self.in_buf);
        }

        let raw_len = self.in_buf.len();
        let max_bound = lz4_compress_bound(raw_len);
        let mut comp_buf = vec![0u8; max_bound];

        let comp_len = if let Some(stream_ptr) = self.cstream {
            let written = if self.dict_history.is_empty() {
                lz4_compress_fast(&self.in_buf, &mut comp_buf, self.compression_level).unwrap_or(0)
            } else {
                let res = unsafe {
                    LZ4_compress_fast_continue(
                        stream_ptr,
                        self.in_buf.as_ptr() as *const libc::c_char,
                        comp_buf.as_mut_ptr() as *mut libc::c_char,
                        raw_len as libc::c_int,
                        max_bound as libc::c_int,
                        self.compression_level.clamp(1, 100) as libc::c_int,
                    )
                };
                if res > 0 {
                    res as usize
                } else {
                    0
                }
            };

            if written > 0 {
                const MAX_DICT_SIZE: usize = 65536;
                if raw_len >= MAX_DICT_SIZE {
                    self.dict_history.clear();
                    self.dict_history
                        .extend_from_slice(&self.in_buf[raw_len - MAX_DICT_SIZE..]);
                } else {
                    let avail = MAX_DICT_SIZE.saturating_sub(raw_len);
                    if self.dict_history.len() > avail {
                        let overflow = self.dict_history.len() - avail;
                        self.dict_history.drain(0..overflow);
                    }
                    self.dict_history.extend_from_slice(&self.in_buf);
                }

                unsafe {
                    LZ4_loadDict(
                        stream_ptr,
                        self.dict_history.as_ptr() as *const libc::c_char,
                        self.dict_history.len() as libc::c_int,
                    );
                }
                written
            } else {
                0
            }
        } else {
            lz4_compress_fast(&self.in_buf, &mut comp_buf, self.compression_level).unwrap_or(0)
        };

        if comp_len > 0 && comp_len < raw_len {
            let block_header = (comp_len as u32).to_le_bytes();
            writer.write_all(&block_header)?;
            writer.write_all(&comp_buf[..comp_len])?;
            if self.descriptor.block_checksum {
                let bc = xxh32(&comp_buf[..comp_len], 0);
                writer.write_all(&bc.to_le_bytes())?;
            }
        } else {
            let block_header = ((raw_len as u32) | 0x8000_0000).to_le_bytes();
            writer.write_all(&block_header)?;
            writer.write_all(&self.in_buf)?;
            if self.descriptor.block_checksum {
                let bc = xxh32(&self.in_buf, 0);
                writer.write_all(&bc.to_le_bytes())?;
            }
        }

        self.in_buf.clear();
        Ok(())
    }

    /// Completes the frame by flushing remaining blocks, emitting EndMark and content checksum.
    pub fn finish(mut self) -> io::Result<W> {
        if !self.finished {
            self.flush_block()?;
            let mut writer = self
                .writer
                .take()
                .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "writer closed"))?;

            writer.write_all(&0u32.to_le_bytes())?;

            if let Some(hasher) = &self.content_hasher {
                let cc = hasher.digest();
                writer.write_all(&cc.to_le_bytes())?;
            }

            writer.flush()?;
            self.finished = true;
            Ok(writer)
        } else {
            self.writer
                .take()
                .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "writer already closed"))
        }
    }
}

impl<W: Write> Write for Lz4FrameEncoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let max_block = self.descriptor.block_max_size.max_bytes();
        let avail = max_block.saturating_sub(self.in_buf.len());
        let to_copy = avail.min(buf.len());
        self.in_buf.extend_from_slice(&buf[..to_copy]);

        if self.in_buf.len() >= max_block {
            self.flush_block()?;
        }
        Ok(to_copy)
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.in_buf.is_empty() {
            self.flush_block()?;
        }
        if let Some(w) = self.writer.as_mut() {
            w.flush()?;
        }
        Ok(())
    }
}

impl<W: Write> Drop for Lz4FrameEncoder<W> {
    fn drop(&mut self) {
        if let Some(stream_ptr) = self.cstream.take() {
            unsafe {
                LZ4_freeStream(stream_ptr);
            }
        }
    }
}
