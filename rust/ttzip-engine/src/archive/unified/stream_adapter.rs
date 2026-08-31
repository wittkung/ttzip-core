// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-seek sliding lookahead reader and micro-buffer streaming adapter.
//!
//! Provides:
//! 1. `LookaheadRead` Trait: Non-destructive lookahead, cursor consumption, and forward skip.
//! 2. `SlidingLookaheadReader<R>`: Monotonically expanding dual-buffer sliding window
//!    with in-place compaction, zero-copy borrowing, and seamless non-seekable degradation.
//! 3. Memory bounding: strictly enforces $\le 64\text{MB}$ resident microkernel invariant.

use std::io::{BufRead, Cursor, Error, ErrorKind, Read, Result, Seek, SeekFrom};

/// Default initial lookahead buffer capacity: 64 KB.
pub const DEFAULT_INITIAL_LOOKAHEAD_CAPACITY: usize = 64 * 1024;

/// Hard ceiling for lookahead buffer allocation: 64 MB (microkernel resident limit).
pub const MAX_LOOKAHEAD_CAPACITY: usize = 64 * 1024 * 1024;

/// Micro-buffer chunk size for adaptive stream skipping / discarding: 8 KB.
pub const MICRO_BUFFER_CHUNK_SIZE: usize = 8 * 1024;

/// Trait for non-destructive lookahead and zero-seek streaming inspection.
pub trait LookaheadRead: Read {
    /// Non-destructively peeks at least `min_bytes` ahead of the current logical cursor.
    ///
    /// If fewer than `min_bytes` are currently buffered, the reader reads from the
    /// underlying stream until at least `min_bytes` are available or EOF is reached.
    ///
    /// # Errors
    /// Returns `ErrorKind::UnexpectedEof` if the underlying stream reaches EOF before
    /// `min_bytes` can be satisfied. If `min_bytes == 0`, returns all currently buffered bytes.
    fn peek_ahead(&mut self, min_bytes: usize) -> Result<&[u8]>;

    /// Explicitly advances and consumes `bytes` from the logical stream.
    ///
    /// # Errors
    /// Returns `ErrorKind::UnexpectedEof` if attempting to consume beyond stream EOF.
    fn consume(&mut self, bytes: usize) -> Result<()>;

    /// Skips `bytes` in the forward direction.
    ///
    /// Uses physical fast seek if supported by the underlying source; otherwise seamlessly
    /// falls back to adaptive 8 KB micro-buffered reading and discarding.
    /// Returns the total number of bytes successfully skipped.
    fn stream_skip(&mut self, bytes: u64) -> Result<u64>;
}

/// Sliding window lookahead reader supporting non-destructive inspection and zero-seek streaming.
pub struct SlidingLookaheadReader<R> {
    inner: R,
    buffer: Vec<u8>,
    head: usize,
    tail: usize,
    initial_capacity: usize,
    eof_reached: bool,
    total_bytes_consumed: u64,
    seek_fn: Option<fn(&mut R, u64) -> Result<u64>>,
}

impl<R: Read> SlidingLookaheadReader<R> {
    /// Creates a new `SlidingLookaheadReader` with the default 64 KB initial capacity.
    #[inline]
    #[must_use]
    pub fn new(inner: R) -> Self {
        Self::with_capacity(inner, DEFAULT_INITIAL_LOOKAHEAD_CAPACITY)
    }

    /// Creates a new `SlidingLookaheadReader` with a specified initial capacity.
    #[must_use]
    pub fn with_capacity(inner: R, capacity: usize) -> Self {
        let bounded_cap = capacity.clamp(MICRO_BUFFER_CHUNK_SIZE, MAX_LOOKAHEAD_CAPACITY);
        Self {
            inner,
            buffer: vec![0u8; bounded_cap],
            head: 0,
            tail: 0,
            initial_capacity: bounded_cap,
            eof_reached: false,
            total_bytes_consumed: 0,
            seek_fn: None,
        }
    }

    /// Returns the number of currently buffered and unconsumed lookahead bytes.
    #[inline]
    #[must_use]
    pub const fn buffered_bytes(&self) -> usize {
        self.tail - self.head
    }

    /// Returns the total number of bytes consumed from this reader so far.
    #[inline]
    #[must_use]
    pub const fn total_consumed(&self) -> u64 {
        self.total_bytes_consumed
    }

    /// Returns the current allocated capacity of the internal lookahead buffer.
    #[inline]
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    /// Returns true if the underlying stream has encountered EOF.
    #[inline]
    #[must_use]
    pub const fn is_eof(&self) -> bool {
        self.eof_reached && (self.head == self.tail)
    }

