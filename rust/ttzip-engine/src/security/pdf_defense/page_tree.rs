// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! PDF Page Tree Depth and Structure Guard.
//!
//! Enforces non-recursive iterative stack traversal over PDF Page Tree hierarchies
//! (`/Pages` -> `/Kids`), protecting the runtime against stack overflow attacks
//! caused by arbitrarily deep or cyclic degenerate page trees.

use std::collections::HashSet;

use super::{
    PdfDefenseError, DEFAULT_MAX_PAGE_COUNT, DEFAULT_MAX_PAGE_TREE_DEPTH,
};

/// Type alias for PDF Object Identifier `(object_number, generation_number)`.
pub type ObjectId = (u32, u16);

/// Represents a node within the PDF Page Tree hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageTreeNode {
    /// Intermediate branch node (`/Type /Pages`) containing child nodes in `/Kids`.
    Pages {
        id: ObjectId,
        kids_count: usize,
    },
    /// Terminal leaf page node (`/Type /Page`).
    Page {
        id: ObjectId,
    },
}

/// Inspection summary of the evaluated PDF Page Tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageTreeInspectionResult {
    /// Total count of valid leaf pages (`/Type /Page`) in document order.
    pub page_count: usize,
    /// Maximum nesting depth observed during tree traversal.
    pub max_depth_observed: usize,
    /// Total number of page tree nodes visited (branches + leaves).
    pub total_nodes_visited: usize,
    /// Ordered list of leaf page object identifiers.
    pub leaf_page_ids: Vec<ObjectId>,
}

/// Worklist entry for explicit iterative stack traversal.
#[derive(Debug, Clone, Copy)]
struct WorkItem {
    id: ObjectId,
    depth: usize,
}

/// Guard enforcing iterative, bounded traversal of PDF Page Trees.
#[derive(Debug, Clone)]
pub struct PageTreeDepthGuard {
    /// Maximum allowable tree nesting depth (default: 32).
    max_depth: usize,
    /// Maximum allowable leaf page count (default: 100,000).
    max_pages: usize,
}

impl Default for PageTreeDepthGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl PageTreeDepthGuard {
    /// Creates a new guard with default security thresholds (depth <= 32, pages <= 100,000).
    pub fn new() -> Self {
        Self {
            max_depth: DEFAULT_MAX_PAGE_TREE_DEPTH,
            max_pages: DEFAULT_MAX_PAGE_COUNT,
        }
    }

    /// Creates a new guard with custom depth and page limits.
    pub fn with_limits(max_depth: usize, max_pages: usize) -> Self {
        Self {
            max_depth,
            max_pages,
        }
    }

    /// Returns the configured maximum tree depth.
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Returns the configured maximum page count.
    pub fn max_pages(&self) -> usize {
        self.max_pages
    }

    /// Iteratively resolves and validates the page tree starting from the document Catalog.
    pub fn collect_pages_iterative(
        &self,
        doc: &lopdf::Document,
    ) -> Result<PageTreeInspectionResult, PdfDefenseError> {
        let catalog_id = doc.trailer.get(b"Root").map_err(|_| {
            PdfDefenseError::MalformedPdf {
                reason: "Document trailer missing /Root catalog reference".to_string(),
                offset: None,
            }
        })?;

        let catalog_obj = match catalog_id {
            lopdf::Object::Reference(id) => doc.get_object(*id).map_err(|e| {
                PdfDefenseError::MalformedPdf {
                    reason: format!("Failed to resolve /Root catalog object: {e}"),
                    offset: None,
                }
            })?,
            lopdf::Object::Dictionary(_) => catalog_id,
            _ => {
                return Err(PdfDefenseError::MalformedPdf {
                    reason: "Invalid /Root entry type in trailer".to_string(),
                    offset: None,
                });
            }
        };

        let catalog_dict = catalog_obj.as_dict().map_err(|_| {
            PdfDefenseError::MalformedPdf {
                reason: "/Root object is not a dictionary".to_string(),
                offset: None,
            }
        })?;

        let pages_ref = catalog_dict.get(b"Pages").map_err(|_| {
            PdfDefenseError::MalformedPdf {
                reason: "Catalog dictionary missing /Pages entry".to_string(),
                offset: None,
            }
        })?;

        let root_pages_id = match pages_ref {
            lopdf::Object::Reference(id) => (id.0, id.1),
            _ => {
                return Err(PdfDefenseError::MalformedPdf {
                    reason: "/Pages entry in Catalog is not an indirect reference".to_string(),
                    offset: None,
                });
            }
        };

        self.traverse_from_root(doc, root_pages_id)
    }

