// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Thread-local Match Builder with 4-Byte L1 Character Caching, RLE Folding, and Small List Pruning.

use super::types::{
    RadixBuildMatch, RadixListTail, MAX_BRUTE_FORCE_LIST_SIZE, MAX_REPEAT, RADIX8_TABLE_SIZE,
    RADIX_LINK_BITS, RADIX_LINK_MASK, RADIX_MAX_LENGTH, RADIX_NULL_LINK,
};
use std::cmp::min;

/// Thread-local match builder containing working buffers and sub-stacks.
pub struct RadixLocalBuilder {
    match_buffer: Vec<RadixBuildMatch>,
    tails_8: [RadixListTail; RADIX8_TABLE_SIZE],
    stack: Vec<(usize, usize, u32)>, // (start_idx, count, depth)
    max_depth: u32,
}

impl RadixLocalBuilder {
    /// Creates a new thread-local builder with pre-allocated buffers.
    pub fn new(max_depth: u32) -> Self {
        Self {
            match_buffer: Vec::with_capacity(1024),
            tails_8: [RadixListTail::default(); RADIX8_TABLE_SIZE],
            stack: Vec::with_capacity(256),
            max_depth,
        }
    }

    /// Processes a single 16-bit bucket directly modifying the match table.
    pub fn process_bucket(
        &mut self,
        data: &[u8],
        table: &mut [u32],
        head: u32,
        count: u32,
    ) {
        if count <= MAX_BRUTE_FORCE_LIST_SIZE as u32 {
            self.brute_force_table(data, table, head, count as usize, 2);
            return;
        }

        self.match_buffer.clear();
        let mut curr = head;
        let mut loaded = 0usize;

        while curr != RADIX_NULL_LINK && loaded < count as usize {
            let from = curr;
            let next_raw = table[curr as usize];
            let next_link = if next_raw == RADIX_NULL_LINK {
                RADIX_NULL_LINK
            } else {
                next_raw & RADIX_LINK_MASK
            };

            let mut node = RadixBuildMatch::new(from, 0, (loaded + 1) as u32, 2);
            node.load_src_u32(data, (from as usize) + 2);
            self.match_buffer.push(node);

            curr = next_link;
            loaded += 1;
        }

        if loaded < 2 {
            return;
        }

        self.expand_buffer(data);

        // Commit buffered results back to the table
        for node in &self.match_buffer {
            let from = node.from as usize;
            let next_idx = node.next_index();
            let depth = node.depth().min(RADIX_MAX_LENGTH);

            if next_idx < self.match_buffer.len() && depth >= 2 {
                let target_from = self.match_buffer[next_idx].from;
                table[from] = (target_from & RADIX_LINK_MASK) | (depth << RADIX_LINK_BITS);
            }
        }
    }

    /// Processes a single 16-bit bucket and records modified entries to an output vector.
    pub fn process_bucket_collect(
        &mut self,
        data: &[u8],
        table: &[u32],
        head: u32,
        count: u32,
        out_updates: &mut Vec<(usize, u32)>,
    ) {
        if count <= MAX_BRUTE_FORCE_LIST_SIZE as u32 {
            self.brute_force_collect(data, table, head, count as usize, 2, out_updates);
            return;
        }

        self.match_buffer.clear();
        let mut curr = head;
        let mut loaded = 0usize;

        while curr != RADIX_NULL_LINK && loaded < count as usize {
            let from = curr;
            let next_raw = table[curr as usize];
            let next_link = if next_raw == RADIX_NULL_LINK {
                RADIX_NULL_LINK
            } else {
                next_raw & RADIX_LINK_MASK
            };

            let mut node = RadixBuildMatch::new(from, 0, (loaded + 1) as u32, 2);
            node.load_src_u32(data, (from as usize) + 2);
            self.match_buffer.push(node);

            curr = next_link;
            loaded += 1;
        }

        if loaded < 2 {
            return;
        }

        self.expand_buffer(data);

        for node in &self.match_buffer {
            let from = node.from as usize;
            let next_idx = node.next_index();
            let depth = node.depth().min(RADIX_MAX_LENGTH);

            if next_idx < self.match_buffer.len() && depth >= 2 {
                let target_from = self.match_buffer[next_idx].from;
                let val = (target_from & RADIX_LINK_MASK) | (depth << RADIX_LINK_BITS);
                out_updates.push((from, val));
            }
        }
    }

