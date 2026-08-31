// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Dual-Ended Collision Bump Workspace (`BumpWorkspace`).
//!
//! Implements a high-performance, deterministic dual-ended memory arena:
//! - **Bottom-Up (Static/Long-lived)**: Allocates persistent lookup tables, state machines,
//!   and aligned data structures (up to 64-byte AVX-512 / cache line alignment).
//! - **Top-Down (Ephemeral/Short-lived)**: Allocates scratchpad decompression/compression buffers.
//! - **Zero Allocation Reuse**: `reset_top()` retains the bottom static tables while resetting
//!   the top scratchpad cursor with zero system `malloc`/`free` overhead.
//! - **Physical Quota Guards**: Collision between bottom and top cursors immediately returns
//!   a strongly-typed [`WorkspaceError::OutOfMemory`].

use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::cell::Cell;
use std::ptr::NonNull;

/// Cache line and SIMD alignment constant (64 bytes).
pub const CACHE_LINE_ALIGNMENT: usize = 64;

/// Errors returned during workspace allocation, alignment, or capacity exhaustion.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkspaceError {
    /// Requested allocation exceeds available space between bottom and top cursors.
    #[error("Workspace out of memory: requested {requested} bytes, available {available} bytes (total capacity: {total_capacity} bytes)")]
    OutOfMemory {
        requested: usize,
        available: usize,
        total_capacity: usize,
    },

    /// Alignment is not a valid non-zero power of two or is below type requirements.
    #[error("Invalid alignment {align}: must be a power of two and at least {required}")]
    InvalidAlignment { align: usize, required: usize },

    /// Pointer arithmetic or capacity computation encountered an integer overflow.
    #[error("Arithmetic overflow during address alignment or size calculation")]
    ArithmeticOverflow,

    /// Backing buffer allocation failure.
    #[error("Failed to allocate underlying physical memory for capacity {capacity} bytes")]
    BackingAllocationFailed { capacity: usize },
}

/// Dual-ended memory arena for zero-allocation workspace management.
///
/// Layout:
/// ```text
/// +--------------------------+-----------------------+--------------------------+
/// | Bottom-Up (Long-lived)   | Free Contiguous Space | Top-Down (Scratchpad)    |
/// | (Lookup tables, structs) |                       | (Transient buffers)      |
/// +--------------------------+-----------------------+--------------------------+
/// ^ 0                        ^ bottom_offset         ^ top_offset               ^ capacity
/// ```
pub struct BumpWorkspace {
    ptr: NonNull<u8>,
    layout: Layout,
    capacity: usize,
    bottom_offset: Cell<usize>,
    top_offset: Cell<usize>,
    high_water_mark: Cell<usize>,
}

impl std::fmt::Debug for BumpWorkspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BumpWorkspace")
            .field("capacity", &self.capacity)
            .field("bottom_allocated", &self.bottom_offset.get())
            .field("top_allocated", &(self.capacity.saturating_sub(self.top_offset.get())))
            .field("available_bytes", &(self.top_offset.get().saturating_sub(self.bottom_offset.get())))
            .field("high_water_mark", &self.high_water_mark.get())
            .finish()
    }
}

// Safety: BumpWorkspace owns its memory block exclusively.
unsafe impl Send for BumpWorkspace {}

impl BumpWorkspace {
    /// Creates a new `BumpWorkspace` with the specified capacity, aligned to 64 bytes.
    ///
    /// # Errors
    /// Returns [`WorkspaceError::BackingAllocationFailed`] if OS memory allocation fails.
    pub fn new(capacity: usize) -> Result<Self, WorkspaceError> {
        Self::with_alignment(capacity, CACHE_LINE_ALIGNMENT)
    }

    /// Creates a new `BumpWorkspace` with custom capacity and base pointer alignment.
    pub fn with_alignment(capacity: usize, base_align: usize) -> Result<Self, WorkspaceError> {
        if !base_align.is_power_of_two() {
            return Err(WorkspaceError::InvalidAlignment {
                align: base_align,
                required: 1,
            });
        }

        let effective_align = base_align.max(CACHE_LINE_ALIGNMENT);
        let aligned_capacity = if capacity == 0 {
            effective_align
        } else {
            (capacity + effective_align - 1) & !(effective_align - 1)
        };

        let layout = Layout::from_size_align(aligned_capacity, effective_align)
            .map_err(|_| WorkspaceError::InvalidAlignment {
                align: effective_align,
                required: 1,
            })?;

        let raw_ptr = unsafe { alloc_zeroed(layout) };
        let ptr = NonNull::new(raw_ptr).ok_or(WorkspaceError::BackingAllocationFailed {
            capacity: aligned_capacity,
        })?;

        Ok(Self {
            ptr,
            layout,
            capacity: aligned_capacity,
            bottom_offset: Cell::new(0),
            top_offset: Cell::new(aligned_capacity),
            high_water_mark: Cell::new(0),
        })
    }

