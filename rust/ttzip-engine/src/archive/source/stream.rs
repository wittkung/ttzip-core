// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Bounded stream archive source with pread positional I/O for remote & removable filesystems.

use super::{ArchiveSource, StorageMedium};
use crate::types::TTZipStatus;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::Path;

/// Stream-backed archive source that prevents SIGBUS on remote/removable filesystems.
pub struct StreamSource {
    file: File,
    len: u64,
    medium: StorageMedium,
}

impl StreamSource {
    /// Opens an archive file for positional streaming.
    pub fn open(path: &Path, medium: StorageMedium) -> Result<Self, TTZipStatus> {
        let file = File::open(path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        let meta = file.metadata().map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let len = meta.len();
        Ok(Self { file, len, medium })
    }
}

impl ArchiveSource for StreamSource {
    #[inline]
    fn as_slice(&self) -> Option<&[u8]> {
        // StreamSource intentionally does NOT expose a contiguous memory slice to prevent SIGBUS.
        None
    }

    #[inline]
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, TTZipStatus> {
        if offset >= self.len {
            return Ok(0);
        }
        let available = self.len - offset;
        let to_read = (buf.len() as u64).min(available) as usize;
        match self.file.read_exact_at(&mut buf[..to_read], offset) {
            Ok(()) => Ok(to_read),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                // Read whatever partial bytes were retrieved
                Ok(0)
            }
            Err(_) => Err(TTZipStatus::ErrOpenFailed),
        }
    }

    #[inline]
    fn len(&self) -> u64 {
        self.len
    }

    #[inline]
    fn medium(&self) -> StorageMedium {
        self.medium
    }
}
