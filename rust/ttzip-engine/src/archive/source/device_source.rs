// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Android Device Archive Source adapter bridging `DeviceStorageDriver` to `ArchiveSource`.
//!
//! Implements the Stream-First Invariant: performs zero-copy remote partial reads
//! (`get_partial_object`) with an internal 64KB micro-chunk LRU cache to optimize
//! backward seeking for ZIP EOCD (End of Central Directory) and directory headers.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use bytes::Bytes;
use parking_lot::Mutex;

use crate::archive::source::{ArchiveSource, StorageMedium};
use crate::types::TTZipStatus;
use ttzip_device_android::traits::DeviceStorageDriver;

/// Default block size for remote partial reads (64 KB).
pub const DEVICE_SOURCE_BLOCK_SIZE: usize = 64 * 1024;

/// Default maximum number of cached blocks (32 blocks = 2 MB resident cache).
pub const DEVICE_SOURCE_DEFAULT_CACHE_CAPACITY: usize = 32;

/// Thread-safe LRU cache entry storing block payload.
#[derive(Debug, Clone)]
struct CachedBlock {
    data: Bytes,
}

/// Compact bounded Least-Recently-Used (LRU) cache for remote 64KB chunks.
struct LruChunkCache {
    capacity: usize,
    chunks: HashMap<u64, CachedBlock>,
    lru_order: VecDeque<u64>,
}

impl LruChunkCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            chunks: HashMap::with_capacity(capacity),
            lru_order: VecDeque::with_capacity(capacity),
        }
    }

    fn get(&mut self, block_idx: u64) -> Option<Bytes> {
        if let Some(entry) = self.chunks.get(&block_idx) {
            if let Some(pos) = self.lru_order.iter().position(|&k| k == block_idx) {
                self.lru_order.remove(pos);
            }
            self.lru_order.push_back(block_idx);
            Some(entry.data.clone())
        } else {
            None
        }
    }

    fn insert(&mut self, block_idx: u64, data: Bytes) {
        if self.chunks.contains_key(&block_idx) {
            if let Some(pos) = self.lru_order.iter().position(|&k| k == block_idx) {
                self.lru_order.remove(pos);
            }
        } else if self.chunks.len() >= self.capacity {
            if let Some(oldest) = self.lru_order.pop_front() {
                self.chunks.remove(&oldest);
            }
        }
        self.chunks.insert(block_idx, CachedBlock { data: data.clone() });
        self.lru_order.push_back(block_idx);
    }
}

/// Virtual archive source backed directly by an active Android device storage volume.
pub struct DeviceArchiveSource {
    driver: Arc<dyn DeviceStorageDriver>,
    remote_path: String,
    total_size: u64,
    cache: Mutex<LruChunkCache>,
}

impl DeviceArchiveSource {
    /// Creates a new `DeviceArchiveSource` for a remote archive on an Android device.
    pub fn new(
        driver: Arc<dyn DeviceStorageDriver>,
        remote_path: impl Into<String>,
        total_size: u64,
    ) -> Self {
        Self::with_cache_capacity(driver, remote_path, total_size, DEVICE_SOURCE_DEFAULT_CACHE_CAPACITY)
    }

    /// Creates a new `DeviceArchiveSource` with a specified cache block capacity.
    pub fn with_cache_capacity(
        driver: Arc<dyn DeviceStorageDriver>,
        remote_path: impl Into<String>,
        total_size: u64,
        cache_capacity: usize,
    ) -> Self {
        Self {
            driver,
            remote_path: remote_path.into(),
            total_size,
            cache: Mutex::new(LruChunkCache::new(cache_capacity)),
        }
    }

    /// Synchronously executes a future on the current or local Tokio runtime.
    fn block_on_driver<F, R>(&self, future: F) -> Result<R, TTZipStatus>
    where
        F: std::future::Future<Output = Result<R, ttzip_device_android::error::DeviceError>> + Send,
        R: Send,
    {
        let res = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| handle.block_on(future))
        } else {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| TTZipStatus::ErrArchiveInitFailed)?
                .block_on(future)
        };

        res.map_err(|_err| TTZipStatus::ErrExtractionFailed)
    }

    /// Fetches a 64KB block either from the internal LRU cache or via `get_partial_object`.
    fn fetch_block(&self, block_idx: u64) -> Result<Bytes, TTZipStatus> {
        {
            let mut cache = self.cache.lock();
            if let Some(bytes) = cache.get(block_idx) {
                return Ok(bytes);
            }
        }

        let block_offset = block_idx.saturating_mul(DEVICE_SOURCE_BLOCK_SIZE as u64);
        if block_offset >= self.total_size {
            return Ok(Bytes::new());
        }

        let remaining = self.total_size - block_offset;
        let read_len = (DEVICE_SOURCE_BLOCK_SIZE as u64).min(remaining) as u32;

        let future = self.driver.get_partial_object(&self.remote_path, block_offset, read_len);
        let fetched = self.block_on_driver(future)?;

        {
            let mut cache = self.cache.lock();
            cache.insert(block_idx, fetched.clone());
        }

        Ok(fetched)
    }
}

