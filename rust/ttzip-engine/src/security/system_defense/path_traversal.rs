// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Path Traversal & Zip-Slip Protection Guard (`PathTraversalProtectionGuard`).
//!
//! Provides single-pass stack-based traversal neutralization, null-byte injection detection,
//! Windows/POSIX reserved device name interception, and canonical relative path validation.

use super::SystemDefenseError;

/// Windows DOS reserved device names and system namespaces.
const WINDOWS_RESERVED_DEVICES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL",
    "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    "CLOCK$",
];

/// Options for path traversal protection guard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathTraversalOptions {
    /// Disallow absolute paths (e.g. `/etc/passwd` or `C:\Windows`).
    pub disallow_absolute: bool,
    /// Disallow Windows DOS reserved device names (e.g. `CON`, `NUL`).
    pub disallow_reserved_devices: bool,
    /// Maximum allowable path length in bytes.
    pub max_path_len: usize,
    /// Disallow internal symlink directory traversal escaping archive or parent boundaries.
    pub disallow_symlink_traversal: bool,
    /// Disallow malicious canonical paths (Windows device namespaces, NTFS streams, encoded traversals).
    pub disallow_malicious_canonical_paths: bool,
}

impl Default for PathTraversalOptions {
    fn default() -> Self {
        Self {
            disallow_absolute: true,
            disallow_reserved_devices: true,
            max_path_len: 4096,
            disallow_symlink_traversal: true,
            disallow_malicious_canonical_paths: true,
        }
    }
}

/// Guard defending against Zip-Slip, directory traversal, null-bytes, and device injection.
#[derive(Debug, Clone)]
pub struct PathTraversalProtectionGuard {
    options: PathTraversalOptions,
}

impl PathTraversalProtectionGuard {
    /// Creates a new guard with specified options.
    #[inline]
    #[must_use]
    pub fn new(options: PathTraversalOptions) -> Self {
        Self { options }
    }

    /// Creates a guard with default strict security parameters.
    #[inline]
    #[must_use]
    pub fn strict() -> Self {
        Self::new(PathTraversalOptions::default())
    }

