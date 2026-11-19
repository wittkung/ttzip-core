// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Bounded, thread-safe verified directory descriptor cache for [`super::DirJail`].
//!
//! Every cached [`VerifiedDirHandle`] is opened strictly via step-by-step
//! `fstatat(AT_SYMLINK_NOFOLLOW)`, `openat(O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)`,
//! and `fstat` `(st_dev, st_ino)` verification from `DirJail::root_fd()`, pinning it to that
//! verified directory inode in the kernel. Cache capacity is bounded below `RLIMIT_NOFILE`
//! with LRU eviction and automatic clear-on-`EMFILE` retry.

use std::collections::HashMap;
use std::ffi::CString;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;

use crate::types::TTZipStatus;

/// Error detail from step-by-step directory traversal, preserving exact OS `errno`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DirStepError {
    pub(crate) status: TTZipStatus,
    pub(crate) errno: i32,
}

impl DirStepError {
    #[inline]
    pub(crate) const fn new(status: TTZipStatus, errno: i32) -> Self {
        Self { status, errno }
    }

    #[inline]
    pub(crate) const fn is_fd_exhaustion(self) -> bool {
        matches!(self.errno, libc::EMFILE | libc::ENFILE)
    }
}

/// Kernel-pinned open handle to a verified directory inode inside a [`super::DirJail`].
#[derive(Debug)]
pub(crate) struct VerifiedDirHandle {
    fd: OwnedFd,
    dev: u64,
    ino: u64,
}

impl VerifiedDirHandle {
    #[inline]
    pub(crate) fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }

    #[inline]
    pub(crate) const fn identity(&self) -> (u64, u64) {
        (self.dev, self.ino)
    }
}

#[derive(Debug)]
struct CacheSlot {
    handle: Arc<VerifiedDirHandle>,
    last_use: AtomicU64,
}

#[derive(Debug)]
pub(crate) struct DirFdCache {
    entries: RwLock<HashMap<String, CacheSlot>>,
    clock: AtomicU64,
    max_entries: usize,
}

fn compute_dir_cache_limit() -> usize {
    let mut rlim: libc::rlimit = unsafe { std::mem::zeroed() };
    let soft = if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlim) } == 0 {
        rlim.rlim_cur as usize
    } else {
        256
    };
    (soft.saturating_sub(128) / 2).clamp(32, 1024)
}

pub(crate) fn join_components_key(comps: &[String]) -> String {
    let cap: usize = comps.iter().map(|s| s.len() + 1).sum();
    let mut key = String::with_capacity(cap);
    for (idx, seg) in comps.iter().enumerate() {
        if idx > 0 {
            key.push('/');
        }
        key.push_str(seg);
    }
    key
}

impl DirFdCache {
    pub(crate) fn new() -> Self {
        Self::with_limit(compute_dir_cache_limit())
    }

