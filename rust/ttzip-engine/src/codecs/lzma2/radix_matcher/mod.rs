// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Radix-16 Initial Bucketing and 4-Byte Character L1 Cache Match Finder.
//!
//! Provides ultra-fast string matching for LZMA2 compression:
//! - 65,536-entry (16-bit) initial radix bucketing via single linear scan.
//! - Compact 12-byte [`RadixBuildMatch`] nodes with 4-byte L1 character prefetch caching,
//!   eliminating 75% of main memory access during depth expansion.
//! - Small list brute-force pruning (`MAX_BRUTE_FORCE_LIST_SIZE = 5`) to eliminate recursion overhead.
//! - Run-Length Encoding (RLE) repeat string detection for distance-1 and distance-2 cycles,
//!   guaranteeing $O(N)$ linear convergence and zero stack overflow on homogeneous data.
//! - Scalable single-threaded and multi-threaded parallel table building via Rayon.

pub mod builder;
pub mod types;

pub use builder::RadixLocalBuilder;
pub use types::{
    MatchEntry, RadixBuildMatch, RadixListTail, BUFFER_LINK_MASK, MAX_BRUTE_FORCE_LIST_SIZE,
    MAX_REPEAT, RADIX16_TABLE_SIZE, RADIX8_TABLE_SIZE, RADIX_LINK_BITS, RADIX_LINK_MASK,
    RADIX_MAX_LENGTH, RADIX_NULL_LINK,
};

use rayon::prelude::*;

/// Radix-16 match finder engine.
pub struct RadixMatchFinder {
    /// Initial 16-bit radix bucket heads (pointing to the latest occurrence index in `table`).
    pub list_heads: Box<[u32; RADIX16_TABLE_SIZE]>,
    /// Number of entries accumulated in each 16-bit radix bucket.
    pub list_counts: Box<[u32; RADIX16_TABLE_SIZE]>,
    /// Packed match links and match lengths for each position in the input data.
    pub table: Vec<u32>,
    /// Active task stack containing non-empty 16-bit bucket indices.
    pub stack: Vec<u32>,
    /// Maximum search depth limit.
    pub max_depth: u32,
}

impl Default for RadixMatchFinder {
    fn default() -> Self {
        Self::new()
    }
}

impl RadixMatchFinder {
    /// Creates a new `RadixMatchFinder` with standard default search depth (64).
    pub fn new() -> Self {
        Self::with_max_depth(RADIX_MAX_LENGTH)
    }

    /// Creates a new `RadixMatchFinder` with a custom maximum search depth limit.
    pub fn with_max_depth(max_depth: u32) -> Self {
        let heads = vec![RADIX_NULL_LINK; RADIX16_TABLE_SIZE]
            .into_boxed_slice()
            .try_into()
            .unwrap_or_else(|_| Box::new([RADIX_NULL_LINK; RADIX16_TABLE_SIZE]));
        let counts = vec![0u32; RADIX16_TABLE_SIZE]
            .into_boxed_slice()
            .try_into()
            .unwrap_or_else(|_| Box::new([0u32; RADIX16_TABLE_SIZE]));

        Self {
            list_heads: heads,
            list_counts: counts,
            table: Vec::new(),
            stack: Vec::with_capacity(RADIX16_TABLE_SIZE),
            max_depth: max_depth.clamp(2, RADIX_MAX_LENGTH),
        }
    }

