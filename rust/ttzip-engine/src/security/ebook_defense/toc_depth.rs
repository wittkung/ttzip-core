// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 2: Table of Contents (TOC) Recursion Depth & Topology Guard.
//!
//! Intercepts deeply nested navigation trees, stack overflow DoS vectors,
//! total node explosions, and circular reference deadlocks during NCX / NavDoc parsing.

use std::collections::HashSet;

use super::{EbookDefenseError, MAX_TOC_DEPTH, MAX_TOC_NODES};

/// Represents a validated navigation node entry within an e-book's Table of Contents hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocEntry {
    /// Unique identifier for the navigation point.
    pub id: String,
    /// Human-readable title/label of the chapter or section.
    pub title: String,
    /// Target content link (e.g. `chapter1.xhtml#section2`).
    pub target_href: String,
    /// 1-based hierarchy nesting depth level.
    pub depth: usize,
    /// Index of parent node in the linear storage buffer, if not a root node.
    pub parent_idx: Option<usize>,
    /// Indices of immediate child navigation nodes.
    pub children_indices: Vec<usize>,
}

/// Guard enforcing explicit non-recursive traversal, depth ceilings, node count limits,
/// and cycle detection across TOC navigation trees.
#[derive(Debug, Default, Clone)]
pub struct TocRecursionDepthGuard {
    nodes: Vec<TocEntry>,
    active_path_set: HashSet<String>,
}

impl TocRecursionDepthGuard {
    /// Creates a new TOC recursion depth guard with pre-allocated capacity.
    pub fn new() -> Self {
        Self {
            nodes: Vec::with_capacity(64),
            active_path_set: HashSet::with_capacity(MAX_TOC_DEPTH + 1),
        }
    }

    /// Pushes a new node onto the TOC hierarchy while enforcing depth, count, and cycle invariants.
    pub fn push_node(
        &mut self,
        id: String,
        title: String,
        target_href: String,
        depth: usize,
        parent_idx: Option<usize>,
    ) -> Result<usize, EbookDefenseError> {
        if depth > MAX_TOC_DEPTH {
            return Err(EbookDefenseError::TocNestingDepthExceeded {
                depth,
                limit: MAX_TOC_DEPTH,
            });
        }

        if self.nodes.len() >= MAX_TOC_NODES {
            return Err(EbookDefenseError::TocTotalNodesExceeded {
                count: self.nodes.len() + 1,
                limit: MAX_TOC_NODES,
            });
        }

        if title.trim().is_empty() {
            return Err(EbookDefenseError::EmptyNavLabel);
        }

        if self.active_path_set.contains(&id) {
            return Err(EbookDefenseError::TocCyclicReferenceDetected { node_id: id });
        }

        let new_idx = self.nodes.len();
        self.nodes.push(TocEntry {
            id,
            title,
            target_href,
            depth,
            parent_idx,
            children_indices: Vec::new(),
        });

        if let Some(p_idx) = parent_idx {
            if p_idx < self.nodes.len() - 1 {
                self.nodes[p_idx].children_indices.push(new_idx);
            }
        }

        Ok(new_idx)
    }

    /// Enters a navigation branch, recording the node ID in the active cycle-detection path.
    pub fn enter_branch(&mut self, node_id: &str) -> Result<(), EbookDefenseError> {
        if !self.active_path_set.insert(node_id.to_string()) {
            return Err(EbookDefenseError::TocCyclicReferenceDetected {
                node_id: node_id.to_string(),
            });
        }
        Ok(())
    }

    /// Leaves a navigation branch, releasing the node ID from the active cycle-detection path.
    pub fn leave_branch(&mut self, node_id: &str) {
        self.active_path_set.remove(node_id);
    }

    /// Validates an arbitrary depth level against configured safety threshold.
    #[inline]
    pub fn validate_depth(&self, depth: usize) -> Result<(), EbookDefenseError> {
        if depth > MAX_TOC_DEPTH {
            Err(EbookDefenseError::TocNestingDepthExceeded {
                depth,
                limit: MAX_TOC_DEPTH,
            })
        } else {
            Ok(())
        }
    }

    /// Returns a slice of all flattened TOC entries.
    #[inline]
    pub fn entries(&self) -> &[TocEntry] {
        &self.nodes
    }

    /// Returns the total number of registered TOC nodes.
    #[inline]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns whether the TOC tree is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Clears the TOC entries and active path tracking set.
    #[inline]
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.active_path_set.clear();
    }
}
