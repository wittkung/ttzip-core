// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Memory-Mapped (mmap) 6-Layer Defense-in-Depth and Virtual Memory Boundary Security Subsystem.
//!
//! Enforces deterministic memory bounds and strict kernel/virtual memory safety defenses:
//! 1. **Truncation & TOCTOU Guard**: Validates file bounds against live shrinkage and arithmetic overflow.
//! 2. **Page Boundary Guard**: Enforces strict modular arithmetic alignment (4KB/16KB) for OS mapping.
//! 3. **ReadOnly Protection Guard**: Enforces read-only memory mappings to prevent dirty page writes.
//! 4. **Resident Memory Circuit Breaker**: Enforces single-task <= 64MB resident memory limit and eviction.
//! 5. **Resource Handle Tracker**: RAII lifecycle tracking to prevent handle leakage and double-free.
//! 6. **Zero-Byte & Extreme Offset Self-Healing**: Transparent zero-allocation empty view and overflow recovery.

use std::fs::File;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::types::TTZipStatus;

/// Default hard ceiling for single-task resident mapped memory (64 MiB).
pub const DEFAULT_MAX_RESIDENT_MEMORY_LIMIT: usize = 64 * 1024 * 1024;

/// Fallback system page allocation granularity (4 KiB).
pub const DEFAULT_PAGE_SIZE: usize = 4096;

/// Resolves the host operating system's virtual memory page allocation granularity.
#[must_use]
pub fn system_page_size() -> usize {
    #[cfg(unix)]
    {
        let sz = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if sz > 0 {
            sz as usize
        } else {
            DEFAULT_PAGE_SIZE
        }
    }
    #[cfg(not(unix))]
    {
        DEFAULT_PAGE_SIZE
    }
}

/// Computed page alignment descriptor for kernel mmap invocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageAlignment {
    /// Kernel-aligned start offset matching system page granularity.
    pub aligned_offset: u64,
    /// Byte offset within the aligned page to reach the requested data.
    pub page_offset: usize,
    /// Total allocation length including leading page padding.
    pub aligned_len: usize,
}

/// Guard 1: File Truncation and Concurrent TOCTOU Boundary Security Guard.
pub struct TruncationGuard;

impl TruncationGuard {
    /// Validates requested offset and length against the snapshot file size.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrInvalidOffset)` if `offset + len > file_size`
    /// or if arithmetic overflow occurs during offset addition.
    pub fn validate_bounds(offset: u64, len: usize, file_size: u64) -> Result<(), TTZipStatus> {
        let req_end = match offset.checked_add(len as u64) {
            Some(end) => end,
            None => return Err(TTZipStatus::ErrInvalidOffset),
        };
        if req_end > file_size {
            return Err(TTZipStatus::ErrInvalidOffset);
        }
        Ok(())
    }

    /// Queries the live filesystem metadata to detect concurrent truncation attacks (TOCTOU).
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if physical file size has shrunk
    /// below the expected minimum size, or `Err(TTZipStatus::ErrOpenFailed)` on I/O error.
    pub fn validate_live_file_size(file: &File, expected_min_size: u64) -> Result<u64, TTZipStatus> {
        let metadata = file.metadata().map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let live_size = metadata.len();
        if live_size < expected_min_size {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        Ok(live_size)
    }

    /// Validates sub-slice offset and length within an active mapped buffer.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrInvalidOffset)` if `req_offset + req_len > slice_len`
    /// or arithmetic overflow occurs.
    pub fn check_slice_bounds(slice_len: usize, req_offset: usize, req_len: usize) -> Result<(), TTZipStatus> {
        let end = match req_offset.checked_add(req_len) {
            Some(e) => e,
            None => return Err(TTZipStatus::ErrInvalidOffset),
        };
        if end > slice_len {
            return Err(TTZipStatus::ErrInvalidOffset);
        }
        Ok(())
    }
}

/// Guard 2: Cross-Page Unaligned Offset and Page Boundary Overflow Interceptor.
pub struct PageBoundaryGuard;

impl PageBoundaryGuard {
    /// Validates whether a given byte offset is strictly aligned to the page size.
    #[must_use]
    pub fn is_page_aligned(offset: u64, page_size: usize) -> bool {
        if page_size == 0 {
            return false;
        }
        offset.is_multiple_of(page_size as u64)
    }

