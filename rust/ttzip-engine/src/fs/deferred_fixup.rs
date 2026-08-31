// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Depth-first reverse deferred directory metadata restoration engine.
//!
//! Inspired by libarchive's `archive_write_disk_posix.c` fixup list architecture.
//! Ensures directories are created with temporary permissive access (`0o700`) during
//! extraction, and deferred attributes (read-only permissions, POSIX/NFSv4 ACLs, ownership,
//! and nanosecond modification timestamps) are restored in strictly descending depth order
//! (deepest leaf directory to root directory) after all child entries have been extracted.

use crate::archive::unified::entry::timestamp::TTZipTimestamp;
use crate::archive::unified::entry::TTZipEntry;
use crate::fs::safe_extract::validate_no_intermediate_symlinks;
use crate::security::acl::Acl;
use crate::types::TTZipStatus;

use std::collections::BTreeMap;
use std::ffi::CString;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
extern "C" {
    fn acl_from_text(buf_p: *const libc::c_char) -> *mut libc::c_void;
    fn acl_set_file(path_p: *const libc::c_char, type_: libc::c_int, acl: *mut libc::c_void) -> libc::c_int;
    fn acl_free(obj_p: *mut libc::c_void) -> libc::c_int;
}

#[cfg(target_os = "macos")]
const ACL_TYPE_EXTENDED: libc::c_int = 0x00000100;

/// Metadata record for a directory pending post-extraction deferred attribute restoration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirFixupItem {
    /// Absolute or destination-relative physical directory path on disk.
    pub path: PathBuf,
    /// Target POSIX file mode (permissions, suid/sgid/sticky bits).
    pub mode: Option<u32>,
    /// High-precision modification timestamp (mtime).
    pub mtime: Option<TTZipTimestamp>,
    /// High-precision access timestamp (atime).
    pub atime: Option<TTZipTimestamp>,
    /// Target user identifier (UID).
    pub uid: Option<u64>,
    /// Target group identifier (GID).
    pub gid: Option<u64>,
    /// Access Control List (ACL) specification.
    pub acl: Option<Acl>,
    /// Raw ACL text fallback representation.
    pub acl_text: Option<String>,
}

