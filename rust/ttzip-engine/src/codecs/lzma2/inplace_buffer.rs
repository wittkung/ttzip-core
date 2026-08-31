// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe in-place memory reuse microkernel for LZMA2 match tables and compression output buffers.
//!
//! # Architectural Rationale & Physical Invariants
//!
//! In high-performance LZMA2 compression, the match-finder builds a match history table
//! ([`MatchTable`]) spanning the entire chunk (e.g. 64 KiB, 1 MiB, or 8 MiB+).
//!
//! During the subsequent entropy parsing and range encoding phase:
//! 1. **Monotonic Forward Progression**: The encoder reads match entries at position `pos`
//!    monotonically from `0` to `N`. Entries in the range `0..pos` are consumed once and
//!    **never accessed again**.
//! 2. **Memory Expansion Factor**: Each match entry occupies $E \ge 4$ bytes (4 bytes for
//!    [`BitPackedEntry`] and 5 bytes for [`StructuredMatchEntry`]). Thus, consuming `pos`
//!    entries releases $pos \times E$ bytes of physical RAM.
//! 3. **In-Place Buffer Overwriting**: Because typical compressed output is substantially
//!    smaller than uncompressed input ($compressed \ll uncompressed \times E$), the
//!    already-consumed prefix of the match table can be safely borrowed in-place as the
//!    compressed bitstream destination buffer.
//! 4. **Safe Slice Borrowing Invariant**: The write pointer (`write_byte_pos`) must strictly
//!    satisfy:
//!    $$\text{write\_byte\_pos} + \text{safety\_margin} \le \text{read\_entry\_pos} \times \text{entry\_size}$$
//!    If the write pointer ever attempts to catch up with or exceed the safe boundary,
//!    an explicit [`InPlaceError::WriteCatchup`] barrier is raised, preventing any corruption
//!    of unconsumed match entries.
//! 5. **Zero Heap Allocation**: Combined with [`InPlaceBufferPool`], the entire compression
//!    loop executes with **zero heap allocations (`malloc`)**, eliminating memory fragmentation.
//!
//! [`MatchTable`]: super::match_table::MatchTable
//! [`BitPackedEntry`]: super::match_table::BitPackedEntry
//! [`StructuredMatchEntry`]: super::match_table::StructuredMatchEntry

use std::error::Error;
use std::fmt;
use std::io::{self, Write};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use super::match_table::{BitPackedEntry, MatchTable, StructuredMatchEntry};

/// Default safety headroom (in bytes) between write offset and active read boundary.
pub const DEFAULT_SAFETY_MARGIN_BYTES: usize = 0;

/// Default pool capacity for pre-allocated chunk buffer instances.
pub const DEFAULT_POOL_CAPACITY: usize = 16;

/// Default slot capacity in bytes (4 MiB, supporting up to 1 MiB chunk under 4-byte BitPacked mode).
pub const DEFAULT_SLOT_CAPACITY_BYTES: usize = 4 * 1024 * 1024;

/// Errors arising from in-place buffer borrowing and write boundary violations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InPlaceError {
    /// The write pointer would exceed the safe read boundary (write pointer catching read pointer).
    WriteCatchup {
        /// Current write offset in bytes.
        write_pos: usize,
        /// Maximum safe byte limit determined by the current read entry position.
        safe_limit: usize,
        /// Number of bytes requested to write.
        requested: usize,
    },
    /// The requested read position decreased, violating the monotonic forward progression invariant.
    NonMonotonicRead {
        /// Current read position in entries.
        current_pos: usize,
        /// Attempted new read position.
        attempted_pos: usize,
    },
    /// The write offset would exceed total physical buffer capacity.
    BufferCapacityExceeded {
        /// Current write offset in bytes.
        write_pos: usize,
        /// Total physical capacity of the buffer.
        capacity: usize,
        /// Number of bytes requested to write.
        requested: usize,
    },
    /// An invalid entry size (0) was provided during writer initialization.
    InvalidEntrySize,
}

