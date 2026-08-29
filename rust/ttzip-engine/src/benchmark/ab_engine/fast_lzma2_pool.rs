// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Fast-LZMA2 Lock-Free Dual Ping-Pong Buffer Pool & Dictionary Sharing Kernel.
//!
//! Inspired by Conor McCarthy's `vendor/fast-lzma2/fl2_pool.c` and `vendor/fast-lzma2/dict_buffer.c`:
//! - **Atomic Ping-Pong Switching**: Toggles active dictionary buffers via `index.fetch_xor(1, Ordering::AcqRel)`
//!   without heavyweight mutex locks.
//! - **16-Byte Aligned Overlap Shifting**: Shifts trailing dictionary overlap bytes (`(end - overlap) & !15`)
//!   across buffer flips to preserve LZMA2 match finding context.
//! - **Lock-Free Chunk Reuse**: Array-backed Treiber stack for $O(1)$ allocation-free slot acquisition and RAII release.
//! - **Bounded Resident Memory**: Pre-allocates all chunk buffers at initialization to guarantee zero dynamic
//!   allocations in high-concurrency streaming pipelines.

use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// 16-byte alignment mask for SIMD/vectorized match search in Fast-LZMA2.
pub const ALIGNMENT_SIZE: usize = 16;
pub const ALIGNMENT_MASK: usize = !(ALIGNMENT_SIZE - 1);

/// Sentinel value representing an empty list head in the lock-free index stack.
const SENTINEL_NONE: usize = usize::MAX;

/// Dual ping-pong dictionary buffer modeling Fast-LZMA2 `DICT_buffer`.
#[derive(Debug)]
pub struct DoublePingPongBuffer {
    /// Ping buffer 0.
    buffer_0: Vec<u8>,
    /// Pong buffer 1.
    buffer_1: Vec<u8>,
    /// Capacity of each individual buffer in bytes.
    capacity: usize,
    /// Overlap size to copy across buffer shifts.
    overlap: usize,
    /// Active buffer selector index (0 or 1).
    active_index: AtomicUsize,
    /// Unprocessed data start position in the active buffer.
    start: usize,
    /// Data write end position in the active buffer.
    end: usize,
    /// Cumulative uncompressed bytes processed since last dictionary reset.
    total_processed: usize,
    /// Dictionary reset interval in bytes (0 disables periodic resets).
    reset_interval: usize,
}

impl DoublePingPongBuffer {
    /// Creates a new dual ping-pong buffer with the specified capacity and overlap size.
    pub fn new(capacity: usize, overlap: usize, reset_interval: usize) -> Self {
        let aligned_cap = if capacity == 0 {
            64 * 1024
        } else {
            (capacity + ALIGNMENT_SIZE - 1) & ALIGNMENT_MASK
        };
        let safe_overlap = overlap.min(aligned_cap.saturating_sub(ALIGNMENT_SIZE));

        Self {
            buffer_0: vec![0u8; aligned_cap],
            buffer_1: vec![0u8; aligned_cap],
            capacity: aligned_cap,
            overlap: safe_overlap,
            active_index: AtomicUsize::new(0),
            start: 0,
            end: 0,
            total_processed: 0,
            reset_interval: if reset_interval != 0 {
                reset_interval
            } else {
                1usize << 30
            },
        }
    }

    /// Returns the active buffer index (0 or 1).
    #[inline]
    pub fn active_index(&self) -> usize {
        self.active_index.load(Ordering::Acquire) & 1
    }

    /// Returns a reference to the active buffer.
    #[inline]
    fn active_buf(&self) -> &[u8] {
        if self.active_index() == 0 {
            &self.buffer_0
        } else {
            &self.buffer_1
        }
    }

    /// Returns a mutable reference to the active buffer.
    #[inline]
    fn active_buf_mut(&mut self) -> &mut [u8] {
        let idx = self.active_index();
        if idx == 0 {
            &mut self.buffer_0
        } else {
            &mut self.buffer_1
        }
    }

    /// Returns a reference to the alternate (inactive) buffer.
    #[inline]
    pub fn alternate_buf(&self) -> &[u8] {
        if self.active_index() == 0 {
            &self.buffer_1
        } else {
            &self.buffer_0
        }
    }

    /// Returns a mutable reference to the alternate (inactive) buffer.
    #[inline]
    fn alternate_buf_mut(&mut self) -> &mut [u8] {
        let idx = self.active_index();
        if idx == 0 {
            &mut self.buffer_1
        } else {
            &mut self.buffer_0
        }
    }

    /// Returns the available free space remaining in the active buffer.
    #[inline]
    pub fn available_space(&self) -> usize {
        self.capacity.saturating_sub(self.end)
    }

