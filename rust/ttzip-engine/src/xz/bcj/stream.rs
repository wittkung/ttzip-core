// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-allocation streaming sliding window wrapper for BCJ hardware instruction filters.
//!
//! Handles arbitrary chunk boundary slicing, lookahead buffer preservation across chunk edges,
//! continuous global Program Counter (`now_pos`) accumulation, and final stream tail flushing.

use super::BranchFilter;

/// Sliding window streaming wrapper around any architecture BCJ filter.
#[derive(Debug, Clone)]
pub struct BcjStreamFilter<F: BranchFilter> {
    /// Inner architecture hardware instruction filter.
    filter: F,
    /// Cumulative uncompressed byte position (program counter offset).
    now_pos: u32,
    /// Internal temporary lookahead buffer for preserving trailing unfiltered bytes.
    temp_buf: [u8; 32],
    /// Number of valid unfiltered bytes currently held in `temp_buf`.
    temp_len: usize,
}

impl<F: BranchFilter> BcjStreamFilter<F> {
    /// Creates a new streaming filter wrapper with a specified initial start offset.
    pub fn new(filter: F, start_offset: u32) -> Self {
        Self {
            filter,
            now_pos: start_offset,
            temp_buf: [0u8; 32],
            temp_len: 0,
        }
    }

    /// Returns a reference to the inner architecture filter.
    #[inline]
    pub fn filter(&self) -> &F {
        &self.filter
    }

    /// Returns a mutable reference to the inner architecture filter.
    #[inline]
    pub fn filter_mut(&mut self) -> &mut F {
        &mut self.filter
    }

    /// Returns the current stream offset (`now_pos`).
    #[inline]
    pub fn now_pos(&self) -> u32 {
        self.now_pos
    }

    /// Resets the streaming state machine and reposition the stream offset.
    pub fn reset(&mut self, start_offset: u32) {
        self.filter.reset();
        self.now_pos = start_offset;
        self.temp_len = 0;
    }

    /// Feeds an input data chunk and returns all newly filtered bytes.
    ///
    /// Preserves any trailing unfiltered bytes (within the filter's lookahead window)
    /// in the internal sliding buffer to be processed with subsequent chunks.
    pub fn process_chunk(&mut self, input: &[u8], is_encoder: bool) -> Vec<u8> {
        if input.is_empty() && self.temp_len == 0 {
            return Vec::new();
        }

        let mut work = Vec::with_capacity(self.temp_len + input.len());
        if self.temp_len > 0 {
            work.extend_from_slice(&self.temp_buf[..self.temp_len]);
        }
        work.extend_from_slice(input);

        let filtered = if is_encoder {
            self.filter.encode(&mut work, self.now_pos)
        } else {
            self.filter.decode(&mut work, self.now_pos)
        };

        self.now_pos = self.now_pos.wrapping_add(filtered as u32);
        let unfiltered = work.len() - filtered;

        if unfiltered > self.temp_buf.len() {
            // Safety clamp: should never exceed maximum filter lookahead (8 bytes)
            let keep = unfiltered.min(self.temp_buf.len());
            self.temp_buf[..keep].copy_from_slice(&work[work.len() - keep..]);
            self.temp_len = keep;
        } else {
            self.temp_buf[..unfiltered].copy_from_slice(&work[filtered..]);
            self.temp_len = unfiltered;
        }

        work.truncate(filtered);
        work
    }

    /// Flushes any remaining lookahead bytes at the end of the stream without transformation.
    pub fn finish(&mut self) -> Vec<u8> {
        let tail = self.temp_buf[..self.temp_len].to_vec();
        self.now_pos = self.now_pos.wrapping_add(self.temp_len as u32);
        self.temp_len = 0;
        tail
    }

    /// Processes the entire input buffer in one shot, including final lookahead flush.
    pub fn process_all(&mut self, input: &[u8], is_encoder: bool) -> Vec<u8> {
        let mut out = self.process_chunk(input, is_encoder);
        out.extend_from_slice(&self.finish());
        out
    }

    /// Applies the filter directly in-place to a single contiguous memory slice.
    ///
    /// Returns the number of bytes filtered.
    pub fn filter_slice(&mut self, buf: &mut [u8], is_encoder: bool) -> usize {
        let filtered = if is_encoder {
            self.filter.encode(buf, self.now_pos)
        } else {
            self.filter.decode(buf, self.now_pos)
        };
        self.now_pos = self.now_pos.wrapping_add(filtered as u32);
        filtered
    }
}
