// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Core TTZipEntry metadata structure and cross-platform lazy filesystem stat adapter.

use std::collections::BTreeMap;
use std::path::Path;

use super::fields::EntryFields;
use super::sparse::{coalesce_sparse_extents, SparseExtent};
use super::timestamp::TTZipTimestamp;
use super::types::TTZipFileType;

/// Comprehensive archive entry metadata with nanosecond precision and extended attributes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TTZipEntry {
    pub pathname: String,
    pub pathname_mbs: Option<Vec<u8>>,
    pub pathname_wcs: Option<Vec<u32>>,
    pub file_type: TTZipFileType,
    pub size: u64,
    pub mtime: Option<TTZipTimestamp>,
    pub atime: Option<TTZipTimestamp>,
    pub ctime: Option<TTZipTimestamp>,
    pub birthtime: Option<TTZipTimestamp>,
    pub mode: u32,
    pub uid: u64,
    pub gid: u64,
    pub uname: Option<String>,
    pub gname: Option<String>,
    pub ino: u64,
    pub dev: u64,
    pub rdev: u64,
    pub nlink: u64,
    pub symlink_target: Option<String>,
    pub symlink_target_mbs: Option<Vec<u8>>,
    pub symlink_target_wcs: Option<Vec<u32>>,
    pub hardlink_target: Option<String>,
    pub hardlink_target_mbs: Option<Vec<u8>>,
    pub hardlink_target_wcs: Option<Vec<u32>>,
    pub xattrs: BTreeMap<String, Vec<u8>>,
    pub acls: Vec<String>,
    pub sparse_extents: Vec<SparseExtent>,
    pub crc32: Option<u32>,
    pub flags: u32,
    pub fields: EntryFields,
}

impl TTZipEntry {
    /// Creates an empty entry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a regular file entry.
    pub fn new_file(path: impl Into<String>, size: u64) -> Self {
        let mut entry = Self::new();
        entry.set_pathname(path);
        entry.set_file_type(TTZipFileType::RegularFile);
        entry.set_size(size);
        entry.set_mode(0o100644);
        entry
    }

    /// Creates a directory entry.
    pub fn new_dir(path: impl Into<String>) -> Self {
        let mut entry = Self::new();
        entry.set_pathname(path);
        entry.set_file_type(TTZipFileType::Directory);
        entry.set_size(0);
        entry.set_mode(0o040755);
        entry
    }

    /// Creates a symbolic link entry.
    pub fn new_symlink(path: impl Into<String>, target: impl Into<String>) -> Self {
        let mut entry = Self::new();
        entry.set_pathname(path);
        entry.set_file_type(TTZipFileType::Symlink);
        entry.set_symlink(target);
        entry.set_size(0);
        entry.set_mode(0o120777);
        entry
    }

    /// Creates a hard link entry.
    pub fn new_hardlink(path: impl Into<String>, target: impl Into<String>) -> Self {
        let mut entry = Self::new();
        entry.set_pathname(path);
        entry.set_file_type(TTZipFileType::Hardlink);
        entry.set_hardlink(target);
        entry.set_size(0);
        entry.set_mode(0o100644);
        entry
    }

    #[inline]
    pub fn is_field_set(&self, field: EntryFields) -> bool {
        self.fields.contains(field)
    }

    pub fn unset_field(&mut self, field: EntryFields) {
        if field.contains(EntryFields::PATHNAME) {
            self.pathname.clear();
            self.pathname_mbs = None;
            self.pathname_wcs = None;
        }
        if field.contains(EntryFields::SIZE) {
            self.size = 0;
        }
        if field.contains(EntryFields::MTIME) {
            self.mtime = None;
        }
        if field.contains(EntryFields::ATIME) {
            self.atime = None;
        }
        if field.contains(EntryFields::CTIME) {
            self.ctime = None;
        }
        if field.contains(EntryFields::BIRTHTIME) {
            self.birthtime = None;
        }
        if field.contains(EntryFields::PERMISSIONS) {
            self.mode &= !0o7777;
        }
        if field.contains(EntryFields::FILE_TYPE) {
            self.file_type = TTZipFileType::Unknown;
            self.mode &= 0o7777;
        }
        if field.contains(EntryFields::UID) {
            self.uid = 0;
        }
        if field.contains(EntryFields::GID) {
            self.gid = 0;
        }
        if field.contains(EntryFields::UNAME) {
            self.uname = None;
        }
        if field.contains(EntryFields::GNAME) {
            self.gname = None;
        }
        if field.contains(EntryFields::INO) {
            self.ino = 0;
        }
        if field.contains(EntryFields::DEV) {
            self.dev = 0;
        }
        if field.contains(EntryFields::RDEV) {
            self.rdev = 0;
        }
        if field.contains(EntryFields::NLINK) {
            self.nlink = 0;
        }
        if field.contains(EntryFields::SYMLINK) {
            self.symlink_target = None;
            self.symlink_target_mbs = None;
            self.symlink_target_wcs = None;
        }
        if field.contains(EntryFields::HARDLINK) {
            self.hardlink_target = None;
            self.hardlink_target_mbs = None;
            self.hardlink_target_wcs = None;
        }
        if field.contains(EntryFields::XATTRS) {
            self.xattrs.clear();
        }
        if field.contains(EntryFields::ACLS) {
            self.acls.clear();
        }
        if field.contains(EntryFields::SPARSE) {
            self.sparse_extents.clear();
        }
        if field.contains(EntryFields::DIGEST) {
            self.crc32 = None;
        }
        if field.contains(EntryFields::FLAGS) {
            self.flags = 0;
        }
        self.fields.remove(field);
    }

