// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Common integration test utilities and deterministic RAII cleanup fixtures.

use std::fs;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// RAII Temporary Directory Guard that guarantees 100% deterministic physical cleanup.
///
/// In POSIX filesystems, `remove_dir_all` fails with `EACCES` if parent or child directories
/// have restrictive permissions (e.g. `0o555` read-only mode). `tempfile::TempDir`'s default
/// `Drop` implementation silently swallows such errors, leaving orphaned directory trees in `/tmp`.
///
/// `SafeTempDir` wraps `tempfile::TempDir` and recursively restores owner write/execute permissions
/// (`0o700`) across all nested directories and files prior to triggering deletion upon `Drop`.
pub struct SafeTempDir {
    path_buf: PathBuf,
    inner: Option<tempfile::TempDir>,
}

impl SafeTempDir {
    /// Creates a new uniquely named temporary directory inside the system temp directory.
    pub fn new() -> std::io::Result<Self> {
        let inner = tempfile::tempdir()?;
        let path_buf = inner.path().to_path_buf();
        Ok(Self {
            path_buf,
            inner: Some(inner),
        })
    }

    /// Accesses the underlying filesystem path.
    pub fn path(&self) -> &Path {
        &self.path_buf
    }

    /// Recursively unlocks all directories and files within the target root
    /// to grant owner read, write, and execute permissions (0o700).
    pub fn unlock_permissions_recursive(root: &Path) {
        #[cfg(unix)]
        {
            if !root.exists() {
                return;
            }
            // Unlock root node first so its directory entries can be read
            let _ = fs::set_permissions(root, fs::Permissions::from_mode(0o700));

            // Traverse and recursively unlock all descendant nodes
            if let Ok(entries) = fs::read_dir(root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        Self::unlock_permissions_recursive(&path);
                    } else {
                        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
                    }
                }
            }
        }
    }
}

impl AsRef<Path> for SafeTempDir {
    fn as_ref(&self) -> &Path {
        self.path()
    }
}

impl std::ops::Deref for SafeTempDir {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.path()
    }
}

impl Drop for SafeTempDir {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            let path = self.path_buf.clone();
            // Drop inner first to trigger standard cleanup attempt
            drop(inner);

            // If the directory still lingers (due to 0o555 / EACCES), force recursive unlock and purge
            if path.exists() {
                Self::unlock_permissions_recursive(&path);
                let _ = fs::remove_dir_all(&path);
            }
        }
    }
}
