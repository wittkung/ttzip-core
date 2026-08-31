// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! SlidingLookaheadReader non-destructive stream inspector.

use std::io::{self, Read};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Buffered stream reader providing non-destructive lookahead and zero-copy slicing.
pub struct SlidingLookaheadReader<R: Read + Send> {
    inner: R,
    buffer: Vec<u8>,
    pos: usize,
    eof: bool,
    bytes_consumed: Arc<AtomicU64>,
}

impl<R: Read + Send> SlidingLookaheadReader<R> {
    /// Default buffer capacity (64 KB).
    pub const DEFAULT_CAPACITY: usize = 64 * 1024;

    /// Creates a new `SlidingLookaheadReader` with default 64KB capacity.
    pub fn new(inner: R) -> Self {
        Self::with_capacity(inner, Self::DEFAULT_CAPACITY)
    }

    /// Creates a new `SlidingLookaheadReader` with custom capacity.
    pub fn with_capacity(inner: R, capacity: usize) -> Self {
        Self {
            inner,
            buffer: Vec::with_capacity(capacity.max(1024)),
            pos: 0,
            eof: false,
            bytes_consumed: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Returns a slice of the upcoming bytes without advancing the read position.
    pub fn peek(&mut self, required_len: usize) -> io::Result<&[u8]> {
        while (self.buffer.len() - self.pos) < required_len && !self.eof {
            if self.pos > 0 {
                self.buffer.drain(..self.pos);
                self.pos = 0;
            }
            let prev_len = self.buffer.len();
            let to_read = (required_len + 4096).max(16384);
            self.buffer.resize(prev_len + to_read, 0);
            let n = self.inner.read(&mut self.buffer[prev_len..])?;
            if n == 0 {
                self.eof = true;
                self.buffer.truncate(prev_len);
                break;
            }
            self.buffer.truncate(prev_len + n);
        }
        Ok(&self.buffer[self.pos..])
    }

    /// Peeks lookahead bytes up to `max_len`.
    pub fn lookahead(&mut self, max_len: usize) -> io::Result<&[u8]> {
        self.peek(max_len)
    }

    /// Discards `amt` bytes from the front of the lookahead buffer.
    pub fn consume_bytes(&mut self, amt: usize) {
        let avail = self.buffer.len() - self.pos;
        let step = amt.min(avail);
        self.pos += step;
        self.bytes_consumed.fetch_add(step as u64, Ordering::Relaxed);
    }

    /// Returns shared handle to consumed byte counter.
    #[inline]
    pub fn counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.bytes_consumed)
    }

    /// Unwraps and returns the underlying reader.
    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: Read + Send> Read for SlidingLookaheadReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        if self.pos < self.buffer.len() {
            let available = self.buffer.len() - self.pos;
            let to_copy = available.min(buf.len());
            buf[..to_copy].copy_from_slice(&self.buffer[self.pos..self.pos + to_copy]);
            self.pos += to_copy;
            self.bytes_consumed.fetch_add(to_copy as u64, Ordering::Relaxed);
            return Ok(to_copy);
        }

        let n = self.inner.read(buf)?;
        self.bytes_consumed.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}