impl ArchiveSource for DeviceArchiveSource {
    fn as_slice(&self) -> Option<&[u8]> {
        // Remote device archives are streamed on-demand; full memory-mapping is rejected to uphold the
        // <= 64MB resident memory ceiling invariant.
        None
    }

    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, TTZipStatus> {
        if offset >= self.total_size || buf.is_empty() {
            return Ok(0);
        }

        let available = self.total_size - offset;
        let to_read = (buf.len() as u64).min(available) as usize;

        let mut written = 0usize;
        let mut current_offset = offset;

        while written < to_read {
            let block_idx = current_offset / (DEVICE_SOURCE_BLOCK_SIZE as u64);
            let offset_in_block = (current_offset % (DEVICE_SOURCE_BLOCK_SIZE as u64)) as usize;

            let block_data = self.fetch_block(block_idx)?;
            if offset_in_block >= block_data.len() {
                break;
            }

            let available_in_block = block_data.len() - offset_in_block;
            let bytes_needed = to_read - written;
            let copy_len = bytes_needed.min(available_in_block);

            buf[written..written + copy_len]
                .copy_from_slice(&block_data[offset_in_block..offset_in_block + copy_len]);

            written += copy_len;
            current_offset += copy_len as u64;
        }

        Ok(written)
    }

    fn len(&self) -> u64 {
        self.total_size
    }

    fn medium(&self) -> StorageMedium {
        StorageMedium::VirtualFilesystem
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ttzip_device_android::models::AndroidVfsNode;
    use ttzip_device_android::error::DeviceError;

    use ttzip_device_android::traits::BoxFuture;

    struct MockStorageDriver {
        backing_data: Vec<u8>,
    }

    impl DeviceStorageDriver for MockStorageDriver {
        fn list_directory<'a>(&'a self, _path: &'a str) -> BoxFuture<'a, Result<Vec<AndroidVfsNode>, DeviceError>> {
            Box::pin(async move { Ok(vec![]) })
        }

        fn get_partial_object<'a>(
            &'a self,
            _path: &'a str,
            offset: u64,
            length: u32,
        ) -> BoxFuture<'a, Result<Bytes, DeviceError>> {
            Box::pin(async move {
                let off = offset as usize;
                if off >= self.backing_data.len() {
                    return Ok(Bytes::new());
                }
                let end = (off + length as usize).min(self.backing_data.len());
                Ok(Bytes::copy_from_slice(&self.backing_data[off..end]))
            })
        }

        fn send_object<'a>(
            &'a self,
            _destination_path: &'a str,
            _data: Bytes,
        ) -> BoxFuture<'a, Result<(), DeviceError>> {
            Box::pin(async move { Ok(()) })
        }

        fn delete_object<'a>(&'a self, _path: &'a str) -> BoxFuture<'a, Result<(), DeviceError>> {
            Box::pin(async move { Ok(()) })
        }

        fn disconnect<'a>(&'a self) -> BoxFuture<'a, Result<(), DeviceError>> {
            Box::pin(async move { Ok(()) })
        }
    }

    #[test]
    fn test_device_source_ranged_reads_and_lru() {
        let size = 150 * 1024; // 150 KB (3 blocks: 64KB + 64KB + 22KB)
        let mut sample_payload = vec![0u8; size];
        for (i, b) in sample_payload.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }

        let driver = Arc::new(MockStorageDriver {
            backing_data: sample_payload.clone(),
        });

        let source = DeviceArchiveSource::new(driver, "/sdcard/test.zip", size as u64);
        assert_eq!(source.len(), size as u64);
        assert_eq!(source.medium(), StorageMedium::VirtualFilesystem);
        assert!(source.as_slice().is_none());

        // 1. Read across block boundary: offset 65530 for 20 bytes
        let mut buf = vec![0u8; 20];
        let bytes_read = source.read_at(&mut buf, 65530).expect("Failed read_at");
        assert_eq!(bytes_read, 20);
        assert_eq!(&buf[..], &sample_payload[65530..65550]);

        // 2. Read tail of the file (EOCD simulation)
        let mut tail_buf = vec![0u8; 64];
        let tail_offset = (size - 64) as u64;
        let tail_read = source.read_at(&mut tail_buf, tail_offset).expect("Tail read");
        assert_eq!(tail_read, 64);
        assert_eq!(&tail_buf[..], &sample_payload[tail_offset as usize..]);

        // 3. Out-of-bounds read returns 0
        let mut oob_buf = vec![0u8; 10];
        let oob_read = source.read_at(&mut oob_buf, size as u64 + 100).expect("OOB read");
        assert_eq!(oob_read, 0);
    }
}