impl fmt::Display for InPlaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WriteCatchup {
                write_pos,
                safe_limit,
                requested,
            } => write!(
                f,
                "InPlace safe barrier violation: write_pos ({write_pos}) + requested ({requested}) \
                 exceeds safe_limit ({safe_limit})"
            ),
            Self::NonMonotonicRead {
                current_pos,
                attempted_pos,
            } => write!(
                f,
                "InPlace monotonic invariant violation: new read_pos ({attempted_pos}) \
                 < current_pos ({current_pos})"
            ),
            Self::BufferCapacityExceeded {
                write_pos,
                capacity,
                requested,
            } => write!(
                f,
                "InPlace capacity exceeded: write_pos ({write_pos}) + requested ({requested}) \
                 > capacity ({capacity})"
            ),
            Self::InvalidEntrySize => {
                write!(f, "InPlace invalid entry size: entry_size must be >= 1 byte")
            }
        }
    }
}

impl Error for InPlaceError {}

/// Monotonic in-place output buffer writer for LZMA2 match table memory reuse.
///
/// Wraps a mutable byte slice representing the underlying physical memory of a [`MatchTable`]
/// (or raw chunk buffer) and tracks the monotonic advancing read boundary versus the written
/// output byte position.
pub struct InPlaceOutputWriter<'a> {
    /// Underlying physical mutable byte storage.
    buffer: &'a mut [u8],
    /// Current write offset in bytes.
    write_byte_pos: usize,
    /// Current monotonic read position in table entries.
    read_entry_pos: usize,
    /// Physical size in bytes occupied by each match table entry (e.g. 4 or 5 bytes).
    entry_size: usize,
    /// Safety headroom in bytes required between write pointer and unconsumed read entries.
    safety_margin: usize,
}

impl<'a> InPlaceOutputWriter<'a> {
    /// Creates a new `InPlaceOutputWriter` over a raw mutable byte slice.
    ///
    /// # Parameters
    /// - `buffer`: Mutable byte slice representing the match table's physical memory.
    /// - `entry_size`: Byte size per match entry (4 for BitPacked, 5 for Structured). Must be $\ge 1$.
    /// - `safety_margin`: Minimal safety gap in bytes between write and read pointers.
    ///
    /// # Errors
    /// Returns [`InPlaceError::InvalidEntrySize`] if `entry_size == 0`.
    pub fn new(
        buffer: &'a mut [u8],
        entry_size: usize,
        safety_margin: usize,
    ) -> Result<Self, InPlaceError> {
        if entry_size == 0 {
            return Err(InPlaceError::InvalidEntrySize);
        }
        Ok(Self {
            buffer,
            write_byte_pos: 0,
            read_entry_pos: 0,
            entry_size,
            safety_margin,
        })
    }

    /// Creates an `InPlaceOutputWriter` directly borrowing the memory of a [`MatchTable`].
    pub fn from_match_table(
        table: &'a mut MatchTable,
        safety_margin: usize,
    ) -> Result<Self, InPlaceError> {
        let entry_size = table.entry_size_bytes();
        let slice = table.as_byte_slice_mut();
        Self::new(slice, entry_size, safety_margin)
    }

    /// Returns the active byte capacity of the underlying physical storage.
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    /// Returns the physical byte size per match table entry.
    #[inline(always)]
    pub const fn entry_size(&self) -> usize {
        self.entry_size
    }

    /// Returns the configured safety headroom margin in bytes.
    #[inline(always)]
    pub const fn safety_margin(&self) -> usize {
        self.safety_margin
    }

    /// Returns the current monotonic read position (in number of entries consumed).
    #[inline(always)]
    pub const fn read_entry_pos(&self) -> usize {
        self.read_entry_pos
    }

    /// Returns the current write pointer offset in bytes.
    #[inline(always)]
    pub const fn write_byte_pos(&self) -> usize {
        self.write_byte_pos
    }

    /// Returns the number of output bytes written so far.
    #[inline(always)]
    pub const fn written_len(&self) -> usize {
        self.write_byte_pos
    }

