// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Header Quota Guard and OOM Circuit Breaker (`HeaderQuotaGuard`).
//!
//! Provides defense-in-depth against malicious Zip-Bombs, 7z malformed headers,
//! and metadata expansion attacks by validating declared entry counts against physical
//! header byte density and configured memory quotas before any memory is allocated.

/// Security and quota errors detected during archive header parsing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HeaderSecurityError {
    /// Declared file count violates bit-density limits or exceeds memory quota.
    #[error("Header OOM Bomb Detected: declared {declared_files} files with only {remaining_header_bytes} header bytes remaining (estimated memory: {estimated_memory_bytes} bytes, quota: {max_memory_limit_bytes} bytes; reason: {reason})")]
    HeaderOomBombDetected {
        declared_files: u64,
        remaining_header_bytes: usize,
        estimated_memory_bytes: u64,
        max_memory_limit_bytes: u64,
        reason: String,
    },

    /// Invalid header parameter or configuration.
    #[error("Invalid header parameter: {0}")]
    InvalidParameter(String),
}

/// Default maximum memory threshold allocated for archive metadata structures (64 MB).
pub const DEFAULT_MAX_HEADER_MEMORY_BYTES: u64 = 64 * 1024 * 1024;

/// Default estimated memory footprint per archive entry (128 bytes).
pub const DEFAULT_ESTIMATED_BYTES_PER_ENTRY: u64 = 128;

/// Default maximum theoretical entries per header byte based on bit-packing (8 entries/byte).
pub const DEFAULT_MAX_ENTRIES_PER_HEADER_BYTE: u64 = 8;

/// Configuration and stateful validator for archive header metadata resource quotas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderQuotaGuard {
    /// Maximum allowed memory footprint for metadata in bytes.
    pub max_memory_limit_bytes: u64,
    /// Estimated memory footprint per entry structure in bytes.
    pub estimated_bytes_per_entry: u64,
    /// Maximum theoretical entry density per remaining header byte.
    pub max_entries_per_header_byte: u64,
}

impl Default for HeaderQuotaGuard {
    fn default() -> Self {
        Self {
            max_memory_limit_bytes: DEFAULT_MAX_HEADER_MEMORY_BYTES,
            estimated_bytes_per_entry: DEFAULT_ESTIMATED_BYTES_PER_ENTRY,
            max_entries_per_header_byte: DEFAULT_MAX_ENTRIES_PER_HEADER_BYTE,
        }
    }
}

impl HeaderQuotaGuard {
    /// Creates a new quota guard with custom memory limits.
    pub fn new(max_memory_limit_bytes: u64, estimated_bytes_per_entry: u64) -> Self {
        Self {
            max_memory_limit_bytes,
            estimated_bytes_per_entry: if estimated_bytes_per_entry == 0 {
                1
            } else {
                estimated_bytes_per_entry
            },
            max_entries_per_header_byte: DEFAULT_MAX_ENTRIES_PER_HEADER_BYTE,
        }
    }

    /// Creates a new quota guard with fully custom parameters.
    pub fn with_custom_limits(
        max_memory_limit_bytes: u64,
        estimated_bytes_per_entry: u64,
        max_entries_per_header_byte: u64,
    ) -> Self {
        Self {
            max_memory_limit_bytes,
            estimated_bytes_per_entry: if estimated_bytes_per_entry == 0 {
                1
            } else {
                estimated_bytes_per_entry
            },
            max_entries_per_header_byte: if max_entries_per_header_byte == 0 {
                1
            } else {
                max_entries_per_header_byte
            },
        }
    }

    /// Computes the estimated memory required to store the declared number of file entries.
    #[inline]
    pub fn estimate_memory_cost(&self, declared_files: u64) -> u64 {
        declared_files.saturating_mul(self.estimated_bytes_per_entry)
    }

