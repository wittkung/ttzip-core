// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 3: Shared String Table (SST) Quota & HashDOS Anti-Exhaustion Guard.
//!
//! Enforces deterministic resource constraints during XLSX Shared String Table parsing:
//! 1. Unique string entry count <= 500,000.
//! 2. Single string entry length <= 32 KiB.
//! 3. Total cumulative string table memory <= 32 MiB.
//! 4. HashDOS collision resistance via cryptographically randomized SipHash hashing state.

use std::collections::HashSet;
use std::io::BufRead;
use quick_xml::events::Event;
use quick_xml::reader::Reader;

use super::{
    OfficeDefenseError, MAX_SST_ENTRY_BYTES, MAX_SST_TOTAL_BYTES, MAX_SST_UNIQUE_ENTRIES,
};

/// Summary report produced after inspecting an SST stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SstInspectionReport {
    pub unique_entries: usize,
    pub total_references: usize,
    pub cumulative_bytes: usize,
}

/// Guard enforcing Shared String Table quotas and memory ceilings.
#[derive(Debug, Clone)]
pub struct SstQuotaGuard {
    max_unique_entries: usize,
    max_entry_bytes: usize,
    max_total_bytes: usize,
    strings: Vec<String>,
    lookup: HashSet<String>,
    cumulative_bytes: usize,
    total_references: usize,
}

impl Default for SstQuotaGuard {
    fn default() -> Self {
        Self::new(
            MAX_SST_UNIQUE_ENTRIES,
            MAX_SST_ENTRY_BYTES,
            MAX_SST_TOTAL_BYTES,
        )
    }
}

impl SstQuotaGuard {
    /// Creates a new SST guard with configured limits.
    pub fn new(max_unique_entries: usize, max_entry_bytes: usize, max_total_bytes: usize) -> Self {
        Self {
            max_unique_entries,
            max_entry_bytes,
            max_total_bytes,
            strings: Vec::new(),
            lookup: HashSet::new(),
            cumulative_bytes: 0,
            total_references: 0,
        }
    }

    /// Validates and registers a single string item into the table.
    pub fn add_entry(&mut self, text: &str) -> Result<u32, OfficeDefenseError> {
        let entry_len = text.len();
        if entry_len > self.max_entry_bytes {
            return Err(OfficeDefenseError::SstEntryTooLarge {
                len: entry_len,
                limit: self.max_entry_bytes,
            });
        }

        self.total_references = self.total_references.saturating_add(1);

        if let Some(idx) = self.lookup.get(text) {
            // Find existing index
            if let Some(pos) = self.strings.iter().position(|s| s == idx) {
                return Ok(pos as u32);
            }
        }

        let new_total = self.cumulative_bytes.saturating_add(entry_len);
        if new_total > self.max_total_bytes {
            return Err(OfficeDefenseError::SstTotalBytesExceeded {
                total: new_total,
                limit: self.max_total_bytes,
            });
        }

        if self.strings.len() >= self.max_unique_entries {
            return Err(OfficeDefenseError::SstUniqueEntriesExceeded {
                count: self.strings.len() + 1,
                limit: self.max_unique_entries,
            });
        }

        let owned = text.to_string();
        self.cumulative_bytes = new_total;
        self.lookup.insert(owned.clone());
        self.strings.push(owned);

        Ok((self.strings.len() - 1) as u32)
    }

    /// Parses an SST XML stream (e.g. `xl/sharedStrings.xml`) with streaming validation.
    pub fn parse_sst_stream<R: BufRead>(
        &mut self,
        reader: R,
    ) -> Result<SstInspectionReport, OfficeDefenseError> {
        let mut xml_reader = Reader::from_reader(reader);
        xml_reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        let mut in_si = false;
        let mut in_t = false;
        let mut current_item_text = String::new();

        loop {
            match xml_reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    let local_name = e.local_name();
                    if local_name.as_ref() == b"si" {
                        in_si = true;
                        current_item_text.clear();
                    } else if in_si && (local_name.as_ref() == b"t" || local_name.as_ref() == b"v") {
                        in_t = true;
                    }
                }
                Ok(Event::Text(ref t)) if in_t => {
                    let txt = t.unescape().map_err(|e| {
                        OfficeDefenseError::MalformedXml(format!("SST text unescape failure: {e}"))
                    })?;
                    current_item_text.push_str(&txt);
                }
                Ok(Event::End(ref e)) => {
                    let local_name = e.local_name();
                    if local_name.as_ref() == b"t" || local_name.as_ref() == b"v" {
                        in_t = false;
                    } else if local_name.as_ref() == b"si" {
                        in_si = false;
                        self.add_entry(&current_item_text)?;
                        current_item_text.clear();
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(OfficeDefenseError::MalformedXml(format!(
                        "SST XML parse error: {e}"
                    )));
                }
                _ => {}
            }
            buf.clear();
        }

        Ok(SstInspectionReport {
            unique_entries: self.strings.len(),
            total_references: self.total_references,
            cumulative_bytes: self.cumulative_bytes,
        })
    }

    /// Retrieves an entry string by index.
    pub fn get(&self, idx: usize) -> Option<&str> {
        self.strings.get(idx).map(|s| s.as_str())
    }

    /// Returns the count of unique strings currently retained.
    pub fn unique_count(&self) -> usize {
        self.strings.len()
    }

    /// Returns the total cumulative memory in bytes retained by stored strings.
    pub fn cumulative_bytes(&self) -> usize {
        self.cumulative_bytes
    }
}
