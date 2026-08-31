// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Cross-Language Memory Barrier, Buffer Pool, and RAII Allocations.
//!
//! Enforces 8-byte buffer alignment, prevents dual-allocator boundary leaks, and
//! provides high-throughput zero-copy borrowing and thread-safe buffer pooling.

use std::alloc::{alloc, dealloc, Layout};
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;

/// Strict 8-byte alignment invariant for UniFFI / ARM64 / SIMD compatibility.
pub const UNIFFI_BUFFER_ALIGNMENT: usize = 8;

/// Aggregated cross-language memory allocation and buffer pool telemetry.
#[derive(Clone, Debug, Default, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIMemoryStats {
    pub allocated_bytes: u64,
    pub deallocated_bytes: u64,
    pub active_allocations: u64,
    pub peak_allocated_bytes: u64,
    pub pooled_buffers_count: u64,
    pub pool_hit_count: u64,
    pub pool_miss_count: u64,
}

/// Global atomic memory counters for telemetry.
#[derive(Debug, Default)]
pub struct GlobalMemoryTracker {
    allocated_bytes: AtomicU64,
    deallocated_bytes: AtomicU64,
    active_allocations: AtomicU64,
    peak_allocated_bytes: AtomicU64,
    pool_hits: AtomicU64,
    pool_misses: AtomicU64,
}

impl GlobalMemoryTracker {
    pub const fn new() -> Self {
        Self {
            allocated_bytes: AtomicU64::new(0),
            deallocated_bytes: AtomicU64::new(0),
            active_allocations: AtomicU64::new(0),
            peak_allocated_bytes: AtomicU64::new(0),
            pool_hits: AtomicU64::new(0),
            pool_misses: AtomicU64::new(0),
        }
    }

    #[inline]
    pub fn record_alloc(&self, bytes: usize) {
        let b = bytes as u64;
        self.allocated_bytes.fetch_add(b, Ordering::Relaxed);
        let _active = self.active_allocations.fetch_add(1, Ordering::Relaxed) + 1;
        
        let current_allocated = self.allocated_bytes.load(Ordering::Relaxed)
            .saturating_sub(self.deallocated_bytes.load(Ordering::Relaxed));
        let mut peak = self.peak_allocated_bytes.load(Ordering::Relaxed);
        while current_allocated > peak {
            match self.peak_allocated_bytes.compare_exchange_weak(
                peak,
                current_allocated,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => peak = actual,
            }
        }
    }

