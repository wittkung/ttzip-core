// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 16-Shard fine-grained concurrent zero-copy slice sharing pool.

use super::mmap::MmapSource;
use super::StorageMedium;
use crate::types::TTZipStatus;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Number of concurrent shards used to eliminate lock contention under massive parallelism.
pub const SHARD_COUNT: usize = 16;

/// RAII zero-copy slice lease maintaining source lifetime and pool metrics.
pub struct SharedSliceLease {
    source: Arc<MmapSource>,
    offset: usize,
    len: usize,
    active_tracker: Option<Arc<AtomicUsize>>,
}

unsafe impl Send for SharedSliceLease {}
unsafe impl Sync for SharedSliceLease {}

impl SharedSliceLease {
    /// Returns the borrowed immutable byte slice view.
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        if self.len == 0 {
            return &[];
        }
        match self.source.as_slice() {
            Some(full) => {
                let end = self.offset.saturating_add(self.len).min(full.len());
                if self.offset <= end {
                    &full[self.offset..end]
                } else {
                    &[]
                }
            }
            None => &[],
        }
    }

    /// Returns the lease length in bytes.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if lease is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the start offset relative to the underlying archive source.
    #[inline]
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns a cloned `Arc` reference to the backing `MmapSource`.
    #[inline]
    #[must_use]
    pub fn source(&self) -> Arc<MmapSource> {
        Arc::clone(&self.source)
    }
}