    /// Recursively/iteratively expands the match buffer using 4-byte L1 cached bucketing.
    fn expand_buffer(&mut self, data: &[u8]) {
        let max_depth = self.max_depth;
        self.stack.clear();
        self.stack.push((0, self.match_buffer.len(), 2));

        while let Some((start_pos, list_count, depth)) = self.stack.pop() {
            if list_count < 2 || depth >= max_depth {
                continue;
            }

            if list_count <= MAX_BRUTE_FORCE_LIST_SIZE {
                self.brute_force_buffer_slice(data, start_pos, list_count, depth);
                continue;
            }

            // RLE check for repeating sequences
            if self.detect_and_fold_rle(data, start_pos, list_count, depth) {
                continue;
            }

            let slot = ((depth - 2) & 3) as usize;
            let next_depth = depth + 1;

            // Reset tails
            for tail in &mut self.tails_8 {
                tail.prev_index = RADIX_NULL_LINK;
                tail.list_count = 0;
            }

            let mut sub_buckets: Vec<(usize, u8)> = Vec::with_capacity(32);
            let mut curr_idx = start_pos;

            for _ in 0..list_count {
                let char_val = self.match_buffer[curr_idx].byte_at(slot);
                let prev = self.tails_8[char_val as usize].prev_index;
                self.tails_8[char_val as usize].prev_index = curr_idx as u32;

                if prev != RADIX_NULL_LINK {
                    self.tails_8[char_val as usize].list_count += 1;
                    self.match_buffer[prev as usize]
                        .set_next_and_depth(curr_idx as u32, next_depth);
                } else {
                    self.tails_8[char_val as usize].list_count = 1;
                    sub_buckets.push((curr_idx, char_val));
                }

                // If slot == 3 and advancing, prefetch the next 4 bytes from memory
                if slot == 3 && next_depth < max_depth {
                    let from = self.match_buffer[curr_idx].from as usize;
                    self.match_buffer[curr_idx].load_src_u32(data, from + next_depth as usize);
                }

                curr_idx = self.match_buffer[curr_idx].next_index();
                if curr_idx >= self.match_buffer.len() {
                    break;
                }
            }

            for (sub_head, char_val) in sub_buckets {
                let sub_count = self.tails_8[char_val as usize].list_count as usize;
                if sub_count >= 2 {
                    self.stack.push((sub_head, sub_count, next_depth));
                }
            }
        }
    }

    /// Detects consecutive runs (distance 1 or 2) and folds them without deep stack recursion.
    fn detect_and_fold_rle(
        &mut self,
        data: &[u8],
        start_pos: usize,
        list_count: usize,
        depth: u32,
    ) -> bool {
        if list_count < MAX_REPEAT {
            return false;
        }

        let curr_idx = start_pos;
        let mut prev_from = self.match_buffer[curr_idx].from as usize;
        let mut dist_1_count = 0usize;
        let mut dist_2_count = 0usize;

        let mut next_idx = self.match_buffer[curr_idx].next_index();
        let mut steps = 1usize;

        while next_idx < self.match_buffer.len() && steps < list_count {
            let curr_from = self.match_buffer[next_idx].from as usize;
            let dist = prev_from.saturating_sub(curr_from);
            if dist == 1 {
                dist_1_count += 1;
            } else if dist == 2 {
                dist_2_count += 1;
            }
            prev_from = curr_from;
            next_idx = self.match_buffer[next_idx].next_index();
            steps += 1;
        }

        let is_rle_1 = dist_1_count >= list_count - 2;
        let is_rle_2 = dist_2_count >= list_count - 2;

        if !is_rle_1 && !is_rle_2 {
            return false;
        }

        let dist = if is_rle_1 { 1 } else { 2 };
        let mut idx = start_pos;
        let max_depth = self.max_depth;

        for _ in 0..list_count {
            let from = self.match_buffer[idx].from as usize;
            let target_from = from.saturating_sub(dist);

            // Compute exact common match length against prior repeated bytes
            let mut match_len = depth as usize;
            let limit = min(
                max_depth as usize,
                min(data.len() - from, data.len() - target_from),
            );

            while match_len < limit && data[from + match_len] == data[target_from + match_len] {
                match_len += 1;
            }

            let next_buf = self.match_buffer[idx].next_index();
            self.match_buffer[idx].set_next_and_depth(next_buf as u32, match_len as u32);
            idx = next_buf;
            if idx >= self.match_buffer.len() {
                break;
            }
        }

        true
    }

