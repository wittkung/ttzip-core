// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! T001 Fixture generator for collision tests.

use std::fs;
use std::path::Path;

pub struct CollisionFixture {
    pub root: tempfile::TempDir,
}

impl CollisionFixture {
    pub fn new() -> std::io::Result<Self> {
        let root = tempfile::tempdir()?;
        Ok(Self { root })
    }
    
    pub fn create_file(&self, path: &str, size: u64) -> std::io::Result<()> {
        let full = self.root.path().join(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = vec![0u8; size as usize];
        fs::write(&full, &data)
    }
    
    pub fn create_dir(&self, path: &str) -> std::io::Result<()> {
        let full = self.root.path().join(path);
        fs::create_dir_all(&full)
    }
    
    pub fn create_symlink(&self, target: &str, link: &str) -> std::io::Result<()> {
        let full_link = self.root.path().join(link);
        if let Some(parent) = full_link.parent() {
            fs::create_dir_all(parent)?;
        }
        std::os::unix::fs::symlink(target, full_link)
    }
    
    pub fn assert_fd_leak<F: FnOnce(&Path)>(&self, op: F) {
        let fds_before = fs::read_dir("/dev/fd").unwrap().count();
        op(self.root.path());
        let fds_after = fs::read_dir("/dev/fd").unwrap().count();
        assert_eq!(fds_before, fds_after, "fd leak detected: before {}, after {}", fds_before, fds_after);
    }
}
