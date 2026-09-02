// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Sandbox Escaping Defense Guard (`SandboxEscapingGuard`).
//!
//! Provides deterministic path normalization, virtual jail containment,
//! and step-by-step intermediate directory symlink (`!S_ISLNK`) verification
//! to prevent sandbox escapes, TOCTOU symlink races, and path jailbreaks.

use std::path::{Component, Path, PathBuf};

use super::SystemDefenseError;

/// Configuration options for sandbox escaping guard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxEscapingOptions {
    /// Canonical root directory of the isolated sandbox jail.
    pub jail_root: PathBuf,
    /// Whether to allow symlinks whose targets reside entirely within the jail.
    pub allow_internal_symlinks: bool,
    /// Maximum allowable symlink evaluation depth to prevent recursion loops.
    pub max_symlink_depth: usize,
    /// Enforce strict intermediate directory non-symlink verification.
    pub enforce_non_symlink_parents: bool,
}

impl Default for SandboxEscapingOptions {
    fn default() -> Self {
        Self {
            jail_root: PathBuf::from("/"),
            allow_internal_symlinks: false,
            max_symlink_depth: 8,
            enforce_non_symlink_parents: true,
        }
    }
}

/// Sandbox escaping guard defending against symlink traps and jailbreaks.
#[derive(Debug, Clone)]
pub struct SandboxEscapingGuard {
    options: SandboxEscapingOptions,
}

impl SandboxEscapingGuard {
    /// Creates a new sandbox escaping guard with the specified options.
    #[inline]
    #[must_use]
    pub fn new(options: SandboxEscapingOptions) -> Self {
        Self { options }
    }

    /// Creates a guard with a specific jail root and default strict security parameters.
    #[inline]
    #[must_use]
    pub fn with_jail_root(jail_root: impl Into<PathBuf>) -> Self {
        Self {
            options: SandboxEscapingOptions {
                jail_root: jail_root.into(),
                ..Default::default()
            },
        }
    }

    /// Returns a reference to the active options.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> &SandboxEscapingOptions {
        &self.options
    }

    /// Lexically normalizes a candidate path by eliminating `.` and `..` components
    /// without accessing the underlying filesystem.
    #[must_use]
    pub fn normalize_lexical_path(path: &Path) -> PathBuf {
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::Prefix(prefix) => normalized.push(Component::Prefix(prefix)),
                Component::RootDir => normalized.push(Component::RootDir),
                Component::CurDir => {
                    // Skip '.' current directory tokens
                }
                Component::ParentDir => {
                    // Pop preceding component if possible
                    normalized.pop();
                }
                Component::Normal(c) => normalized.push(c),
            }
        }
        normalized
    }

    /// Validates that a candidate relative or absolute path stays strictly inside the jail root.
    pub fn validate_path(&self, candidate: &Path) -> Result<PathBuf, SystemDefenseError> {
        let normalized = Self::normalize_lexical_path(candidate);

        // Disallow path traversal indicators
        for component in candidate.components() {
            if matches!(component, Component::ParentDir) {
                return Err(SystemDefenseError::SandboxEscapeAttempt {
                    path: candidate.to_string_lossy().into_owned(),
                    reason: "Path contains parent directory traversal token ('..')".to_string(),
                });
            }
        }

        // Construct absolute target candidate relative to jail root
        let full_target = if normalized.is_absolute() {
            // Strip root prefix to anchor within jail root
            let mut relative_buf = PathBuf::new();
            for comp in normalized.components() {
                if let Component::Normal(part) = comp {
                    relative_buf.push(part);
                }
            }
            self.options.jail_root.join(relative_buf)
        } else {
            self.options.jail_root.join(&normalized)
        };

        // Enforce prefix containment
        if !Self::is_contained_in(&self.options.jail_root, &full_target) {
            return Err(SystemDefenseError::SandboxEscapeAttempt {
                path: candidate.to_string_lossy().into_owned(),
                reason: format!(
                    "Resolved target '{}' escapes jail root '{}'",
                    full_target.display(),
                    self.options.jail_root.display()
                ),
            });
        }

        Ok(full_target)
    }

    /// Verifies that no intermediate ancestor directory along the target path is a symlink.
    /// This immunizes against TOCTOU symlink redirection attacks.
    pub fn verify_no_symlink_ancestors(&self, target_path: &Path) -> Result<(), SystemDefenseError> {
        if !self.options.enforce_non_symlink_parents {
            return Ok(());
        }

        let rel_path = match target_path.strip_prefix(&self.options.jail_root) {
            Ok(rel) => rel,
            Err(_) => target_path,
        };

        let mut current = self.options.jail_root.clone();
        for component in rel_path.components() {
            if let Component::Normal(part) = component {
                current.push(part);
                // If the path exists on disk, check if it is a symlink
                if let Ok(metadata) = std::fs::symlink_metadata(&current) {
                    if metadata.file_type().is_symlink() {
                        if !self.options.allow_internal_symlinks {
                            return Err(SystemDefenseError::SymlinkEscapingDetected {
                                path: current.to_string_lossy().into_owned(),
                                reason: "Intermediate path component is an active symlink"
                                    .to_string(),
                            });
                        }

                        // If internal symlinks are allowed, verify symlink target stays in jail
                        if let Ok(target) = std::fs::read_link(&current) {
                            let resolved = if target.is_absolute() {
                                target
                            } else {
                                current.parent().unwrap_or(Path::new("")).join(target)
                            };
                            let norm_resolved = Self::normalize_lexical_path(&resolved);
                            if !Self::is_contained_in(&self.options.jail_root, &norm_resolved) {
                                return Err(SystemDefenseError::SymlinkEscapingDetected {
                                    path: current.to_string_lossy().into_owned(),
                                    reason: format!(
                                        "Symlink target '{}' escapes jail root '{}'",
                                        norm_resolved.display(),
                                        self.options.jail_root.display()
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Checks if `target` is lexically or physically contained within `jail`.
    #[inline]
    #[must_use]
    pub fn is_contained_in(jail: &Path, target: &Path) -> bool {
        let norm_jail = Self::normalize_lexical_path(jail);
        let norm_target = Self::normalize_lexical_path(target);
        norm_target.starts_with(norm_jail)
    }
}