    /// Performs pairwise brute-force comparison on a small slice within `match_buffer`.
    fn brute_force_buffer_slice(
        &mut self,
        data: &[u8],
        start_pos: usize,
        list_count: usize,
        depth: u32,
    ) {
        let mut indices = [0usize; MAX_BRUTE_FORCE_LIST_SIZE + 1];
        let count = min(list_count, MAX_BRUTE_FORCE_LIST_SIZE);
        let mut curr = start_pos;

        for i in 0..count {
            indices[i] = curr;
            curr = self.match_buffer[curr].next_index();
            if curr >= self.match_buffer.len() {
                break;
            }
        }

        let limit = (self.max_depth - depth) as usize;

        for i in 0..count.saturating_sub(1) {
            let idx_i = indices[i];
            let from_i = self.match_buffer[idx_i].from as usize;
            let mut best_len = 0usize;
            let mut best_target_idx = indices[i + 1];

            for j in (i + 1)..count {
                let idx_j = indices[j];
                let from_j = self.match_buffer[idx_j].from as usize;

                let max_avail = min(
                    limit,
                    min(
                        data.len().saturating_sub(from_i + depth as usize),
                        data.len().saturating_sub(from_j + depth as usize),
                    ),
                );
                let mut match_len = 0usize;

                let src_i = &data[from_i + depth as usize..from_i + depth as usize + max_avail];
                let src_j = &data[from_j + depth as usize..from_j + depth as usize + max_avail];

                while match_len < max_avail && src_i[match_len] == src_j[match_len] {
                    match_len += 1;
                }

                if match_len > best_len {
                    best_len = match_len;
                    best_target_idx = idx_j;
                    if match_len >= limit {
                        break;
                    }
                }
            }

            let total_len = depth as usize + best_len;
            self.match_buffer[idx_i].set_next_and_depth(best_target_idx as u32, total_len as u32);
        }
    }

    /// Performs brute-force matching directly on the `table` slice.
    fn brute_force_table(
        &self,
        data: &[u8],
        table: &mut [u32],
        head: u32,
        count: usize,
        depth: u32,
    ) {
        let mut positions = [0usize; MAX_BRUTE_FORCE_LIST_SIZE + 1];
        let n = min(count, MAX_BRUTE_FORCE_LIST_SIZE);
        let mut curr = head;

        for i in 0..n {
            positions[i] = curr as usize;
            let next_raw = table[curr as usize];
            if next_raw == RADIX_NULL_LINK {
                break;
            }
            curr = next_raw & RADIX_LINK_MASK;
        }

        let limit = (self.max_depth - depth) as usize;

        for i in 0..n.saturating_sub(1) {
            let pos_i = positions[i];
            let mut best_len = 0usize;
            let mut best_target = positions[i + 1];

            for j in (i + 1)..n {
                let pos_j = positions[j];
                let max_avail = min(
                    limit,
                    min(
                        data.len().saturating_sub(pos_i + depth as usize),
                        data.len().saturating_sub(pos_j + depth as usize),
                    ),
                );

                let src_i = &data[pos_i + depth as usize..pos_i + depth as usize + max_avail];
                let src_j = &data[pos_j + depth as usize..pos_j + depth as usize + max_avail];
                let mut match_len = 0usize;

                while match_len < max_avail && src_i[match_len] == src_j[match_len] {
                    match_len += 1;
                }

                if match_len > best_len {
                    best_len = match_len;
                    best_target = pos_j;
                    if match_len >= limit {
                        break;
                    }
                }
            }

            let total_len = (depth as usize + best_len).min(RADIX_MAX_LENGTH as usize);
            table[pos_i] =
                (best_target as u32 & RADIX_LINK_MASK) | ((total_len as u32) << RADIX_LINK_BITS);
        }
    }

    /// Performs brute-force matching and records results into `out_updates`.
    fn brute_force_collect(
        &self,
        data: &[u8],
        table: &[u32],
        head: u32,
        count: usize,
        depth: u32,
        out_updates: &mut Vec<(usize, u32)>,
    ) {
        let mut positions = [0usize; MAX_BRUTE_FORCE_LIST_SIZE + 1];
        let n = min(count, MAX_BRUTE_FORCE_LIST_SIZE);
        let mut curr = head;

        for i in 0..n {
            positions[i] = curr as usize;
            let next_raw = table[curr as usize];
            if next_raw == RADIX_NULL_LINK {
                break;
            }
            curr = next_raw & RADIX_LINK_MASK;
        }

        let limit = (self.max_depth - depth) as usize;

        for i in 0..n.saturating_sub(1) {
            let pos_i = positions[i];
            let mut best_len = 0usize;
            let mut best_target = positions[i + 1];

            for j in (i + 1)..n {
                let pos_j = positions[j];
                let max_avail = min(
                    limit,
                    min(
                        data.len().saturating_sub(pos_i + depth as usize),
                        data.len().saturating_sub(pos_j + depth as usize),
                    ),
                );

                let src_i = &data[pos_i + depth as usize..pos_i + depth as usize + max_avail];
                let src_j = &data[pos_j + depth as usize..pos_j + depth as usize + max_avail];
                let mut match_len = 0usize;

                while match_len < max_avail && src_i[match_len] == src_j[match_len] {
                    match_len += 1;
                }

                if match_len > best_len {
                    best_len = match_len;
                    best_target = pos_j;
                    if match_len >= limit {
                        break;
                    }
                }
            }

            let total_len = (depth as usize + best_len).min(RADIX_MAX_LENGTH as usize);
            let val = (best_target as u32 & RADIX_LINK_MASK) | ((total_len as u32) << RADIX_LINK_BITS);
            out_updates.push((pos_i, val));
        }
    }
}
