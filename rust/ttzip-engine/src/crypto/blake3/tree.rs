// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! BLAKE3 binary tree hierarchical reduction stack, parent node merger, and geometry partitioning.
//!
//! Provides stack-allocated, zero-heap binary tree reduction supporting arbitrary streaming
//! inputs up to $2^{64}$ bytes ($2^{54} \times 1024$-byte chunks) with Bit-Exact BLAKE3 conformance.

use zeroize::{Zeroize, ZeroizeOnDrop};

use super::constants::{BLOCK_LEN, PARENT};
use super::output::Output;

/// Maximum depth of the BLAKE3 subtree stack.
///
/// Supports inputs up to $2^{64}$ bytes ($2^{54} \times 1024$-byte chunks).
pub const MAX_DEPTH: usize = 54;

/// Capacity of the inline stack array (MAX_DEPTH + 1 = 55).
pub const STACK_CAPACITY: usize = MAX_DEPTH + 1;

/// Given the length in bytes of an input or subtree, returns the number of bytes
/// that belong to its left child subtree.
///
/// Concretely, this returns the largest power-of-two number of bytes that is
/// strictly less than `input_len`. All left subtrees in BLAKE3 are complete
/// and at least as large as their sibling right subtrees.
///
/// # Panics
/// Panics in debug builds if `input_len <= 1024` (single chunks have no children).
#[inline(always)]
pub fn left_subtree_len(input_len: u64) -> u64 {
    debug_assert!(input_len > 1024, "Subtrees with <= 1 chunk cannot be split");
    input_len.div_ceil(2).next_power_of_two()
}

/// Constructs an `Output` representing a parent node from two 32-byte child chaining values.
///
/// The parent block contains the 32-byte left child CV followed by the 32-byte right child CV.
/// Counter is set to 0, block length is 64 bytes, and flags include `PARENT`.
#[inline]
pub fn parent_output(
    left_child: &[u8; 32],
    right_child: &[u8; 32],
    key: &[u32; 8],
    flags: u8,
) -> Output {
    let mut block = [0u8; BLOCK_LEN];
    block[..32].copy_from_slice(left_child);
    block[32..].copy_from_slice(right_child);
    Output {
        input_chaining_value: *key,
        block,
        block_len: BLOCK_LEN as u8,
        counter: 0,
        flags: flags | PARENT,
    }
}

/// Computes the 32-byte chaining value of a parent node constructed from left and right children.
#[inline]
pub fn parent_cv(
    left_child: &[u8; 32],
    right_child: &[u8; 32],
    key: &[u32; 8],
    flags: u8,
) -> [u8; 32] {
    parent_output(left_child, right_child, key, flags).chaining_value()
}

/// Fixed-size stack-allocated reduction stack for BLAKE3 subtree chaining values.
///
/// Holds up to 55 chaining values on the stack with zero heap allocation,
/// enabling incremental and parallel tree reduction for streams up to 2^64 bytes.
#[derive(Clone, Debug, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct TreeStack {
    /// 55-element fixed-size inline array holding chaining values.
    pub stack: [[u8; 32]; STACK_CAPACITY],
    /// Current number of valid chaining values in the stack.
    pub len: usize,
}

impl Default for TreeStack {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl TreeStack {
    /// Creates a new empty `TreeStack`.
    #[inline]
    pub const fn new() -> Self {
        Self {
            stack: [[0u8; 32]; STACK_CAPACITY],
            len: 0,
        }
    }

    /// Returns the number of chaining values currently in the stack.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the stack contains no chaining values.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Clears the stack, setting length to zero and wiping stored chaining values.
    #[inline]
    pub fn clear(&mut self) {
        self.stack.zeroize();
        self.len = 0;
    }

    /// Returns a slice of the active chaining values currently stored in the stack.
    #[inline]
    pub fn as_slice(&self) -> &[[u8; 32]] {
        &self.stack[..self.len]
    }

    /// Pushes a chaining value onto the top of the stack.
    ///
    /// # Panics
    /// Panics if the stack is already full (`len == STACK_CAPACITY`).
    #[inline]
    pub fn push(&mut self, cv: [u8; 32]) {
        assert!(
            self.len < STACK_CAPACITY,
            "TreeStack overflow: stack exceeds 55 entries"
        );
        self.stack[self.len] = cv;
        self.len += 1;
    }

    /// Pops the top chaining value from the stack, or returns `None` if empty.
    #[inline]
    pub fn pop(&mut self) -> Option<[u8; 32]> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            let cv = self.stack[self.len];
            self.stack[self.len].zeroize();
            Some(cv)
        }
    }

    /// Performs lazy merging of subtrees in the stack based on Hamming weight (`count_ones()`).
    ///
    /// The target number of entries remaining on the stack is `total_chunks.count_ones()`.
    /// Merges proceed by popping the top two subtrees (right child, left child), computing their
    /// parent chaining value, and pushing the result back onto the stack until `len == target_len`.
    #[inline]
    pub fn merge_cv_stack(&mut self, total_chunks: u64, key: &[u32; 8], flags: u8) {
        let post_merge_stack_len = total_chunks.count_ones() as usize;
        while self.len > post_merge_stack_len {
            let right_child = self.pop().expect("Right child must exist during merge");
            let left_child = self.pop().expect("Left child must exist during merge");
            let parent = parent_cv(&left_child, &right_child, key, flags);
            self.push(parent);
        }
    }

    /// Folds the entire remaining right spine of the tree with `right_output`.
    ///
    /// Progressively merges the top of the stack as left child with the running right output
    /// until the stack is exhausted, producing the top-level parent `Output` (or original output
    /// if the stack was already empty).
    #[inline]
    pub fn fold_right_spine(
        &mut self,
        mut right_output: Output,
        key: &[u32; 8],
        flags: u8,
    ) -> Output {
        while let Some(left_child) = self.pop() {
            let right_cv = right_output.chaining_value();
            right_output = parent_output(&left_child, &right_cv, key, flags);
        }
        right_output
    }
}