    pub fn set_pathname(&mut self, path: impl Into<String>) {
        let s = path.into();
        self.pathname_mbs = Some(s.as_bytes().to_vec());
        self.pathname_wcs = Some(s.chars().map(|c| c as u32).collect());
        self.pathname = s;
        self.fields.insert(EntryFields::PATHNAME);
    }

    pub fn set_pathname_mbs(&mut self, bytes: &[u8]) {
        self.pathname_mbs = Some(bytes.to_vec());
        let s = String::from_utf8_lossy(bytes).into_owned();
        self.pathname_wcs = Some(s.chars().map(|c| c as u32).collect());
        self.pathname = s;
        self.fields.insert(EntryFields::PATHNAME);
    }

    pub fn set_pathname_wcs(&mut self, wcs: &[u32]) {
        self.pathname_wcs = Some(wcs.to_vec());
        let s: String = wcs
            .iter()
            .filter_map(|&cp| char::from_u32(cp))
            .collect();
        self.pathname_mbs = Some(s.as_bytes().to_vec());
        self.pathname = s;
        self.fields.insert(EntryFields::PATHNAME);
    }

    pub fn set_file_type(&mut self, file_type: TTZipFileType) {
        self.file_type = file_type;
        self.mode = (self.mode & 0o7777) | file_type.to_posix_mode_bits();
        self.fields.insert(EntryFields::FILE_TYPE);
    }

    pub fn set_size(&mut self, size: u64) {
        self.size = size;
        self.fields.insert(EntryFields::SIZE);
    }

    pub fn set_mtime(&mut self, ts: TTZipTimestamp) {
        self.mtime = Some(ts);
        self.fields.insert(EntryFields::MTIME);
    }

    pub fn set_atime(&mut self, ts: TTZipTimestamp) {
        self.atime = Some(ts);
        self.fields.insert(EntryFields::ATIME);
    }

    pub fn set_ctime(&mut self, ts: TTZipTimestamp) {
        self.ctime = Some(ts);
        self.fields.insert(EntryFields::CTIME);
    }

    pub fn set_birthtime(&mut self, ts: TTZipTimestamp) {
        self.birthtime = Some(ts);
        self.fields.insert(EntryFields::BIRTHTIME);
    }

    pub fn set_mode(&mut self, mode: u32) {
        self.mode = mode;
        let ft = TTZipFileType::from_posix_mode(mode);
        if ft != TTZipFileType::Unknown {
            self.file_type = ft;
            self.fields.insert(EntryFields::FILE_TYPE);
        }
        self.fields.insert(EntryFields::PERMISSIONS);
    }

    pub fn set_permissions(&mut self, perms: u32) {
        self.mode = (self.mode & 0o170000) | (perms & 0o7777);
        self.fields.insert(EntryFields::PERMISSIONS);
    }

    pub fn set_uid(&mut self, uid: u64) {
        self.uid = uid;
        self.fields.insert(EntryFields::UID);
    }

    pub fn set_gid(&mut self, gid: u64) {
        self.gid = gid;
        self.fields.insert(EntryFields::GID);
    }

    pub fn set_uname(&mut self, uname: impl Into<String>) {
        self.uname = Some(uname.into());
        self.fields.insert(EntryFields::UNAME);
    }

    pub fn set_gname(&mut self, gname: impl Into<String>) {
        self.gname = Some(gname.into());
        self.fields.insert(EntryFields::GNAME);
    }

    pub fn set_ino(&mut self, ino: u64) {
        self.ino = ino;
        self.fields.insert(EntryFields::INO);
    }