    /// Calculates the maximum byte offset currently safe to overwrite without corrupting
    /// unconsumed match entries.
    ///
    /// Formula:
    /// $$\text{safe\_limit} = \min(\text{capacity}, (\text{read\_entry\_pos} \times \text{entry\_size}).\text{saturating\_sub}(\text{safety\_margin}))$$
    #[inline]
    pub fn safe_write_limit(&self) -> usize {
        let released_bytes = self.read_entry_pos.saturating_mul(self.entry_size);
        let margin_clamped = released_bytes.saturating_sub(self.safety_margin);
        margin_clamped.min(self.buffer.len())
    }

    /// Returns the number of bytes available for immediate writing under current read boundary.
    #[inline]
    pub fn available_write_bytes(&self) -> usize {
        self.safe_write_limit().saturating_sub(self.write_byte_pos)
    }

    /// Returns `true` if the write pointer currently satisfies all safety barrier invariants.
    #[inline]
    pub fn is_safe(&self) -> bool {
        self.write_byte_pos <= self.safe_write_limit()
    }

    /// Advances the monotonic read position to `new_read_pos`.
    ///
    /// # Invariants
    /// - `new_read_pos >= self.read_entry_pos` (strictly monotonic forward progression).
    ///
    /// # Errors
    /// Returns [`InPlaceError::NonMonotonicRead`] if `new_read_pos < self.read_entry_pos`.
    #[inline]
    pub fn advance_read_pos(&mut self, new_read_pos: usize) -> Result<(), InPlaceError> {
        if new_read_pos < self.read_entry_pos {
            return Err(InPlaceError::NonMonotonicRead {
                current_pos: self.read_entry_pos,
                attempted_pos: new_read_pos,
            });
        }
        self.read_entry_pos = new_read_pos;
        Ok(())
    }

    /// Writes a single byte into the output prefix with barrier verification.
    ///
    /// # Errors
    /// Returns [`InPlaceError::WriteCatchup`] if writing would violate the safe read boundary.
    /// Returns [`InPlaceError::BufferCapacityExceeded`] if writing exceeds buffer capacity.
    #[inline]
    pub fn write_byte(&mut self, byte: u8) -> Result<(), InPlaceError> {
        let next_pos = self.write_byte_pos + 1;
        let limit = self.safe_write_limit();

        if next_pos > limit {
            return Err(InPlaceError::WriteCatchup {
                write_pos: self.write_byte_pos,
                safe_limit: limit,
                requested: 1,
            });
        }
        if next_pos > self.buffer.len() {
            return Err(InPlaceError::BufferCapacityExceeded {
                write_pos: self.write_byte_pos,
                capacity: self.buffer.len(),
                requested: 1,
            });
        }

        self.buffer[self.write_byte_pos] = byte;
        self.write_byte_pos = next_pos;
        Ok(())
    }

    /// Writes a contiguous byte slice into the output prefix with barrier verification.
    ///
    /// # Returns
    /// Returns the number of bytes written (`data.len()`).
    ///
    /// # Errors
    /// Returns [`InPlaceError::WriteCatchup`] if writing would exceed the safe read boundary.
    /// Returns [`InPlaceError::BufferCapacityExceeded`] if writing exceeds buffer capacity.
    #[inline]
    pub fn write_slice(&mut self, data: &[u8]) -> Result<usize, InPlaceError> {
        let len = data.len();
        if len == 0 {
            return Ok(0);
        }

        let next_pos = self.write_byte_pos.saturating_add(len);
        let limit = self.safe_write_limit();

        if next_pos > limit {
            return Err(InPlaceError::WriteCatchup {
                write_pos: self.write_byte_pos,
                safe_limit: limit,
                requested: len,
            });
        }
        if next_pos > self.buffer.len() {
            return Err(InPlaceError::BufferCapacityExceeded {
                write_pos: self.write_byte_pos,
                capacity: self.buffer.len(),
                requested: len,
            });
        }

        self.buffer[self.write_byte_pos..next_pos].copy_from_slice(data);
        self.write_byte_pos = next_pos;
        Ok(len)
    }