    /// Validates the declared file count against physical header bytes and memory quotas.
    pub fn validate(
        &self,
        declared_files: u64,
        remaining_header_bytes: usize,
    ) -> Result<(), HeaderSecurityError> {
        if declared_files == 0 {
            return Ok(());
        }

        // 1. Bit-density validation: In 7z/Zip, an entry requires at least 1 bit in stream bitmaps.
        let max_possible_entries = (remaining_header_bytes as u64)
            .saturating_mul(self.max_entries_per_header_byte);

        if declared_files > max_possible_entries {
            let estimated_mem = self.estimate_memory_cost(declared_files);
            return Err(HeaderSecurityError::HeaderOomBombDetected {
                declared_files,
                remaining_header_bytes,
                estimated_memory_bytes: estimated_mem,
                max_memory_limit_bytes: self.max_memory_limit_bytes,
                reason: format!(
                    "Declared file count ({}) exceeds maximum physical bit-density bound ({}) for {} header bytes",
                    declared_files, max_possible_entries, remaining_header_bytes
                ),
            });
        }

        // 2. Memory quota validation: Check whether estimated metadata memory exceeds physical budget.
        let estimated_mem = self.estimate_memory_cost(declared_files);
        if estimated_mem > self.max_memory_limit_bytes {
            return Err(HeaderSecurityError::HeaderOomBombDetected {
                declared_files,
                remaining_header_bytes,
                estimated_memory_bytes: estimated_mem,
                max_memory_limit_bytes: self.max_memory_limit_bytes,
                reason: format!(
                    "Estimated metadata memory consumption ({} bytes) exceeds security quota ({} bytes)",
                    estimated_mem, self.max_memory_limit_bytes
                ),
            });
        }

        Ok(())
    }
}

/// Global standard validation helper using default 64MB security limits.
///
/// Ensures:
/// 1. `declared_files <= (remaining_header_bytes as u64) * 8`
/// 2. `declared_files * 128 <= 64 MB`
#[inline]
pub fn validate_header_entry_count(
    declared_files: u64,
    remaining_header_bytes: usize,
) -> Result<(), HeaderSecurityError> {
    HeaderQuotaGuard::default().validate(declared_files, remaining_header_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_header_entry_count() {
        // 100 files in a 1000-byte header (100 <= 8000, 100 * 128 = 12.8 KB <= 64 MB)
        assert!(validate_header_entry_count(100, 1000).is_ok());

        // 0 files in 0 bytes header
        assert!(validate_header_entry_count(0, 0).is_ok());

        // 800 files in 100 bytes header (exact 8 entries/byte boundary)
        assert!(validate_header_entry_count(800, 100).is_ok());
    }

    #[test]
    fn test_density_overflow_rejection() {
        // 801 files in 100 bytes header (> 800)
        let err = validate_header_entry_count(801, 100).unwrap_err();
        match err {
            HeaderSecurityError::HeaderOomBombDetected {
                declared_files,
                remaining_header_bytes,
                ..
            } => {
                assert_eq!(declared_files, 801);
                assert_eq!(remaining_header_bytes, 100);
            }
            _ => panic!("Expected HeaderOomBombDetected error"),
        }
    }

    #[test]
    fn test_four_billion_files_zip_bomb_circuit_breaker() {
        // Malicious 4,000,000,000 declared entries in small 32-byte header
        let malicious_count = 4_000_000_000u64;
        let remaining_header_bytes = 32usize;

        let err = validate_header_entry_count(malicious_count, remaining_header_bytes).unwrap_err();
        match err {
            HeaderSecurityError::HeaderOomBombDetected {
                declared_files,
                remaining_header_bytes: rem,
                ..
            } => {
                assert_eq!(declared_files, malicious_count);
                assert_eq!(rem, remaining_header_bytes);
            }
            _ => panic!("Expected HeaderOomBombDetected error for 4-billion entry bomb"),
        }
    }

    #[test]
    fn test_memory_quota_exceeded_with_large_header() {
        // Even if header is large (e.g. 100MB header), 1,000,000 files * 128 bytes = 128 MB > 64 MB quota
        let files = 1_000_000u64;
        let large_header_bytes = 100 * 1024 * 1024; // 100 MB

        let err = validate_header_entry_count(files, large_header_bytes).unwrap_err();
        match err {
            HeaderSecurityError::HeaderOomBombDetected {
                declared_files,
                estimated_memory_bytes,
                max_memory_limit_bytes,
                ..
            } => {
                assert_eq!(declared_files, files);
                assert_eq!(estimated_memory_bytes, 128_000_000);
                assert_eq!(max_memory_limit_bytes, DEFAULT_MAX_HEADER_MEMORY_BYTES);
            }
            _ => panic!("Expected memory quota rejection"),
        }
    }

    #[test]
    fn test_custom_quota_guard() {
        // Custom 1MB quota, 64 bytes per entry
        let guard = HeaderQuotaGuard::new(1024 * 1024, 64);
        assert!(guard.validate(1000, 500).is_ok()); // 64 KB <= 1 MB

        let err = guard.validate(20_000, 5000).unwrap_err(); // 1.28 MB > 1 MB
        match err {
            HeaderSecurityError::HeaderOomBombDetected {
                declared_files,
                max_memory_limit_bytes,
                ..
            } => {
                assert_eq!(declared_files, 20_000);
                assert_eq!(max_memory_limit_bytes, 1024 * 1024);
            }
            _ => panic!("Expected custom quota rejection"),
        }
    }
}
