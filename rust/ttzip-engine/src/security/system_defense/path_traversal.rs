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
}

impl Default for PathTraversalOptions {
    fn default() -> Self {
        Self {
            disallow_absolute: true,
            disallow_reserved_devices: true,
            max_path_len: 4096,
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

        // 3. Absolute path checks
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