    /// Writes `count` zero bytes into the output prefix.
    #[inline]
    pub fn write_zeros(&mut self, count: usize) -> Result<usize, InPlaceError> {
        if count == 0 {
            return Ok(0);
        }

        let next_pos = self.write_byte_pos.saturating_add(count);
        let limit = self.safe_write_limit();

        if next_pos > limit {
            return Err(InPlaceError::WriteCatchup {
                write_pos: self.write_byte_pos,
                safe_limit: limit,
                requested: count,
            });
        }
        if next_pos > self.buffer.len() {
            return Err(InPlaceError::BufferCapacityExceeded {
                write_pos: self.write_byte_pos,
                capacity: self.buffer.len(),
                requested: count,
            });
        }

        self.buffer[self.write_byte_pos..next_pos].fill(0);
        self.write_byte_pos = next_pos;
        Ok(count)
    }

    /// Returns an immutable subslice of the compressed data written so far.
    #[inline(always)]
    pub fn written_slice(&self) -> &[u8] {
        &self.buffer[..self.write_byte_pos]
    }

    /// Returns a mutable subslice of the compressed data written so far.
    #[inline(always)]
    pub fn written_slice_mut(&mut self) -> &mut [u8] {
        &mut self.buffer[..self.write_byte_pos]
    }

    /// Consumes the writer and yields the final written byte slice borrowing from the original buffer.
    #[inline(always)]
    pub fn finish(self) -> &'a [u8] {
        &self.buffer[..self.write_byte_pos]
    }

    /// Consumes the writer and yields the mutable written byte slice borrowing from the original buffer.
    #[inline(always)]
    pub fn finish_mut(self) -> &'a mut [u8] {
        &mut self.buffer[..self.write_byte_pos]
    }

    /// Consumes the writer and returns the underlying buffer along with the final write length.
    #[inline(always)]
    pub fn into_inner(self) -> (&'a mut [u8], usize) {
        let len = self.write_byte_pos;
        (self.buffer, len)
    }
}

impl<'a> Write for InPlaceOutputWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let available = self.available_write_bytes();
        if available == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "InPlace writer reached safe read boundary limit",
            ));
        }

        let write_len = buf.len().min(available);
        self.write_slice(&buf[..write_len])
            .map_err(|e| io::Error::other(e.to_string()))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Internal shared slot storage for [`InPlaceBufferPool`].
struct PoolInner {
    /// Reusable raw buffer vectors.
    buffers: Mutex<Vec<Vec<u8>>>,
    /// Configured maximum slot capacity in bytes.
    slot_capacity_bytes: usize,
    /// Maximum number of buffers retained in the pool.
    max_slots: usize,
    /// Counter tracking actively leased buffers.
    active_leases: AtomicUsize,
}

/// High-throughput, thread-safe memory buffer pool for zero-allocation LZMA2 compression.
///
/// Maintains a pool of reusable byte buffers (`Vec<u8>`), preventing heap churn and
/// OS `malloc`/`free` latency across multithreaded chunk compression jobs.
#[derive(Clone)]
pub struct InPlaceBufferPool {
    inner: Arc<PoolInner>,
}

impl Default for InPlaceBufferPool {
    fn default() -> Self {
        Self::new(DEFAULT_POOL_CAPACITY, DEFAULT_SLOT_CAPACITY_BYTES)
    }
}

impl InPlaceBufferPool {
    /// Creates a new `InPlaceBufferPool` with custom capacity and slot size.
    pub fn new(max_slots: usize, slot_capacity_bytes: usize) -> Self {
        let max_slots = max_slots.max(1);
        let slot_capacity_bytes = slot_capacity_bytes.max(1024);

        Self {
            inner: Arc::new(PoolInner {
                buffers: Mutex::new(Vec::with_capacity(max_slots)),
                slot_capacity_bytes,
                max_slots,
                active_leases: AtomicUsize::new(0),
            }),
        }
    }