    pub(crate) fn with_limit(max_entries: usize) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            clock: AtomicU64::new(1),
            max_entries: max_entries.max(4),
        }
    }

    #[inline]
    fn next_tick(&self) -> u64 {
        self.clock.fetch_add(1, Ordering::Relaxed)
    }

    /// Finds the deepest cached ancestor for `comps`, returning `(matched_count, handle)`.
    /// Uses only a shared read lock and slices a single joined key backward without reallocation.
    pub(crate) fn find_deepest_prefix(
        &self,
        comps: &[String],
    ) -> (usize, Option<Arc<VerifiedDirHandle>>) {
        if comps.is_empty() {
            return (0, None);
        }
        let full_key = join_components_key(comps);
        let guard = self.entries.read();
        if guard.is_empty() {
            return (0, None);
        }
        let mut slice = full_key.as_str();
        let mut depth = comps.len();
        loop {
            if let Some(slot) = guard.get(slice) {
                slot.last_use.store(self.next_tick(), Ordering::Relaxed);
                return (depth, Some(Arc::clone(&slot.handle)));
            }
            match slice.rfind('/') {
                Some(pos) => {
                    slice = &slice[..pos];
                    depth -= 1;
                }
                None => return (0, None),
            }
        }
    }

    pub(crate) fn insert(&self, key: String, handle: Arc<VerifiedDirHandle>) {
        let tick = self.next_tick();
        let mut guard = self.entries.write();
        if let Some(existing) = guard.get_mut(&key) {
            existing.handle = handle;
            existing.last_use.store(tick, Ordering::Relaxed);
            return;
        }
        if guard.len() >= self.max_entries {
            evict_oldest_quarter(&mut guard, self.max_entries);
        }
        guard.insert(
            key,
            CacheSlot {
                handle,
                last_use: AtomicU64::new(tick),
            },
        );
    }

    pub(crate) fn invalidate_prefix(&self, key: &str) {
        let mut guard = self.entries.write();
        if guard.is_empty() {
            return;
        }
        guard.retain(|k, _| {
            !(k == key
                || (k.len() > key.len()
                    && k.starts_with(key)
                    && k.as_bytes()[key.len()] == b'/'))
        });
    }

    pub(crate) fn clear(&self) {
        self.entries.write().clear();
    }

    pub(crate) fn resolve_dir_handle_detailed(
        &self,
        root_fd: RawFd,
        comps: &[String],
        create_if_missing: bool,
    ) -> Result<Option<Arc<VerifiedDirHandle>>, DirStepError> {
        if comps.is_empty() {
            return Ok(None);
        }
        match self.resolve_dir_handle_once(root_fd, comps, create_if_missing) {
            Ok(h) => Ok(h),
            Err(err) if err.is_fd_exhaustion() => {
                self.clear();
                self.resolve_dir_handle_once(root_fd, comps, create_if_missing)
            }
            Err(err) => Err(err),
        }
    }

    fn resolve_dir_handle_once(
        &self,
        root_fd: RawFd,
        comps: &[String],
        create_if_missing: bool,
    ) -> Result<Option<Arc<VerifiedDirHandle>>, DirStepError> {
        let (matched, mut cur_handle) = self.find_deepest_prefix(comps);
        if matched == comps.len() {
            return Ok(cur_handle);
        }
        let mut prefix_key = if matched > 0 {
            join_components_key(&comps[..matched])
        } else {
            String::new()
        };
        for seg in &comps[matched..] {
            let seg_c = CString::new(seg.as_bytes())
                .map_err(|_| DirStepError::new(TTZipStatus::ErrSecurityViolation, libc::EINVAL))?;
            let cur_fd = cur_handle.as_ref().map_or(root_fd, |h| h.as_raw_fd());
            let next = Arc::new(step_into_subdir_verified(
                cur_fd,
                &seg_c,
                create_if_missing,
            )?);
            if !prefix_key.is_empty() {
                prefix_key.push('/');
            }
            prefix_key.push_str(seg);
            self.insert(prefix_key.clone(), Arc::clone(&next));
            cur_handle = Some(next);
        }
        Ok(cur_handle)
    }

    pub(crate) fn open_dir_chain_uncached(
        &self,
        root_fd: RawFd,
        comps: &[String],
        create_if_missing: bool,
    ) -> Result<Option<VerifiedDirHandle>, TTZipStatus> {
        let mut cur: Option<VerifiedDirHandle> = None;
        for seg in comps {
            let seg_c =
                CString::new(seg.as_bytes()).map_err(|_| TTZipStatus::ErrSecurityViolation)?;
            let cur_fd = cur.as_ref().map_or(root_fd, |h| h.as_raw_fd());
            let next = match step_into_subdir_verified(cur_fd, &seg_c, create_if_missing) {
                Ok(h) => h,
                Err(err) if err.is_fd_exhaustion() => {
                    self.clear();
                    step_into_subdir_verified(cur_fd, &seg_c, create_if_missing)
                        .map_err(|e| e.status)?
                }
                Err(err) => return Err(err.status),
            };
            cur = Some(next);
        }
        Ok(cur)
    }
}

fn evict_oldest_quarter(entries: &mut HashMap<String, CacheSlot>, max_entries: usize) {
    let evict_count = (max_entries / 4).max(1).min(entries.len());
    let mut ranked: Vec<(u64, String)> = entries
        .iter()
        .map(|(k, slot)| (slot.last_use.load(Ordering::Relaxed), k.clone()))
        .collect();
    ranked.select_nth_unstable_by_key(evict_count - 1, |(t, _)| *t);
    for (_, old_key) in ranked.into_iter().take(evict_count) {
        entries.remove(&old_key);
    }
}

#[inline]
const fn dir_open_flags() -> libc::c_int {
    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC
}

fn fstatat_nofollow_raw(dir_fd: RawFd, seg_c: &CString) -> Result<Option<libc::stat>, DirStepError> {
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::fstatat(dir_fd, seg_c.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) };
    if rc == 0 {
        return Ok(Some(st));
    }
    let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(libc::EIO);
    if errno == libc::ENOENT {
        Ok(None)
    } else {
        Err(DirStepError::new(TTZipStatus::ErrSecurityViolation, errno))
    }
}

fn mkdir_if_allowed(
    cur_fd: RawFd,
    seg_c: &CString,
    create_if_missing: bool,
) -> Result<(), DirStepError> {
    if !create_if_missing {
        return Err(DirStepError::new(TTZipStatus::ErrFileNotFound, libc::ENOENT));
    }
    let rc = unsafe { libc::mkdirat(cur_fd, seg_c.as_ptr(), 0o700) };
    if rc != 0 {
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(libc::EIO);
        if errno != libc::EEXIST {
            return Err(DirStepError::new(TTZipStatus::ErrExtractionFailed, errno));
        }
    }
    Ok(())
}

