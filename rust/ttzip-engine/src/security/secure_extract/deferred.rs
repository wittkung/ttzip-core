// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Deferred metadata structures for two-stage bottom-up restoration.

use std::path::PathBuf;

/// Metadata record for deferred application after archive extraction completes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredSecureEntry {
    /// Relative path within the sandbox root.
    pub rel_path: PathBuf,
    /// POSIX file mode / permissions.
    pub mode: u32,
    /// Modified time epoch seconds.
    pub mtime_epoch_secs: i64,
    /// Modified time nanoseconds.
    pub mtime_nanos: u32,
    /// Whether this entry is a directory.
    pub is_directory: bool,
}

impl DeferredSecureEntry {
    /// Depth of the path hierarchy for bottom-up sorting.
    #[inline]
    #[must_use]
    pub fn depth(&self) -> usize {
        self.rel_path.components().count()
    }
}
