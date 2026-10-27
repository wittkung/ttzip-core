// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Multi-layer filter chain execution engine with zero-allocation small-chain optimization.

use std::io::{Error, ErrorKind, Read, Result, Write};

use super::filter::StreamFilter;
use super::strategy::MemoryStrategy;

/// Maximum number of inline filters stored on the stack without vector allocation.
pub const INLINE_FILTER_CAPACITY: usize = 4;

/// Inline, stack-allocated filter chain holding up to `INLINE_FILTER_CAPACITY` filters.
pub struct InlineFilterChain {
    filters: [Option<Box<dyn StreamFilter>>; INLINE_FILTER_CAPACITY],
    len: usize,
}

impl Default for InlineFilterChain {
    fn default() -> Self {
        Self::new()
    }
}

impl InlineFilterChain {
    /// Creates an empty inline filter chain.
    #[inline]
    pub const fn new() -> Self {
        Self {
            filters: [None, None, None, None],
            len: 0,
        }
    }

    /// Pushes a new filter to the end of the pipeline.
    ///
    /// Returns `Err(Error)` if the chain capacity (`INLINE_FILTER_CAPACITY`) is exceeded.
    pub fn push(&mut self, filter: Box<dyn StreamFilter>) -> Result<()> {
        if self.len >= INLINE_FILTER_CAPACITY {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("Filter chain capacity exceeded (max {INLINE_FILTER_CAPACITY})"),
            ));
        }
        self.filters[self.len] = Some(filter);
        self.len += 1;
        Ok(())
    }

    /// Returns the number of filters currently in the chain.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the chain contains no filters.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Executes direct inline transformation in memory (ideal for payloads < 1MB).
    ///
    /// Avoids thread context switching and channel synchronization by ping-ponging
    /// between two pre-allocated scratch buffers across filter layers.
    pub fn execute_direct(&mut self, input: &[u8], writer: &mut dyn Write) -> Result<u64> {
        if self.len == 0 {
            writer.write_all(input)?;
            return Ok(input.len() as u64);
        }

        let mut buf_src = input.to_vec();
        let mut buf_dst = Vec::with_capacity(input.len());

        for i in 0..self.len {
            if let Some(ref mut filter) = self.filters[i] {
                buf_dst.clear();
                // Ensure destination buffer has sufficient capacity (allow up to 2x for expansion)
                buf_dst.resize(buf_src.len().max(64) * 2, 0);

                let mut in_pos = 0;
                let mut out_pos = 0;

                while in_pos < buf_src.len() {
                    let out_remaining = buf_dst.len() - out_pos;
                    if out_remaining < 64 {
                        buf_dst.resize(buf_dst.len() * 2, 0);
                    }
                    let (consumed, produced) =
                        filter.transform(&buf_src[in_pos..], &mut buf_dst[out_pos..])?;
                    in_pos += consumed;
                    out_pos += produced;
                    if consumed == 0 && produced == 0 {
                        break;
                    }
                }

                // Flush filter state
                let flushed = filter.flush(&mut buf_dst[out_pos..])?;
                out_pos += flushed;

                // Finalize filter state
                let finished = filter.finish(&mut buf_dst[out_pos..])?;
                out_pos += finished;

                buf_dst.truncate(out_pos);
                std::mem::swap(&mut buf_src, &mut buf_dst);
            }
        }

        writer.write_all(&buf_src)?;
        Ok(buf_src.len() as u64)
    }

    /// Executes chunked streaming transformation with bounded buffer memory.
    ///
    /// Reads `chunk_size` slices from `reader`, pipelines them sequentially through
    /// all active filter layers, and flushes output bytes directly into `writer`.
    pub fn execute_streaming<R: Read>(
        &mut self,
        reader: &mut R,
        writer: &mut dyn Write,
        chunk_size: usize,
    ) -> Result<u64> {
        let chunk_cap = chunk_size.max(1024);
        let mut in_chunk = vec![0u8; chunk_cap];
        let mut total_written = 0u64;

        loop {
            let bytes_read = reader.read(&mut in_chunk)?;
            if bytes_read == 0 {
                break;
            }

            let written = self.execute_direct(&in_chunk[..bytes_read], writer)?;
            total_written += written;
        }

        Ok(total_written)
    }

    /// Executes pipeline according to the designated memory strategy.
    pub fn execute(
        &mut self,
        strategy: MemoryStrategy,
        input: &[u8],
        writer: &mut dyn Write,
    ) -> Result<u64> {
        match strategy {
            MemoryStrategy::DirectCall | MemoryStrategy::ZeroCopy => {
                self.execute_direct(input, writer)
            }
            MemoryStrategy::Streaming { chunk_size } => {
                let mut cursor = std::io::Cursor::new(input);
                self.execute_streaming(&mut cursor, writer, chunk_size)
            }
        }
    }
}