/// Steps one directory hop from `cur_fd` into `seg_c` with full `AT_SYMLINK_NOFOLLOW`,
/// `O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`, and `fstat` `(st_dev, st_ino)` identity verification.
pub(crate) fn step_into_subdir_verified(
    cur_fd: RawFd,
    seg_c: &CString,
    create_if_missing: bool,
) -> Result<VerifiedDirHandle, DirStepError> {
    let expected_pre = match fstatat_nofollow_raw(cur_fd, seg_c)? {
        Some(pre_st) => {
            let fmt = pre_st.st_mode & libc::S_IFMT;
            if fmt == libc::S_IFLNK {
                return Err(DirStepError::new(TTZipStatus::ErrSecurityViolation, libc::ELOOP));
            }
            if fmt != libc::S_IFDIR {
                return Err(DirStepError::new(TTZipStatus::ErrSecurityViolation, libc::ENOTDIR));
            }
            Some((pre_st.st_dev as u64, pre_st.st_ino))
        }
        None => {
            mkdir_if_allowed(cur_fd, seg_c, create_if_missing)?;
            None
        }
    };

    let next_raw = unsafe { libc::openat(cur_fd, seg_c.as_ptr(), dir_open_flags()) };
    if next_raw < 0 {
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(libc::EIO);
        let status = if errno == libc::ENOENT && !create_if_missing {
            TTZipStatus::ErrFileNotFound
        } else {
            TTZipStatus::ErrSecurityViolation
        };
        return Err(DirStepError::new(status, errno));
    }
    let fd = unsafe { OwnedFd::from_raw_fd(next_raw) };
    let mut post_st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd.as_raw_fd(), &mut post_st) } != 0 {
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(libc::EIO);
        return Err(DirStepError::new(TTZipStatus::ErrSecurityViolation, errno));
    }
    let post_id = (post_st.st_dev as u64, post_st.st_ino);
    if (post_st.st_mode & libc::S_IFMT) != libc::S_IFDIR
        || expected_pre.is_some_and(|pre_id| pre_id != post_id)
    {
        return Err(DirStepError::new(TTZipStatus::ErrSecurityViolation, libc::EPERM));
    }
    Ok(VerifiedDirHandle {
        fd,
        dev: post_id.0,
        ino: post_id.1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::dir_jail::DirJail;
    use tempfile::TempDir;

    #[test]
    fn test_dir_fd_cache_eviction_and_prefix_lookup() {
        let tmp = TempDir::new().unwrap();
        let jail = DirJail::open(tmp.path()).unwrap();
        let cache = DirFdCache::with_limit(4);

        for i in 0..8 {
            let name = format!("d_{i}");
            let comps = vec![name.clone()];
            let handle = jail.open_dir_chain_uncached(&comps, true).unwrap().unwrap();
            cache.insert(name, Arc::new(handle));
        }
        assert!(cache.entries.read().len() <= 4);

        let deep = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let parent = vec!["a".to_string(), "b".to_string()];
        let h_ab = Arc::new(jail.open_dir_chain_uncached(&parent, true).unwrap().unwrap());
        cache.insert("a/b".to_string(), Arc::clone(&h_ab));

        let (matched, found) = cache.find_deepest_prefix(&deep);
        assert_eq!(matched, 2);
        assert_eq!(found.unwrap().identity(), h_ab.identity());

        cache.invalidate_prefix("a");
        let (matched_after, found_after) = cache.find_deepest_prefix(&deep);
        assert_eq!(matched_after, 0);
        assert!(found_after.is_none());
    }

    #[test]
    fn test_dir_jail_cache_preserves_symlink_rejection_and_uncached_recheck() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("dest");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        let jail = DirJail::open(&dest).unwrap();
        let comps = vec!["sub".to_string(), "inner".to_string()];
        let h1 = jail.resolve_dir_handle(&comps, true).unwrap().unwrap();
        let h2 = jail.resolve_dir_handle(&comps, true).unwrap().unwrap();
        assert_eq!(h1.as_raw_fd(), h2.as_raw_fd(), "cached handle reused");
        assert_eq!(h1.identity(), h2.identity());

        // Externally replace sub with symlink to outside: uncached walk and fresh DirJail must reject.
        std::fs::rename(dest.join("sub"), dest.join("sub_detached")).unwrap();
        std::os::unix::fs::symlink(&outside, dest.join("sub")).unwrap();
        assert_eq!(
            jail.open_dir_chain_uncached(&comps, false).unwrap_err(),
            TTZipStatus::ErrSecurityViolation
        );
        let fresh_jail = DirJail::open(&dest).unwrap();
        assert_eq!(
            fresh_jail.resolve_dir_handle(&comps, false).unwrap_err(),
            TTZipStatus::ErrSecurityViolation
        );
    }
}