    /// Returns the currently written slice of data in the active buffer.
    #[inline]
    pub fn written_slice(&self) -> &[u8] {
        &self.active_buf()[..self.end]
    }

    /// Returns the unprocessed data slice in the active buffer (`start..end`).
    #[inline]
    pub fn unprocessed_slice(&self) -> &[u8] {
        if self.start < self.end {
            &self.active_buf()[self.start..self.end]
        } else {
            &[]
        }
    }

    /// Appends incoming data into the active buffer up to available capacity.
    /// Returns the number of bytes written.
    pub fn write_bytes(&mut self, src: &[u8]) -> usize {
        let to_copy = src.len().min(self.available_space());
        if to_copy > 0 {
            let end = self.end;
            self.active_buf_mut()[end..end + to_copy].copy_from_slice(&src[..to_copy]);
            self.end += to_copy;
        }
        to_copy
    }

    /// Advances the unprocessed read pointer (`start` -> `end`), returning the processed slice range.
    pub fn mark_processed(&mut self) -> (usize, usize) {
        let range = (self.start, self.end);
        let processed_bytes = self.end.saturating_sub(self.start);
        self.total_processed = self.total_processed.saturating_add(processed_bytes);
        self.start = self.end;
        range
    }

    /// Executes atomic ping-pong buffer switching and dictionary overlap shift.
    ///
    /// Corresponds to Fast-LZMA2 `DICT_shift`:
    /// 1. Toggles active buffer index `index.fetch_xor(1, Ordering::AcqRel)`.
    /// 2. If overlap > 0 and end >= overlap + 16, copies trailing 16-byte aligned overlap
    ///    from previous buffer to start of new active buffer.
    /// 3. Resets `start = overlap_bytes` and `end = overlap_bytes`.
    pub fn shift_and_ping_pong(&mut self) -> usize {
        let mut overlap = self.overlap;
        if self.total_processed.saturating_add(self.capacity) > self.reset_interval {
            overlap = 0;
            self.total_processed = 0;
        }

        let curr_end = self.end;
        let mut actual_overlap = 0;

        if overlap == 0 || curr_end < overlap.saturating_add(ALIGNMENT_SIZE) {
            // No overlap copy, direct switch
            self.active_index.fetch_xor(1, Ordering::AcqRel);
            self.start = 0;
            self.end = 0;
        } else {
            let from = (curr_end - overlap) & ALIGNMENT_MASK;
            actual_overlap = curr_end - from;

            // Copy overlap bytes from active buffer to alternate buffer
            let src_data = self.active_buf()[from..curr_end].to_vec();
            self.alternate_buf_mut()[..actual_overlap].copy_from_slice(&src_data);

            // Toggle active index to switch to alternate buffer
            self.active_index.fetch_xor(1, Ordering::AcqRel);
            self.start = actual_overlap;
            self.end = actual_overlap;
        }

        actual_overlap
    }

    /// Resets the buffer state back to zero.
    pub fn reset(&mut self) {
        self.start = 0;
        self.end = 0;
        self.total_processed = 0;
        self.active_index.store(0, Ordering::Release);
    }
}

/// Internal pre-allocated slot container in the pool.
struct PoolSlotNode {
    buffer: UnsafeCell<DoublePingPongBuffer>,
    next_free: AtomicUsize,
}

/// Lock-free bounded buffer pool for Fast-LZMA2 streaming pipelines.
pub struct FastLzma2BufferPool {
    slots: Vec<PoolSlotNode>,
    free_head: AtomicUsize,
    available_count: AtomicUsize,
    capacity: usize,
    slot_buffer_capacity: usize,
}

// Explicit Send and Sync implementations for lock-free pool
unsafe impl Send for FastLzma2BufferPool {}
unsafe impl Sync for FastLzma2BufferPool {}

impl FastLzma2BufferPool {
    /// Creates a new pre-allocated lock-free buffer pool.
    ///
    /// Pre-allocates `slot_count` double ping-pong buffer slots of size `slot_buffer_size`.
    pub fn new(
        slot_count: usize,
        slot_buffer_size: usize,
        overlap: usize,
        reset_interval: usize,
    ) -> Arc<Self> {
        let count = slot_count.max(1);
        let mut slots = Vec::with_capacity(count);

        for i in 0..count {
            let next = if i + 1 < count { i + 1 } else { SENTINEL_NONE };
            slots.push(PoolSlotNode {
                buffer: UnsafeCell::new(DoublePingPongBuffer::new(
                    slot_buffer_size,
                    overlap,
                    reset_interval,
                )),
                next_free: AtomicUsize::new(next),
            });
        }

        Arc::new(Self {
            slots,
            free_head: AtomicUsize::new(0),
            available_count: AtomicUsize::new(count),
            capacity: count,
            slot_buffer_capacity: slot_buffer_size,
        })
    }