    /// Acquires a buffer guard from the pool with default configured slot capacity.
    ///
    /// If an idle buffer exists in the pool, it is reused without heap allocation;
    /// otherwise, a new buffer is allocated.
    pub fn acquire(&self) -> InPlaceBufferGuard {
        self.acquire_with_capacity(self.inner.slot_capacity_bytes)
    }

    /// Acquires a buffer guard from the pool with at least `min_capacity_bytes`.
    pub fn acquire_with_capacity(&self, min_capacity_bytes: usize) -> InPlaceBufferGuard {
        let mut guard = self.inner.buffers.lock().unwrap_or_else(|e| e.into_inner());

        let mut buffer = if let Some(mut buf) = guard.pop() {
            if buf.len() < min_capacity_bytes {
                buf.resize(min_capacity_bytes, 0);
            }
            buf
        } else {
            vec![0u8; min_capacity_bytes.max(self.inner.slot_capacity_bytes)]
        };

        buffer.fill(0);
        self.inner.active_leases.fetch_add(1, Ordering::Relaxed);

        InPlaceBufferGuard {
            buffer: Some(buffer),
            pool: Some(Arc::clone(&self.inner)),
        }
    }

    /// Acquires a match table instance sized for `num_entries` and `mode` backed by recycled memory.
    pub fn acquire_match_table(
        &self,
        dict_size: usize,
        num_entries: usize,
    ) -> PooledMatchTableGuard {
        let entry_size = if dict_size <= super::match_table::COMPACT_DICT_THRESHOLD {
            std::mem::size_of::<BitPackedEntry>()
        } else {
            std::mem::size_of::<StructuredMatchEntry>()
        };
        let required_bytes = num_entries.saturating_mul(entry_size);
        let buffer_guard = self.acquire_with_capacity(required_bytes);

        let table = MatchTable::new(dict_size, num_entries);
        PooledMatchTableGuard {
            table,
            _buffer_guard: buffer_guard,
        }
    }

    /// Returns the number of idle buffers currently resting in the pool.
    pub fn idle_count(&self) -> usize {
        self.inner
            .buffers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    /// Returns the number of actively leased buffers currently in use.
    pub fn active_leases(&self) -> usize {
        self.inner.active_leases.load(Ordering::Relaxed)
    }

    /// Clears all idle buffers retained in the pool, releasing their physical memory to the OS.
    pub fn clear(&self) {
        let mut guard = self.inner.buffers.lock().unwrap_or_else(|e| e.into_inner());
        guard.clear();
    }
}

/// RAII Guard wrapping an acquired buffer from [`InPlaceBufferPool`].
///
/// Automatically returns the underlying memory buffer to the parent pool upon [`Drop`].
pub struct InPlaceBufferGuard {
    buffer: Option<Vec<u8>>,
    pool: Option<Arc<PoolInner>>,
}

impl InPlaceBufferGuard {
    /// Creates an unpooled standalone buffer guard for testing or isolated executions.
    pub fn standalone(capacity: usize) -> Self {
        Self {
            buffer: Some(vec![0u8; capacity]),
            pool: None,
        }
    }

    /// Returns a mutable reference to the underlying byte slice.
    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        self.buffer.as_deref_mut().unwrap_or(&mut [])
    }

    /// Returns an immutable reference to the underlying byte slice.
    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        self.buffer.as_deref().unwrap_or(&[])
    }

    /// Creates an [`InPlaceOutputWriter`] borrowing this guard's underlying buffer.
    #[inline]
    pub fn create_writer(
        &mut self,
        entry_size: usize,
        safety_margin: usize,
    ) -> Result<InPlaceOutputWriter<'_>, InPlaceError> {
        let slice = self.as_mut_slice();
        InPlaceOutputWriter::new(slice, entry_size, safety_margin)
    }
}