    /// Explicit non-recursive DFS traversal of the page tree with cycle and depth detection.
    pub fn traverse_from_root(
        &self,
        doc: &lopdf::Document,
        root_pages_id: ObjectId,
    ) -> Result<PageTreeInspectionResult, PdfDefenseError> {
        let mut worklist: Vec<WorkItem> = Vec::new();
        worklist.push(WorkItem {
            id: root_pages_id,
            depth: 1,
        });

        let mut visited_nodes: HashSet<ObjectId> = HashSet::new();
        let mut leaf_pages: Vec<ObjectId> = Vec::new();
        let mut max_depth_observed = 1;
        let mut total_nodes_visited = 0;

        while let Some(item) = worklist.pop() {
            total_nodes_visited += 1;

            if item.depth > self.max_depth {
                return Err(PdfDefenseError::PageTreeDepthExceeded {
                    depth: item.depth,
                    max_depth: self.max_depth,
                });
            }

            if item.depth > max_depth_observed {
                max_depth_observed = item.depth;
            }

            if !visited_nodes.insert(item.id) {
                return Err(PdfDefenseError::CycleDetected {
                    obj_id: item.id,
                    path: format!("Page tree cyclic reference detected at object {:?}", item.id),
                });
            }

            let obj = doc
                .get_object((item.id.0, item.id.1))
                .map_err(|e| PdfDefenseError::MalformedPdf {
                    reason: format!("Unresolvable page tree node {:?}: {e}", item.id),
                    offset: None,
                })?;

            let dict = match obj {
                lopdf::Object::Dictionary(d) => d,
                lopdf::Object::Stream(s) => &s.dict,
                _ => {
                    return Err(PdfDefenseError::MalformedPdf {
                        reason: format!("Page tree object {:?} is not a dictionary", item.id),
                        offset: None,
                    });
                }
            };

            let node_type = dict
                .get(b"Type")
                .ok()
                .and_then(|o| o.as_name_str().ok())
                .unwrap_or("Page"); // default to Page if omitted on leaves

            if node_type == "Pages" {
                // Branch node: resolve /Kids
                if let Ok(kids_obj) = dict.get(b"Kids") {
                    let kids_arr = kids_obj.as_array().map_err(|_| {
                        PdfDefenseError::MalformedPdf {
                            reason: format!("/Kids entry in Pages node {:?} is not an array", item.id),
                            offset: None,
                        }
                    })?;

                    // Push kids in reverse order so leftmost child is popped first
                    for kid in kids_arr.iter().rev() {
                        if let lopdf::Object::Reference(kid_id) = kid {
                            worklist.push(WorkItem {
                                id: (kid_id.0, kid_id.1),
                                depth: item.depth + 1,
                            });
                        }
                    }
                }
            } else if node_type == "Page" {
                // Leaf node
                leaf_pages.push(item.id);
                if leaf_pages.len() > self.max_pages {
                    return Err(PdfDefenseError::PageCountExceeded {
                        count: leaf_pages.len(),
                        max_count: self.max_pages,
                    });
                }
            }
        }

        Ok(PageTreeInspectionResult {
            page_count: leaf_pages.len(),
            max_depth_observed,
            total_nodes_visited,
            leaf_page_ids: leaf_pages,
        })
    }
}