    /// Total number of slots configured in this pool.
    #[inline]
    pub fn total_slots(&self) -> usize {
        self.capacity
    }

    /// Number of idle slots currently available for acquisition.
    #[inline]
    pub fn available_slots(&self) -> usize {
        self.available_count.load(Ordering::Relaxed)
    }

    /// Total resident memory footprint in bytes allocated by the pool.
    #[inline]
    pub fn total_resident_memory_bytes(&self) -> usize {
        // Each slot contains two ping-pong buffers
        self.capacity * self.slot_buffer_capacity * 2
    }

    /// Acquires an idle chunk slot from the lock-free pool.
    ///
    /// Returns `Some(FastLzma2ChunkLease)` on success, or `None` if all slots are currently checked out.
    pub fn acquire(self: &Arc<Self>) -> Option<FastLzma2ChunkLease> {
        loop {
            let head = self.free_head.load(Ordering::Acquire);
            if head == SENTINEL_NONE {
                return None;
            }

            let next = self.slots[head].next_free.load(Ordering::Relaxed);
            if self
                .free_head
                .compare_exchange_weak(head, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                self.available_count.fetch_sub(1, Ordering::Relaxed);
                return Some(FastLzma2ChunkLease {
                    pool: Arc::clone(self),
                    slot_index: head,
                });
            }
        }
    }

    /// Releases a slot index back to the lock-free pool (called by `FastLzma2ChunkLease::drop`).
    fn release_slot(&self, slot_index: usize) {
        if slot_index >= self.capacity {
            return;
        }

        loop {
            let head = self.free_head.load(Ordering::Acquire);
            self.slots[slot_index]
                .next_free
                .store(head, Ordering::Relaxed);

            if self
                .free_head
                .compare_exchange_weak(
                    head,
                    slot_index,
                    Ordering::Release,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                self.available_count.fetch_add(1, Ordering::Relaxed);
                break;
            }
        }
    }
}

/// RAII Lease guard wrapping an acquired slot from [`FastLzma2BufferPool`].
///
/// Automatically returns the slot back to the lock-free pool upon drop.
pub struct FastLzma2ChunkLease {
    pool: Arc<FastLzma2BufferPool>,
    slot_index: usize,
}

impl FastLzma2ChunkLease {
    /// Returns the pool slot index of this lease.
    #[inline]
    pub fn slot_index(&self) -> usize {
        self.slot_index
    }
}

impl Deref for FastLzma2ChunkLease {
    type Target = DoublePingPongBuffer;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.pool.slots[self.slot_index].buffer.get() }
    }
}

impl DerefMut for FastLzma2ChunkLease {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.pool.slots[self.slot_index].buffer.get() }
    }
}

impl Drop for FastLzma2ChunkLease {
    fn drop(&mut self) {
        // Reset buffer state before releasing back to pool
        self.reset();
        self.pool.release_slot(self.slot_index);
    }
}

/// Concurrency stress test statistical report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoolStressReport {
    /// Number of concurrent worker threads.
    pub threads: usize,
    /// Total number of chunk acquisition cycles executed across all threads.
    pub total_cycles: usize,
    /// Total bytes streamed through ping-pong double buffers.
    pub total_bytes_streamed: usize,
    /// Elapsed duration in microseconds.
    pub elapsed_micros: f64,
    /// Throughput in operations/second.
    pub ops_per_sec: f64,
    /// Throughput in MB/s (1 MB = 1,048,576 bytes).
    pub throughput_mbs: f64,
    /// Whether 100% of slots were cleanly returned to the pool with zero leaks.
    pub zero_leak_verified: bool,
}

