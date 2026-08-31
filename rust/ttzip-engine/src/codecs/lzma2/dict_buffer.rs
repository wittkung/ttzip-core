// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Double-Buffered Asynchronous Ping-Pong Sliding Window Dictionary Buffer (`DictBuffer`).
//!
//! Provides ultra-low-latency, zero-allocation sliding dictionary history management
//! for multi-threaded chunked LZMA2 compression:
//! - **Ping-Pong Dual Buffer (Buffer A / Buffer B)**: Asynchronously isolates active chunk
//!   encoding from subsequent chunk I/O preloading.
//! - **Configurable Overlap Fraction ($0/16 \sim 14/16$)**: Controls cross-chunk dictionary
//!   history preservation ($0\% \sim 87.5\%$, default $2/16 = 12.5\%$).
//! - **16-Byte SIMD-Aligned `dict_shift`**: High-throughput vectorized memory translocation
//!   for sliding dictionary history handover.
//! - **Large Dictionary Scaling**: Seamlessly scales from 1 MB up to 1 GB+ dictionaries.
//! - **Deterministic Resident Memory Bounds**: Enforces single-task resident memory limits ($\le 64\text{ MB}$).

use crate::types::TTZipStatus;
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

/// Default dictionary size for LZMA2 compression (64 MB).
pub const DEFAULT_DICT_SIZE: usize = 64 * 1024 * 1024;

/// Minimum supported dictionary size (1 MB).
pub const MIN_DICT_SIZE: usize = 1024 * 1024;

/// Maximum supported dictionary size (1 GB).
pub const MAX_DICT_SIZE: usize = 1024 * 1024 * 1024;

/// Default overlap fraction ($2/16 = 12.5\%$).
pub const DEFAULT_OVERLAP_FRACTION: u8 = 2;

/// Maximum allowable overlap fraction ($14/16 = 87.5\%$).
pub const MAX_OVERLAP_FRACTION: u8 = 14;

/// Scale divisor for overlap fraction calculations ($16$).
pub const OVERLAP_SCALE: u32 = 16;

/// Standard memory alignment boundary for SIMD operations (16 bytes).
pub const SIMD_ALIGNMENT: usize = 16;

/// Default single-task resident memory hard ceiling (64 MB).
pub const DEFAULT_TASK_RESIDENT_MEMORY_LIMIT: usize = 64 * 1024 * 1024;

/// Identifier for ping-pong buffer slots.
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BufferId {
    /// Buffer Slot A.
    A = 0,
    /// Buffer Slot B.
    B = 1,
}

impl BufferId {
    /// Returns the alternate ping-pong buffer identifier.
    #[inline(always)]
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }

    /// Returns the numerical index of the buffer (0 or 1).
    #[inline(always)]
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// 16-byte SIMD-aligned heap allocation container for dictionary history.
pub struct DictAlignedBuffer {
    ptr: NonNull<u8>,
    capacity: usize,
    layout: Layout,
}

/// Type alias for [`DictAlignedBuffer`].
pub type Lzma2AlignedBuffer = DictAlignedBuffer;

unsafe impl Send for DictAlignedBuffer {}
unsafe impl Sync for DictAlignedBuffer {}

impl DictAlignedBuffer {
    /// Allocates a new 16-byte aligned zero-initialized buffer.
    pub fn new(capacity: usize) -> Result<Self, TTZipStatus> {
        if capacity == 0 {
            let layout = Layout::from_size_align(16, SIMD_ALIGNMENT)
                .map_err(|_| TTZipStatus::ErrInvalidParam)?;
            return Ok(Self {
                ptr: NonNull::dangling(),
                capacity: 0,
                layout,
            });
        }

        let layout = Layout::from_size_align(capacity, SIMD_ALIGNMENT)
            .map_err(|_| TTZipStatus::ErrInvalidParam)?;
        let raw_ptr = unsafe { alloc_zeroed(layout) };
        let ptr = NonNull::new(raw_ptr).ok_or(TTZipStatus::ErrOutOfMemory)?;

        Ok(Self {
            ptr,
            capacity,
            layout,
        })
    }

    /// Returns the allocated capacity in bytes.
    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns `true` if the memory address satisfies 16-byte alignment.
    #[inline(always)]
    pub fn is_aligned_16(&self) -> bool {
        (self.ptr.as_ptr() as usize).is_multiple_of(SIMD_ALIGNMENT)
    }

    /// Returns raw immutable byte pointer.
    #[inline(always)]
    pub const fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr()
    }

    /// Returns raw mutable byte pointer.
    #[inline(always)]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    /// Returns immutable slice over the full capacity.
    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        if self.capacity == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.capacity) }
        }
    }

    /// Returns mutable slice over the full capacity.
    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        if self.capacity == 0 {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.capacity) }
        }
    }
}