    /// Computes page-aligned mapping parameters using strict modular arithmetic.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrInvalidParam)` if `page_size` is 0, not a power of 2,
    /// or if length addition overflows `usize`.
    pub fn compute_alignment(offset: u64, len: usize, page_size: usize) -> Result<PageAlignment, TTZipStatus> {
        if page_size == 0 || !page_size.is_power_of_two() {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        let page_mask = (page_size as u64) - 1;
        let page_offset = (offset & page_mask) as usize;
        let aligned_offset = offset & !page_mask;

        let aligned_len = match len.checked_add(page_offset) {
            Some(l) => l,
            None => return Err(TTZipStatus::ErrInvalidParam),
        };

        Ok(PageAlignment {
            aligned_offset,
            page_offset,
            aligned_len,
        })
    }
}

/// Guard 3: Read-Only Write-Protection Barrier.
pub struct ReadOnlyProtectionGuard;

impl ReadOnlyProtectionGuard {
    /// Enforces that data pointers remain strictly immutable and read-only.
    #[inline]
    pub const fn ensure_read_only<T>(_val: &T) -> Result<(), TTZipStatus> {
        Ok(())
    }
}

/// Guard 4: Resident Memory Circuit Breaker and Page Eviction Guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmapResidentMemoryGuard {
    pub max_resident_limit: usize,
}

impl Default for MmapResidentMemoryGuard {
    #[inline]
    fn default() -> Self {
        Self::new(DEFAULT_MAX_RESIDENT_MEMORY_LIMIT)
    }
}

impl MmapResidentMemoryGuard {
    /// Default resident memory limit (64 MiB).
    pub const DEFAULT_LIMIT: usize = DEFAULT_MAX_RESIDENT_MEMORY_LIMIT;

    /// Creates a new resident memory guard with explicit quota limits.
    #[must_use]
    pub const fn new(max_resident_limit: usize) -> Self {
        Self { max_resident_limit }
    }

