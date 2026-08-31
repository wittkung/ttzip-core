// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Sparse file extent descriptor, boundary coalescing, and non-sparse normalization.

/// Sparse file continuous data block descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SparseExtent {
    pub offset: u64,
    pub length: u64,
}

impl SparseExtent {
    #[inline]
    pub const fn new(offset: u64, length: u64) -> Self {
        Self { offset, length }
    }

    #[inline]
    pub const fn end_offset(&self) -> u64 {
        self.offset.saturating_add(self.length)
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Checks if this extent is contiguous or overlapping with `other`.
    #[inline]
    pub fn can_coalesce_with(&self, other: &Self) -> bool {
        self.offset <= other.end_offset() && other.offset <= self.end_offset()
    }

    /// Merges two overlapping or adjacent extents into a single bounding extent.
    pub fn coalesce_with(&self, other: &Self) -> Option<Self> {
        if self.can_coalesce_with(other) {
            let start = self.offset.min(other.offset);
            let end = self.end_offset().max(other.end_offset());
            Some(Self {
                offset: start,
                length: end.saturating_sub(start),
            })
        } else {
            None
        }
    }
}

/// Merges adjacent and overlapping extents in-place, eliminating empty blocks.
pub fn coalesce_sparse_extents(extents: &mut Vec<SparseExtent>) {
    extents.retain(|e| !e.is_empty());
    if extents.len() <= 1 {
        return;
    }

    extents.sort_unstable_by_key(|e| (e.offset, e.length));

    let mut merged = Vec::with_capacity(extents.len());
    let mut current = extents[0];

    for next in extents.iter().skip(1) {
        if let Some(combined) = current.coalesce_with(next) {
            current = combined;
        } else {
            merged.push(current);
            current = *next;
        }
    }
    merged.push(current);
    *extents = merged;
}

/// Normalizes sparse extents against total file size and eliminates non-sparse degenerations.
///
/// Returns `true` if the file contains genuine holes (sparse), or `false` if dense.
pub fn clean_sparse_extents(extents: &mut Vec<SparseExtent>, total_file_size: u64) -> bool {
    coalesce_sparse_extents(extents);
    if extents.is_empty() {
        return false;
    }

    if extents.len() == 1 && extents[0].offset == 0 && extents[0].length >= total_file_size {
        extents.clear();
        return false;
    }

    let total_bytes: u64 = extents.iter().map(|e| e.length).sum();
    total_bytes < total_file_size
}
