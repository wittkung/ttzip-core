// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Immutable snapshot handle and reference-counted RAII lifetime anchor for TTZip VFS.
//!
//! # Architecture & Safety Invariants
//! 1. Immutability: Once wrapped in a [`VfsTableHandle`], the underlying [`UnifiedVfsTable`]
//!    is strictly immutable.
//! 2. Zero-Copy FFI Lifetime: Swift holds an opaque pointer to this handle. Calling
//!    [`ttzip_vfs_table_retain`] increments the internal atomic reference count, preventing
//!    Use-After-Free (UAF) and dangling pointer bugs.
//! 3. Copy-on-Write (COW): Any modification to the VFS tree (e.g. background archive scanning)
//!    allocates a brand-new [`VfsTableHandle`] with an incremented generation ID. Active views
//!    continue reading the old frozen snapshot without lock contention, tear, or invalidation.

use crate::mach_vm::MachVmAllocation;
use crate::{UnifiedEntryNode, UnifiedVfsTable};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Monotonically increasing snapshot generation counter for Copy-on-Write (COW) detection.
static GLOBAL_SNAPSHOT_GEN: AtomicU64 = AtomicU64::new(1);

/// Contiguous metadata view slice exposing unified entry nodes and path string arena matching CTTZipBridge.h.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TTZipMetadataPayloadView {
    pub entries: *const UnifiedEntryNode,
    pub entry_count: u32,
    pub string_arena: *const std::os::raw::c_char,
    pub string_arena_len: u32,
    pub root_entry_id: u32,
}

unsafe impl Send for TTZipMetadataPayloadView {}
unsafe impl Sync for TTZipMetadataPayloadView {}

/// Opaque reference-counted handle wrapping an immutable `Arc<UnifiedVfsTable>`.
#[repr(C)]
pub struct VfsTableHandle {
    table: Arc<UnifiedVfsTable>,
    generation: u64,
}

impl VfsTableHandle {
    /// Creates a new immutable VFS snapshot handle with an incremented generation ID.
    #[inline]
    pub fn new(table: UnifiedVfsTable) -> Self {
        let generation = GLOBAL_SNAPSHOT_GEN.fetch_add(1, Ordering::Relaxed);
        Self {
            table: Arc::new(table),
            generation,
        }
    }

    /// Creates a handle wrapping an existing `Arc<UnifiedVfsTable>`.
    #[inline]
    pub fn from_arc(table: Arc<UnifiedVfsTable>) -> Self {
        let generation = GLOBAL_SNAPSHOT_GEN.fetch_add(1, Ordering::Relaxed);
        Self { table, generation }
    }

    /// Returns the snapshot generation ID.
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns an immutable reference to the wrapped VFS table.
    #[inline]
    pub fn table(&self) -> &UnifiedVfsTable {
        &self.table
    }

    /// Clones the internal `Arc<UnifiedVfsTable>`.
    #[inline]
    pub fn clone_arc(&self) -> Arc<UnifiedVfsTable> {
        Arc::clone(&self.table)
    }

    /// Generates a contiguous C-ABI payload view into the immutable table.
    #[inline]
    pub fn as_payload_view(&self) -> TTZipMetadataPayloadView {
        TTZipMetadataPayloadView {
            entries: if self.table.nodes.is_empty() {
                std::ptr::null()
            } else {
                self.table.nodes.as_ptr()
            },
            entry_count: self.table.nodes.len() as u32,
            string_arena: if self.table.strings.is_empty() {
                std::ptr::null()
            } else {
                self.table.strings.as_bytes().as_ptr() as *const std::os::raw::c_char
            },
            string_arena_len: self.table.strings.len() as u32,
            root_entry_id: self.table.root_id,
        }
    }
}

// ==============================================================================
// C-ABI Safe Lifetime & Retain/Release Exports for Swift RAII Anchors
// ==============================================================================

/// Allocates an immutable VFS table handle from a populated table (Rust-internal constructor).
#[inline]
pub fn ttzip_vfs_table_create(table: UnifiedVfsTable) -> *mut VfsTableHandle {
    let handle = Box::new(VfsTableHandle::new(table));
    Box::into_raw(handle)
}

/// Retains an immutable VFS table handle, cloning the underlying `Arc<UnifiedVfsTable>`.
///
/// # Safety
/// `handle` must be a valid pointer to a `VfsTableHandle`.
#[no_mangle]
pub unsafe extern "C" fn ttzip_vfs_table_retain(handle: *const VfsTableHandle) -> *mut VfsTableHandle {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    let original = &*handle;
    let cloned = Box::new(VfsTableHandle {
        table: Arc::clone(&original.table),
        generation: original.generation,
    });
    Box::into_raw(cloned)
}

