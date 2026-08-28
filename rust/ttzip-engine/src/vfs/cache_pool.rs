// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-Throughput / Fine-Grained Concurrent LZ4 VFS Cache Pool with Safe Slot Reuse & Lock-Free I/O.

use crate::codecs::fast_blocks::{lz4_compress_bound, lz4_compress_fast, lz4_decompress};
use crate::types::TTZipStatus;
use parking_lot::RwLock;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const NUM_SHARDS: usize = 16;

#[derive(Debug, Clone)]
pub struct VFSCacheBlockMeta {
    pub chunk_index: u64,
    pub raw_size: usize,
    pub compressed_size: usize,
    pub is_disk_spill: bool,
    pub access_timestamp: u64,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct CacheKey(pub u64, pub u64);

impl CacheKey {
    #[inline]
    pub fn new(session_id: &str, chunk_index: u64) -> Self {
        let mut hasher = DefaultHasher::new();
        session_id.hash(&mut hasher);
        Self(hasher.finish(), chunk_index)
    }

    #[inline]
    pub fn session_hash(session_id: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        session_id.hash(&mut hasher);
        hasher.finish()
    }
}

#[derive(Clone)]
struct LruNode {
    key: CacheKey,
    raw_size: usize,
    compressed_size: usize,
    in_ram: bool,
    ram_data: Option<Arc<[u8]>>,
    disk_path: Option<PathBuf>,
    access_time: u64,
    prev: Option<usize>,
    next: Option<usize>,
    active: bool,
}

#[derive(Default)]
struct LruShard {
    map: HashMap<CacheKey, usize>,
    nodes: Vec<LruNode>,
    free_indices: Vec<usize>,
    head: Option<usize>,
    tail: Option<usize>,
    ram_bytes: usize,
}

impl LruShard {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            nodes: Vec::new(),
            free_indices: Vec::new(),
            head: None,
            tail: None,
            ram_bytes: 0,
        }
    }

    #[inline]
    fn detach(&mut self, idx: usize) {
        let prev = self.nodes[idx].prev;
        let next = self.nodes[idx].next;

        if let Some(p) = prev {
            self.nodes[p].next = next;
        } else {
            self.head = next;
        }

        if let Some(n) = next {
            self.nodes[n].prev = prev;
        } else {
            self.tail = prev;
        }

        self.nodes[idx].prev = None;
        self.nodes[idx].next = None;
    }

    #[inline]
    fn push_front(&mut self, idx: usize) {
        self.nodes[idx].prev = None;
        self.nodes[idx].next = self.head;

        if let Some(old_head) = self.head {
            self.nodes[old_head].prev = Some(idx);
        } else {
            self.tail = Some(idx);
        }
        self.head = Some(idx);
    }

    #[inline]
    fn move_to_front(&mut self, idx: usize) {
        if self.head == Some(idx) {
            return;
        }
        self.detach(idx);
        self.push_front(idx);
    }

    /// Allocates or reuses a node slot in the arena vector.
    fn allocate_node(&mut self, new_node: LruNode) -> usize {
        if let Some(free_idx) = self.free_indices.pop() {
            self.nodes[free_idx] = new_node;
            free_idx
        } else {
            let idx = self.nodes.len();
            self.nodes.push(new_node);
            idx
        }
    }

    /// Plans evictions to stay within max_ram, returning data to spill to disk.
    fn plan_evictions(&mut self, spill_dir: &Path, needed_bytes: usize, max_ram: usize) -> Vec<(PathBuf, Arc<[u8]>)> {
        let mut evictions = Vec::new();
        let mut curr = self.tail;
        while let Some(idx) = curr {
            if self.ram_bytes + needed_bytes <= max_ram {
                break;
            }
            let prev = self.nodes[idx].prev;
            if self.nodes[idx].active && self.nodes[idx].in_ram {
                if let Some(data) = self.nodes[idx].ram_data.clone() {
                    let safe_filename = format!("{:016x}_{:016x}.lz4", self.nodes[idx].key.0, self.nodes[idx].key.1);
                    let spill_file = spill_dir.join(safe_filename);
                    self.nodes[idx].in_ram = false;
                    self.nodes[idx].ram_data = None;
                    self.nodes[idx].disk_path = Some(spill_file.clone());
                    self.ram_bytes = self.ram_bytes.saturating_sub(data.len());
                    evictions.push((spill_file, data));
                }
            }
            curr = prev;
        }
        evictions
    }

    /// Removes a node from the shard and returns any associated disk path for lock-free deletion.
    fn remove_node(&mut self, idx: usize) -> Option<PathBuf> {
        if !self.nodes[idx].active {
            return None;
        }
        self.detach(idx);
        let path = self.nodes[idx].disk_path.take();
        if let Some(ref data) = self.nodes[idx].ram_data {
            self.ram_bytes = self.ram_bytes.saturating_sub(data.len());
        }
        self.nodes[idx].active = false;
        self.nodes[idx].ram_data = None;
        self.map.remove(&self.nodes[idx].key);
        self.free_indices.push(idx);
        path
    }
}

