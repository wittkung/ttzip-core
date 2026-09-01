// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! PDF Indirect Reference Cycle Guard.
//!
//! Intercepts cyclic indirect object references, recursive graph bombs, and object
//! exhaustion attacks by maintaining an active ancestor stack and tracking cumulative
//! object visits across the document reference graph.

use std::collections::HashSet;

use super::{
    PdfDefenseError, DEFAULT_MAX_INDIRECT_DEPTH, DEFAULT_MAX_OBJECT_VISITS,
};

/// Lightweight token representing an active indirect object scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveReferenceScope {
    /// PDF Object Identifier `(object_number, generation_number)`.
    pub obj_id: (u32, u16),
    /// Recursion depth at which this object was entered.
    pub depth: usize,
}

/// Guard protecting against cyclic indirect references and infinite resolution loops in PDF graphs.
#[derive(Debug, Clone)]
pub struct IndirectReferenceCycleGuard {
    /// Active ancestor set along the current resolution branch.
    active_ancestors: HashSet<(u32, u16)>,
    /// Active path stack for diagnostics in error reporting.
    path_stack: Vec<(u32, u16)>,
    /// Maximum allowable recursion depth.
    max_depth: usize,
    /// Maximum allowable cumulative object resolutions.
    max_objects: usize,
    /// Total cumulative objects resolved in this session.
    visited_count: usize,
}

impl Default for IndirectReferenceCycleGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl IndirectReferenceCycleGuard {
    /// Creates a new guard with default security thresholds (depth <= 64, objects <= 100,000).
    pub fn new() -> Self {
        Self {
            active_ancestors: HashSet::new(),
            path_stack: Vec::new(),
            max_depth: DEFAULT_MAX_INDIRECT_DEPTH,
            max_objects: DEFAULT_MAX_OBJECT_VISITS,
            visited_count: 0,
        }
    }

    /// Creates a new guard with customized depth and visit limits.
    pub fn with_limits(max_depth: usize, max_objects: usize) -> Self {
        Self {
            active_ancestors: HashSet::new(),
            path_stack: Vec::new(),
            max_depth,
            max_objects,
            visited_count: 0,
        }
    }

    /// Returns the maximum configured recursion depth.
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Returns the maximum configured cumulative object resolution quota.
    pub fn max_objects(&self) -> usize {
        self.max_objects
    }

    /// Returns the current active recursion depth.
    pub fn current_depth(&self) -> usize {
        self.path_stack.len()
    }

    /// Returns the cumulative count of objects resolved.
    pub fn visited_count(&self) -> usize {
        self.visited_count
    }

    /// Resets the guard state for a new document inspection.
    pub fn reset(&mut self) {
        self.active_ancestors.clear();
        self.path_stack.clear();
        self.visited_count = 0;
    }

    /// Attempts to enter an indirect object reference, validating cycle freedom and recursion depth.
    pub fn enter_object(&mut self, obj_id: (u32, u16)) -> Result<ActiveReferenceScope, PdfDefenseError> {
        // 1. Check for active ancestor cycle
        if self.active_ancestors.contains(&obj_id) {
            let path_str = self
                .path_stack
                .iter()
                .map(|(o, g)| format!("{o}_{g}R"))
                .collect::<Vec<_>>()
                .join(" -> ");
            let cycle_path = format!("{path_str} -> {}_{}R", obj_id.0, obj_id.1);
            return Err(PdfDefenseError::CycleDetected {
                obj_id,
                path: cycle_path,
            });
        }

        // 2. Check depth ceiling
        if self.path_stack.len() >= self.max_depth {
            return Err(PdfDefenseError::MaxRecursionDepthExceeded {
                depth: self.path_stack.len() + 1,
                max_depth: self.max_depth,
            });
        }

        // 3. Check cumulative object visit limit
        if self.visited_count >= self.max_objects {
            return Err(PdfDefenseError::ObjectCountExceeded {
                count: self.visited_count + 1,
                max_count: self.max_objects,
            });
        }

        self.active_ancestors.insert(obj_id);
        self.path_stack.push(obj_id);
        self.visited_count += 1;

        Ok(ActiveReferenceScope {
            obj_id,
            depth: self.path_stack.len(),
        })
    }

    /// Alias for enter_object to enter an indirect object reference scope.
    pub fn enter_object_ref(&mut self, obj_id: (u32, u16)) -> Result<ActiveReferenceScope, PdfDefenseError> {
        self.enter_object(obj_id)
    }

    /// Exits an indirect object reference scope by object identifier.
    pub fn leave_object(&mut self, obj_id: (u32, u16)) {
        self.active_ancestors.remove(&obj_id);
        if let Some(pos) = self.path_stack.iter().rposition(|&id| id == obj_id) {
            self.path_stack.truncate(pos);
        }
    }

    /// Exits an indirect object reference scope using a returned token.
    pub fn leave_scope(&mut self, scope: ActiveReferenceScope) {
        self.leave_object(scope.obj_id);
    }

    /// Executes a closure within an entered indirect object scope, guaranteeing safe cleanup.
    pub fn with_object<R, F>(
        &mut self,
        obj_id: (u32, u16),
        f: F,
    ) -> Result<R, PdfDefenseError>
    where
        F: FnOnce(&mut Self) -> Result<R, PdfDefenseError>,
    {
        let scope = self.enter_object(obj_id)?;
        let res = f(self);
        self.leave_scope(scope);
        res
    }

    /// Validates an entire `lopdf::Document` object graph for cycles and depth limits.
    pub fn validate_lopdf_doc(&mut self, doc: &lopdf::Document) -> Result<usize, PdfDefenseError> {
        self.reset();

        // Validate trailer objects
        for (_k, obj) in doc.trailer.iter() {
            self.validate_lopdf_object(doc, obj)?;
        }

        // Validate all root indirect objects in xref
        for (&(obj_nr, gen_nr), obj) in &doc.objects {
            let obj_id = (obj_nr, gen_nr);
            self.with_object(obj_id, |guard| guard.validate_lopdf_object(doc, obj))?;
        }

        Ok(self.visited_count)
    }

    /// Recursively validates a lopdf Object against cycles and deep recursion.
    fn validate_lopdf_object(
        &mut self,
        doc: &lopdf::Document,
        obj: &lopdf::Object,
    ) -> Result<(), PdfDefenseError> {
        match obj {
            lopdf::Object::Reference(id) => {
                let obj_id = (id.0, id.1);
                self.with_object(obj_id, |guard| {
                    if let Ok(target) = doc.get_object(*id) {
                        guard.validate_lopdf_object(doc, target)?;
                    }
                    Ok(())
                })?;
            }
            lopdf::Object::Array(arr) => {
                for item in arr {
                    self.validate_lopdf_object(doc, item)?;
                }
            }
            lopdf::Object::Dictionary(dict) => {
                for (key, item) in dict.iter() {
                    // Skip standard structural back-pointers (Parent in tree structures, Prev in sibling chains)
                    if key == b"Parent" || key == b"Prev" {
                        continue;
                    }
                    self.validate_lopdf_object(doc, item)?;
                }
            }
            lopdf::Object::Stream(stream) => {
                for (key, item) in stream.dict.iter() {
                    if key == b"Parent" || key == b"Prev" {
                        continue;
                    }
                    self.validate_lopdf_object(doc, item)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
