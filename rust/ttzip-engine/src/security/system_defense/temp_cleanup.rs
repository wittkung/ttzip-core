// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Temporary Directory Cleanup & Workspace Isolation Guard (`TempDirectoryCleanupGuard`).
//!
//! Provides high-entropy UUID workspace isolation, deterministic RAII on-drop cleanup,
//! and self-healing startup orphan workspace purging to prevent disk accumulation and artifact leakage.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use super::SystemDefenseError;

/// Monotonic thread-safe counter for entropy mixing in UUID generation.
static MONOTONIC_COUNTER: AtomicU64 = AtomicU64::new(1);

/// RAII Guard that owns an isolated temporary directory and purges it upon drop.
#[derive(Debug)]
pub struct TempDirectoryGuard {
    path: PathBuf,
    keep_on_drop: bool,
}

impl TempDirectoryGuard {
    /// Creates a new guard for an existing directory.
    #[inline]
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            keep_on_drop: false,
        }
    }

    /// Returns the filesystem path to the temporary directory.
    #[inline]
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Explicitly disarms the RAII cleanup, preserving the directory upon drop.
    #[inline]
    pub fn disarm(&mut self) {
        self.keep_on_drop = true;
    }

    /// Securely removes the directory immediately and marks the guard as disarmed.
    pub fn close(mut self) -> Result<(), std::io::Error> {
        self.keep_on_drop = true;
        if self.path.exists() {
            fs::remove_dir_all(&self.path)?;
        }
        Ok(())
    }
}

impl Drop for TempDirectoryGuard {
    fn drop(&mut self) {
        if !self.keep_on_drop && self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// Guard managing isolated temporary workspaces and orphan cleaning.
#[derive(Debug, Clone, Default)]
pub struct TempDirectoryCleanupGuard;

impl TempDirectoryCleanupGuard {
    /// Creates a new `TempDirectoryCleanupGuard`.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Generates a high-entropy UUID v4 string for isolated folder naming.
    #[must_use]
    pub fn generate_uuid_v4() -> String {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0));
        let nanos = now.as_nanos();
        let count = MONOTONIC_COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id() as u64;

        // Mix bits using splitmix64-style mixing
        let mut seed = (nanos as u64) ^ (count.wrapping_mul(0x9e3779b97f4a7c15)) ^ (pid << 32);
        let mut rng = || -> u32 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 32) as u32
        };

        let time_low = rng();
        let time_mid = (rng() as u16) & 0xffff;
        let time_hi_and_version = ((rng() as u16) & 0x0fff) | 0x4000;
        let clock_seq_hi_and_reserved = ((rng() as u16) & 0x3fff) | 0x8000;
        let node_hi = (rng() as u16) & 0xffff;
        let node_lo = rng();

        format!(
            "{:08x}-{:04x}-{:04x}-{:04x}-{:04x}{:08x}",
            time_low,
            time_mid,
            time_hi_and_version,
            clock_seq_hi_and_reserved,
            node_hi,
            node_lo
        )
    }

    /// Creates a new isolated temporary workspace directory guarded by RAII drop cleanup.
    pub fn create_isolated_temp_dir(
        &self,
        prefix: &str,
        base_dir: Option<&Path>,
    ) -> Result<TempDirectoryGuard, SystemDefenseError> {
        let parent = match base_dir {
            Some(p) => p.to_path_buf(),
            None => std::env::temp_dir(),
        };

        let uuid = Self::generate_uuid_v4();
        let folder_name = format!("{}_{}", prefix.trim_end_matches('_'), uuid);
        let target_path = parent.join(folder_name);

        fs::create_dir_all(&target_path).map_err(|e| {
            SystemDefenseError::TempDirectoryCreationFailed {
                path: target_path.to_string_lossy().into_owned(),
                reason: e.to_string(),
            }
        })?;

        Ok(TempDirectoryGuard::new(target_path))
    }

    /// Creates an isolated temporary workspace directory under the default system temp directory.
    #[inline]
    pub fn create_workspace(&self, prefix: &str) -> Result<TempDirectoryGuard, SystemDefenseError> {
        self.create_isolated_temp_dir(prefix, None)
    }

    /// Scans the given directory for orphan temporary directories older than `max_age` and removes them.
    /// Returns the count of successfully cleaned orphan directories.
    pub fn clean_orphan_workspaces(
        &self,
        temp_root: &Path,
        prefix: &str,
        max_age: Duration,
    ) -> Result<usize, SystemDefenseError> {
        if !temp_root.exists() || !temp_root.is_dir() {
            return Ok(0);
        }

        let now = SystemTime::now();
        let mut cleaned_count = 0;

        let entries = fs::read_dir(temp_root).map_err(|e| {
            SystemDefenseError::TempDirectoryCleanupFailed {
                reason: format!("Failed to read directory {}: {e}", temp_root.display()),
            }
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.starts_with(prefix) {
                    // Check directory creation/modification age
                    if let Ok(metadata) = entry.metadata() {
                        let mod_time = metadata.modified().unwrap_or(now);
                        if let Ok(age) = now.duration_since(mod_time) {
                            if age >= max_age {
                                if fs::remove_dir_all(&path).is_ok() {
                                    cleaned_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(cleaned_count)
    }
}
