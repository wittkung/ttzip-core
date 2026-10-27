// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Single-Root-FD (`DirJail`) step-by-step path resolver and chained-symlink defense.
//!
//! Enforces TOCTOU-immune extraction within a destination directory descriptor (`root_fd`):
//! 1. Every intermediate directory component is created via `mkdirat` and opened step-by-step
//!    using `openat` with `O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`.
//! 2. Leaf regular files are verified via `fstatat(..., AT_SYMLINK_NOFOLLOW)` to reject
//!    pre-existing symlinks or directories, then opened via `openat(..., O_NOFOLLOW)`.
//! 3. Symbolic links are validated against lexical stack underflow, ancestor-directory loops,
//!    and multi-hop / chained symlinks via step-by-step `fstatat` inspection from `root_fd`.

use crate::fs::safe_extract::sanitize_and_validate_path;
use crate::types::TTZipStatus;
use std::ffi::{CStr, CString};
use std::fs::{self, File};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::FromRawFd;
use std::path::{Component, Path, PathBuf};

#[inline]
const fn dir_step_open_flags() -> libc::c_int {
    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC
}

#[inline]
const fn file_leaf_open_flags(overwrite: bool) -> libc::c_int {
    let collision_bits = if overwrite {
        libc::O_TRUNC
    } else {
        libc::O_EXCL
    };
    libc::O_WRONLY | libc::O_CREAT | collision_bits | libc::O_NOFOLLOW | libc::O_CLOEXEC
}

struct OwnedFdGuard {
    fd: libc::c_int,
    owned: bool,
}

impl OwnedFdGuard {
    #[inline]
    const fn borrowed(fd: libc::c_int) -> Self {
        Self { fd, owned: false }
    }

    #[inline]
    const fn owned(fd: libc::c_int) -> Self {
        Self { fd, owned: true }
    }
}

impl Drop for OwnedFdGuard {
    fn drop(&mut self) {
        if self.owned && self.fd >= 0 {
            unsafe {
                libc::close(self.fd);
            }
        }
    }
}

/// Root-directory-handle (`DirFD`) sandbox jail for step-by-step extraction.
#[derive(Debug)]
pub struct DirJail {
    root_fd: libc::c_int,
    owns_fd: bool,
    dest_dir: PathBuf,
    canonical_dest_dir: Option<PathBuf>,
}

impl Drop for DirJail {
    fn drop(&mut self) {
        if self.owns_fd && self.root_fd >= 0 {
            unsafe {
                libc::close(self.root_fd);
            }
        }
    }
}