    /// Returns a reference to the underlying reader.
    #[inline]
    #[must_use]
    pub const fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Returns a mutable reference to the underlying reader.
    ///
    /// # Safety / Warning
    /// Direct mutations of the underlying stream state may desynchronize the lookahead buffer.
    #[inline]
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Consumes this reader and unwraps the underlying reader.
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> R {
        self.inner
    }

    /// Non-destructively peeks at least `min_bytes` ahead from the current position.
    pub fn peek_ahead(&mut self, min_bytes: usize) -> Result<&[u8]> {
        if min_bytes == 0 {
            return Ok(&self.buffer[self.head..self.tail]);
        }

        if min_bytes > MAX_LOOKAHEAD_CAPACITY {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "requested lookahead size ({} bytes) exceeds maximum limit of {} bytes",
                    min_bytes, MAX_LOOKAHEAD_CAPACITY
                ),
            ));
        }

        let available = self.tail - self.head;
        if available >= min_bytes {
            return Ok(&self.buffer[self.head..self.tail]);
        }

        if self.eof_reached {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                format!(
                    "unexpected EOF: requested {} lookahead bytes, but only {} available before EOF",
                    min_bytes, available
                ),
            ));
        }

        // Compact buffer by shifting remaining unconsumed bytes to index 0
        if self.head > 0 {
            if available > 0 {
                self.buffer.copy_within(self.head..self.tail, 0);
            }
            self.head = 0;
            self.tail = available;
        }

        // Expand buffer capacity if necessary (monotonic doubling up to 64 MB)
        if self.buffer.len() < min_bytes {
            let mut new_cap = self.buffer.len().max(self.initial_capacity);
            while new_cap < min_bytes && new_cap < MAX_LOOKAHEAD_CAPACITY {
                new_cap = new_cap.saturating_mul(2);
            }
            let target_cap = new_cap.max(min_bytes).min(MAX_LOOKAHEAD_CAPACITY);
            self.buffer.resize(target_cap, 0);
        }

        // Fill buffer from underlying reader until min_bytes are satisfied or EOF
        while self.tail < min_bytes {
            let n = self.inner.read(&mut self.buffer[self.tail..])?;
            if n == 0 {
                self.eof_reached = true;
                if self.tail < min_bytes {
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        format!(
                            "unexpected EOF: requested {} lookahead bytes, but stream ended at {} bytes",
                            min_bytes, self.tail
                        ),
                    ));
                }
                break;
            }
            self.tail += n;
        }

        Ok(&self.buffer[self.head..self.tail])
    }

    /// Explicitly consumes and advances the read cursor by `bytes`.
    pub fn consume(&mut self, bytes: usize) -> Result<()> {
        if bytes == 0 {
            return Ok(());
        }

        let available = self.tail - self.head;
        if bytes <= available {
            self.head += bytes;
            self.total_bytes_consumed += bytes as u64;
            if self.head == self.tail {
                self.head = 0;
                self.tail = 0;
            }
            return Ok(());
        }

        // Consume all currently buffered bytes
        let buffered_bytes = available as u64;
        self.total_bytes_consumed += buffered_bytes;
        let remaining_to_skip = (bytes - available) as u64;
        self.head = 0;
        self.tail = 0;

        let skipped = self.skip_inner(remaining_to_skip)?;
        self.total_bytes_consumed += skipped;

        if skipped < remaining_to_skip {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                format!(
                    "unexpected EOF: requested {} bytes to consume, but stream ended after {} bytes",
                    bytes,
                    buffered_bytes + skipped
                ),
            ));
        }

        Ok(())
    }

    /// Skips `bytes` in the forward direction.
    pub fn stream_skip(&mut self, bytes: u64) -> Result<u64> {
        if bytes == 0 {
            return Ok(0);
        }

        let available = (self.tail - self.head) as u64;
        if bytes <= available {
            let bytes_usize = bytes as usize;
            self.head += bytes_usize;
            self.total_bytes_consumed += bytes;
            if self.head == self.tail {
                self.head = 0;
                self.tail = 0;
            }
            return Ok(bytes);
        }

        // Consume entire buffered segment
        self.total_bytes_consumed += available;
        let remaining = bytes - available;
        self.head = 0;
        self.tail = 0;

        let skipped = self.skip_inner(remaining)?;
        self.total_bytes_consumed += skipped;
        Ok(available + skipped)
    }

    /// Internal helper to skip bytes using fast seek if available or adaptive 8 KB discard loop.
    fn skip_inner(&mut self, remaining: u64) -> Result<u64> {
        if remaining == 0 {
            return Ok(0);
        }

        if let Some(seek_fn) = self.seek_fn {
            return seek_fn(&mut self.inner, remaining);
        }

        // Adaptive non-seekable fallback: 8 KB micro-buffer loop discard
        let mut discard_buf = [0u8; MICRO_BUFFER_CHUNK_SIZE];
        let mut total_discarded = 0u64;

        while total_discarded < remaining {
            let to_read = ((remaining - total_discarded) as usize).min(discard_buf.len());
            let n = self.inner.read(&mut discard_buf[..to_read])?;
            if n == 0 {
                self.eof_reached = true;
                break;
            }
            total_discarded += n as u64;
        }

        Ok(total_discarded)
    }
}