/// Releases an immutable VFS table handle, decrementing the atomic reference count.
///
/// # Safety
/// `handle` must be a pointer returned by `ttzip_vfs_table_create` or `ttzip_vfs_table_retain`.
#[no_mangle]
pub unsafe extern "C" fn ttzip_vfs_table_release(handle: *mut VfsTableHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

/// Extracts a contiguous metadata payload view and snapshot generation from an immutable table handle.
///
/// # Safety
/// `handle` and `out_view` must be non-null and valid.
#[no_mangle]
pub unsafe extern "C" fn ttzip_vfs_table_get_view(
    handle: *const VfsTableHandle,
    out_view: *mut TTZipMetadataPayloadView,
    out_generation: *mut u64,
) -> i32 {
    if handle.is_null() || out_view.is_null() {
        return -1;
    }
    let handle_ref = &*handle;
    *out_view = handle_ref.as_payload_view();
    if !out_generation.is_null() {
        *out_generation = handle_ref.generation;
    }
    0
}

// ==============================================================================
// Canonical C-ABI Snapshot Lease Handle (32 bytes, 8-byte aligned)
// ==============================================================================

/// Canonical 32-byte C-ABI snapshot lease handle matching `struct ttvfs_lease_s`.
#[repr(C, align(8))]
pub struct ttvfs_lease_s {
    pub mapping: *mut u8,
    pub size: usize,
    pub ref_count: AtomicU64,
    pub alloc_ptr: *const core::ffi::c_void,
}

unsafe impl Send for ttvfs_lease_s {}
unsafe impl Sync for ttvfs_lease_s {}

const _: () = {
    assert!(core::mem::size_of::<ttvfs_lease_s>() == 32);
    assert!(core::mem::align_of::<ttvfs_lease_s>() == 8);
};

// ==============================================================================
// TTVfsLease (MachVmAllocation wrapper)
// ==============================================================================

/// Opaque reference-counted lease wrapping an immutable `Arc<MachVmAllocation>`.
#[repr(C)]
pub struct TTVfsLease {
    allocation: Arc<MachVmAllocation>,
    generation: u64,
}

impl TTVfsLease {
    /// Creates a new immutable VFS lease handle with an incremented generation ID.
    #[inline]
    pub fn new(allocation: Arc<MachVmAllocation>) -> Self {
        let generation = GLOBAL_SNAPSHOT_GEN.fetch_add(1, Ordering::Relaxed);
        Self {
            allocation,
            generation,
        }
    }

    /// Returns the snapshot generation ID.
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns an immutable reference to the wrapped Mach VM allocation.
    #[inline]
    pub fn allocation(&self) -> &Arc<MachVmAllocation> {
        &self.allocation
    }
}

/// Allocates an immutable VFS lease from a Mach VM allocation (Rust-internal constructor).
#[inline]
pub fn ttzip_vfs_lease_create(allocation: Arc<MachVmAllocation>) -> *mut TTVfsLease {
    let lease = Box::new(TTVfsLease::new(allocation));
    Box::into_raw(lease)
}

/// Retains an immutable VFS lease, cloning the underlying `Arc<MachVmAllocation>`.
///
/// # Safety
/// `lease` must be a valid pointer to a `TTVfsLease`.
#[no_mangle]
pub unsafe extern "C" fn ttzip_vfs_lease_retain(lease: *const TTVfsLease) -> *mut TTVfsLease {
    if lease.is_null() {
        return std::ptr::null_mut();
    }
    let original = &*lease;
    let cloned = Box::new(TTVfsLease {
        allocation: Arc::clone(&original.allocation),
        generation: original.generation,
    });
    Box::into_raw(cloned)
}

/// Releases an immutable VFS lease, decrementing the atomic reference count.
///
/// # Safety
/// `lease` must be a pointer returned by `ttzip_vfs_lease_create` or `ttzip_vfs_lease_retain`.
#[no_mangle]
pub unsafe extern "C" fn ttzip_vfs_lease_release(lease: *mut TTVfsLease) {
    if !lease.is_null() {
        drop(Box::from_raw(lease));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vfs_table_handle_lifecycle_and_cow() {
        let mut table = UnifiedVfsTable::new();
        table.add_entry(UnifiedEntryNode::empty(), "file1.txt");

        let handle1 = VfsTableHandle::new(table.clone());
        let gen1 = handle1.generation();
        assert!(gen1 > 0);

        // Retain
        let raw_ptr = Box::into_raw(Box::new(handle1));
        let retained_ptr = unsafe { ttzip_vfs_table_retain(raw_ptr) };
        assert!(!retained_ptr.is_null());

        let mut view = TTZipMetadataPayloadView {
            entries: std::ptr::null(),
            entry_count: 0,
            string_arena: std::ptr::null(),
            string_arena_len: 0,
            root_entry_id: 0,
        };
        let mut gen = 0u64;
        let res = unsafe { ttzip_vfs_table_get_view(retained_ptr, &mut view, &mut gen) };
        assert_eq!(res, 0);
        assert_eq!(view.entry_count, 1);
        assert_eq!(gen, gen1);

        // Mutate and create second generation
        table.add_entry(UnifiedEntryNode::empty(), "file2.txt");
        let handle2 = VfsTableHandle::new(table);
        assert!(handle2.generation() > gen1);

        // Release pointers safely
        unsafe {
            ttzip_vfs_table_release(raw_ptr);
            ttzip_vfs_table_release(retained_ptr);
        }
    }

    #[test]
    fn test_ttvfs_lease_s_geometry() {
        assert_eq!(core::mem::size_of::<ttvfs_lease_s>(), 32);
        assert_eq!(core::mem::align_of::<ttvfs_lease_s>(), 8);
    }
}
