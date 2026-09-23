// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! RAII-based atomic extraction staging sandbox.
//!
//! Decompresses archive contents into a hidden sibling directory on the same filesystem volume.
//! Upon successful completion, the staging directory is atomically committed to the final destination.
//! If extraction encounters an error, is cancelled, or panics, `StagingSandbox` automatically wipes
//! the staging directory on drop, preventing truncated or corrupted files from polluting the destination.

use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use crate::types::TTZipStatus;

static STAGING_COUNTER: AtomicU64 = AtomicU64::new(1);

/// RAII staging sandbox guaranteeing atomic extraction commit and cleanup on failure.
#[derive(Debug)]
pub struct StagingSandbox {
    final_dest: PathBuf,
    staging_dir: PathBuf,
    committed: bool,
}

impl StagingSandbox {
    /// Creates a new staging sandbox located in the same parent directory and volume as `final_dest`.
    pub fn new(final_dest: &Path) -> Result<Self, TTZipStatus> {
        let parent = match final_dest.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };

        if !parent.exists() {
            fs::create_dir_all(&parent).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        }

        let pid = std::process::id();
        let counter = STAGING_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dest_name = final_dest
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("archive");
        let staging_name = format!(".ttzip_staging_{}_{}_{}", dest_name, pid, counter);
        let staging_dir = parent.join(staging_name);

        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        builder.mode(0o700);
        builder.create(&staging_dir).map_err(|_| TTZipStatus::ErrOpenFailed)?;

        Ok(Self {
            final_dest: final_dest.to_path_buf(),
            staging_dir,
            committed: false,
        })
    }

    /// Returns the active staging directory path where files should be extracted.
    #[inline]
    pub fn path(&self) -> &Path {
        &self.staging_dir
    }

    /// Atomically commits the staged files to the final destination directory.
    pub fn commit(mut self) -> Result<(), TTZipStatus> {
        if self.committed {
            return Ok(());
        }

        // If final_dest doesn't exist, simple atomic rename
        if !self.final_dest.exists() {
            fs::rename(&self.staging_dir, &self.final_dest)
                .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
            self.committed = true;
            return Ok(());
        }

        // If final_dest already exists:
        // Try atomic rename first (succeeds on Unix if final_dest is an empty directory)
        if fs::rename(&self.staging_dir, &self.final_dest).is_ok() {
            self.committed = true;
            return Ok(());
        }

        // Otherwise promote items from staging_dir into final_dest
        Self::promote_dir_contents(&self.staging_dir, &self.final_dest)?;

        // Remove the empty staging dir
        let _ = fs::remove_dir_all(&self.staging_dir);
        self.committed = true;
        Ok(())
    }

    fn promote_dir_contents(src: &Path, dst: &Path) -> Result<(), TTZipStatus> {
        let entries = fs::read_dir(src).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        for entry in entries {
            let entry = entry.map_err(|_| TTZipStatus::ErrExtractionFailed)?;
            let src_path = entry.path();
            let file_name = entry.file_name();
            let dst_path = dst.join(file_name);

            // Check if dst_path exists and is a symlink; if so, sever it immediately to prevent symlink traversal.
            if let Ok(dst_meta) = fs::symlink_metadata(&dst_path) {
                if dst_meta.file_type().is_symlink() {
                    fs::remove_file(&dst_path).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                }
            }

            if let Ok(meta) = fs::symlink_metadata(&src_path) {
                if meta.is_dir() {
                    if !dst_path.exists() {
                        fs::create_dir_all(&dst_path).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                    }
                    Self::promote_dir_contents(&src_path, &dst_path)?;
                } else if meta.file_type().is_symlink() {
                    if dst_path.exists() || fs::symlink_metadata(&dst_path).is_ok() {
                        let _ = fs::remove_file(&dst_path);
                    }
                    if fs::rename(&src_path, &dst_path).is_err() {
                        let link_target = fs::read_link(&src_path).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                        #[cfg(unix)]
                        std::os::unix::fs::symlink(&link_target, &dst_path).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                        let _ = fs::remove_file(&src_path);
                    }
                } else {
                    if dst_path.exists() || fs::symlink_metadata(&dst_path).is_ok() {
                        let _ = fs::remove_file(&dst_path);
                    }
                    if fs::rename(&src_path, &dst_path).is_err() {
                        fs::copy(&src_path, &dst_path).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                        let _ = fs::remove_file(&src_path);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Drop for StagingSandbox {
    fn drop(&mut self) {
        if !self.committed && self.staging_dir.exists() {
            let _ = fs::remove_dir_all(&self.staging_dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_staging_promote_symlink_directory_immunity() {
        let tmp = tempdir().unwrap();
        let final_dest = tmp.path().join("final_target");
        fs::create_dir_all(&final_dest).unwrap();

        // Create an outside target directory
        let outside_dir = tmp.path().join("outside_vault");
        fs::create_dir_all(&outside_dir).unwrap();

        // Place a symlink inside final_dest pointing to outside_dir
        let symlink_in_dest = final_dest.join("infiltrated_link");
        std::os::unix::fs::symlink(&outside_dir, &symlink_in_dest).unwrap();

        // Now create a staging sandbox that extracts a directory named "infiltrated_link"
        let sandbox = StagingSandbox::new(&final_dest).unwrap();
        let staged_dir = sandbox.path().join("infiltrated_link");
        fs::create_dir_all(&staged_dir).unwrap();
        let payload = staged_dir.join("pwn.txt");
        let mut f = fs::File::create(&payload).unwrap();
        f.write_all(b"attack payload").unwrap();
        drop(f);

        // Commit sandbox
        sandbox.commit().unwrap();

        // Verify that outside_dir was NOT written to
        assert!(!outside_dir.join("pwn.txt").exists());
        // Verify that the destination now contains the directory with pwn.txt and is not a symlink
        assert!(final_dest.join("infiltrated_link/pwn.txt").exists());
        let meta = fs::symlink_metadata(final_dest.join("infiltrated_link")).unwrap();
        assert!(!meta.file_type().is_symlink());
        assert!(meta.is_dir());
    }

    #[test]
    fn test_staging_promote_symlink_preservation() {
        let tmp = tempdir().unwrap();
        let final_dest = tmp.path().join("final_target");
        fs::create_dir_all(&final_dest).unwrap();

        let sandbox = StagingSandbox::new(&final_dest).unwrap();
        let staged_file = sandbox.path().join("real.txt");
        fs::write(&staged_file, b"content").unwrap();

        let staged_link = sandbox.path().join("link.txt");
        std::os::unix::fs::symlink("real.txt", &staged_link).unwrap();

        sandbox.commit().unwrap();

        let promoted_link = final_dest.join("link.txt");
        let meta = fs::symlink_metadata(&promoted_link).unwrap();
        assert!(meta.file_type().is_symlink());
        assert_eq!(fs::read_link(&promoted_link).unwrap(), PathBuf::from("real.txt"));
    }
}