impl<R: Read + Seek> SlidingLookaheadReader<R> {
    /// Creates a new `SlidingLookaheadReader` with fast seek enabled using 64 KB buffer.
    #[inline]
    #[must_use]
    pub fn new_seekable(inner: R) -> Self {
        Self::with_capacity_seekable(inner, DEFAULT_INITIAL_LOOKAHEAD_CAPACITY)
    }

    /// Creates a new `SlidingLookaheadReader` with fast seek enabled and specified capacity.
    #[must_use]
    pub fn with_capacity_seekable(inner: R, capacity: usize) -> Self {
        let mut reader = Self::with_capacity(inner, capacity);
        reader.seek_fn = Some(|inner, remaining| {
            let cur = inner.stream_position()?;
            let target = cur.saturating_add(remaining);
            let new_pos = inner.seek(SeekFrom::Start(target))?;
            Ok(new_pos.saturating_sub(cur))
        });
        reader
    }

    /// Explicit fast seek skip utilizing the underlying reader's `Seek` implementation.
    pub fn seek_skip(&mut self, bytes: u64) -> Result<u64> {
        let available = (self.tail - self.head) as u64;
        if bytes <= available {
            self.head += bytes as usize;
            self.total_bytes_consumed += bytes;
            if self.head == self.tail {
                self.head = 0;
                self.tail = 0;
            }
            return Ok(bytes);
        }

        let buffered_consumed = available;
        self.total_bytes_consumed += buffered_consumed;
        let remaining = bytes - buffered_consumed;
        self.head = 0;
        self.tail = 0;

        let cur = self.inner.stream_position()?;
        let target = cur.saturating_add(remaining);
        let new_pos = self.inner.seek(SeekFrom::Start(target))?;
        let skipped = new_pos.saturating_sub(cur);
        self.total_bytes_consumed += skipped;
        Ok(buffered_consumed + skipped)
    }
}

impl<R: Read> Read for SlidingLookaheadReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let available = self.tail - self.head;
        if available > 0 {
            let to_copy = buf.len().min(available);
            buf[..to_copy].copy_from_slice(&self.buffer[self.head..self.head + to_copy]);
            self.head += to_copy;
            self.total_bytes_consumed += to_copy as u64;
            if self.head == self.tail {
                self.head = 0;
                self.tail = 0;
            }
            return Ok(to_copy);
        }

        let n = self.inner.read(buf)?;
        self.total_bytes_consumed += n as u64;
        if n == 0 {
            self.eof_reached = true;
        }
        Ok(n)
    }
}

impl<R: Read> BufRead for SlidingLookaheadReader<R> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        if self.head == self.tail && !self.eof_reached {
            self.head = 0;
            self.tail = 0;
            if self.buffer.is_empty() {
                self.buffer.resize(self.initial_capacity, 0);
            }
            let n = self.inner.read(&mut self.buffer)?;
            self.tail = n;
            if n == 0 {
                self.eof_reached = true;
            }
        }
        Ok(&self.buffer[self.head..self.tail])
    }

    fn consume(&mut self, amt: usize) {
        let available = self.tail - self.head;
        let actual = amt.min(available);
        self.head += actual;
        self.total_bytes_consumed += actual as u64;
        if self.head == self.tail {
            self.head = 0;
            self.tail = 0;
        }
    }
}