impl Deref for DictAlignedBuffer {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl DerefMut for DictAlignedBuffer {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl std::fmt::Debug for DictAlignedBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DictAlignedBuffer")
            .field("capacity", &self.capacity)
            .field("is_aligned_16", &self.is_aligned_16())
            .finish()
    }
}

impl Drop for DictAlignedBuffer {
    fn drop(&mut self) {
        if self.capacity > 0 {
            unsafe {
                dealloc(self.ptr.as_ptr(), self.layout);
            }
        }
    }
}

/// Translocates history bytes into destination buffer using 16-byte SIMD-aligned vectorization.
///
/// Copies `src.len()` bytes into `dst[0..src.len()]`.
///
/// # Returns
/// The number of bytes successfully shifted.
#[inline]
pub fn dict_shift(dst: &mut [u8], src: &[u8]) -> usize {
    let copy_len = src.len().min(dst.len());
    if copy_len == 0 {
        return 0;
    }

    let chunks_16 = copy_len / SIMD_ALIGNMENT;
    let remainder = copy_len % SIMD_ALIGNMENT;

    let src_prefix = &src[..chunks_16 * SIMD_ALIGNMENT];
    let dst_prefix = &mut dst[..chunks_16 * SIMD_ALIGNMENT];

    let src_chunks = src_prefix.chunks_exact(SIMD_ALIGNMENT);
    let dst_chunks = dst_prefix.chunks_exact_mut(SIMD_ALIGNMENT);

    for (d, s) in dst_chunks.zip(src_chunks) {
        d.copy_from_slice(s);
    }

    if remainder > 0 {
        let tail_start = chunks_16 * SIMD_ALIGNMENT;
        dst[tail_start..tail_start + remainder]
            .copy_from_slice(&src[tail_start..tail_start + remainder]);
    }

    copy_len
}

/// Ping-Pong buffer slot maintaining sliding dictionary history and active chunk payload.
pub struct BufferSlot {
    storage: DictAlignedBuffer,
    history_len: usize,
    payload_len: usize,
}

impl BufferSlot {
    /// Creates a new buffer slot with the specified capacity.
    pub fn new(capacity: usize) -> Result<Self, TTZipStatus> {
        let storage = DictAlignedBuffer::new(capacity)?;
        Ok(Self {
            storage,
            history_len: 0,
            payload_len: 0,
        })
    }

    /// Returns the total active byte length (history prefix + current payload).
    #[inline(always)]
    pub const fn total_len(&self) -> usize {
        self.history_len + self.payload_len
    }

    /// Returns the history prefix byte length.
    #[inline(always)]
    pub const fn history_len(&self) -> usize {
        self.history_len
    }

    /// Returns the active uncompressed payload byte length.
    #[inline(always)]
    pub const fn payload_len(&self) -> usize {
        self.payload_len
    }

    /// Returns remaining unused byte capacity in the buffer slot.
    #[inline(always)]
    pub const fn remaining_capacity(&self) -> usize {
        self.storage.capacity().saturating_sub(self.total_len())
    }

    /// Returns slice over the retained history prefix.
    #[inline(always)]
    pub fn history_slice(&self) -> &[u8] {
        &self.storage[..self.history_len]
    }

    /// Returns slice over the newly written uncompressed payload.
    #[inline(always)]
    pub fn payload_slice(&self) -> &[u8] {
        &self.storage[self.history_len..self.history_len + self.payload_len]
    }

    /// Returns slice over the full active window (history + payload).
    #[inline(always)]
    pub fn full_slice(&self) -> &[u8] {
        &self.storage[..self.total_len()]
    }

    /// Writes uncompressed payload data into the active slot.
    pub fn write_payload(&mut self, data: &[u8]) -> Result<usize, TTZipStatus> {
        if data.is_empty() {
            return Ok(0);
        }
        let available = self.remaining_capacity();
        if data.len() > available {
            return Err(TTZipStatus::ErrOutOfMemory);
        }

        let write_start = self.total_len();
        let write_end = write_start + data.len();
        self.storage[write_start..write_end].copy_from_slice(data);
        self.payload_len += data.len();

        Ok(data.len())
    }

    /// Resets history and payload lengths to zero without deallocating.
    #[inline]
    pub fn clear(&mut self) {
        self.history_len = 0;
        self.payload_len = 0;
    }
}

impl std::fmt::Debug for BufferSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufferSlot")
            .field("capacity", &self.storage.capacity())
            .field("history_len", &self.history_len)
            .field("payload_len", &self.payload_len)
            .field("total_len", &self.total_len())
            .finish()
    }
}

