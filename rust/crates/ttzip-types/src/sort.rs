// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use crate::node::{TTWorkspaceEntryNode, TTWORKSPACE_S_IFDIR};
use std::sync::Arc;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum SortKey {
    Name,
    Size,
    ModifiedTime,
    Extension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Generates a sorted permutation vector for a contiguous slice of child nodes.
///
/// Invariant:
/// - Flat CSR stores nodes in lexicographical order (Name + Ascending).
/// - When sort_by == Name and direction == Ascending, returns None (indicating direct O(1) indexing).
/// - For any other sorting criteria, computes a Vec<u32> of relative indices using Rust's pdqsort
///   with zero heap allocation during comparisons.
pub fn compute_permutation_vector(
    children: &[TTWorkspaceEntryNode],
    arena: &[u8],
    sort_by: SortKey,
    direction: SortDirection,
) -> Option<Vec<u32>> {
    if sort_by == SortKey::Name && direction == SortDirection::Ascending {
        return None;
    }

    let mut perm: Vec<u32> = (0..children.len() as u32).collect();

    perm.sort_unstable_by(|&a_idx, &b_idx| {
        let node_a = &children[a_idx as usize];
        let node_b = &children[b_idx as usize];

        // Folders always precede files (Finder / standard desktop UX invariant)
        let is_dir_a = (node_a.file_type & TTWORKSPACE_S_IFDIR) != 0;
        let is_dir_b = (node_b.file_type & TTWORKSPACE_S_IFDIR) != 0;

        if is_dir_a != is_dir_b {
            return if is_dir_a {
                core::cmp::Ordering::Less
            } else {
                core::cmp::Ordering::Greater
            };
        }

        let ord = match sort_by {
            SortKey::Name => {
                let name_a = node_a.name(arena).unwrap_or(b"");
                let name_b = node_b.name(arena).unwrap_or(b"");
                name_a.cmp(name_b)
            }
            SortKey::Size => {
                node_a.uncompressed_size.cmp(&node_b.uncompressed_size)
            }
            SortKey::ModifiedTime => {
                node_a.mtime_epoch_s.cmp(&node_b.mtime_epoch_s)
            }
            SortKey::Extension => {
                let ext_a = extract_extension(node_a.name(arena).unwrap_or(b""));
                let ext_b = extract_extension(node_b.name(arena).unwrap_or(b""));
                ext_a.cmp(ext_b)
            }
        };

        let ord = match direction {
            SortDirection::Ascending => ord,
            SortDirection::Descending => ord.reverse(),
        };

        // Stable secondary tie-breaker: sort by name ascending
        if ord == core::cmp::Ordering::Equal {
            let name_a = node_a.name(arena).unwrap_or(b"");
            let name_b = node_b.name(arena).unwrap_or(b"");
            name_a.cmp(name_b)
        } else {
            ord
        }
    });

    Some(perm)
}

#[inline]
fn extract_extension(name: &[u8]) -> &[u8] {
    if let Some(pos) = name.iter().rposition(|&b| b == b'.') {
        &name[pos + 1..]
    } else {
        b""
    }
}

/// Thread-safe or local cache of permutation vectors keyed by (parent_id, sort_by, direction).
#[derive(Default)]
pub struct DirectorySortCache {
    cache: HashMap<(u32, SortKey, SortDirection), Arc<Vec<u32>>>,
}

impl DirectorySortCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn get_or_compute(
        &mut self,
        parent_id: u32,
        children: &[TTWorkspaceEntryNode],
        arena: &[u8],
        sort_by: SortKey,
        direction: SortDirection,
    ) -> Option<Arc<Vec<u32>>> {
        if sort_by == SortKey::Name && direction == SortDirection::Ascending {
            return None;
        }

        let key = (parent_id, sort_by, direction);
        if let Some(perm) = self.cache.get(&key) {
            return Some(Arc::clone(perm));
        }

        if let Some(perm) = compute_permutation_vector(children, arena, sort_by, direction) {
            let arc = Arc::new(perm);
            self.cache.insert(key, Arc::clone(&arc));
            Some(arc)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }
}