impl<R: Read + Seek> Seek for SlidingLookaheadReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        if let SeekFrom::Current(offset) = pos {
            if offset >= 0 {
                let offset_u64 = offset as u64;
                let available = (self.tail - self.head) as u64;
                if offset_u64 <= available {
                    self.head += offset_u64 as usize;
                    self.total_bytes_consumed += offset_u64;
                    if self.head == self.tail {
                        self.head = 0;
                        self.tail = 0;
                    }
                    return self.stream_position();
                }
            }
        }

        let physical_pos = self.inner.stream_position()?;
        let buffered_unconsumed = (self.tail - self.head) as u64;
        let virtual_pos = physical_pos.saturating_sub(buffered_unconsumed);

        let target_pos = match pos {
            SeekFrom::Start(p) => SeekFrom::Start(p),
            SeekFrom::End(p) => SeekFrom::End(p),
            SeekFrom::Current(offset) => {
                let new_virtual = if offset >= 0 {
                    virtual_pos.saturating_add(offset as u64)
                } else {
                    virtual_pos.saturating_sub((-offset) as u64)
                };
                SeekFrom::Start(new_virtual)
            }
        };

        self.head = 0;
        self.tail = 0;
        self.eof_reached = false;
        self.inner.seek(target_pos)
    }

    fn stream_position(&mut self) -> Result<u64> {
        let physical_pos = self.inner.stream_position()?;
        let buffered_unconsumed = (self.tail - self.head) as u64;
        Ok(physical_pos.saturating_sub(buffered_unconsumed))
    }
}

impl<R: Read> LookaheadRead for SlidingLookaheadReader<R> {
    #[inline]
    fn peek_ahead(&mut self, min_bytes: usize) -> Result<&[u8]> {
        self.peek_ahead(min_bytes)
    }

    #[inline]
    fn consume(&mut self, bytes: usize) -> Result<()> {
        self.consume(bytes)
    }

    #[inline]
    fn stream_skip(&mut self, bytes: u64) -> Result<u64> {
        self.stream_skip(bytes)
    }
}

impl LookaheadRead for &[u8] {
    fn peek_ahead(&mut self, min_bytes: usize) -> Result<&[u8]> {
        if self.len() < min_bytes {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                format!(
                    "unexpected EOF: requested {} peek bytes, but slice has only {}",
                    min_bytes,
                    self.len()
                ),
            ));
        }
        Ok(*self)
    }

    fn consume(&mut self, bytes: usize) -> Result<()> {
        if self.len() < bytes {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                format!(
                    "unexpected EOF: requested {} consume bytes, but slice has only {}",
                    bytes,
                    self.len()
                ),
            ));
        }
        *self = &self[bytes..];
        Ok(())
    }

    fn stream_skip(&mut self, bytes: u64) -> Result<u64> {
        let to_skip = (bytes as usize).min(self.len());
        *self = &self[to_skip..];
        Ok(to_skip as u64)
    }
}

impl<T: AsRef<[u8]>> LookaheadRead for Cursor<T> {
    fn peek_ahead(&mut self, min_bytes: usize) -> Result<&[u8]> {
        let slice = self.get_ref().as_ref();
        let pos = self.position() as usize;
        let available = slice.len().saturating_sub(pos);
        if available < min_bytes {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                format!(
                    "unexpected EOF: requested {} peek bytes, but cursor has only {}",
                    min_bytes, available
                ),
            ));
        }
        Ok(&slice[pos..])
    }

    fn consume(&mut self, bytes: usize) -> Result<()> {
        let slice = self.get_ref().as_ref();
        let pos = self.position() as usize;
        let available = slice.len().saturating_sub(pos);
        if available < bytes {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                format!(
                    "unexpected EOF: requested {} consume bytes, but cursor has only {}",
                    bytes, available
                ),
            ));
        }
        self.set_position((pos + bytes) as u64);
        Ok(())
    }

    fn stream_skip(&mut self, bytes: u64) -> Result<u64> {
        let slice = self.get_ref().as_ref();
        let pos = self.position() as usize;
        let available = slice.len().saturating_sub(pos);
        let to_skip = (bytes as usize).min(available);
        self.set_position((pos + to_skip) as u64);
        Ok(to_skip as u64)
    }
}

impl<T: LookaheadRead + ?Sized> LookaheadRead for &mut T {
    #[inline]
    fn peek_ahead(&mut self, min_bytes: usize) -> Result<&[u8]> {
        (**self).peek_ahead(min_bytes)
    }

    #[inline]
    fn consume(&mut self, bytes: usize) -> Result<()> {
        (**self).consume(bytes)
    }

    #[inline]
    fn stream_skip(&mut self, bytes: u64) -> Result<u64> {
        (**self).stream_skip(bytes)
    }
}

impl<T: LookaheadRead + ?Sized> LookaheadRead for Box<T> {
    #[inline]
    fn peek_ahead(&mut self, min_bytes: usize) -> Result<&[u8]> {
        (**self).peek_ahead(min_bytes)
    }

    #[inline]
    fn consume(&mut self, bytes: usize) -> Result<()> {
        (**self).consume(bytes)
    }

    #[inline]
    fn stream_skip(&mut self, bytes: u64) -> Result<u64> {
        (**self).stream_skip(bytes)
    }
}