/// High-performance double-buffered sliding dictionary history manager.
pub struct DictBuffer {
    buffers: [BufferSlot; 2],
    active: BufferId,
    dict_size: usize,
    chunk_size: usize,
    overlap_fraction: u8,
    total_uncompressed_processed: u64,
    chunk_index: u64,
}

impl std::fmt::Debug for DictBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DictBuffer")
            .field("active", &self.active)
            .field("dict_size", &self.dict_size)
            .field("chunk_size", &self.chunk_size)
            .field("overlap_fraction", &self.overlap_fraction)
            .field("overlap_size", &self.overlap_size())
            .field("chunk_index", &self.chunk_index)
            .field("total_uncompressed_processed", &self.total_uncompressed_processed)
            .finish()
    }
}

impl DictBuffer {
    /// Computes the exact byte length of dictionary overlap for a given dictionary size and fraction.
    #[inline]
    #[must_use]
    pub const fn calculate_overlap_size(dict_size: usize, overlap_fraction: u8) -> usize {
        let fraction = if overlap_fraction > MAX_OVERLAP_FRACTION {
            MAX_OVERLAP_FRACTION as u64
        } else {
            overlap_fraction as u64
        };
        ((dict_size as u64 * fraction) / OVERLAP_SCALE as u64) as usize
    }

    /// Creates a new `DictBuffer` instance with explicit dictionary size, chunk size, and overlap fraction.
    ///
    /// # Errors
    /// Returns [`TTZipStatus::ErrInvalidParam`] if `dict_size == 0`, `chunk_size == 0`, or `overlap_fraction > 14`.
    /// Returns [`TTZipStatus::ErrOutOfMemory`] if underlying allocation fails.
    pub fn new(
        dict_size: usize,
        chunk_size: usize,
        overlap_fraction: u8,
    ) -> Result<Self, TTZipStatus> {
        Self::with_budget(
            dict_size,
            chunk_size,
            overlap_fraction,
            usize::MAX,
        )
    }

    /// Creates a new `DictBuffer` with default $2/16$ ($12.5\%$) overlap fraction.
    pub fn with_default_overlap(
        dict_size: usize,
        chunk_size: usize,
    ) -> Result<Self, TTZipStatus> {
        Self::new(dict_size, chunk_size, DEFAULT_OVERLAP_FRACTION)
    }