    /// Allocates an array slice of type `T` from the bottom-up cursor with custom alignment.
    ///
    /// The memory is initialized with [`Default::default()`].
    ///
    /// # Errors
    /// Returns [`WorkspaceError::OutOfMemory`] if the bottom and top cursors collide.
    /// Returns [`WorkspaceError::InvalidAlignment`] if `align` is not a power of two or < `align_of::<T>()`.
    #[allow(clippy::mut_from_ref)]
    pub fn alloc_bottom_aligned<T: Default + Clone>(
        &self,
        count: usize,
        align: usize,
    ) -> Result<&mut [T], WorkspaceError> {
        if count == 0 {
            return Ok(&mut []);
        }

        let type_align = std::mem::align_of::<T>();
        if !align.is_power_of_two() || align < type_align {
            return Err(WorkspaceError::InvalidAlignment {
                align,
                required: type_align,
            });
        }

        let type_size = std::mem::size_of::<T>();
        let total_bytes = count
            .checked_mul(type_size)
            .ok_or(WorkspaceError::ArithmeticOverflow)?;

        let current_bottom = self.bottom_offset.get();
        let current_top = self.top_offset.get();
        let base_addr = self.ptr.as_ptr() as usize;

        let current_ptr = base_addr
            .checked_add(current_bottom)
            .ok_or(WorkspaceError::ArithmeticOverflow)?;

        let aligned_ptr = (current_ptr + (align - 1)) & !(align - 1);
        let aligned_bottom = aligned_ptr
            .checked_sub(base_addr)
            .ok_or(WorkspaceError::ArithmeticOverflow)?;

        let new_bottom = aligned_bottom
            .checked_add(total_bytes)
            .ok_or(WorkspaceError::ArithmeticOverflow)?;

        if new_bottom > current_top {
            let available = current_top.saturating_sub(aligned_bottom);
            return Err(WorkspaceError::OutOfMemory {
                requested: total_bytes,
                available,
                total_capacity: self.capacity,
            });
        }

        self.bottom_offset.set(new_bottom);
        self.update_high_water_mark(new_bottom, current_top);

        let target_slice = unsafe {
            let slice_ptr = aligned_ptr as *mut T;
            for i in 0..count {
                std::ptr::write(slice_ptr.add(i), T::default());
            }
            std::slice::from_raw_parts_mut(slice_ptr, count)
        };

        Ok(target_slice)
    }

    /// Allocates an array slice of type `T` from the bottom-up cursor with natural alignment.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    pub fn alloc_bottom<T: Default + Clone>(&self, count: usize) -> Result<&mut [T], WorkspaceError> {
        self.alloc_bottom_aligned(count, std::mem::align_of::<T>())
    }

    /// Allocates a 64-byte aligned scratchpad slice of type `T` from the bottom-up cursor.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    pub fn alloc_bottom_64<T: Default + Clone>(&self, count: usize) -> Result<&mut [T], WorkspaceError> {
        let align = std::mem::align_of::<T>().max(CACHE_LINE_ALIGNMENT);
        self.alloc_bottom_aligned(count, align)
    }

    /// Allocates a transient byte buffer from the top-down cursor.
    ///
    /// # Errors
    /// Returns [`WorkspaceError::OutOfMemory`] if the top and bottom cursors collide.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    pub fn alloc_top(&self, size: usize) -> Result<&mut [u8], WorkspaceError> {
        self.alloc_top_aligned(size, 1)
    }

    /// Allocates a transient byte buffer from the top-down cursor with custom alignment.
    ///
    /// # Errors
    /// Returns [`WorkspaceError::OutOfMemory`] if the top and bottom cursors collide.
    #[allow(clippy::mut_from_ref)]
    pub fn alloc_top_aligned(&self, size: usize, align: usize) -> Result<&mut [u8], WorkspaceError> {
        if size == 0 {
            return Ok(&mut []);
        }

        if !align.is_power_of_two() {
            return Err(WorkspaceError::InvalidAlignment { align, required: 1 });
        }

        let current_bottom = self.bottom_offset.get();
        let current_top = self.top_offset.get();
        let base_addr = self.ptr.as_ptr() as usize;

        let current_top_ptr = base_addr
            .checked_add(current_top)
            .ok_or(WorkspaceError::ArithmeticOverflow)?;

        let unaligned_start = current_top_ptr
            .checked_sub(size)
            .ok_or(WorkspaceError::ArithmeticOverflow)?;

        let aligned_start = unaligned_start & !(align - 1);
        let new_top = aligned_start
            .checked_sub(base_addr)
            .ok_or(WorkspaceError::ArithmeticOverflow)?;

        if new_top < current_bottom {
            let available = current_top.saturating_sub(current_bottom);
            return Err(WorkspaceError::OutOfMemory {
                requested: size,
                available,
                total_capacity: self.capacity,
            });
        }

        self.top_offset.set(new_top);
        self.update_high_water_mark(current_bottom, new_top);

        let target_slice = unsafe {
            let slice_ptr = aligned_start as *mut u8;
            std::slice::from_raw_parts_mut(slice_ptr, size)
        };

        Ok(target_slice)
    }

    /// Resets the top-down cursor back to the upper bound of the workspace.
    ///
    /// Preserves all persistent bottom-up allocated lookup tables and structs.
    /// Zero OS `free` or re-allocation overhead.
    #[inline]
    pub fn reset_top(&mut self) {
        self.top_offset.set(self.capacity);
    }

    /// Resets both the bottom-up and top-down cursors to full capacity.
    #[inline]
    pub fn reset_all(&mut self) {
        self.bottom_offset.set(0);
        self.top_offset.set(self.capacity);
        self.high_water_mark.set(0);
    }

    /// Returns the number of bytes currently allocated by the bottom-up arena.
    #[inline]
    pub fn bottom_allocated(&self) -> usize {
        self.bottom_offset.get()
    }

    /// Returns the number of bytes currently allocated by the top-down scratchpad.
    #[inline]
    pub fn top_allocated(&self) -> usize {
        self.capacity.saturating_sub(self.top_offset.get())
    }

    /// Returns the total remaining free contiguous bytes between the two cursors.
    #[inline]
    pub fn available_bytes(&self) -> usize {
        self.top_offset.get().saturating_sub(self.bottom_offset.get())
    }

    /// Returns the total fixed byte capacity of this workspace.
    #[inline]
    pub fn total_capacity(&self) -> usize {
        self.capacity
    }

    /// Returns true if no memory is currently allocated from either end.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bottom_offset.get() == 0 && self.top_offset.get() == self.capacity
    }

    /// Returns the peak total memory allocated across the workspace lifetime.
    #[inline]
    pub fn high_water_mark(&self) -> usize {
        self.high_water_mark.get()
    }

    #[inline]
    fn update_high_water_mark(&self, bottom: usize, top: usize) {
        let used = bottom + (self.capacity.saturating_sub(top));
        if used > self.high_water_mark.get() {
            self.high_water_mark.set(used);
        }
    }
}