impl DirFixupItem {
    /// Creates a new directory fixup item for `path` with unpopulated attributes.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            mode: None,
            mtime: None,
            atime: None,
            uid: None,
            gid: None,
            acl: None,
            acl_text: None,
        }
    }

    /// Creates a fixup item from a unified `TTZipEntry` metadata model.
    pub fn from_entry(entry: &TTZipEntry, destination_root: &Path) -> Self {
        let full_path = if Path::new(&entry.pathname).is_absolute() {
            PathBuf::from(&entry.pathname)
        } else {
            destination_root.join(&entry.pathname)
        };

        let mut item = Self::new(full_path);
        if entry.fields.contains(crate::archive::unified::entry::fields::EntryFields::PERMISSIONS) {
            item.mode = Some(entry.mode);
        }
        if entry.fields.contains(crate::archive::unified::entry::fields::EntryFields::MTIME) {
            item.mtime = entry.mtime;
        }
        if entry.fields.contains(crate::archive::unified::entry::fields::EntryFields::ATIME) {
            item.atime = entry.atime;
        }
        if entry.fields.contains(crate::archive::unified::entry::fields::EntryFields::UID) {
            item.uid = Some(entry.uid);
        }
        if entry.fields.contains(crate::archive::unified::entry::fields::EntryFields::GID) {
            item.gid = Some(entry.gid);
        }
        if !entry.acls.is_empty() {
            let joined_text = entry.acls.join("\n");
            if let Ok(parsed_acl) = Acl::parse_posix1e(&joined_text).or_else(|_| Acl::parse_nfs4(&joined_text)) {
                item.acl = Some(parsed_acl);
            }
            item.acl_text = Some(joined_text);
        }

        item
    }

    /// Builder method to attach target POSIX mode.
    pub fn with_mode(mut self, mode: u32) -> Self {
        self.mode = Some(mode);
        self
    }

    /// Builder method to attach target high-precision modification timestamp.
    pub fn with_mtime(mut self, mtime: TTZipTimestamp) -> Self {
        self.mtime = Some(mtime);
        self
    }

    /// Builder method to attach target high-precision access timestamp.
    pub fn with_atime(mut self, atime: TTZipTimestamp) -> Self {
        self.atime = Some(atime);
        self
    }

    /// Builder method to attach target owner UID and GID.
    pub fn with_owner(mut self, uid: u64, gid: u64) -> Self {
        self.uid = Some(uid);
        self.gid = Some(gid);
        self
    }

    /// Builder method to attach parsed ACL.
    pub fn with_acl(mut self, acl: Acl) -> Self {
        self.acl_text = Some(acl.to_text());
        self.acl = Some(acl);
        self
    }

    /// Builder method to attach raw ACL text.
    pub fn with_acl_text(mut self, text: impl Into<String>) -> Self {
        let s = text.into();
        if let Ok(parsed) = Acl::parse_posix1e(&s).or_else(|_| Acl::parse_nfs4(&s)) {
            self.acl = Some(parsed);
        }
        self.acl_text = Some(s);
        self
    }

    /// Computes the directory tree depth based on normalized path component count.
    #[inline]
    pub fn depth(&self) -> usize {
        self.path.components().count()
    }

    /// Merges attributes from another fixup item into this record.
    pub fn merge_with(&mut self, other: Self) {
        if other.mode.is_some() {
            self.mode = other.mode;
        }
        if other.mtime.is_some() {
            self.mtime = other.mtime;
        }
        if other.atime.is_some() {
            self.atime = other.atime;
        }
        if other.uid.is_some() {
            self.uid = other.uid;
        }
        if other.gid.is_some() {
            self.gid = other.gid;
        }
        if other.acl.is_some() {
            self.acl = other.acl;
        }
        if other.acl_text.is_some() {
            self.acl_text = other.acl_text;
        }
    }
}

/// Reverse depth-first directory attributes and timestamp fixup engine.
#[derive(Debug, Default)]
pub struct DepthFirstDirFixup {
    items: BTreeMap<PathBuf, DirFixupItem>,
}

impl DepthFirstDirFixup {
    /// Creates a new empty fixup engine.
    pub fn new() -> Self {
        Self {
            items: BTreeMap::new(),
        }
    }

    /// Registers a directory fixup item, merging attributes if the path was already registered.
    pub fn register(&mut self, item: DirFixupItem) {
        let path = item.path.clone();
        self.items
            .entry(path)
            .and_modify(|existing| existing.merge_with(item.clone()))
            .or_insert(item);
    }

    /// Registers a directory with basic mode and timestamp attributes.
    pub fn register_dir(
        &mut self,
        path: &Path,
        mode: Option<u32>,
        mtime: Option<TTZipTimestamp>,
        atime: Option<TTZipTimestamp>,
    ) {
        let mut item = DirFixupItem::new(path.to_path_buf());
        item.mode = mode;
        item.mtime = mtime;
        item.atime = atime;
        self.register(item);
    }

    /// Creates directory hierarchy with temporary permissive permissions (`0o700`),
    /// ensuring subfiles can be created without permission obstacles, and registers
    /// the directory for deferred attribute restoration.
    pub fn create_dir_all_secure(
        &mut self,
        path: &Path,
        mode: Option<u32>,
        mtime: Option<TTZipTimestamp>,
        atime: Option<TTZipTimestamp>,
    ) -> std::io::Result<()> {
        let parent = path.parent().unwrap_or(path);
        validate_no_intermediate_symlinks(parent, path)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Symlink in parent path"))?;

        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        builder.mode(0o700); // Stage 1: Temporary permissive user-accessible mode

        builder.create(path)?;

        self.register_dir(path, mode, mtime, atime);
        Ok(())
    }

