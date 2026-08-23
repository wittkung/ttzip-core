// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Unified Archive Source abstraction with filesystem-aware medium dispatch.

pub mod factory;
pub mod mmap;
pub mod stream;

pub use factory::*;
pub use mmap::*;
pub use stream::*;

use crate::types::TTZipStatus;

/// Storage medium backing the archive file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageMedium {
    /// Local APFS internal NVMe SSD with zero-copy CoW and mmap fast path.
    LocalFastApfs,
    /// Local HFS+ or standard filesystem.
    LocalStandard,
    /// Remote network mount (SMB, NFS, WebDAV).
    RemoteNetwork,
    /// Virtual filesystem (FUSE, FileProvider, Cloud).
    VirtualFilesystem,
}

/// Unified source interface for reading archive streams safely without OOM.
pub trait ArchiveSource: Send + Sync {
    /// Returns the full contiguous slice if backed by an mmap or in-memory buffer.
    fn as_slice(&self) -> Option<&[u8]>;

    /// Reads up to `buf.len()` bytes at `offset` without mutating cursor position.
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, TTZipStatus>;

    /// Returns the total archive length in bytes.
    fn len(&self) -> u64;

    /// Returns true if archive has 0 bytes.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the underlying storage medium.
    fn medium(&self) -> StorageMedium;
}