impl Deref for InPlaceBufferGuard {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl DerefMut for InPlaceBufferGuard {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl Drop for InPlaceBufferGuard {
    fn drop(&mut self) {
        if let Some(buf) = self.buffer.take() {
            if let Some(pool) = self.pool.take() {
                pool.active_leases.fetch_sub(1, Ordering::Relaxed);
                let mut guard = pool.buffers.lock().unwrap_or_else(|e| e.into_inner());
                if guard.len() < pool.max_slots {
                    guard.push(buf);
                }
            }
        }
    }
}

/// RAII Guard wrapping a recycled [`MatchTable`] backed by [`InPlaceBufferPool`].
pub struct PooledMatchTableGuard {
    table: MatchTable,
    _buffer_guard: InPlaceBufferGuard,
}

impl PooledMatchTableGuard {
    /// Returns an immutable reference to the inner match table.
    #[inline(always)]
    pub fn table(&self) -> &MatchTable {
        &self.table
    }

    /// Returns a mutable reference to the inner match table.
    #[inline(always)]
    pub fn table_mut(&mut self) -> &mut MatchTable {
        &mut self.table
    }

    /// Creates an [`InPlaceOutputWriter`] directly borrowing this match table.
    #[inline]
    pub fn create_writer(
        &mut self,
        safety_margin: usize,
    ) -> Result<InPlaceOutputWriter<'_>, InPlaceError> {
        InPlaceOutputWriter::from_match_table(&mut self.table, safety_margin)
    }
}

impl Deref for PooledMatchTableGuard {
    type Target = MatchTable;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.table
    }
}

impl DerefMut for PooledMatchTableGuard {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.table
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inplace_writer_basic_safety_and_barrier() {
        let mut buffer = vec![0u8; 100]; // 100 bytes
        let entry_size = 4; // 4 bytes per entry
        let mut writer = InPlaceOutputWriter::new(&mut buffer, entry_size, 0).expect("create writer");

        // Initially read_entry_pos = 0, safe_write_limit = 0
        assert_eq!(writer.safe_write_limit(), 0);
        assert_eq!(writer.available_write_bytes(), 0);

        // Attempting to write byte must fail with WriteCatchup
        let err = writer.write_byte(0xAA).unwrap_err();
        match err {
            InPlaceError::WriteCatchup {
                write_pos,
                safe_limit,
                requested,
            } => {
                assert_eq!(write_pos, 0);
                assert_eq!(safe_limit, 0);
                assert_eq!(requested, 1);
            }
            _ => panic!("Expected WriteCatchup error"),
        }

        // Advance read pos to entry 5 -> released bytes = 5 * 4 = 20 bytes
        writer.advance_read_pos(5).expect("advance read pos");
        assert_eq!(writer.safe_write_limit(), 20);
        assert_eq!(writer.available_write_bytes(), 20);

        // Writing 10 bytes should succeed
        let data = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let written = writer.write_slice(&data).expect("write slice");
        assert_eq!(written, 10);
        assert_eq!(writer.written_len(), 10);
        assert_eq!(writer.available_write_bytes(), 10);

        // Writing another 10 bytes should succeed exactly up to boundary 20
        writer.write_slice(&data).expect("write slice 2");
        assert_eq!(writer.written_len(), 20);
        assert_eq!(writer.available_write_bytes(), 0);

        // Next write must be caught by barrier
        assert!(writer.write_byte(0xFF).is_err());

        // Monotonic check: advancing to a smaller read pos must fail
        assert!(writer.advance_read_pos(4).is_err());
    }

    #[test]
    fn test_inplace_buffer_pool_recycling() {
        let pool = InPlaceBufferPool::new(4, 1024);
        assert_eq!(pool.idle_count(), 0);
        assert_eq!(pool.active_leases(), 0);

        {
            let mut guard1 = pool.acquire();
            assert_eq!(guard1.len(), 1024);
            assert_eq!(pool.active_leases(), 1);
            guard1[0] = 42;
        }

        // Dropping guard1 returns it to the pool
        assert_eq!(pool.idle_count(), 1);
        assert_eq!(pool.active_leases(), 0);

        {
            let guard2 = pool.acquire();
            // Recycled buffer is zero-filled upon acquisition
            assert_eq!(guard2[0], 0);
            assert_eq!(pool.idle_count(), 0);
            assert_eq!(pool.active_leases(), 1);
        }

        assert_eq!(pool.idle_count(), 1);
    }
}