    pub fn set_dev(&mut self, dev: u64) {
        self.dev = dev;
        self.fields.insert(EntryFields::DEV);
    }

    pub fn set_rdev(&mut self, rdev: u64) {
        self.rdev = rdev;
        self.fields.insert(EntryFields::RDEV);
    }

    pub fn set_nlink(&mut self, nlink: u64) {
        self.nlink = nlink;
        self.fields.insert(EntryFields::NLINK);
    }

    pub fn set_symlink(&mut self, target: impl Into<String>) {
        let s = target.into();
        self.symlink_target_mbs = Some(s.as_bytes().to_vec());
        self.symlink_target_wcs = Some(s.chars().map(|c| c as u32).collect());
        self.symlink_target = Some(s);
        self.file_type = TTZipFileType::Symlink;
        self.fields.insert(EntryFields::SYMLINK | EntryFields::FILE_TYPE);
    }

    pub fn set_hardlink(&mut self, target: impl Into<String>) {
        let s = target.into();
        self.hardlink_target_mbs = Some(s.as_bytes().to_vec());
        self.hardlink_target_wcs = Some(s.chars().map(|c| c as u32).collect());
        self.hardlink_target = Some(s);
        self.file_type = TTZipFileType::Hardlink;
        self.fields.insert(EntryFields::HARDLINK | EntryFields::FILE_TYPE);
    }

    pub fn add_xattr(&mut self, key: impl Into<String>, value: Vec<u8>) {
        self.xattrs.insert(key.into(), value);
        self.fields.insert(EntryFields::XATTRS);
    }

    pub fn add_acl(&mut self, acl: impl Into<String>) {
        self.acls.push(acl.into());
        self.fields.insert(EntryFields::ACLS);
    }

    pub fn add_sparse_extent(&mut self, offset: u64, length: u64) {
        self.sparse_extents.push(SparseExtent::new(offset, length));
        coalesce_sparse_extents(&mut self.sparse_extents);
        self.fields.insert(EntryFields::SPARSE);
    }

    pub fn set_sparse_extents(&mut self, mut extents: Vec<SparseExtent>) {
        coalesce_sparse_extents(&mut extents);
        self.sparse_extents = extents;
        self.fields.insert(EntryFields::SPARSE);
    }

    pub fn set_crc32(&mut self, crc: u32) {
        self.crc32 = Some(crc);
        self.fields.insert(EntryFields::DIGEST);
    }

    pub fn set_flags(&mut self, flags: u32) {
        self.flags = flags;
        self.fields.insert(EntryFields::FLAGS);
    }

    /// Constructs a `TTZipEntry` by reading file system metadata (lazy stat adapter).
    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        let meta = std::fs::symlink_metadata(path)?;
        Ok(Self::from_metadata(path, &meta))
    }

    /// Populates entry from existing filesystem metadata.
    pub fn from_metadata(path: &Path, meta: &std::fs::Metadata) -> Self {
        let mut entry = Self::new();
        entry.set_pathname(path.to_string_lossy().to_string());
        entry.set_size(meta.len());

        let ft = meta.file_type();
        if ft.is_dir() {
            entry.set_file_type(TTZipFileType::Directory);
        } else if ft.is_symlink() {
            entry.set_file_type(TTZipFileType::Symlink);
            if let Ok(target) = std::fs::read_link(path) {
                entry.set_symlink(target.to_string_lossy().to_string());
            }
        } else {
            entry.set_file_type(TTZipFileType::RegularFile);
        }

        if let Ok(mtime) = meta.modified() {
            entry.set_mtime(TTZipTimestamp::from_system_time(mtime));
        }
        if let Ok(atime) = meta.accessed() {
            entry.set_atime(TTZipTimestamp::from_system_time(atime));
        }
        if let Ok(created) = meta.created() {
            entry.set_birthtime(TTZipTimestamp::from_system_time(created));
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            entry.set_mode(meta.mode());
            entry.set_uid(meta.uid() as u64);
            entry.set_gid(meta.gid() as u64);
            entry.set_ino(meta.ino());
            entry.set_dev(meta.dev());
            entry.set_rdev(meta.rdev());
            entry.set_nlink(meta.nlink());

            entry.set_mtime(TTZipTimestamp::new(meta.mtime(), meta.mtime_nsec() as u32));
            entry.set_atime(TTZipTimestamp::new(meta.atime(), meta.atime_nsec() as u32));
            entry.set_ctime(TTZipTimestamp::new(meta.ctime(), meta.ctime_nsec() as u32));
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            entry.set_flags(meta.file_attributes());
        }

        entry
    }
}