    /// Performs a single linear scan over `data` to initialize the 16-bit radix buckets.
    ///
    /// Non-empty bucket indices are pushed onto `stack` for subsequent depth expansion.
    pub fn init_table(&mut self, data: &[u8]) {
        self.list_heads.fill(RADIX_NULL_LINK);
        self.list_counts.fill(0);
        self.stack.clear();

        let len = data.len();
        self.table.clear();
        self.table.resize(len, RADIX_NULL_LINK);

        if len < 2 {
            return;
        }

        let block_size = len - 1;
        for i in 0..block_size {
            let radix_16 = ((data[i] as usize) << 8) | (data[i + 1] as usize);
            let prev = self.list_heads[radix_16];

            if prev != RADIX_NULL_LINK {
                // Link this position to the previous occurrence with initial length 2
                self.table[i] = (prev & RADIX_LINK_MASK) | (2u32 << RADIX_LINK_BITS);
                self.list_heads[radix_16] = i as u32;
                self.list_counts[radix_16] += 1;
            } else {
                self.table[i] = RADIX_NULL_LINK;
                self.list_heads[radix_16] = i as u32;
                self.list_counts[radix_16] = 1;
                self.stack.push(radix_16 as u32);
            }
        }
    }

    /// Builds the radix match table, expanding 16-bit initial chains up to `max_depth`.
    ///
    /// Supports single-threaded execution when `threads <= 1` or Rayon multi-threading when `threads > 1`.
    pub fn build_table(&mut self, data: &[u8], threads: usize) {
        if data.len() < 2 || self.stack.is_empty() {
            return;
        }

        let max_depth = self.max_depth;

        if threads <= 1 || self.stack.len() < 64 {
            let mut builder = RadixLocalBuilder::new(max_depth);
            for &bucket_idx in &self.stack {
                let count = self.list_counts[bucket_idx as usize];
                if count < 2 {
                    continue;
                }
                let head = self.list_heads[bucket_idx as usize];
                builder.process_bucket(data, &mut self.table, head, count);
            }
        } else {
            // Multi-threaded processing: partition independent buckets across rayon workers
            let bucket_tasks: Vec<(u32, u32)> = self
                .stack
                .iter()
                .filter_map(|&bucket_idx| {
                    let count = self.list_counts[bucket_idx as usize];
                    if count >= 2 {
                        Some((self.list_heads[bucket_idx as usize], count))
                    } else {
                        None
                    }
                })
                .collect();

            // Each worker processes its slice of buckets and produces updates
            let updates: Vec<Vec<(usize, u32)>> = bucket_tasks
                .par_chunks(64)
                .map(|chunk| {
                    let mut local_builder = RadixLocalBuilder::new(max_depth);
                    let mut local_updates = Vec::with_capacity(chunk.len() * 16);
                    for &(head, count) in chunk {
                        local_builder.process_bucket_collect(
                            data,
                            &self.table,
                            head,
                            count,
                            &mut local_updates,
                        );
                    }
                    local_updates
                })
                .collect();

            for chunk_updates in updates {
                for (pos, val) in chunk_updates {
                    self.table[pos] = val;
                }
            }
        }
    }

    /// Retrieves the best match link and length for a given buffer position.
    #[inline(always)]
    pub fn get_match(&self, pos: usize) -> Option<MatchEntry> {
        if pos >= self.table.len() {
            return None;
        }
        let entry = self.table[pos];
        if entry == RADIX_NULL_LINK {
            return None;
        }
        let link = (entry & RADIX_LINK_MASK) as usize;
        let length = (entry >> RADIX_LINK_BITS) as usize;
        if length < 2 {
            None
        } else {
            Some(MatchEntry { link, length })
        }
    }

    /// Returns the raw link index at position `pos`.
    #[inline(always)]
    pub fn get_link(&self, pos: usize) -> u32 {
        if pos >= self.table.len() {
            RADIX_NULL_LINK
        } else {
            let entry = self.table[pos];
            if entry == RADIX_NULL_LINK {
                RADIX_NULL_LINK
            } else {
                entry & RADIX_LINK_MASK
            }
        }
    }

    /// Returns the match length at position `pos`.
    #[inline(always)]
    pub fn get_length(&self, pos: usize) -> u32 {
        if pos >= self.table.len() {
            0
        } else {
            let entry = self.table[pos];
            if entry == RADIX_NULL_LINK {
                0
            } else {
                entry >> RADIX_LINK_BITS
            }
        }
    }
}