impl DirJail {
    /// Opens (or safely creates then opens) `dest_dir` as a non-symlink directory handle.
    pub fn open(dest_dir: &Path) -> Result<Self, TTZipStatus> {
        if let Ok(meta) = fs::symlink_metadata(dest_dir) {
            if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        } else {
            fs::create_dir_all(dest_dir).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        }

        let c_dest = CString::new(dest_dir.as_os_str().as_bytes())
            .map_err(|_| TTZipStatus::ErrInvalidParam)?;
        let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        let fd = unsafe { libc::open(c_dest.as_ptr(), flags) };
        if fd < 0 {
            let err = std::io::Error::last_os_error();
            if matches!(err.raw_os_error(), Some(libc::ELOOP | libc::ENOTDIR)) {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
            return Err(TTZipStatus::ErrOpenFailed);
        }

        if !verify_fd_is_dir(fd) {
            unsafe {
                libc::close(fd);
            }
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        Ok(Self {
            root_fd: fd,
            owns_fd: true,
            dest_dir: dest_dir.to_path_buf(),
            canonical_dest_dir: fs::canonicalize(dest_dir).ok(),
        })
    }

    /// Wraps an already-open directory descriptor without taking ownership of closing it.
    pub fn from_borrowed_fd(root_fd: libc::c_int) -> Result<Self, TTZipStatus> {
        if !verify_fd_is_dir(root_fd) {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        Ok(Self {
            root_fd,
            owns_fd: false,
            dest_dir: PathBuf::new(),
            canonical_dest_dir: None,
        })
    }

    /// Returns the underlying root directory file descriptor.
    #[inline]
    pub const fn root_fd(&self) -> libc::c_int {
        self.root_fd
    }

    /// Returns the destination directory path associated with this jail.
    #[inline]
    pub fn dest_dir(&self) -> &Path {
        &self.dest_dir
    }

    /// Extracts validated `Component::Normal` segments from a raw relative entry path.
    pub fn extract_clean_components(raw_entry_path: &str) -> Result<Vec<String>, TTZipStatus> {
        let dummy_root = Path::new("/ttzip_jail_root");
        let sanitized = sanitize_and_validate_path(dummy_root, raw_entry_path)?;
        let rel = sanitized
            .strip_prefix(dummy_root)
            .map_err(|_| TTZipStatus::ErrSecurityViolation)?;
        collect_normal_components(rel)
    }

    /// Extracts validated `Component::Normal` segments from either a `dest_dir`-prefixed path or relative path.
    pub fn resolve_path_components(&self, path: &Path) -> Result<Vec<String>, TTZipStatus> {
        let rel = if !self.dest_dir.as_os_str().is_empty() && path.starts_with(&self.dest_dir) {
            path.strip_prefix(&self.dest_dir)
                .map_err(|_| TTZipStatus::ErrSecurityViolation)?
        } else if let Some(canon) = self
            .canonical_dest_dir
            .as_ref()
            .filter(|c| path.starts_with(c))
        {
            path.strip_prefix(canon)
                .map_err(|_| TTZipStatus::ErrSecurityViolation)?
        } else if path.is_relative() {
            path
        } else {
            return Err(TTZipStatus::ErrSecurityViolation);
        };
        if rel.as_os_str().is_empty() {
            return Ok(Vec::new());
        }
        let rel_str = rel.to_str().ok_or(TTZipStatus::ErrSecurityViolation)?;
        Self::extract_clean_components(rel_str)
    }

    /// Step-by-step creates and validates a directory hierarchy beneath `root_fd`.
    pub fn create_dir_all_in_jail(&self, raw_rel_path: &str) -> Result<(), TTZipStatus> {
        let comps = Self::extract_clean_components(raw_rel_path)?;
        self.create_dir_all_components(&comps)
    }

    /// Step-by-step creates and validates a directory hierarchy from pre-validated components.
    pub fn create_dir_all_components(&self, comps: &[String]) -> Result<(), TTZipStatus> {
        let _dir_fd = self.open_dir_chain(comps, true)?;
        Ok(())
    }

    /// Step-by-step resolves parent directories and safely opens a regular leaf file inside the jail.
    pub fn open_file_in_jail(
        &self,
        raw_rel_path: &str,
        mode: u32,
        overwrite: bool,
    ) -> Result<File, TTZipStatus> {
        let comps = Self::extract_clean_components(raw_rel_path)?;
        self.open_file_components(&comps, mode, overwrite)
    }

    /// Safely opens a regular leaf file from pre-validated path components.
    pub fn open_file_components(
        &self,
        comps: &[String],
        mode: u32,
        overwrite: bool,
    ) -> Result<File, TTZipStatus> {
        let (leaf, parent_comps) = comps.split_last().ok_or(TTZipStatus::ErrSecurityViolation)?;
        let parent_guard = self.open_dir_chain(parent_comps, true)?;
        let leaf_c = CString::new(leaf.as_bytes()).map_err(|_| TTZipStatus::ErrSecurityViolation)?;

        if let Some(existing_fmt) = fstatat_mode_type(parent_guard.fd, &leaf_c)? {
            if existing_fmt == libc::S_IFLNK || existing_fmt == libc::S_IFDIR {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
            if !overwrite {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
        }

        let create_mode = ((mode & 0o777) | 0o600) as libc::c_uint;
        let fd = unsafe {
            libc::openat(
                parent_guard.fd,
                leaf_c.as_ptr(),
                file_leaf_open_flags(overwrite),
                create_mode,
            )
        };
        if fd < 0 {
            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            if matches!(
                errno,
                libc::ELOOP | libc::ENOTDIR | libc::EISDIR | libc::EPERM | libc::EACCES
            ) {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
            return Err(TTZipStatus::ErrExtractionFailed);
        }

        if !verify_fd_is_regular(fd) {
            unsafe {
                libc::close(fd);
            }
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        Ok(unsafe { File::from_raw_fd(fd) })
    }

    /// Validates symlink target against escape and chained symlinks, then creates it via `symlinkat`.
    pub fn create_symlink_in_jail(
        &self,
        raw_rel_path: &str,
        link_target: &str,
    ) -> Result<(), TTZipStatus> {
        let comps = Self::extract_clean_components(raw_rel_path)?;
        self.create_symlink_components(&comps, link_target)
    }

    /// Validates and creates a symlink inside the jail from pre-validated entry components.
    pub fn create_symlink_components(
        &self,
        comps: &[String],
        link_target: &str,
    ) -> Result<(), TTZipStatus> {
        let (leaf, parent_comps) = comps.split_last().ok_or(TTZipStatus::ErrSecurityViolation)?;
        let parent_guard = self.open_dir_chain(parent_comps, true)?;

        self.validate_symlink_target_components(comps, parent_comps, link_target)?;

        let leaf_c = CString::new(leaf.as_bytes()).map_err(|_| TTZipStatus::ErrSecurityViolation)?;
        let target_c =
            CString::new(link_target.as_bytes()).map_err(|_| TTZipStatus::ErrSecurityViolation)?;

        if let Some(existing_fmt) = fstatat_mode_type(parent_guard.fd, &leaf_c)? {
            if existing_fmt == libc::S_IFLNK || existing_fmt == libc::S_IFDIR {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
            unsafe {
                libc::unlinkat(parent_guard.fd, leaf_c.as_ptr(), 0);
            }
        }

        let rc = unsafe { libc::symlinkat(target_c.as_ptr(), parent_guard.fd, leaf_c.as_ptr()) };
        if rc != 0 {
            return Err(TTZipStatus::ErrExtractionFailed);
        }

        if let Err(err) = self.verify_all_jail_symlinks() {
            unsafe {
                libc::unlinkat(parent_guard.fd, leaf_c.as_ptr(), 0);
            }
            return Err(err);
        }

        Ok(())
    }

    /// Creates a hardlink inside the jail using `linkat` without following symlinks.
    pub fn create_hardlink_in_jail(
        &self,
        raw_entry_path: &str,
        raw_target_path: &str,
    ) -> Result<(), TTZipStatus> {
        let dst_comps = Self::extract_clean_components(raw_entry_path)?;
        let src_comps = Self::extract_clean_components(raw_target_path)?;
        self.create_hardlink_components(&dst_comps, &src_comps)
    }

    /// Creates a hardlink from pre-validated source and destination component slices.
    pub fn create_hardlink_components(
        &self,
        dst_comps: &[String],
        src_comps: &[String],
    ) -> Result<(), TTZipStatus> {
        if dst_comps == src_comps {
            return Ok(());
        }
        let (src_leaf, src_parent) = src_comps
            .split_last()
            .ok_or(TTZipStatus::ErrSecurityViolation)?;
        let (dst_leaf, dst_parent) = dst_comps
            .split_last()
            .ok_or(TTZipStatus::ErrSecurityViolation)?;

        let src_parent_guard = self.open_dir_chain(src_parent, false)?;
        let src_leaf_c =
            CString::new(src_leaf.as_bytes()).map_err(|_| TTZipStatus::ErrSecurityViolation)?;
        match fstatat_mode_type(src_parent_guard.fd, &src_leaf_c)? {
            Some(libc::S_IFREG) => {}
            Some(_) => return Err(TTZipStatus::ErrSecurityViolation),
            None => return Err(TTZipStatus::ErrFileNotFound),
        }

        let (dst_parent_guard, dst_leaf_c) =
            self.prepare_hardlink_destination(dst_parent, dst_leaf)?;

        let rc = unsafe {
            libc::linkat(
                src_parent_guard.fd,
                src_leaf_c.as_ptr(),
                dst_parent_guard.fd,
                dst_leaf_c.as_ptr(),
                0,
            )
        };
        if rc != 0 {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        Ok(())
    }

    fn prepare_hardlink_destination(
        &self,
        dst_parent: &[String],
        dst_leaf: &str,
    ) -> Result<(OwnedFdGuard, CString), TTZipStatus> {
        let dst_parent_guard = self.open_dir_chain(dst_parent, true)?;
        let dst_leaf_c =
            CString::new(dst_leaf.as_bytes()).map_err(|_| TTZipStatus::ErrSecurityViolation)?;
        if let Some(existing_fmt) = fstatat_mode_type(dst_parent_guard.fd, &dst_leaf_c)? {
            if existing_fmt == libc::S_IFLNK || existing_fmt == libc::S_IFDIR {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
            unsafe {
                libc::unlinkat(dst_parent_guard.fd, dst_leaf_c.as_ptr(), 0);
            }
        }
        Ok((dst_parent_guard, dst_leaf_c))
    }

    fn open_dir_chain(
        &self,
        comps: &[String],
        create_if_missing: bool,
    ) -> Result<OwnedFdGuard, TTZipStatus> {
        let mut cur_guard = OwnedFdGuard::borrowed(self.root_fd);
        for seg in comps {
            let seg_c =
                CString::new(seg.as_bytes()).map_err(|_| TTZipStatus::ErrSecurityViolation)?;
            let next_fd = step_into_subdir(cur_guard.fd, &seg_c, create_if_missing)?;
            cur_guard = OwnedFdGuard::owned(next_fd);
        }
        Ok(cur_guard)
    }

    fn apply_symlink_target_component(
        &self,
        stack: &mut Vec<String>,
        comp: Component<'_>,
    ) -> Result<(), TTZipStatus> {
        match comp {
            Component::CurDir => Ok(()),
            Component::ParentDir => {
                self.verify_no_symlink_along_components(stack)?;
                if stack.pop().is_none() {
                    return Err(TTZipStatus::ErrSecurityViolation);
                }
                Ok(())
            }
            Component::Normal(seg) => {
                let s = seg.to_str().ok_or(TTZipStatus::ErrSecurityViolation)?;
                if s.is_empty() || s.chars().all(|ch| ch == '.') || s.contains(':') {
                    return Err(TTZipStatus::ErrSecurityViolation);
                }
                stack.push(s.to_string());
                self.verify_no_symlink_along_components(stack)
            }
            Component::RootDir | Component::Prefix(_) => Err(TTZipStatus::ErrSecurityViolation),
        }
    }

    fn validate_symlink_target_components(
        &self,
        entry_comps: &[String],
        parent_comps: &[String],
        link_target: &str,
    ) -> Result<Vec<String>, TTZipStatus> {
        validate_raw_symlink_target_lexical(link_target)?;

        let mut stack: Vec<String> = parent_comps.to_vec();
        self.verify_no_symlink_along_components(&stack)?;

        for comp in Path::new(link_target).components() {
            self.apply_symlink_target_component(&mut stack, comp)?;
        }

        if stack.is_empty() || entry_comps.starts_with(&stack) {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        self.verify_no_symlink_along_components(&stack)?;
        Ok(stack)
    }

    fn verify_no_symlink_along_components(&self, comps: &[String]) -> Result<(), TTZipStatus> {
        let mut cur_guard = OwnedFdGuard::borrowed(self.root_fd);
        for (idx, seg) in comps.iter().enumerate() {
            let seg_c =
                CString::new(seg.as_bytes()).map_err(|_| TTZipStatus::ErrSecurityViolation)?;
            let Some(fmt) = fstatat_mode_type(cur_guard.fd, &seg_c)? else {
                return Ok(());
            };
            if fmt == libc::S_IFLNK {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
            if idx + 1 < comps.len() {
                if fmt != libc::S_IFDIR {
                    return Err(TTZipStatus::ErrSecurityViolation);
                }
                let next_fd = unsafe {
                    libc::openat(cur_guard.fd, seg_c.as_ptr(), dir_step_open_flags())
                };
                if next_fd < 0 {
                    return Err(TTZipStatus::ErrSecurityViolation);
                }
                cur_guard = OwnedFdGuard::owned(next_fd);
            }
        }
        Ok(())
    }

    /// Recursively scans the jail directory tree and verifies that every symlink remains valid
    /// and never chains through another symlink or escapes the root directory.
    pub fn verify_all_jail_symlinks(&self) -> Result<(), TTZipStatus> {
        let mut dir_stack = Vec::new();
        self.scan_dir_symlinks_recursive(self.root_fd, &mut dir_stack)
    }

    fn scan_dir_symlinks_recursive(
        &self,
        dir_fd: libc::c_int,
        dir_stack: &mut Vec<String>,
    ) -> Result<(), TTZipStatus> {
        let (symlinks, child_dirs) = collect_dir_symlinks_and_subdirs(dir_fd)?;

        for (sym_name, target) in symlinks {
            let mut entry_comps = dir_stack.clone();
            entry_comps.push(sym_name);
            self.validate_symlink_target_components(&entry_comps, dir_stack, &target)?;
        }

        for (sub_name, sub_c) in child_dirs {
            let next_fd = unsafe { libc::openat(dir_fd, sub_c.as_ptr(), dir_step_open_flags()) };
            if next_fd < 0 {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
            let sub_guard = OwnedFdGuard::owned(next_fd);
            dir_stack.push(sub_name);
            let res = self.scan_dir_symlinks_recursive(sub_guard.fd, dir_stack);
            dir_stack.pop();
            res?;
        }

        Ok(())
    }
}

type DirSymlinksAndSubdirs = (Vec<(String, String)>, Vec<(String, CString)>);

fn collect_dir_symlinks_and_subdirs(dir_fd: libc::c_int) -> Result<DirSymlinksAndSubdirs, TTZipStatus> {
    let dup_fd = unsafe { libc::fcntl(dir_fd, libc::F_DUPFD_CLOEXEC, 0) };
    if dup_fd < 0 {
        return Err(TTZipStatus::ErrExtractionFailed);
    }
    let dir_ptr = unsafe { libc::fdopendir(dup_fd) };
    if dir_ptr.is_null() {
        unsafe {
            libc::close(dup_fd);
        }
        return Err(TTZipStatus::ErrExtractionFailed);
    }

    let mut child_dirs = Vec::new();
    let mut symlinks = Vec::new();

    loop {
        let ent = unsafe { libc::readdir(dir_ptr) };
        if ent.is_null() {
            break;
        }
        let name_c = unsafe { CStr::from_ptr((*ent).d_name.as_ptr()) };
        let name_bytes = name_c.to_bytes();
        if name_bytes == b"." || name_bytes == b".." {
            continue;
        }
        let (Ok(name_str), Ok(c_owned)) = (std::str::from_utf8(name_bytes), CString::new(name_bytes)) else {
            continue;
        };
        let Some(fmt) = fstatat_mode_type(dir_fd, &c_owned)? else {
            continue;
        };
        if fmt == libc::S_IFLNK {
            let target = readlink_at_fd(dir_fd, &c_owned)?;
            symlinks.push((name_str.to_string(), target));
        } else if fmt == libc::S_IFDIR {
            child_dirs.push((name_str.to_string(), c_owned));
        }
    }
    unsafe {
        libc::closedir(dir_ptr);
    }
    Ok((symlinks, child_dirs))
}

fn validate_raw_symlink_target_lexical(link_target: &str) -> Result<(), TTZipStatus> {
    if link_target.is_empty()
        || link_target.len() > 4096
        || link_target.contains('\0')
        || link_target.contains("://")
        || link_target.contains('\\')
        || link_target.starts_with('/')
    {
        return Err(TTZipStatus::ErrSecurityViolation);
    }
    let bytes = link_target.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(TTZipStatus::ErrSecurityViolation);
    }
    Ok(())
}

fn collect_normal_components(rel: &Path) -> Result<Vec<String>, TTZipStatus> {
    let mut comps = Vec::new();
    for comp in rel.components() {
        match comp {
            Component::Normal(c) => {
                let s = c.to_str().ok_or(TTZipStatus::ErrSecurityViolation)?;
                comps.push(s.to_string());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }
    }
    if comps.is_empty() {
        return Err(TTZipStatus::ErrSecurityViolation);
    }
    Ok(comps)
}

fn step_into_subdir(
    cur_fd: libc::c_int,
    seg_c: &CString,
    create_if_missing: bool,
) -> Result<libc::c_int, TTZipStatus> {
    match fstatat_mode_type(cur_fd, seg_c)? {
        Some(libc::S_IFDIR) => {}
        Some(_) => return Err(TTZipStatus::ErrSecurityViolation),
        None => {
            if !create_if_missing {
                return Err(TTZipStatus::ErrFileNotFound);
            }
            let rc = unsafe { libc::mkdirat(cur_fd, seg_c.as_ptr(), 0o700) };
            if rc != 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() != Some(libc::EEXIST) {
                    return Err(TTZipStatus::ErrExtractionFailed);
                }
            }
        }
    }

    let next_fd = unsafe { libc::openat(cur_fd, seg_c.as_ptr(), dir_step_open_flags()) };
    if next_fd < 0 {
        return Err(TTZipStatus::ErrSecurityViolation);
    }
    if !verify_fd_is_dir(next_fd) {
        unsafe {
            libc::close(next_fd);
        }
        return Err(TTZipStatus::ErrSecurityViolation);
    }
    Ok(next_fd)
}

fn fstatat_mode_type(
    dir_fd: libc::c_int,
    name_c: &CString,
) -> Result<Option<libc::mode_t>, TTZipStatus> {
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::fstatat(dir_fd, name_c.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) };
    if rc == 0 {
        Ok(Some(st.st_mode & libc::S_IFMT))
    } else {
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        if errno == libc::ENOENT {
            Ok(None)
        } else {
            Err(TTZipStatus::ErrSecurityViolation)
        }
    }
}

fn readlink_at_fd(dir_fd: libc::c_int, name_c: &CString) -> Result<String, TTZipStatus> {
    let mut buf = [0u8; 4096];
    let n = unsafe {
        libc::readlinkat(
            dir_fd,
            name_c.as_ptr(),
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len(),
        )
    };
    if n <= 0 || (n as usize) >= buf.len() {
        return Err(TTZipStatus::ErrSecurityViolation);
    }
    std::str::from_utf8(&buf[..n as usize])
        .map(|s| s.to_string())
        .map_err(|_| TTZipStatus::ErrSecurityViolation)
}

fn verify_fd_is_dir(fd: libc::c_int) -> bool {
    if fd < 0 {
        return false;
    }
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut st) } != 0 {
        return false;
    }
    (st.st_mode & libc::S_IFMT) == libc::S_IFDIR
}

fn verify_fd_is_regular(fd: libc::c_int) -> bool {
    if fd < 0 {
        return false;
    }
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut st) } != 0 {
        return false;
    }
    (st.st_mode & libc::S_IFMT) == libc::S_IFREG
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_windows_drive_letter_rejection() {
        assert_eq!(
            DirJail::extract_clean_components("C:evil.txt"),
            Err(TTZipStatus::ErrSecurityViolation)
        );
        assert_eq!(
            DirJail::extract_clean_components("C:\\Windows\\System32\\cmd.exe"),
            Err(TTZipStatus::ErrSecurityViolation)
        );
        assert_eq!(
            DirJail::extract_clean_components("folder/file:ADS"),
            Err(TTZipStatus::ErrSecurityViolation)
        );
    }

    #[test]
    fn test_all_dots_component_rejection() {
        assert_eq!(
            DirJail::extract_clean_components("a/.../b"),
            Err(TTZipStatus::ErrSecurityViolation)
        );
        assert_eq!(
            DirJail::extract_clean_components("...."),
            Err(TTZipStatus::ErrSecurityViolation)
        );
    }

    #[test]
    fn test_dir_jail_blocks_outside_and_chained_symlinks() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("dest");
        std::fs::create_dir_all(&dest).unwrap();
        let outside = tmp.path().join("harmless_marker.txt");
        std::fs::write(&outside, b"SAFE_ORIGINAL_CONTENT").unwrap();

        let jail = DirJail::open(&dest).unwrap();
        assert_eq!(
            jail.create_symlink_in_jail("dir_link", ".."),
            Err(TTZipStatus::ErrSecurityViolation)
        );
        assert_eq!(
            jail.create_symlink_in_jail("a/b", ".."),
            Err(TTZipStatus::ErrSecurityViolation)
        );

        // Even if a malicious symlink were planted on disk externally, DirJail blocks traversal through it
        std::fs::create_dir_all(dest.join("a")).unwrap();
        std::os::unix::fs::symlink("..", dest.join("a/b")).unwrap();
        assert_eq!(
            jail.create_symlink_in_jail("a/c", "b/.."),
            Err(TTZipStatus::ErrSecurityViolation)
        );
        assert_eq!(
            jail.open_file_in_jail("a/b/harmless_marker.txt", 0o644, true)
                .unwrap_err(),
            TTZipStatus::ErrSecurityViolation
        );
        let mut f = jail.open_file_in_jail("safe/inside.txt", 0o644, true).unwrap();
        f.write_all(b"ok").unwrap();
        assert_eq!(
            std::fs::read_to_string(&outside).unwrap(),
            "SAFE_ORIGINAL_CONTENT"
        );
    }
}

