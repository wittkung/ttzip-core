// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! StreamFilter abstraction for multi-layer transformation pipelines.

use std::io::Result;

/// An in-stream transformation filter (e.g. cipher, decompressor, byte filter, checksum).
pub trait StreamFilter: Send {
    /// Transforms `input` bytes and writes processed bytes into `output`.
    /// Returns `(consumed_input_bytes, produced_output_bytes)`.
    fn transform(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize)>;

    /// Flushes any buffered state into `output`.
    /// Returns `produced_output_bytes`.
    fn flush(&mut self, output: &mut [u8]) -> Result<usize>;

    /// Finalizes the stream filter, producing any terminal bytes (e.g. padding, trailer).
    /// Returns `produced_output_bytes`.
    fn finish(&mut self, output: &mut [u8]) -> Result<usize>;
}

/// A transparent pass-through filter.
#[derive(Debug, Default, Clone, Copy)]
pub struct IdentityFilter;

impl StreamFilter for IdentityFilter {
    #[inline]
    fn transform(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize)> {
        let to_copy = input.len().min(output.len());
        output[..to_copy].copy_from_slice(&input[..to_copy]);
        Ok((to_copy, to_copy))
    }

    #[inline]
    fn flush(&mut self, _output: &mut [u8]) -> Result<usize> {
        Ok(0)
    }

    #[inline]
    fn finish(&mut self, _output: &mut [u8]) -> Result<usize> {
        Ok(0)
    }
}

/// A filter that performs byte-by-byte transformation using a closure or function.
pub struct ByteMapFilter<F: Fn(u8) -> u8 + Send> {
    map_fn: F,
}

impl<F: Fn(u8) -> u8 + Send> ByteMapFilter<F> {
    /// Constructs a new ByteMapFilter with the given byte transformation function.
    pub fn new(map_fn: F) -> Self {
        Self { map_fn }
    }
}

impl<F: Fn(u8) -> u8 + Send> StreamFilter for ByteMapFilter<F> {
    #[inline]
    fn transform(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize)> {
        let count = input.len().min(output.len());
        for i in 0..count {
            output[i] = (self.map_fn)(input[i]);
        }
        Ok((count, count))
    }

    #[inline]
    fn flush(&mut self, _output: &mut [u8]) -> Result<usize> {
        Ok(0)
    }

    #[inline]
    fn finish(&mut self, _output: &mut [u8]) -> Result<usize> {
        Ok(0)
    }
}