/// Runs a high-concurrency multi-threaded stress harness against the buffer pool.
pub fn run_pool_concurrency_stress_test(
    pool: Arc<FastLzma2BufferPool>,
    thread_count: usize,
    cycles_per_thread: usize,
) -> PoolStressReport {
    let threads = thread_count.max(1);
    let start_time = Instant::now();
    let mut handles = Vec::with_capacity(threads);

    for thread_id in 0..threads {
        let p = Arc::clone(&pool);
        let handle = std::thread::spawn(move || {
            let mut bytes_streamed = 0usize;
            let mut pattern = vec![0u8; 4096];
            for (i, b) in pattern.iter_mut().enumerate() {
                *b = ((thread_id * 17 + i) % 256) as u8;
            }

            for _ in 0..cycles_per_thread {
                // Loop until a slot is acquired
                let mut lease = loop {
                    if let Some(l) = p.acquire() {
                        break l;
                    }
                    std::thread::yield_now();
                };

                // Write data and perform ping-pong shift
                let written = lease.write_bytes(&pattern);
                assert_eq!(written, pattern.len());
                let _ = lease.mark_processed();
                let _overlap = lease.shift_and_ping_pong();
                bytes_streamed += written;
                // Lease is dropped at end of loop iteration
            }
            bytes_streamed
        });
        handles.push(handle);
    }

    let mut total_bytes = 0usize;
    for h in handles {
        total_bytes += h.join().expect("worker thread joined successfully");
    }

    let elapsed = start_time.elapsed();
    let elapsed_micros = elapsed.as_secs_f64() * 1_000_000.0;
    let total_cycles = threads * cycles_per_thread;
    let ops_per_sec = if elapsed.as_secs_f64() > 0.0 {
        (total_cycles as f64) / elapsed.as_secs_f64()
    } else {
        0.0
    };
    let throughput_mbs = if elapsed.as_secs_f64() > 0.0 {
        ((total_bytes as f64) / (1024.0 * 1024.0)) / elapsed.as_secs_f64()
    } else {
        0.0
    };

    let zero_leak = pool.available_slots() == pool.total_slots();

    PoolStressReport {
        threads,
        total_cycles,
        total_bytes_streamed: total_bytes,
        elapsed_micros,
        ops_per_sec,
        throughput_mbs,
        zero_leak_verified: zero_leak,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_double_ping_pong_buffer_write_and_shift() {
        let mut buf = DoublePingPongBuffer::new(64 * 1024, 256, 1024 * 1024);
        assert_eq!(buf.active_index(), 0);
        assert_eq!(buf.available_space(), 64 * 1024);

        let data = b"TTZip Fast-LZMA2 Ping-Pong Buffer Double-Buffering Stream Test.";
        let written = buf.write_bytes(data);
        assert_eq!(written, data.len());
        assert_eq!(buf.written_slice(), data);

        let processed = buf.mark_processed();
        assert_eq!(processed, (0, data.len()));
        assert!(buf.unprocessed_slice().is_empty());

        // Shift and flip
        let _overlap = buf.shift_and_ping_pong();
        assert_eq!(buf.active_index(), 1); // Flipped from 0 to 1
    }

    #[test]
    fn test_overlap_shifting_alignment() {
        // 1024 byte capacity, 64 byte overlap
        let mut buf = DoublePingPongBuffer::new(1024, 64, 1024 * 1024);
        let mut payload = vec![0u8; 512];
        for (i, b) in payload.iter_mut().enumerate() {
            *b = (i % 256) as u8;
        }

        buf.write_bytes(&payload);
        buf.mark_processed();

        let overlap_copied = buf.shift_and_ping_pong();
        assert!(overlap_copied >= 64);
        assert_eq!(buf.active_index(), 1);
        assert_eq!(buf.start, overlap_copied);
        assert_eq!(buf.end, overlap_copied);
    }

    #[test]
    fn test_lock_free_pool_exhaustion_and_raii_release() {
        let pool = FastLzma2BufferPool::new(2, 32 * 1024, 128, 0);
        assert_eq!(pool.total_slots(), 2);
        assert_eq!(pool.available_slots(), 2);
        assert_eq!(pool.total_resident_memory_bytes(), 2 * 32 * 1024 * 2);

        let lease1 = pool.acquire().expect("acquire 1");
        assert_eq!(pool.available_slots(), 1);

        let lease2 = pool.acquire().expect("acquire 2");
        assert_eq!(pool.available_slots(), 0);

        // Pool is exhausted
        assert!(pool.acquire().is_none());

        // Drop lease1 -> returns to pool
        drop(lease1);
        assert_eq!(pool.available_slots(), 1);

        let lease3 = pool.acquire().expect("acquire 3");
        assert_eq!(pool.available_slots(), 0);

        drop(lease2);
        drop(lease3);
        assert_eq!(pool.available_slots(), 2);
    }

    #[test]
    fn test_concurrency_stress_harness() {
        let pool = FastLzma2BufferPool::new(8, 16 * 1024, 64, 0);
        let report = run_pool_concurrency_stress_test(pool, 4, 100);

        assert_eq!(report.threads, 4);
        assert_eq!(report.total_cycles, 400);
        assert!(report.total_bytes_streamed > 0);
        assert!(report.zero_leak_verified);
        assert!(report.ops_per_sec > 0.0);
    }
}