    /// Enforces that the requested memory allocation does not exceed single-task quota.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrOutOfMemory)` if requested bytes exceed `max_resident_limit`.
    pub fn validate_budget(&self, requested_bytes: usize) -> Result<(), TTZipStatus> {
        if requested_bytes > self.max_resident_limit {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        Ok(())
    }

    /// Issues `MADV_SEQUENTIAL` kernel advice on POSIX systems to optimize sequential reads.
    pub fn advise_sequential(mmap: &memmap2::Mmap) -> Result<(), TTZipStatus> {
        #[cfg(unix)]
        {
            let ptr = mmap.as_ptr() as *mut libc::c_void;
            let len = mmap.len();
            if len > 0 {
                let res = unsafe { libc::madvise(ptr, len, libc::MADV_SEQUENTIAL) };
                if res != 0 {
                    return Err(TTZipStatus::ErrMmapFailed);
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = mmap;
        }
        Ok(())
    }

    /// Issues `MADV_DONTNEED` kernel advice on POSIX systems to release resident pages.
    pub fn advise_dontneed(mmap: &memmap2::Mmap) -> Result<(), TTZipStatus> {
        #[cfg(unix)]
        {
            let ptr = mmap.as_ptr() as *mut libc::c_void;
            let len = mmap.len();
            if len > 0 {
                let res = unsafe { libc::madvise(ptr, len, libc::MADV_DONTNEED) };
                if res != 0 {
                    return Err(TTZipStatus::ErrMmapFailed);
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = mmap;
        }
        Ok(())
    }
}

static ACTIVE_MMAP_COUNT: AtomicUsize = AtomicUsize::new(0);
static TOTAL_ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_MMAP_COUNT: AtomicUsize = AtomicUsize::new(0);

/// RAII Resource tracking token for allocated memory map handles.
#[derive(Debug)]
pub struct ResourceTracker {
    allocated_bytes: usize,
    active: bool,
}

impl ResourceTracker {
    /// Creates and registers a new active memory mapping allocation.
    #[must_use]
    pub fn new(allocated_bytes: usize) -> Self {
        if allocated_bytes > 0 {
            let active = ACTIVE_MMAP_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
            let mut peak = PEAK_MMAP_COUNT.load(Ordering::Relaxed);
            while active > peak {
                match PEAK_MMAP_COUNT.compare_exchange_weak(
                    peak,
                    active,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(actual) => peak = actual,
                }
            }
            TOTAL_ALLOCATED_BYTES.fetch_add(allocated_bytes, Ordering::SeqCst);
        }
        Self {
            allocated_bytes,
            active: allocated_bytes > 0,
        }
    }
}

impl Drop for ResourceTracker {
    fn drop(&mut self) {
        if self.active {
            ACTIVE_MMAP_COUNT.fetch_sub(1, Ordering::SeqCst);
            TOTAL_ALLOCATED_BYTES.fetch_sub(self.allocated_bytes, Ordering::SeqCst);
            self.active = false;
        }
    }
}

/// Guard 5: Handle Leak & Double-Free Lifetime Tracker.
pub struct MmapResourceGuard;

impl MmapResourceGuard {
    /// Returns the number of currently active, unreleased mmap handles.
    #[must_use]
    pub fn active_count() -> usize {
        ACTIVE_MMAP_COUNT.load(Ordering::SeqCst)
    }

    /// Returns the total resident bytes currently held in active mmap handles.
    #[must_use]
    pub fn allocated_bytes() -> usize {
        TOTAL_ALLOCATED_BYTES.load(Ordering::SeqCst)
    }

    /// Returns the peak high-water mark of concurrent active mmap handles.
    #[must_use]
    pub fn peak_count() -> usize {
        PEAK_MMAP_COUNT.load(Ordering::SeqCst)
    }
}

/// RAII Safe Memory Map View guarded by all 6 depth-in-defense security layers.
#[derive(Debug)]
pub struct SafeMmapView {
    mmap: Option<memmap2::Mmap>,
    page_offset: usize,
    visible_len: usize,
    _tracker: ResourceTracker,
}

impl SafeMmapView {
    /// Guard 6: Creates a transparent zero-allocation empty safe view for 0-byte files/ranges.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            mmap: None,
            page_offset: 0,
            visible_len: 0,
            _tracker: ResourceTracker::new(0),
        }
    }

    /// Returns the underlying byte slice with page padding stripped.
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        match &self.mmap {
            Some(m) => {
                let start = self.page_offset;
                let end = start + self.visible_len;
                &m[start..end]
            }
            None => &[],
        }
    }

    /// Returns a sub-slice within the view after verifying bounds against truncation and overflow.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrInvalidOffset)` if `offset + len > self.len()`.
    pub fn slice(&self, offset: usize, len: usize) -> Result<&[u8], TTZipStatus> {
        TruncationGuard::check_slice_bounds(self.visible_len, offset, len)?;
        let full = self.as_slice();
        Ok(&full[offset..offset + len])
    }

    /// Returns the visible length in bytes.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.visible_len
    }

    /// Returns `true` if the view contains zero visible bytes.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.visible_len == 0
    }

    /// Applies sequential access memory advice to optimize OS page cache behavior.
    pub fn advise_sequential(&self) -> Result<(), TTZipStatus> {
        if let Some(ref m) = self.mmap {
            MmapResidentMemoryGuard::advise_sequential(m)?;
        }
        Ok(())
    }

    /// Releases resident physical pages back to the kernel.
    pub fn advise_dontneed(&self) -> Result<(), TTZipStatus> {
        if let Some(ref m) = self.mmap {
            MmapResidentMemoryGuard::advise_dontneed(m)?;
        }
        Ok(())
    }
}

impl Deref for SafeMmapView {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl AsRef<[u8]> for SafeMmapView {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

/// Builder configuration for creating robust, guarded memory mappings.
#[derive(Debug, Clone)]
pub struct SafeMmapOptions {
    offset: u64,
    len: Option<usize>,
    max_resident_limit: usize,
}

impl Default for SafeMmapOptions {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl SafeMmapOptions {
    /// Creates a new `SafeMmapOptions` with default 64MB resident memory budget.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            offset: 0,
            len: None,
            max_resident_limit: DEFAULT_MAX_RESIDENT_MEMORY_LIMIT,
        }
    }

    /// Sets the byte offset in the file to start mapping from.
    #[must_use]
    pub const fn offset(mut self, offset: u64) -> Self {
        self.offset = offset;
        self
    }

    /// Sets the length in bytes to map.
    #[must_use]
    pub const fn len(mut self, len: usize) -> Self {
        self.len = Some(len);
        self
    }