    /// Returns the number of registered directory fixup records.
    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true if no directory fixup records are registered.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Clears all registered directory fixup records.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Returns all registered items sorted in strictly descending depth order (deepest child first).
    pub fn sorted_items_descending_depth(&self) -> Vec<DirFixupItem> {
        let mut list: Vec<(usize, usize, &DirFixupItem)> = self
            .items
            .values()
            .map(|item| (item.depth(), item.path.as_os_str().len(), item))
            .collect();
        // Sort by path depth descending, breaking ties deterministically with path length and lexical ordering
        list.sort_by(|(depth_a, len_a, item_a), (depth_b, len_b, item_b)| {
            depth_b
                .cmp(depth_a)
                .then_with(|| len_b.cmp(len_a))
                .then_with(|| item_b.path.cmp(&item_a.path))
        });
        list.into_iter().map(|(_, _, item)| item.clone()).collect()
    }

    /// Applies all deferred metadata attributes in reverse depth-first (bottom-up) order.
    ///
    /// For each directory from deepest leaf to root:
    /// 1. Ownership (`chown`/`lchown`) is applied if specified and permitted;
    /// 2. ACL (POSIX.1e or NFSv4) is applied if specified;
    /// 3. POSIX mode / permissions (`chmod`) are applied (preserving read-only states safely);
    /// 4. High-precision nanosecond timestamps (`utimensat`) are applied last, guaranteeing
    ///    that subsequent child operations never overwrite the parent directory's mtime.
    pub fn apply_all(&mut self, preserve_permissions: bool) -> Result<(), TTZipStatus> {
        let sorted = self.sorted_items_descending_depth();
        self.items.clear();

        for item in sorted {
            Self::apply_single_dir_fixup(&item, preserve_permissions)?;
        }

        Ok(())
    }

    /// Applies deferred metadata attributes to a single directory.
    fn apply_single_dir_fixup(item: &DirFixupItem, preserve_permissions: bool) -> Result<(), TTZipStatus> {
        let path_str = match item.path.to_str() {
            Some(s) => s,
            None => return Err(TTZipStatus::ErrInvalidParam),
        };

        let c_path = match CString::new(path_str) {
            Ok(c) => c,
            Err(_) => return Err(TTZipStatus::ErrInvalidParam),
        };

        unsafe {
            // Verify path existence and verify it is not a symlink (TOCTOU defense)
            let mut st: libc::stat = std::mem::zeroed();
            if libc::lstat(c_path.as_ptr(), &mut st) != 0 {
                return Err(TTZipStatus::ErrFileNotFound);
            }

            if (st.st_mode & libc::S_IFMT) == libc::S_IFLNK {
                return Err(TTZipStatus::ErrSecurityViolation);
            }

            // 1. Apply UID / GID ownership if requested (ignore EPERM for non-root users)
            if item.uid.is_some() || item.gid.is_some() {
                let target_uid = item.uid.map(|u| u as libc::uid_t).unwrap_or(st.st_uid);
                let target_gid = item.gid.map(|g| g as libc::gid_t).unwrap_or(st.st_gid);
                let _ = libc::lchown(c_path.as_ptr(), target_uid, target_gid);
            }

            // 2. Apply ACL if present
            if let Some(acl) = &item.acl {
                Self::apply_acl_internal(&c_path, acl)?;
            } else if let Some(acl_text) = &item.acl_text {
                Self::apply_acl_text_internal(&c_path, acl_text)?;
            }

            // 3. Apply POSIX mode / permissions
            if preserve_permissions {
                if let Some(target_mode) = item.mode {
                    let mode_bits = (target_mode & 0o7777) as libc::mode_t;
                    if libc::chmod(c_path.as_ptr(), mode_bits) != 0 {
                        return Err(TTZipStatus::ErrExtractionFailed);
                    }
                }
            }

            // 4. Apply high-precision nanosecond timestamps (applied last to preserve parent mtime)
            if item.mtime.is_some() || item.atime.is_some() {
                let mtime = item.mtime.unwrap_or_else(|| TTZipTimestamp::new(st.st_mtime as i64, 0));
                let atime = item.atime.unwrap_or_else(|| TTZipTimestamp::new(st.st_atime as i64, 0));

                let times = [
                    libc::timespec {
                        tv_sec: atime.sec as libc::time_t,
                        tv_nsec: atime.nsec as libc::c_long,
                    },
                    libc::timespec {
                        tv_sec: mtime.sec as libc::time_t,
                        tv_nsec: mtime.nsec as libc::c_long,
                    },
                ];

                let res = libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), libc::AT_SYMLINK_NOFOLLOW);
                if res != 0 {
                    return Err(TTZipStatus::ErrExtractionFailed);
                }
            }
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn apply_acl_internal(c_path: &CString, acl: &Acl) -> Result<(), TTZipStatus> {
        let acl_text = acl.to_text();
        Self::apply_acl_text_internal(c_path, &acl_text)
    }