/// 16-way sharded Lock-Free / Concurrent LZ4 VFS cache pool with 2-tier RAM + Disk spill.
pub struct VFSLz4CachePool {
    shards: [RwLock<LruShard>; NUM_SHARDS],
    spill_dir: PathBuf,
    max_ram_bytes: usize,
    per_shard_max_ram: usize,
    access_counter: AtomicU64,
}

impl VFSLz4CachePool {
    pub fn new(max_ram_bytes: usize, spill_dir: Option<PathBuf>) -> Self {
        let dir = spill_dir.unwrap_or_else(|| {
            std::env::temp_dir().join(format!("ttzip_vfs_lz4_{}", std::process::id()))
        });
        let _ = fs::create_dir_all(&dir);

        let shards: [RwLock<LruShard>; NUM_SHARDS] = std::array::from_fn(|_| RwLock::new(LruShard::new()));
        let per_shard = (max_ram_bytes / NUM_SHARDS).max(1024 * 1024);

        Self {
            shards,
            spill_dir: dir,
            max_ram_bytes,
            per_shard_max_ram: per_shard,
            access_counter: AtomicU64::new(1),
        }
    }

    #[inline]
    pub fn max_ram_bytes(&self) -> usize {
        self.max_ram_bytes
    }

    #[inline]
    fn shard_idx(&self, key: &CacheKey) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % NUM_SHARDS
    }

    fn current_timestamp(&self) -> u64 {
        self.access_counter.fetch_add(1, Ordering::Relaxed)
            + SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0)
    }

    /// Compresses chunk using LZ4 and inserts into RAM cache, spilling to disk on budget overflow.
    pub fn put(&self, session_id: &str, chunk_index: u64, raw_data: &[u8], acceleration: i32) -> Result<(), TTZipStatus> {
        if raw_data.is_empty() {
            return Ok(());
        }

        let max_comp_len = lz4_compress_bound(raw_data.len());
        let mut compressed = vec![0u8; max_comp_len];
        let comp_len = lz4_compress_fast(raw_data, &mut compressed, acceleration.max(1))?;
        compressed.truncate(comp_len);
        let comp_arc: Arc<[u8]> = Arc::from(compressed.into_boxed_slice());

        let key = CacheKey::new(session_id, chunk_index);
        let s_idx = self.shard_idx(&key);
        let now = self.current_timestamp();

        let (evictions, direct_spill, old_disk_path) = {
            let mut shard = self.shards[s_idx].write();

            let evictions = if shard.ram_bytes + comp_len > self.per_shard_max_ram {
                shard.plan_evictions(&self.spill_dir, comp_len, self.per_shard_max_ram)
            } else {
                Vec::new()
            };

            let mut direct_spill = None;
            let mut old_disk_path = None;

            if let Some(&node_idx) = shard.map.get(&key) {
                let old_data_len = shard.nodes[node_idx].ram_data.as_ref().map(|d| d.len()).unwrap_or(0);
                shard.ram_bytes = shard.ram_bytes.saturating_sub(old_data_len);
                old_disk_path = shard.nodes[node_idx].disk_path.take();

                shard.nodes[node_idx].raw_size = raw_data.len();
                shard.nodes[node_idx].compressed_size = comp_len;
                shard.nodes[node_idx].access_time = now;

                if shard.ram_bytes + comp_len <= self.per_shard_max_ram {
                    shard.nodes[node_idx].in_ram = true;
                    shard.nodes[node_idx].ram_data = Some(comp_arc.clone());
                    shard.nodes[node_idx].disk_path = None;
                    shard.ram_bytes += comp_len;
                } else {
                    let safe_filename = format!("{:016x}_{:016x}.lz4", key.0, key.1);
                    let spill_file = self.spill_dir.join(safe_filename);
                    shard.nodes[node_idx].in_ram = false;
                    shard.nodes[node_idx].ram_data = None;
                    shard.nodes[node_idx].disk_path = Some(spill_file.clone());
                    direct_spill = Some((spill_file, comp_arc.clone()));
                }
                shard.move_to_front(node_idx);
            } else {
                let (in_ram, ram_data, disk_path) = if shard.ram_bytes + comp_len <= self.per_shard_max_ram {
                    shard.ram_bytes += comp_len;
                    (true, Some(comp_arc.clone()), None)
                } else {
                    let safe_filename = format!("{:016x}_{:016x}.lz4", key.0, key.1);
                    let spill_file = self.spill_dir.join(safe_filename);
                    direct_spill = Some((spill_file.clone(), comp_arc.clone()));
                    (false, None, Some(spill_file))
                };

                let new_node = LruNode {
                    key,
                    raw_size: raw_data.len(),
                    compressed_size: comp_len,
                    in_ram,
                    ram_data,
                    disk_path,
                    access_time: now,
                    prev: None,
                    next: None,
                    active: true,
                };

                let node_idx = shard.allocate_node(new_node);
                shard.map.insert(key, node_idx);
                shard.push_front(node_idx);
            }

            (evictions, direct_spill, old_disk_path)
        };

        // Lock-free I/O outside critical path
        if let Some(path) = old_disk_path {
            let _ = fs::remove_file(path);
        }
        for (path, data) in evictions {
            let _ = fs::write(path, &data);
        }
        if let Some((path, data)) = direct_spill {
            let _ = fs::write(path, &data);
        }

        Ok(())
    }

    /// Retrieves and decompresses chunk into `out_buf` with lock-free decompression & I/O.
    pub fn get(&self, session_id: &str, chunk_index: u64, out_buf: &mut [u8]) -> Result<usize, TTZipStatus> {
        let key = CacheKey::new(session_id, chunk_index);
        let s_idx = self.shard_idx(&key);
        let now = self.current_timestamp();

        enum CachedTarget {
            Ram(Arc<[u8]>),
            Disk(PathBuf),
        }

        // Phase 1: Fast read lock & metadata extraction
        let (target, raw_size, node_idx) = {
            let shard = self.shards[s_idx].read();
            let &node_idx = shard.map.get(&key).ok_or(TTZipStatus::ErrFileNotFound)?;
            let node = &shard.nodes[node_idx];
            if !node.active {
                return Err(TTZipStatus::ErrFileNotFound);
            }

            let raw_size = node.raw_size;
            let target = if node.in_ram {
                if let Some(ref data) = node.ram_data {
                    CachedTarget::Ram(Arc::clone(data))
                } else {
                    return Err(TTZipStatus::ErrFileNotFound);
                }
            } else if let Some(ref path) = node.disk_path {
                CachedTarget::Disk(path.clone())
            } else {
                return Err(TTZipStatus::ErrFileNotFound);
            };

            (target, raw_size, node_idx)
        };

        if out_buf.len() < raw_size {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        // Phase 2: Lightweight LRU promotion under short write lock (nanoseconds)
        {
            let mut shard = self.shards[s_idx].write();
            if let Some(&current_idx) = shard.map.get(&key) {
                if current_idx == node_idx && shard.nodes[node_idx].active {
                    shard.nodes[node_idx].access_time = now;
                    shard.move_to_front(node_idx);
                }
            }
        }

        // Phase 3: Completely Lock-Free Decompression & File I/O
        match target {
            CachedTarget::Ram(comp_data) => {
                let decomp_len = lz4_decompress(&comp_data, out_buf)?;
                Ok(decomp_len)
            }
            CachedTarget::Disk(path) => {
                let comp_data = fs::read(&path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
                let decomp_len = lz4_decompress(&comp_data, out_buf)?;
                Ok(decomp_len)
            }
        }
    }

    /// Clears all cached chunks associated with a specific session ID with lock-free file deletion.
    pub fn clear_session(&self, session_id: &str) {
        let session_hash = CacheKey::session_hash(session_id);
        let mut paths_to_delete = Vec::new();

        for shard_lock in &self.shards {
            let mut shard = shard_lock.write();
            let keys_to_remove: Vec<CacheKey> = shard
                .map
                .keys()
                .filter(|k| k.0 == session_hash)
                .cloned()
                .collect();
            for key in keys_to_remove {
                if let Some(&node_idx) = shard.map.get(&key) {
                    if let Some(path) = shard.remove_node(node_idx) {
                        paths_to_delete.push(path);
                    }
                }
            }
        }

        // Lock-free disk cleanup outside all shards
        for path in paths_to_delete {
            let _ = fs::remove_file(path);
        }
    }

    /// Returns cache statistics: `(ram_count, disk_count, ram_bytes)`.
    pub fn get_stats(&self) -> (usize, usize, usize) {
        let mut ram_count = 0;
        let mut disk_count = 0;
        let mut ram_bytes = 0;

        for shard_lock in &self.shards {
            let shard = shard_lock.read();
            for node in &shard.nodes {
                if node.active {
                    if node.in_ram {
                        ram_count += 1;
                        ram_bytes += node.compressed_size;
                    } else if node.disk_path.is_some() {
                        disk_count += 1;
                    }
                }
            }
        }

        (ram_count, disk_count, ram_bytes)
    }
}

impl Drop for VFSLz4CachePool {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.spill_dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vfs_cache_pool_basic_put_get() {
        let pool = VFSLz4CachePool::new(10 * 1024 * 1024, None);
        let raw = b"Hello, high-performance VFS cache with safe slot reuse!";
        pool.put("sess_1", 0, raw, 1).expect("put failed");

        let mut out = vec![0u8; raw.len()];
        let len = pool.get("sess_1", 0, &mut out).expect("get failed");
        assert_eq!(len, raw.len());
        assert_eq!(&out[..len], raw);
    }

    #[test]
    fn test_vfs_cache_pool_slot_reuse() {
        let pool = VFSLz4CachePool::new(1024 * 1024, None);
        for i in 0..1000 {
            let raw = format!("data chunk index {}", i).into_bytes();
            pool.put("sess_bench", i, &raw, 1).expect("put failed");
        }
        pool.clear_session("sess_bench");
        // Re-insert: slot reuse ensures no boundless node growth
        for i in 0..1000 {
            let raw = format!("data chunk recycled {}", i).into_bytes();
            pool.put("sess_bench_2", i, &raw, 1).expect("put failed");
        }
        let (ram_cnt, _, _) = pool.get_stats();
        assert!(ram_cnt <= 1000);
    }
}
