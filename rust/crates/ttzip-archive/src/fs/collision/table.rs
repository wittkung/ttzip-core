// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Columnar conflict table.

use crate::fs::collision::{CollisionError, ConflictKind, ConflictSummary, DetectionPhase, ItemMeta};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConflictDefect {
    #[error("relative path is empty")]
    EmptyRelativePath,
    #[error("existing name is empty")]
    EmptyExistingName,
    #[error("directory onto directory merges and is not a conflict: {relative_path}")]
    DirectoryOntoDirectory { relative_path: String },
    #[error("relative path recorded twice: {relative_path}")]
    DuplicatePath { relative_path: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewConflict<'a> {
    pub relative_path: &'a str,
    pub existing_name: &'a str,
    pub existing: ItemMeta,
    pub incoming: ItemMeta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConflictRef<'t> {
    pub id: u32,
    pub relative_path: &'t str,
    pub existing_name: &'t str,
    pub kind: ConflictKind,
    pub existing: ItemMeta,
    pub incoming: ItemMeta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollisionRecord {
    pub id: u32,
    pub relative_path: String,
    pub existing_name: String,
    pub kind: ConflictKind,
    pub existing: ItemMeta,
    pub incoming: ItemMeta,
}

impl ConflictRef<'_> {
    pub fn to_record(&self) -> CollisionRecord {
        CollisionRecord {
            id: self.id,
            relative_path: self.relative_path.to_owned(),
            existing_name: self.existing_name.to_owned(),
            kind: self.kind,
            existing: self.existing,
            incoming: self.incoming,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Span {
    start: u32,
    len: u32,
}

#[derive(Debug, Clone, Copy)]
struct Row {
    id: u32,
    path: Span,
    existing_name: Span,
    kind: ConflictKind,
    existing: ItemMeta,
    incoming: ItemMeta,
}

#[derive(Debug, Clone)]
pub struct ConflictTable {
    arena: String,
    rows: Vec<Row>, // sorted by path
    by_id: Vec<usize>, // maps id to row index
    type_mismatch: u32,
}

pub struct ConflictTableBuilder {
    arena: String,
    rows: Vec<Row>,
    type_mismatch: u32,
}

impl ConflictTableBuilder {
    pub fn new() -> Self {
        Self {
            arena: String::new(),
            rows: Vec::new(),
            type_mismatch: 0,
        }
    }
    
    pub fn push(&mut self, item: NewConflict<'_>) -> Result<(), CollisionError> {
        if item.relative_path.is_empty() {
            return Err(ConflictDefect::EmptyRelativePath.into());
        }
        if item.existing_name.is_empty() {
            return Err(ConflictDefect::EmptyExistingName.into());
        }
        let kind = ConflictKind::classify(item.existing.kind, item.incoming.kind)
            .ok_or_else(|| ConflictDefect::DirectoryOntoDirectory { relative_path: item.relative_path.to_owned() })?;
            
        let path = self.intern(item.relative_path)?;
        let existing_name = self.intern(item.existing_name)?;
        
        let id = u32::try_from(self.rows.len()).map_err(|_| CollisionError::CapacityExceeded)?;
        if id == u32::MAX {
            return Err(CollisionError::CapacityExceeded);
        }
        
        if kind == ConflictKind::TypeMismatch {
            self.type_mismatch += 1;
        }
        
        self.rows.push(Row {
            id,
            path,
            existing_name,
            kind,
            existing: item.existing,
            incoming: item.incoming,
        });
        
        Ok(())
    }
    
    fn intern(&mut self, text: &str) -> Result<Span, CollisionError> {
        let start = u32::try_from(self.arena.len()).map_err(|_| CollisionError::CapacityExceeded)?;
        let len = u32::try_from(text.len()).map_err(|_| CollisionError::CapacityExceeded)?;
        start.checked_add(len).ok_or(CollisionError::CapacityExceeded)?;
        self.arena.push_str(text);
        Ok(Span { start, len })
    }
    
    pub fn build(mut self) -> Result<ConflictTable, CollisionError> {
        self.rows.sort_by(|a, b| {
            let path_a = &self.arena.as_bytes()[a.path.start as usize..(a.path.start + a.path.len) as usize];
            let path_b = &self.arena.as_bytes()[b.path.start as usize..(b.path.start + b.path.len) as usize];
            path_a.cmp(path_b)
        });
        
        for i in 1..self.rows.len() {
            let prev = &self.rows[i - 1];
            let curr = &self.rows[i];
            let path_prev = &self.arena[prev.path.start as usize..(prev.path.start + prev.path.len) as usize];
            let path_curr = &self.arena[curr.path.start as usize..(curr.path.start + curr.path.len) as usize];
            if path_prev == path_curr {
                return Err(ConflictDefect::DuplicatePath { relative_path: path_curr.to_owned() }.into());
            }
        }
        
        let mut by_id = vec![0; self.rows.len()];
        for (idx, row) in self.rows.iter().enumerate() {
            by_id[row.id as usize] = idx;
        }
        
        Ok(ConflictTable {
            arena: self.arena,
            rows: self.rows,
            by_id,
            type_mismatch: self.type_mismatch,
        })
    }
}

impl Default for ConflictTableBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConflictTable {
    pub fn builder() -> ConflictTableBuilder {
        ConflictTableBuilder::new()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn get(&self, id: u32) -> Option<ConflictRef<'_>> {
        let idx = *self.by_id.get(id as usize)?;
        let row = &self.rows[idx];
        Some(ConflictRef {
            id,
            relative_path: &self.arena[row.path.start as usize..(row.path.start + row.path.len) as usize],
            existing_name: &self.arena[row.existing_name.start as usize..(row.existing_name.start + row.existing_name.len) as usize],
            kind: row.kind,
            existing: row.existing,
            incoming: row.incoming,
        })
    }
    
    pub fn find_by_path(&self, path: &str) -> Option<CollisionRecord> {
        let idx = self.rows.binary_search_by(|row| {
            let row_path = &self.arena[row.path.start as usize..(row.path.start + row.path.len) as usize];
            row_path.as_bytes().cmp(path.as_bytes())
        }).ok()?;
        
        let row = &self.rows[idx];
        Some(CollisionRecord {
            id: row.id,
            relative_path: self.arena[row.path.start as usize..(row.path.start + row.path.len) as usize].to_owned(),
            existing_name: self.arena[row.existing_name.start as usize..(row.existing_name.start + row.existing_name.len) as usize].to_owned(),
            kind: row.kind,
            existing: row.existing,
            incoming: row.incoming,
        })
    }
    
    pub fn summary(&self, phase: DetectionPhase) -> ConflictSummary {
        ConflictSummary {
            total: self.rows.len() as u32,
            same_type: self.rows.len() as u32 - self.type_mismatch,
            type_mismatch: self.type_mismatch,
            phase,
        }
    }
    
    pub fn iter_from(&self, from_id: u32) -> impl Iterator<Item = ConflictRef<'_>> {
        let mut id = from_id;
        std::iter::from_fn(move || {
            let res = self.get(id);
            if res.is_some() {
                id += 1;
            }
            res
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::fs::collision::ItemKind;
    use super::*;

    fn meta(kind: ItemKind, size: u64) -> ItemMeta {
        ItemMeta { kind, size, mtime_unix_ns: 1_700_000_000_000_000_000 + size as i64 }
    }

    fn file_conflict<'a>(path: &'a str, name: &'a str, size: u64) -> NewConflict<'a> {
        NewConflict { relative_path: path, existing_name: name, existing: meta(ItemKind::File, size), incoming: meta(ItemKind::File, size + 1) }
    }

    #[test]
    fn round_trip_preserves_every_field_and_assigns_insertion_ids() {
        let inputs = [
            file_conflict("docs/report.pdf", "Report.pdf", 10),
            NewConflict { relative_path: "a.txt", existing_name: "a.txt", existing: meta(ItemKind::Directory, 0), incoming: meta(ItemKind::File, 3) },
            file_conflict("caf\u{e9}/n\u{f6}tes.md", "cafe\u{301}", 7),
        ];
        
        let mut builder = ConflictTable::builder();
        for input in &inputs {
            builder.push(*input).unwrap();
        }
        let table = builder.build().expect("valid batch");
        
        assert_eq!(table.len(), 3);
        for (id, input) in (0u32..).zip(inputs.iter()) {
            let got = table.get(id).expect("id in range");
            assert_eq!(got.id, id);
            assert_eq!(got.relative_path, input.relative_path);
            assert_eq!(got.existing_name, input.existing_name);
            assert_eq!(got.existing, input.existing);
            assert_eq!(got.incoming, input.incoming);
            
            let found = table.find_by_path(input.relative_path).unwrap();
            assert_eq!(found.id, id);
        }
        
        assert_eq!(table.get(1).map(|c| c.kind), Some(ConflictKind::TypeMismatch));
        assert_eq!(table.get(3), None);
        assert_eq!(table.find_by_path("docs"), None);
        assert_eq!(table.find_by_path("zzz"), None);
    }
}