    /// Validates and sanitizes a raw path string, returning a clean relative path.
    pub fn sanitize_path(&self, raw_path: &str) -> Result<String, SystemDefenseError> {
        // 1. Length bound check
        if raw_path.len() > self.options.max_path_len {
            return Err(SystemDefenseError::PathTooLong {
                len: raw_path.len(),
                max_len: self.options.max_path_len,
            });
        }

        // 2. Null-byte injection check
        if raw_path.contains('\0') {
            return Err(SystemDefenseError::NullByteInjectionDetected {
                path: raw_path.replace('\0', "\\0"),
            });
        }

        // 3. Absolute path and Windows namespace device checks
        let trimmed = raw_path.trim();
        if self.options.disallow_absolute {
            if trimmed.starts_with('/') || trimmed.starts_with('\\') {
                return Err(SystemDefenseError::PathTraversalAttackDetected {
                    path: raw_path.to_string(),
                    reason: "Absolute root prefix is prohibited".to_string(),
                });
            }
            // Check Windows drive letter: `C:` or `D:\`
            if trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':' {
                let drive_letter = trimmed.as_bytes()[0];
                if drive_letter.is_ascii_alphabetic() {
                    return Err(SystemDefenseError::PathTraversalAttackDetected {
                        path: raw_path.to_string(),
                        reason: "Drive letter prefix is prohibited".to_string(),
                    });
                }
            }
        }

        // Check malicious canonical device namespace prefixes
        if self.options.disallow_malicious_canonical_paths
            && (trimmed.starts_with(r"\\?\")
                || trimmed.starts_with(r"\\.\")
                || trimmed.starts_with(r"\??\")
                || trimmed.starts_with("//?/")
                || trimmed.starts_with("//./"))
        {
            return Err(SystemDefenseError::PathTraversalAttackDetected {
                path: raw_path.to_string(),
                reason: "Windows namespace device prefix is prohibited".to_string(),
            });
        }

        // Check for URL-encoded traversal patterns (%2e%2e, %2f, %5c)
        if self.options.disallow_malicious_canonical_paths {
            let upper = trimmed.to_ascii_uppercase();
            if upper.contains("%2E%2E") || upper.contains("%2F") || upper.contains("%5C") {
                return Err(SystemDefenseError::PathTraversalAttackDetected {
                    path: raw_path.to_string(),
                    reason: "URL-encoded traversal sequence is prohibited".to_string(),
                });
            }
        }

        // 4. Single-pass stack-based segment normalization
        let mut stack: Vec<&str> = Vec::new();
        for segment in raw_path.split(['/', '\\']) {
            let clean_seg = segment.trim();
            if clean_seg.is_empty() || clean_seg == "." {
                continue;
            }

            if clean_seg == ".." {
                if stack.pop().is_none() {
                    return Err(SystemDefenseError::PathTraversalAttackDetected {
                        path: raw_path.to_string(),
                        reason: "Parent directory traversal escapes target root ('..')".to_string(),
                    });
                }
            } else {
                // Check for NTFS Alternate Data Stream syntax
                if self.options.disallow_malicious_canonical_paths && clean_seg.contains(':') {
                    return Err(SystemDefenseError::PathTraversalAttackDetected {
                        path: raw_path.to_string(),
                        reason: "NTFS alternate data stream syntax (':') is prohibited".to_string(),
                    });
                }

                // Check for multi-dot canonical bypass patterns (e.g. "...", "....")
                if clean_seg.len() > 2 && clean_seg.chars().all(|c| c == '.') {
                    return Err(SystemDefenseError::PathTraversalAttackDetected {
                        path: raw_path.to_string(),
                        reason: "Multi-dot sequence is prohibited".to_string(),
                    });
                }

                // 5. Reserved device names check
                if self.options.disallow_reserved_devices
                    && Self::is_reserved_device_name(clean_seg)
                {
                    return Err(SystemDefenseError::ReservedDeviceNameDetected {
                        segment: clean_seg.to_string(),
                    });
                }
                stack.push(clean_seg);
            }
        }

        if stack.is_empty() {
            return Err(SystemDefenseError::PathTraversalAttackDetected {
                path: raw_path.to_string(),
                reason: "Path evaluates to empty after normalization".to_string(),
            });
        }

        Ok(stack.join("/"))
    }

    /// Validates a symlink target relative to the symlink's path inside an archive or virtual sandbox.
    ///
    /// Defends against internal symlink directory traversal attacks where a symlink points
    /// to parent directories (`../../etc/passwd`) or absolute paths escaping the isolation boundary.
    pub fn validate_symlink_target(
        &self,
        symlink_path: &str,
        target: &str,
    ) -> Result<String, SystemDefenseError> {
        if target.len() > self.options.max_path_len {
            return Err(SystemDefenseError::PathTooLong {
                len: target.len(),
                max_len: self.options.max_path_len,
            });
        }

        if target.contains('\0') {
            return Err(SystemDefenseError::NullByteInjectionDetected {
                path: target.replace('\0', "\\0"),
            });
        }

        let trimmed_target = target.trim();
        if self.options.disallow_absolute {
            if trimmed_target.starts_with('/') || trimmed_target.starts_with('\\') {
                return Err(SystemDefenseError::SymlinkEscapingDetected {
                    path: symlink_path.to_string(),
                    reason: "Symlink points to absolute path".to_string(),
                });
            }

            if trimmed_target.len() >= 2 && trimmed_target.as_bytes()[1] == b':' {
                let drive = trimmed_target.as_bytes()[0];
                if drive.is_ascii_alphabetic() {
                    return Err(SystemDefenseError::SymlinkEscapingDetected {
                        path: symlink_path.to_string(),
                        reason: "Symlink points to drive letter root".to_string(),
                    });
                }
            }
        }

        if self.options.disallow_malicious_canonical_paths
            && (trimmed_target.starts_with(r"\\?\")
                || trimmed_target.starts_with(r"\\.\")
                || trimmed_target.starts_with(r"\??\")
                || trimmed_target.starts_with("//?/")
                || trimmed_target.starts_with("//./"))
        {
            return Err(SystemDefenseError::SymlinkEscapingDetected {
                path: symlink_path.to_string(),
                reason: "Symlink points to Windows device namespace".to_string(),
            });
        }

        if self.options.disallow_malicious_canonical_paths {
            let upper = trimmed_target.to_ascii_uppercase();
            if upper.contains("%2E%2E") || upper.contains("%2F") || upper.contains("%5C") {
                return Err(SystemDefenseError::SymlinkEscapingDetected {
                    path: symlink_path.to_string(),
                    reason: "Symlink target contains URL-encoded traversal sequence".to_string(),
                });
            }
        }

        // Resolve target relative to symlink's parent directory
        let symlink_clean = symlink_path.replace('\\', "/");
        let parent_components: Vec<&str> = symlink_clean
            .split('/')
            .filter(|s| !s.is_empty() && *s != ".")
            .collect();

        let parent_depth = if !parent_components.is_empty() {
            parent_components.len() - 1
        } else {
            0
        };

        let mut stack: Vec<&str> = parent_components[..parent_depth].to_vec();

        for segment in target.split(['/', '\\']) {
            let clean = segment.trim();
            if clean.is_empty() || clean == "." {
                continue;
            }

            if clean == ".." {
                if stack.pop().is_none() {
                    return Err(SystemDefenseError::SymlinkEscapingDetected {
                        path: symlink_path.to_string(),
                        reason: format!(
                            "Internal symlink directory traversal escapes archive root ('{}' -> '{}')",
                            symlink_path, target
                        ),
                    });
                }
            } else {
                if self.options.disallow_reserved_devices && Self::is_reserved_device_name(clean) {
                    return Err(SystemDefenseError::ReservedDeviceNameDetected {
                        segment: clean.to_string(),
                    });
                }

                if self.options.disallow_malicious_canonical_paths && clean.contains(':') {
                    return Err(SystemDefenseError::PathTraversalAttackDetected {
                        path: target.to_string(),
                        reason: "NTFS alternate data stream in symlink target is prohibited".to_string(),
                    });
                }

                stack.push(clean);
            }
        }

        if stack.is_empty() {
            return Err(SystemDefenseError::SymlinkEscapingDetected {
                path: symlink_path.to_string(),
                reason: "Symlink target resolves to empty root".to_string(),
            });
        }

        Ok(stack.join("/"))
    }

    /// Validates a canonical path anchored within `base_root`, ensuring lexical containment
    /// and verifying that intermediate ancestors are not symlinks escaping the boundary.
    pub fn validate_canonical_path(
        &self,
        base_root: &std::path::Path,
        candidate_rel_path: &str,
    ) -> Result<std::path::PathBuf, SystemDefenseError> {
        let clean_rel = self.sanitize_path(candidate_rel_path)?;

        let abs_base = if base_root.is_absolute() {
            base_root.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("/"))
                .join(base_root)
        };

        let target = abs_base.join(&clean_rel);

        // Lexical normalization without disk I/O
        let mut normalized = std::path::PathBuf::new();
        for component in target.components() {
            match component {
                std::path::Component::Prefix(p) => normalized.push(std::path::Component::Prefix(p)),
                std::path::Component::RootDir => normalized.push(std::path::Component::RootDir),
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    normalized.pop();
                }
                std::path::Component::Normal(c) => normalized.push(c),
            }
        }

        let mut norm_base = std::path::PathBuf::new();
        for component in abs_base.components() {
            match component {
                std::path::Component::Prefix(p) => norm_base.push(std::path::Component::Prefix(p)),
                std::path::Component::RootDir => norm_base.push(std::path::Component::RootDir),
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    norm_base.pop();
                }
                std::path::Component::Normal(c) => norm_base.push(c),
            }
        }

        if !normalized.starts_with(&norm_base) {
            return Err(SystemDefenseError::PathTraversalAttackDetected {
                path: candidate_rel_path.to_string(),
                reason: format!(
                    "Canonical path '{}' escapes base root '{}'",
                    normalized.display(),
                    norm_base.display()
                ),
            });
        }

        // If path or ancestors exist on disk and symlink traversal is disallowed, verify ancestors
        if self.options.disallow_symlink_traversal {
            let mut current = norm_base.clone();
            if let Ok(rel_components) = normalized.strip_prefix(&norm_base) {
                for comp in rel_components.components() {
                    if let std::path::Component::Normal(part) = comp {
                        current.push(part);
                        if let Ok(meta) = std::fs::symlink_metadata(&current) {
                            if meta.file_type().is_symlink() {
                                return Err(SystemDefenseError::SymlinkEscapingDetected {
                                    path: current.display().to_string(),
                                    reason: "Intermediate canonical directory component is an active symlink".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(normalized)
    }

    /// Checks if a segment matches Windows or POSIX reserved device names.
    #[must_use]
    pub fn is_reserved_device_name(segment: &str) -> bool {
        if segment.is_empty() {
            return false;
        }

        let upper = segment.to_ascii_uppercase();
        if upper.starts_with("PHYSICALDRIVE") || upper.starts_with("\\\\.\\") || upper.starts_with("/DEV/") {
            return true;
        }

        // Strip file extension to test stem
        let stem = match segment.find('.') {
            Some(idx) => &segment[..idx],
            None => segment,
        };

        let trimmed_stem = stem.trim_end_matches([' ', '.']).to_ascii_uppercase();
        WINDOWS_RESERVED_DEVICES.contains(&trimmed_stem.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_symlink_target_validation_safe() {
        let guard = PathTraversalProtectionGuard::strict();

        // Symlink inside nested subdirectory navigating within boundary
        assert_eq!(
            guard.validate_symlink_target("a/b/link", "../c").unwrap(),
            "a/c"
        );
        assert_eq!(
            guard.validate_symlink_target("a/b/link", "sub/target.txt").unwrap(),
            "a/b/sub/target.txt"
        );
        assert_eq!(
            guard.validate_symlink_target("a/b/link", "../../c/d.bin").unwrap(),
            "c/d.bin"
        );
    }

    #[test]
    fn test_symlink_target_validation_traversal_escapes() {
        let guard = PathTraversalProtectionGuard::strict();

        // Target escapes archive root by traversing past root parent
        assert!(guard
            .validate_symlink_target("a/b/link", "../../../outside.txt")
            .is_err());

        // Target at root level attempting to traverse upward
        assert!(guard
            .validate_symlink_target("link", "../escape.txt")
            .is_err());

        // Absolute path targets
        assert!(guard
            .validate_symlink_target("link", "/etc/passwd")
            .is_err());
        assert!(guard
            .validate_symlink_target("link", "C:\\Windows\\System32")
            .is_err());
        assert!(guard
            .validate_symlink_target("link", r"\\?\C:\foo")
            .is_err());

        // Target with null byte injection
        assert!(guard
            .validate_symlink_target("link", "target\0evil")
            .is_err());

        // Target with NTFS ADS
        assert!(guard
            .validate_symlink_target("link", "target.txt:stream")
            .is_err());

        // Target with URL-encoded traversal
        assert!(guard
            .validate_symlink_target("link", "foo/%2e%2e/outside")
            .is_err());
    }

    #[test]
    fn test_malicious_canonical_path_checks() {
        let guard = PathTraversalProtectionGuard::strict();

        // Windows device namespace prefixes
        assert!(guard.sanitize_path(r"\\?\C:\evil.exe").is_err());
        assert!(guard.sanitize_path(r"\\.\PhysicalDrive0").is_err());
        assert!(guard.sanitize_path(r"\??\C:\malicious").is_err());

        // URL-encoded traversal sequences
        assert!(guard.sanitize_path("foo/%2e%2e/bar").is_err());
        assert!(guard.sanitize_path("foo/%2E%2E/bar").is_err());
        assert!(guard.sanitize_path("foo/%2f/bar").is_err());

        // NTFS Alternate Data Stream syntax
        assert!(guard.sanitize_path("invoice.pdf:hidden.exe").is_err());
        assert!(guard.sanitize_path("file.txt::$DATA").is_err());

        // Multi-dot canonical bypass patterns
        assert!(guard.sanitize_path("foo/.../bar").is_err());
        assert!(guard.sanitize_path("foo/..../bar").is_err());

        // Canonical base root validation
        let base = Path::new("/tmp/ttzip_sandbox_test");
        assert!(guard.validate_canonical_path(base, "docs/manual.pdf").is_ok());
        assert!(guard.validate_canonical_path(base, "../etc/shadow").is_err());
    }
}