    #[cfg(not(target_os = "macos"))]
    fn apply_acl_internal(_c_path: &CString, _acl: &Acl) -> Result<(), TTZipStatus> {
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn apply_acl_text_internal(c_path: &CString, acl_text: &str) -> Result<(), TTZipStatus> {
        if acl_text.trim().is_empty() {
            return Ok(());
        }

        let c_acl_text = match CString::new(acl_text) {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };

        unsafe {
            let native_acl = acl_from_text(c_acl_text.as_ptr());
            if !native_acl.is_null() {
                let _ = acl_set_file(c_path.as_ptr(), ACL_TYPE_EXTENDED, native_acl);
                acl_free(native_acl);
            }
        }

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    fn apply_acl_text_internal(_c_path: &CString, _acl_text: &str) -> Result<(), TTZipStatus> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn test_dir_fixup_descending_depth_sorting() {
        let mut fixup = DepthFirstDirFixup::new();

        fixup.register_dir(Path::new("/a"), Some(0o755), None, None);
        fixup.register_dir(Path::new("/a/b/c/d"), Some(0o755), None, None);
        fixup.register_dir(Path::new("/a/b"), Some(0o755), None, None);
        fixup.register_dir(Path::new("/a/b/c"), Some(0o755), None, None);

        let sorted = fixup.sorted_items_descending_depth();
        assert_eq!(sorted.len(), 4);
        assert_eq!(sorted[0].path, PathBuf::from("/a/b/c/d"));
        assert_eq!(sorted[1].path, PathBuf::from("/a/b/c"));
        assert_eq!(sorted[2].path, PathBuf::from("/a/b"));
        assert_eq!(sorted[3].path, PathBuf::from("/a"));
    }

    #[test]
    fn test_dir_fixup_merge_attributes() {
        let mut fixup = DepthFirstDirFixup::new();
        let path = Path::new("/target/dir");

        fixup.register_dir(path, Some(0o755), None, None);
        fixup.register_dir(path, None, Some(TTZipTimestamp::new(1700000000, 500)), None);

        let sorted = fixup.sorted_items_descending_depth();
        assert_eq!(sorted.len(), 1);
        assert_eq!(sorted[0].mode, Some(0o755));
        assert_eq!(sorted[0].mtime, Some(TTZipTimestamp::new(1700000000, 500)));
    }

    #[test]
    fn test_create_dir_all_secure_and_apply() {
        let tmp = tempdir().unwrap();
        let parent = tmp.path().join("parent_readonly");
        let child = parent.join("child_readonly");

        let mut fixup = DepthFirstDirFixup::new();
        fixup
            .create_dir_all_secure(&child, Some(0o555), Some(TTZipTimestamp::new(1700000000, 100)), None)
            .unwrap();
        fixup.register_dir(&parent, Some(0o555), Some(TTZipTimestamp::new(1700000000, 200)), None);

        // Before fixup, directory is writable
        let meta_before = fs::metadata(&child).unwrap();
        assert_eq!(meta_before.permissions().mode() & 0o777, 0o700);

        // Apply fixup bottom-up
        fixup.apply_all(true).unwrap();

        let meta_parent = fs::metadata(&parent).unwrap();
        assert_eq!(meta_parent.permissions().mode() & 0o777, 0o555);

        let meta_child = fs::metadata(&child).unwrap();
        assert_eq!(meta_child.permissions().mode() & 0o777, 0o555);
    }
}