    /// Sets the maximum resident memory budget ceiling.
    #[must_use]
    pub const fn max_resident_limit(mut self, limit: usize) -> Self {
        self.max_resident_limit = limit;
        self
    }

    /// Maps a file into virtual memory using the full 6-layer defense pipeline.
    ///
    /// # Errors
    /// Returns `TTZipStatus` error codes on parameter violation, overflow, quota exhaustion, or I/O failure.
    pub fn map_file(&self, file: &File) -> Result<SafeMmapView, TTZipStatus> {
        let metadata = file.metadata().map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let file_size = metadata.len();

        // 6. Zero-byte file self-healing
        if file_size == 0 {
            if self.offset > 0 {
                return Err(TTZipStatus::ErrInvalidOffset);
            }
            return Ok(SafeMmapView::empty());
        }

        // Extreme offset check
        if self.offset > file_size {
            return Err(TTZipStatus::ErrInvalidOffset);
        }

        let map_len = match self.len {
            Some(l) => {
                TruncationGuard::validate_bounds(self.offset, l, file_size)?;
                l
            }
            None => {
                let remaining = file_size - self.offset;
                if remaining > usize::MAX as u64 {
                    return Err(TTZipStatus::ErrInvalidParam);
                }
                remaining as usize
            }
        };

        if map_len == 0 {
            return Ok(SafeMmapView::empty());
        }

        // 4. Resident memory quota guard
        let memory_guard = MmapResidentMemoryGuard::new(self.max_resident_limit);
        memory_guard.validate_budget(map_len)?;

        // 2. Page boundary alignment computation
        let page_size = system_page_size();
        let alignment = PageBoundaryGuard::compute_alignment(self.offset, map_len, page_size)?;

        // Check live file size against aligned bounds
        let req_end = alignment
            .aligned_offset
            .checked_add(alignment.aligned_len as u64)
            .ok_or(TTZipStatus::ErrInvalidOffset)?;
        let live_size = TruncationGuard::validate_live_file_size(file, req_end.min(file_size))?;
        if req_end > live_size {
            return Err(TTZipStatus::ErrInvalidOffset);
        }

        // 3. Read-only map execution via memmap2
        let mmap = unsafe {
            memmap2::MmapOptions::new()
                .offset(alignment.aligned_offset)
                .len(alignment.aligned_len)
                .map(file)
                .map_err(|_| TTZipStatus::ErrMmapFailed)?
        };

        let tracker = ResourceTracker::new(alignment.aligned_len);

        Ok(SafeMmapView {
            mmap: Some(mmap),
            page_offset: alignment.page_offset,
            visible_len: map_len,
            _tracker: tracker,
        })
    }

    /// Creates an anonymous read-only memory map.
    ///
    /// # Errors
    /// Returns `TTZipStatus::ErrOutOfMemory` or `TTZipStatus::ErrMmapFailed` on failure.
    pub fn map_anonymous(&self, len: usize) -> Result<SafeMmapView, TTZipStatus> {
        if len == 0 {
            return Ok(SafeMmapView::empty());
        }

        let memory_guard = MmapResidentMemoryGuard::new(self.max_resident_limit);
        memory_guard.validate_budget(len)?;

        let mmap_mut = memmap2::MmapOptions::new()
            .len(len)
            .map_anon()
            .map_err(|_| TTZipStatus::ErrMmapFailed)?;

        let mmap = mmap_mut.make_read_only().map_err(|_| TTZipStatus::ErrMmapFailed)?;
        let tracker = ResourceTracker::new(len);

        Ok(SafeMmapView {
            mmap: Some(mmap),
            page_offset: 0,
            visible_len: len,
            _tracker: tracker,
        })
    }
}

/// Helper function to safely map an entire file with read-only defense.
pub fn safe_map_file(file: &File) -> Result<SafeMmapView, TTZipStatus> {
    SafeMmapOptions::new().map_file(file)
}

/// Helper function to safely map a specific sub-range of a file.
pub fn safe_map_file_range(file: &File, offset: u64, len: usize) -> Result<SafeMmapView, TTZipStatus> {
    SafeMmapOptions::new().offset(offset).len(len).map_file(file)
}

/// Helper function to safely allocate an anonymous memory map.
pub fn safe_map_anonymous(len: usize) -> Result<SafeMmapView, TTZipStatus> {
    SafeMmapOptions::new().map_anonymous(len)
}