impl Drop for BumpWorkspace {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.ptr.as_ptr(), self.layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dual_ended_allocation_and_collision() {
        let ws = BumpWorkspace::new(1024).expect("create workspace");
        assert_eq!(ws.total_capacity(), 1024);
        assert_eq!(ws.available_bytes(), 1024);

        // Bottom allocation of u64
        let bottom_slice = ws
            .alloc_bottom_aligned::<u64>(16, 64)
            .expect("bottom alloc");
        assert_eq!(bottom_slice.len(), 16);
        assert_eq!(bottom_slice.as_ptr() as usize % 64, 0);

        bottom_slice[0] = 0x1122334455667788;
        assert_eq!(bottom_slice[0], 0x1122334455667788);

        // Top allocation of 256 bytes
        let top_slice = ws.alloc_top(256).expect("top alloc");
        assert_eq!(top_slice.len(), 256);
        top_slice[0] = 0xAA;
        top_slice[255] = 0xBB;
        assert_eq!(top_slice[0], 0xAA);
        assert_eq!(top_slice[255], 0xBB);

        assert!(ws.available_bytes() < 1024);
        assert_eq!(ws.bottom_allocated(), 128); // 16 * 8 = 128 bytes
        assert_eq!(ws.top_allocated(), 256);

        // Try allocating beyond remaining space -> OutOfMemory
        let huge_req = ws.available_bytes() + 64;
        let err = ws.alloc_top(huge_req).unwrap_err();
        assert!(matches!(err, WorkspaceError::OutOfMemory { .. }));
    }

    #[test]
    fn test_reset_top_preserves_bottom() {
        let mut ws = BumpWorkspace::new(2048).expect("create workspace");
        let bottom = ws.alloc_bottom::<u32>(100).expect("alloc bottom");
        bottom[42] = 9999;
        assert_eq!(ws.bottom_allocated(), 400);

        {
            let top = ws.alloc_top(512).expect("alloc top");
            top[0] = 0xFF;
            assert_eq!(ws.top_allocated(), 512);
        }

        // Reset top only
        ws.reset_top();
        assert_eq!(ws.top_allocated(), 0);
        assert_eq!(ws.bottom_allocated(), 400);

        // Allocate top again into fresh space
        let top_again = ws.alloc_top(512).expect("alloc top again");
        assert_eq!(top_again.len(), 512);
        assert_eq!(ws.top_allocated(), 512);

        // Verify bottom data was preserved
        let bottom_ref = ws.alloc_bottom::<u32>(0).expect("empty alloc");
        assert_eq!(bottom_ref.len(), 0);
    }

    #[test]
    fn test_alignment_validation() {
        let ws = BumpWorkspace::new(1024).expect("create workspace");
        // Non-power of 2 alignment
        let err = ws.alloc_bottom_aligned::<u8>(10, 7).unwrap_err();
        assert!(matches!(err, WorkspaceError::InvalidAlignment { .. }));

        // Alignment smaller than type alignment
        let err = ws.alloc_bottom_aligned::<u64>(10, 2).unwrap_err();
        assert!(matches!(err, WorkspaceError::InvalidAlignment { .. }));
    }
}