    #[inline]
    pub fn record_dealloc(&self, bytes: usize) {
        let b = bytes as u64;
        self.deallocated_bytes.fetch_add(b, Ordering::Relaxed);
        self.active_allocations.fetch_sub(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_pool_hit(&self) {
        self.pool_hits.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_pool_miss(&self) {
        self.pool_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self, pooled_count: usize) -> UniFFIMemoryStats {
        UniFFIMemoryStats {
            allocated_bytes: self.allocated_bytes.load(Ordering::Relaxed),
            deallocated_bytes: self.deallocated_bytes.load(Ordering::Relaxed),
            active_allocations: self.active_allocations.load(Ordering::Relaxed),
            peak_allocated_bytes: self.peak_allocated_bytes.load(Ordering::Relaxed),
            pooled_buffers_count: pooled_count as u64,
            pool_hit_count: self.pool_hits.load(Ordering::Relaxed),
            pool_miss_count: self.pool_misses.load(Ordering::Relaxed),
        }
    }

    pub fn reset(&self) {
        self.allocated_bytes.store(0, Ordering::Relaxed);
        self.deallocated_bytes.store(0, Ordering::Relaxed);
        self.active_allocations.store(0, Ordering::Relaxed);
        self.peak_allocated_bytes.store(0, Ordering::Relaxed);
        self.pool_hits.store(0, Ordering::Relaxed);
        self.pool_misses.store(0, Ordering::Relaxed);
    }
}

static GLOBAL_MEMORY_TRACKER: GlobalMemoryTracker = GlobalMemoryTracker::new();

/// Safe RAII Memory Barrier Guard around 8-byte aligned raw allocations.
///
/// Prevents dual-allocator boundary leaks and guarantees deterministic destruction.
#[derive(Debug)]
pub struct RustBufferGuard {
    ptr: NonNull<u8>,
    capacity: usize,
    len: usize,
}

unsafe impl Send for RustBufferGuard {}
unsafe impl Sync for RustBufferGuard {}

impl RustBufferGuard {
    /// Allocates an 8-byte aligned memory buffer with requested capacity.
    pub fn with_capacity(capacity: usize) -> Option<Self> {
        let align = UNIFFI_BUFFER_ALIGNMENT;
        let effective_cap = capacity.max(8);
        let layout = Layout::from_size_align(effective_cap, align).ok()?;
        
        let raw_ptr = unsafe { alloc(layout) };
        let ptr = NonNull::new(raw_ptr)?;
        
        GLOBAL_MEMORY_TRACKER.record_alloc(effective_cap);

        Some(Self {
            ptr,
            capacity: effective_cap,
            len: 0,
        })
    }

    /// Creates a guard from existing byte slice by allocating and copying.
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        let mut guard = Self::with_capacity(slice.len())?;
        unsafe {
            std::ptr::copy_nonoverlapping(slice.as_ptr(), guard.ptr.as_ptr(), slice.len());
        }
        guard.len = slice.len();
        Some(guard)
    }

    /// Verifies whether the buffer pointer conforms to 8-byte alignment.
    #[inline]
    pub fn is_8byte_aligned(&self) -> bool {
        (self.ptr.as_ptr() as usize).is_multiple_of(UNIFFI_BUFFER_ALIGNMENT)
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }

    #[inline]
    pub fn set_len(&mut self, new_len: usize) {
        assert!(new_len <= self.capacity, "set_len exceeds buffer capacity");
        self.len = new_len;
    }

    /// Converts this guard into an owned `Vec<u8>`.
    pub fn into_vec(self) -> Vec<u8> {
        let slice = self.as_slice();
        slice.to_vec()
    }
}

impl Deref for RustBufferGuard {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl DerefMut for RustBufferGuard {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl Drop for RustBufferGuard {
    fn drop(&mut self) {
        if self.capacity > 0 {
            if let Ok(layout) = Layout::from_size_align(self.capacity, UNIFFI_BUFFER_ALIGNMENT) {
                unsafe {
                    dealloc(self.ptr.as_ptr(), layout);
                }
                GLOBAL_MEMORY_TRACKER.record_dealloc(self.capacity);
            }
        }
    }
}

/// Zero-copy borrowing descriptor for foreign byte slices across FFI boundaries.
#[derive(Clone, Copy, Debug)]
pub struct ForeignBorrowedBytes<'a> {
    data: &'a [u8],
}

impl<'a> ForeignBorrowedBytes<'a> {
    /// Constructs a safe zero-copy borrow from a raw foreign pointer and length.
    ///
    /// # Safety
    /// The caller must ensure `ptr` is valid, non-null, properly aligned, and points to
    /// `len` readable bytes that remain valid for lifetime `'a`.
    pub unsafe fn from_raw_parts(ptr: *const u8, len: usize) -> Option<Self> {
        if ptr.is_null() && len > 0 {
            return None;
        }
        if len == 0 {
            return Some(Self { data: &[] });
        }
        let data = std::slice::from_raw_parts(ptr, len);
        Some(Self { data })
    }

    #[inline]
    pub fn as_slice(&self) -> &'a [u8] {
        self.data
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Thread-safe bounded memory buffer pool for high-throughput cross-language transfers.
#[derive(uniffi::Object)]
pub struct UniFFIBufferPool {
    max_capacity: usize,
    default_buffer_size: usize,
    pool: Mutex<Vec<Vec<u8>>>,
}

#[uniffi::export]
impl UniFFIBufferPool {
    /// Creates a new buffer pool with bounded entry count and default chunk size.
    #[uniffi::constructor]
    pub fn new(max_entries: u32, default_size: u32) -> Arc<Self> {
        Arc::new(Self {
            max_capacity: (max_entries as usize).clamp(1, 1024),
            default_buffer_size: (default_size as usize).clamp(64, 64 * 1024 * 1024),
            pool: Mutex::new(Vec::with_capacity(max_entries as usize)),
        })
    }

    /// Acquires a byte buffer with at least `min_capacity` bytes.
    pub fn acquire(&self, min_capacity: u32) -> Vec<u8> {
        let req_cap = if min_capacity == 0 {
            self.default_buffer_size
        } else {
            min_capacity as usize
        };

        let mut lock = self.pool.lock();
        if let Some(pos) = lock.iter().position(|b| b.capacity() >= req_cap) {
            GLOBAL_MEMORY_TRACKER.record_pool_hit();
            let mut buf = lock.swap_remove(pos);
            buf.clear();
            buf
        } else {
            GLOBAL_MEMORY_TRACKER.record_pool_miss();
            GLOBAL_MEMORY_TRACKER.record_alloc(req_cap);
            Vec::with_capacity(req_cap)
        }
    }

    /// Returns a used buffer back to the pool if within capacity bounds.
    pub fn release(&self, mut buffer: Vec<u8>) {
        let mut lock = self.pool.lock();
        if lock.len() < self.max_capacity && buffer.capacity() >= self.default_buffer_size / 2 {
            buffer.clear();
            lock.push(buffer);
        } else {
            GLOBAL_MEMORY_TRACKER.record_dealloc(buffer.capacity());
            drop(buffer);
        }
    }

    /// Takes a current snapshot of memory metrics and pool occupancy.
    pub fn get_stats(&self) -> UniFFIMemoryStats {
        let lock = self.pool.lock();
        GLOBAL_MEMORY_TRACKER.snapshot(lock.len())
    }

    /// Clears all pooled buffers and releases memory to the OS.
    pub fn clear(&self) {
        let mut lock = self.pool.lock();
        for buf in lock.drain(..) {
            GLOBAL_MEMORY_TRACKER.record_dealloc(buf.capacity());
        }
    }

    /// Resets memory statistics counters.
    pub fn reset_stats(&self) {
        GLOBAL_MEMORY_TRACKER.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_buffer_guard_lifecycle_and_alignment() {
        let guard = RustBufferGuard::with_capacity(128).expect("Allocation should succeed");
        assert!(guard.is_8byte_aligned(), "Buffer must be 8-byte aligned");
        assert!(guard.capacity() >= 128);
        assert_eq!(guard.len(), 0);
        assert!(guard.is_empty());

        let slice_guard = RustBufferGuard::from_slice(b"TTZip UniFFI Memory Guard").expect("Slice alloc failed");
        assert_eq!(slice_guard.len(), 25);
        assert_eq!(slice_guard.as_slice(), b"TTZip UniFFI Memory Guard");
        assert!(slice_guard.is_8byte_aligned());

        let vec_data = slice_guard.into_vec();
        assert_eq!(vec_data, b"TTZip UniFFI Memory Guard");
    }

    #[test]
    fn test_foreign_borrowed_bytes() {
        let data = b"Foreign zero-copy payload";
        let borrowed = unsafe {
            ForeignBorrowedBytes::from_raw_parts(data.as_ptr(), data.len())
        }.expect("Should borrow slice");

        assert_eq!(borrowed.len(), data.len());
        assert_eq!(borrowed.as_slice(), data);

        let empty_borrowed = unsafe {
            ForeignBorrowedBytes::from_raw_parts(std::ptr::null(), 0)
        }.expect("Empty borrow with null should succeed");
        assert!(empty_borrowed.is_empty());
    }

    #[test]
    fn test_uniffi_buffer_pool_acquire_and_release() {
        let pool = UniFFIBufferPool::new(8, 1024);
        pool.reset_stats();

        let mut buf1 = pool.acquire(512);
        assert!(buf1.capacity() >= 512);
        buf1.extend_from_slice(b"payload 1");

        pool.release(buf1);

        let buf2 = pool.acquire(256);
        assert!(buf2.is_empty());
        assert!(buf2.capacity() >= 512);

        let stats = pool.get_stats();
        assert!(stats.pool_hit_count >= 1);

        pool.clear();
        assert_eq!(pool.get_stats().pooled_buffers_count, 0);
    }
}