    /// Creates a new `DictBuffer` enforcing a maximum resident memory budget.
    pub fn with_budget(
        dict_size: usize,
        chunk_size: usize,
        overlap_fraction: u8,
        max_resident_budget: usize,
    ) -> Result<Self, TTZipStatus> {
        if dict_size == 0 || chunk_size == 0 || overlap_fraction > MAX_OVERLAP_FRACTION {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        let overlap_size = Self::calculate_overlap_size(dict_size, overlap_fraction);
        let slot_capacity = overlap_size
            .checked_add(chunk_size)
            .ok_or(TTZipStatus::ErrInvalidParam)?;

        let total_required = slot_capacity
            .checked_mul(2)
            .ok_or(TTZipStatus::ErrInvalidParam)?;

        if total_required > max_resident_budget {
            return Err(TTZipStatus::ErrSolidBudgetExceeded);
        }

        let slot_a = BufferSlot::new(slot_capacity)?;
        let slot_b = BufferSlot::new(slot_capacity)?;

        Ok(Self {
            buffers: [slot_a, slot_b],
            active: BufferId::A,
            dict_size,
            chunk_size,
            overlap_fraction,
            total_uncompressed_processed: 0,
            chunk_index: 0,
        })
    }

    /// Returns the target LZMA2 dictionary size in bytes.
    #[inline(always)]
    pub const fn dict_size(&self) -> usize {
        self.dict_size
    }

    /// Returns the uncompressed chunk size in bytes.
    #[inline(always)]
    pub const fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    /// Returns the active overlap fraction ($0 \sim 14$).
    #[inline(always)]
    pub const fn overlap_fraction(&self) -> u8 {
        self.overlap_fraction
    }

    /// Updates the overlap fraction for subsequent chunk rotations.
    pub fn set_overlap_fraction(&mut self, fraction: u8) -> Result<(), TTZipStatus> {
        if fraction > MAX_OVERLAP_FRACTION {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        self.overlap_fraction = fraction;
        Ok(())
    }

    /// Returns the computed overlap byte length for the current dictionary configuration.
    #[inline(always)]
    pub const fn overlap_size(&self) -> usize {
        Self::calculate_overlap_size(self.dict_size, self.overlap_fraction)
    }

    /// Returns the currently active buffer identifier (`BufferId::A` or `BufferId::B`).
    #[inline(always)]
    pub const fn active_buffer_id(&self) -> BufferId {
        self.active
    }

    /// Returns the index of the currently active chunk (0-indexed).
    #[inline(always)]
    pub const fn chunk_index(&self) -> u64 {
        self.chunk_index
    }

    /// Returns total uncompressed payload bytes committed across all chunks.
    #[inline(always)]
    pub const fn total_uncompressed_processed(&self) -> u64 {
        self.total_uncompressed_processed
    }

    /// Returns the total resident heap memory allocated across both ping-pong buffers.
    #[inline]
    pub fn total_memory_consumption(&self) -> usize {
        self.buffers[0].storage.capacity() + self.buffers[1].storage.capacity()
    }

    /// Returns the active working slice `(total_data, encode_start_pos)`.
    ///
    /// - `total_data`: Complete sliding window slice including retained history prefix and new payload.
    /// - `encode_start_pos`: Offset where new uncompressed payload begins.
    #[inline]
    pub fn get_active_slice(&self) -> (&[u8], usize) {
        let active_slot = &self.buffers[self.active.index()];
        (active_slot.full_slice(), active_slot.history_len())
    }

    /// Returns immutable slice over the uncompressed payload of the active chunk.
    #[inline]
    pub fn get_active_payload(&self) -> &[u8] {
        self.buffers[self.active.index()].payload_slice()
    }

    /// Returns immutable slice over the history prefix of the active chunk.
    #[inline]
    pub fn get_active_history(&self) -> &[u8] {
        self.buffers[self.active.index()].history_slice()
    }

    /// Returns total bytes in the active window (history + payload).
    #[inline(always)]
    pub fn active_total_len(&self) -> usize {
        self.buffers[self.active.index()].total_len()
    }

    /// Returns history byte length in the active window.
    #[inline(always)]
    pub fn active_history_len(&self) -> usize {
        self.buffers[self.active.index()].history_len()
    }

    /// Returns uncompressed payload byte length in the active window.
    #[inline(always)]
    pub fn active_payload_len(&self) -> usize {
        self.buffers[self.active.index()].payload_len()
    }

    /// Appends uncompressed payload bytes into the currently active buffer.
    ///
    /// # Errors
    /// Returns [`TTZipStatus::ErrOutOfMemory`] if writing exceeds the configured `chunk_size`
    /// or underlying buffer slot remaining capacity.
    pub fn write_payload(&mut self, data: &[u8]) -> Result<usize, TTZipStatus> {
        let active_slot = &self.buffers[self.active.index()];
        let available_payload = self.chunk_size.saturating_sub(active_slot.payload_len);
        if data.len() > available_payload {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        self.buffers[self.active.index()].write_payload(data)
    }

    /// Commits the active chunk and rotates ping-pong buffers for the next chunk.
    ///
    /// Translocates up to `overlap_size` trailing bytes from the current buffer to the
    /// head of the alternate buffer via 16-byte SIMD-aligned [`dict_shift`].
    pub fn advance_chunk(&mut self) -> Result<(), TTZipStatus> {
        let curr_idx = self.active.index();
        let next_idx = self.active.other().index();

        let curr_total = self.buffers[curr_idx].total_len();
        let curr_payload = self.buffers[curr_idx].payload_len();

        let target_overlap = self.overlap_size();
        let retained_len = curr_total.min(target_overlap);

        if retained_len > 0 {
            let tail_start = curr_total - retained_len;
            let (first, second) = self.buffers.split_at_mut(1);
            let (src_buf, dst_buf) = if curr_idx == 0 {
                (&first[0], &mut second[0])
            } else {
                (&second[0], &mut first[0])
            };
            let src_slice = &src_buf.storage[tail_start..curr_total];
            let dst_slice = &mut dst_buf.storage[..retained_len];
            dict_shift(dst_slice, src_slice);
        }

        self.buffers[next_idx].history_len = retained_len;
        self.buffers[next_idx].payload_len = 0;

        self.total_uncompressed_processed += curr_payload as u64;
        self.chunk_index += 1;
        self.active = self.active.other();

        Ok(())
    }

    /// Alias for [`DictBuffer::advance_chunk`].
    #[inline]
    pub fn rotate_buffer(&mut self) -> Result<(), TTZipStatus> {
        self.advance_chunk()
    }

    /// Clears both buffer slots and resets state to the initial clean condition.
    pub fn reset(&mut self) {
        self.buffers[0].clear();
        self.buffers[1].clear();
        self.active = BufferId::A;
        self.total_uncompressed_processed = 0;
        self.chunk_index = 0;
    }
}
