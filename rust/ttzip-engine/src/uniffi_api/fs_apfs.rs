// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! UniFFI exports for APFS copy-on-write cloning, extent preallocation,
//! and archive path sanitization.

use std::path::Path;

/// Clones a file using APFS Copy-on-Write (CoW) or fallback physical copy.
#[uniffi::export]
pub fn apfs_clone_file(src: String, dst: String, overwrite: bool) -> bool {
    crate::fs::apfs::apfs_clone_file(Path::new(&src), Path::new(&dst), overwrite).is_ok()
}

/// Clones file descriptor range via APFS `fcopyfile`.
#[uniffi::export]
pub fn apfs_clone_range(in_fd: i32, out_fd: i32) -> bool {
    crate::fs::apfs::apfs_fcopyfile_clone(in_fd, out_fd).is_ok()
}

/// Preallocates contiguous physical extent space on APFS filesystems.
#[uniffi::export]
pub fn apfs_preallocate(fd: i32, target_size: i64) -> bool {
    crate::fs::apfs::apfs_preallocate(fd, target_size).is_ok()
}

/// Sanitizes, normalizes, and audits a path against ZipSlip traversal, Win32 reserved device names,
/// NTFS Alternate Data Streams (ADS), and canonical NFC encoding.
#[uniffi::export]
pub fn sanitize_archive_path(raw_path: String) -> String {
    crate::security::sanitize_path(&raw_path).normalized_path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_archive_path() {
        assert_eq!(sanitize_archive_path("../../etc/passwd".to_string()), "etc/passwd");
        assert_eq!(sanitize_archive_path("foo/bar/baz.txt".to_string()), "foo/bar/baz.txt");
        assert_eq!(sanitize_archive_path("".to_string()), "");
    }

    #[test]
    fn test_apfs_preallocate_invalid_fd() {
        assert!(!apfs_preallocate(-1, 1024));
        assert!(!apfs_preallocate(1, -10));
    }

    #[test]
    fn test_apfs_clone_range_invalid_fd() {
        assert!(!apfs_clone_range(-1, -1));
    }
}
