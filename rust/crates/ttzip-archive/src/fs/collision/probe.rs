// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Destination probe.

use std::os::unix::io::RawFd;
use std::path::Path;
use crate::fs::collision::ItemMeta;
use crate::fs::collision::fs_ops::FsOps;

#[derive(Debug, Clone)]
pub enum ProbeResult {
    Absent,
    Item(ItemMeta, String), // Meta and on-disk name
}

pub fn probe_destination<F: FsOps>(fs: &F, dirfd: RawFd, path: &Path) -> Result<ProbeResult, std::io::Error> {
    if let Some(meta) = fs.fstatat_nofollow(dirfd, path)? {
        let name = fs.getattrlistat_name(dirfd, path)?.unwrap_or_else(|| {
            path.file_name().unwrap_or_default().to_string_lossy().into_owned()
        });
        Ok(ProbeResult::Item(meta, name))
    } else {
        Ok(ProbeResult::Absent)
    }
}
