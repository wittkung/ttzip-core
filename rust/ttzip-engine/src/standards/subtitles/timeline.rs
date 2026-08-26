// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Microsecond-level binary search subtitle timeline indexer.

use crate::standards::subtitles::types::{SubtitleDialogue, SubtitleScript};

/// High-performance time-indexed searchable subtitle timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleTimeline {
    dialogues: Vec<SubtitleDialogue>,
    max_duration_ms: i64,
}

impl SubtitleTimeline {
    /// Builds a new timeline index from a dialogue list, sorting them chronologically.
    pub fn new(mut dialogues: Vec<SubtitleDialogue>) -> Self {
        dialogues.sort_by(|a, b| {
            a.start_ms.cmp(&b.start_ms)
                .then_with(|| a.end_ms.cmp(&b.end_ms))
                .then_with(|| a.layer.cmp(&b.layer))
        });
        let mut max_dur: i64 = 0;
        for d in &dialogues {
            let dur = d.end_ms.saturating_sub(d.start_ms);
            if dur > max_dur { max_dur = dur; }
        }
        Self { dialogues, max_duration_ms: max_dur }
    }

    /// Creates a timeline index from a parsed SubtitleScript AST.
    pub fn from_script(script: &SubtitleScript) -> Self {
        Self::new(script.dialogues.clone())
    }

    /// Binary-search retrieves all active dialogues overlapping the given timestamp in milliseconds.
    pub fn find_active_dialogues(&self, timestamp_ms: i64) -> Vec<SubtitleDialogue> {
        if self.dialogues.is_empty() || timestamp_ms < 0 {
            return Vec::new();
        }

        let upper_bound = self.dialogues.partition_point(|d| d.start_ms <= timestamp_ms);
        let mut active = Vec::new();

        for d in self.dialogues[..upper_bound].iter().rev() {
            if timestamp_ms.saturating_sub(d.start_ms) > self.max_duration_ms {
                break;
            }
            if d.is_active_at(timestamp_ms) {
                active.push(d.clone());
            }
        }

        active.reverse();
        active
    }

    /// Binary-search retrieves all active dialogues overlapping the given timestamp in microseconds.
    pub fn find_active_dialogues_micros(&self, timestamp_us: i64) -> Vec<SubtitleDialogue> {
        if self.dialogues.is_empty() || timestamp_us < 0 {
            return Vec::new();
        }
        let timestamp_ms = timestamp_us / 1000;
        let upper_bound = self.dialogues.partition_point(|d| d.start_ms <= timestamp_ms);
        let mut active = Vec::new();

        for d in self.dialogues[..upper_bound].iter().rev() {
            if timestamp_ms.saturating_sub(d.start_ms) > self.max_duration_ms {
                break;
            }
            if d.is_active_at_micros(timestamp_us) {
                active.push(d.clone());
            }
        }

        active.reverse();
        active
    }

    /// Retrieves all dialogues intersecting the time interval `[start_ms, end_ms)`.
    pub fn find_dialogues_in_range(&self, start_ms: i64, end_ms: i64) -> Vec<SubtitleDialogue> {
        if self.dialogues.is_empty() || end_ms <= start_ms || end_ms < 0 {
            return Vec::new();
        }
        let upper_bound = self.dialogues.partition_point(|d| d.start_ms < end_ms);
        let mut active = Vec::new();

        for d in self.dialogues[..upper_bound].iter().rev() {
            if start_ms.saturating_sub(d.start_ms) > self.max_duration_ms {
                break;
            }
            if d.start_ms < end_ms && d.end_ms > start_ms {
                active.push(d.clone());
            }
        }

        active.reverse();
        active
    }

    /// Number of dialogues in the timeline.
    #[inline] pub fn len(&self) -> usize { self.dialogues.len() }

    /// Returns true if the timeline contains no dialogues.
    #[inline] pub fn is_empty(&self) -> bool { self.dialogues.is_empty() }

    /// Read-only slice of all sorted dialogues.
    #[inline] pub fn dialogues(&self) -> &[SubtitleDialogue] { &self.dialogues }

    /// Total timeline duration in milliseconds.
    pub fn total_duration_ms(&self) -> i64 {
        let mut max_end = 0i64;
        for d in &self.dialogues {
            if d.end_ms > max_end { max_end = d.end_ms; }
        }
        max_end
    }

    /// Total timeline duration in microseconds.
    #[inline] pub fn total_duration_micros(&self) -> i64 {
        self.total_duration_ms().saturating_mul(1000)
    }
}