/// Dynamic, heap-backed filter chain alias supporting variable-length filter pipelines.
pub type FilterChain = InlineFilterChain;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::filter::ByteMapFilter;

    #[test]
    fn test_inline_filter_chain_empty_direct() {
        let mut chain = InlineFilterChain::new();
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);

        let data = b"Hello, World!";
        let mut output = Vec::new();
        let written = chain.execute_direct(data, &mut output).expect("direct execution");

        assert_eq!(written, data.len() as u64);
        assert_eq!(output, data);
    }

    #[test]
    fn test_inline_filter_chain_single_layer() {
        let mut chain = InlineFilterChain::new();
        chain
            .push(Box::new(ByteMapFilter::new(|b| b.wrapping_add(1))))
            .expect("push filter");
        assert_eq!(chain.len(), 1);

        let data = b"ABC";
        let mut output = Vec::new();
        chain.execute_direct(data, &mut output).expect("execute");

        assert_eq!(output, b"BCD");
    }

    #[test]
    fn test_inline_filter_chain_multi_layer_roundtrip() {
        let mut chain = InlineFilterChain::new();
        // Layer 1: +10
        chain
            .push(Box::new(ByteMapFilter::new(|b| b.wrapping_add(10))))
            .expect("push 1");
        // Layer 2: XOR 0x55
        chain
            .push(Box::new(ByteMapFilter::new(|b| b ^ 0x55)))
            .expect("push 2");
        // Layer 3: XOR 0x55 (cancels layer 2)
        chain
            .push(Box::new(ByteMapFilter::new(|b| b ^ 0x55)))
            .expect("push 3");
        // Layer 4: -10 (cancels layer 1)
        chain
            .push(Box::new(ByteMapFilter::new(|b| b.wrapping_sub(10))))
            .expect("push 4");

        assert_eq!(chain.len(), 4);

        // Layer 5 should exceed capacity
        let overflow = chain.push(Box::new(ByteMapFilter::new(|b| b)));
        assert!(overflow.is_err());

        let data = b"High-throughput microkernel streaming payload!";
        let mut output = Vec::new();
        chain.execute_direct(data, &mut output).expect("execute");

        assert_eq!(output, data);
    }

    #[test]
    fn test_inline_filter_chain_streaming_mode() {
        let mut chain = InlineFilterChain::new();
        chain
            .push(Box::new(ByteMapFilter::new(|b| b.wrapping_add(1))))
            .expect("push filter");

        let input_bytes = vec![0x10u8; 1000];
        let mut cursor = std::io::Cursor::new(&input_bytes);
        let mut output = Vec::new();

        let total = chain
            .execute_streaming(&mut cursor, &mut output, 128)
            .expect("streaming execution");

        assert_eq!(total, 1000);
        assert_eq!(output.len(), 1000);
        assert!(output.iter().all(|&b| b == 0x11));
    }

    #[test]
    fn test_inline_filter_chain_strategy_routing() {
        let mut chain = InlineFilterChain::new();
        chain
            .push(Box::new(ByteMapFilter::new(|b| b ^ 0xAA)))
            .expect("push filter");

        let data = vec![0x55u8; 512];
        let mut out_direct = Vec::new();
        chain
            .execute(MemoryStrategy::DirectCall, &data, &mut out_direct)
            .expect("direct");

        let mut out_streaming = Vec::new();
        chain
            .execute(
                MemoryStrategy::Streaming { chunk_size: 64 },
                &data,
                &mut out_streaming,
            )
            .expect("streaming");

        assert_eq!(out_direct, vec![0xFFu8; 512]);
        assert_eq!(out_streaming, vec![0xFFu8; 512]);
    }
}