impl Deref for SharedSliceLease {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl AsRef<[u8]> for SharedSliceLease {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl Drop for SharedSliceLease {
    fn drop(&mut self) {
        if let Some(ref tracker) = self.active_tracker {
            tracker.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

/// A single lock-sharded partition within `MmapSlicePool`.
struct Shard {
    sources: RwLock<HashMap<u64, Arc<MmapSource>>>,
    active_leases: Arc<AtomicUsize>,
}

impl Shard {
    fn new() -> Self {
        Self {
            sources: RwLock::new(HashMap::new()),
            active_leases: Arc::new(AtomicUsize::new(0)),
        }
    }
}

/// Statistics snapshot of the zero-copy slice sharing pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolStats {
    /// Total number of cached MmapSource instances across all shards.
    pub total_cached_sources: usize,
    /// Total number of active slice leases currently held by worker threads.
    pub total_active_leases: usize,
    /// Number of cached sources per shard.
    pub shard_cached_distribution: [usize; SHARD_COUNT],
    /// Number of active leases per shard.
    pub shard_lease_distribution: [usize; SHARD_COUNT],
}

/// High-concurrency zero-copy slice sharing pool with 16 fine-grained shards.
pub struct MmapSlicePool {
    shards: [Shard; SHARD_COUNT],
}

impl Default for MmapSlicePool {
    fn default() -> Self {
        Self::new()
    }
}

impl MmapSlicePool {
    /// Creates a new 16-shard memory-mapped slice pool.
    pub fn new() -> Self {
        Self {
            shards: [
                Shard::new(),
                Shard::new(),
                Shard::new(),
                Shard::new(),
                Shard::new(),
                Shard::new(),
                Shard::new(),
                Shard::new(),
                Shard::new(),
                Shard::new(),
                Shard::new(),
                Shard::new(),
                Shard::new(),
                Shard::new(),
                Shard::new(),
                Shard::new(),
            ],
        }
    }

    /// Resolves shard index for a given 64-bit archive identifier or path hash.
    #[inline]
    #[must_use]
    pub const fn shard_index(key: u64) -> usize {
        ((key ^ (key >> 16) ^ (key >> 32)) as usize) & (SHARD_COUNT - 1)
    }

    /// Retrieves an existing `MmapSource` or opens and caches a new one.
    pub fn get_or_open(
        &self,
        key: u64,
        path: &Path,
        medium: StorageMedium,
    ) -> Result<Arc<MmapSource>, TTZipStatus> {
        let shard_idx = Self::shard_index(key);
        let shard = &self.shards[shard_idx];

        // Fast path: optimistic read lock
        {
            let guard = shard.sources.read();
            if let Some(source) = guard.get(&key) {
                return Ok(Arc::clone(source));
            }
        }

        // Slow path: open archive source and insert under write lock
        let new_source = Arc::new(MmapSource::open(path, medium)?);
        let mut guard = shard.sources.write();
        let entry = guard.entry(key).or_insert_with(|| Arc::clone(&new_source));
        Ok(Arc::clone(entry))
    }

    /// Leases a zero-copy byte slice `[offset, offset + len)` from the pool.
    pub fn lease_slice(
        &self,
        key: u64,
        path: &Path,
        offset: u64,
        len: usize,
        medium: StorageMedium,
    ) -> Result<SharedSliceLease, TTZipStatus> {
        let source = self.get_or_open(key, path, medium)?;
        self.lease_from_source(key, source, offset, len)
    }

    /// Leases a zero-copy byte slice `[offset, offset + len)` directly from a resolved `Arc<MmapSource>`.
    pub fn lease_from_source(
        &self,
        key: u64,
        source: Arc<MmapSource>,
        offset: u64,
        len: usize,
    ) -> Result<SharedSliceLease, TTZipStatus> {
        let file_len = source.len();
        let offset_usize = usize::try_from(offset).map_err(|_| TTZipStatus::ErrInvalidParam)?;

        if offset.checked_add(len as u64).is_none_or(|end| end > file_len) {
            return Err(TTZipStatus::ErrInvalidOffset);
        }

        let shard_idx = Self::shard_index(key);
        let tracker = Arc::clone(&self.shards[shard_idx].active_leases);
        tracker.fetch_add(1, Ordering::Relaxed);

        Ok(SharedSliceLease {
            source,
            offset: offset_usize,
            len,
            active_tracker: Some(tracker),
        })
    }

    /// Explicitly registers an externally created `MmapSource` into the pool.
    pub fn insert_source(&self, key: u64, source: Arc<MmapSource>) {
        let shard_idx = Self::shard_index(key);
        let mut guard = self.shards[shard_idx].sources.write();
        guard.insert(key, source);
    }

    /// Evicts an `MmapSource` from the cache by key.
    pub fn evict(&self, key: u64) -> Option<Arc<MmapSource>> {
        let shard_idx = Self::shard_index(key);
        let mut guard = self.shards[shard_idx].sources.write();
        guard.remove(&key)
    }

    /// Clears all cached sources across all 16 shards.
    pub fn clear(&self) {
        for shard in &self.shards {
            shard.sources.write().clear();
        }
    }

    /// Returns the total number of cached sources across all shards.
    #[must_use]
    pub fn cached_sources_count(&self) -> usize {
        self.shards.iter().map(|s| s.sources.read().len()).sum()
    }

    /// Returns the total number of currently active leases.
    #[must_use]
    pub fn active_leases_count(&self) -> usize {
        self.shards
            .iter()
            .map(|s| s.active_leases.load(Ordering::Relaxed))
            .sum()
    }

    /// Collects a point-in-time statistics snapshot of the pool.
    #[must_use]
    pub fn stats(&self) -> PoolStats {
        let mut shard_cached_distribution = [0usize; SHARD_COUNT];
        let mut shard_lease_distribution = [0usize; SHARD_COUNT];
        let mut total_cached_sources = 0;
        let mut total_active_leases = 0;

        for (i, shard) in self.shards.iter().enumerate() {
            let cached = shard.sources.read().len();
            let leases = shard.active_leases.load(Ordering::Relaxed);

            shard_cached_distribution[i] = cached;
            shard_lease_distribution[i] = leases;
            total_cached_sources += cached;
            total_active_leases += leases;
        }

        PoolStats {
            total_cached_sources,
            total_active_leases,
            shard_cached_distribution,
            shard_lease_distribution,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Arc;
    use tempfile::NamedTempFile;

    #[test]
    fn test_shard_index_distribution() {
        let mut hit_shards = [false; SHARD_COUNT];
        for key in 0..1024 {
            let idx = MmapSlicePool::shard_index(key);
            assert!(idx < SHARD_COUNT);
            hit_shards[idx] = true;
        }
        for (i, &hit) in hit_shards.iter().enumerate() {
            assert!(hit, "Shard {} was never hit", i);
        }
    }

    #[test]
    fn test_lease_slice_and_deref() {
        let mut temp = NamedTempFile::new().unwrap();
        let payload: Vec<u8> = (0..16384).map(|i| (i % 251) as u8).collect();
        temp.write_all(&payload).unwrap();
        temp.flush().unwrap();

        let pool = MmapSlicePool::new();
        let key = 42u64;

        let lease = pool
            .lease_slice(key, temp.path(), 1000, 2000, StorageMedium::LocalFastApfs)
            .unwrap();

        assert_eq!(lease.len(), 2000);
        assert!(!lease.is_empty());
        assert_eq!(lease.offset(), 1000);
        assert_eq!(&lease[..], &payload[1000..3000]);
        assert_eq!(lease.as_ref(), &payload[1000..3000]);

        assert_eq!(pool.cached_sources_count(), 1);
        assert_eq!(pool.active_leases_count(), 1);

        drop(lease);
        assert_eq!(pool.active_leases_count(), 0);
    }

    #[test]
    fn test_concurrent_multithreaded_leasing() {
        let mut temp = NamedTempFile::new().unwrap();
        let payload: Vec<u8> = (0..65536).map(|i| (i % 255) as u8).collect();
        temp.write_all(&payload).unwrap();
        temp.flush().unwrap();

        let pool = Arc::new(MmapSlicePool::new());
        let path = temp.path().to_path_buf();
        let payload = Arc::new(payload);

        let mut handles = Vec::new();
        for thread_idx in 0..16 {
            let pool_clone = Arc::clone(&pool);
            let path_clone = path.clone();
            let payload_clone = Arc::clone(&payload);

            handles.push(std::thread::spawn(move || {
                let key = (thread_idx as u64) * 100;
                for iter in 0..50 {
                    let offset = ((iter * 100) % 30000) as u64;
                    let len = 1024;
                    let lease = pool_clone
                        .lease_slice(key, &path_clone, offset, len, StorageMedium::LocalFastApfs)
                        .unwrap();

                    let expected = &payload_clone[offset as usize..(offset as usize + len)];
                    assert_eq!(&lease[..], expected);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(pool.active_leases_count(), 0);
        let stats = pool.stats();
        assert_eq!(stats.total_active_leases, 0);
        assert!(stats.total_cached_sources > 0);
    }

    #[test]
    fn test_evict_and_clear() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"test data").unwrap();
        temp.flush().unwrap();

        let pool = MmapSlicePool::new();
        let _ = pool.get_or_open(1, temp.path(), StorageMedium::LocalFastApfs).unwrap();
        let _ = pool.get_or_open(2, temp.path(), StorageMedium::LocalFastApfs).unwrap();

        assert_eq!(pool.cached_sources_count(), 2);
        assert!(pool.evict(1).is_some());
        assert_eq!(pool.cached_sources_count(), 1);

        pool.clear();
        assert_eq!(pool.cached_sources_count(), 0);
    }

    #[test]
    fn test_lease_out_of_bounds() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"short").unwrap();
        temp.flush().unwrap();

        let pool = MmapSlicePool::new();
        let res = pool.lease_slice(99, temp.path(), 0, 100, StorageMedium::LocalFastApfs);
        assert_eq!(res.err(), Some(TTZipStatus::ErrInvalidOffset));
    }
}
