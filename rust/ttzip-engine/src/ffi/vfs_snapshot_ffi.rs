// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! C-ABI / FFI Mach-VM VFS Snapshot lifecycle functions (`open_snapshot`, `open_compact_catalog`, `release_snapshot`).

use libc::c_char;
use std::ffi::CStr;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::types::unified_node::UnifiedVfsTable;
use crate::types::{TTWorkspaceEntryNode, TTWorkspaceSnapshotHeader};
use ttzip_types::builder::VfsSnapshotBuilder;
use ttzip_types::mach_vm::MachVmAllocation;
use ttzip_types::snapshot::{
    TT_FEATURE_FLAG_COMPACT_NODES, TT_SECTION_STRING_ARENA, TT_SECTION_VFS_NODES,
};
use ttzip_types::vfs_handle::ttvfs_lease_s;

use super::vfs_ffi::{TTWorkspaceCompactNode, TTWorkspaceVfsView};

unsafe fn inspect_archive_table(
    archive_path: *const c_char,
    password: *const c_char,
) -> Option<UnifiedVfsTable> {
    let path_str = unsafe { CStr::from_ptr(archive_path) }.to_str().ok()?;
    let path = std::path::Path::new(path_str);
    let pwd_str = if password.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(password) }.to_str().ok()
    };
    let mut table = UnifiedVfsTable::new();
    crate::archive::unified::inspect::inspect_archive_into_vfs(path, pwd_str, true, &mut table)
        .ok()?;
    Some(table)
}

/// Builds a 64-byte CSR Mach VM snapshot (`TT_FEATURE_FLAG_CSR | TT_FEATURE_FLAG_LEXSORT | TT_FEATURE_FLAG_NAME_MASK`),
/// populates `out_view` with borrowed pointers, and returns an owning `ttvfs_lease_s` (`ref_count == 1`), or null on failure.
/// # Safety
/// `archive_path` and `password` must be null or valid null-terminated UTF-8 C strings; `out_view` must be null or
/// writable and 8-byte aligned; the caller must release a non-null lease via `ttzip_vfs_release_snapshot`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ttzip_vfs_open_snapshot(
    archive_path: *const c_char,
    password: *const c_char,
    out_view: *mut TTWorkspaceVfsView,
) -> *mut ttvfs_lease_s {
    unsafe {
        let result = catch_unwind(AssertUnwindSafe(|| {
            if archive_path.is_null() || out_view.is_null() {
                return std::ptr::null_mut();
            }
            let Some(table) = inspect_archive_table(archive_path, password) else {
                return std::ptr::null_mut();
            };

            let alloc = VfsSnapshotBuilder::build_snapshot(&table);
            let ptr = alloc.as_ptr();
            let Ok((header, mapping_slice)) =
                TTWorkspaceSnapshotHeader::validate_mapping(ptr, alloc.len())
            else {
                return std::ptr::null_mut();
            };
            let Some((n_sec, nodes_payload)) = header.validate_nodes_section(mapping_slice) else {
                return std::ptr::null_mut();
            };
            let Some((a_sec, arena_payload)) =
                header.validate_section_bounds(mapping_slice, TT_SECTION_STRING_ARENA, 1, 1)
            else {
                return std::ptr::null_mut();
            };

            (*out_view).header = header as *const _;
            (*out_view).nodes = nodes_payload.as_ptr() as *const _;
            (*out_view).node_count = n_sec.entry_count;
            (*out_view).string_arena = arena_payload.as_ptr() as *const _;
            (*out_view).string_arena_len = a_sec.size_bytes;
            (*out_view).generation = header.generation;

            let alloc_raw = Arc::into_raw(alloc);
            Box::into_raw(Box::new(ttvfs_lease_s {
                mapping: ptr as *mut u8,
                size: mapping_slice.len(),
                ref_count: AtomicU64::new(1),
                alloc_ptr: alloc_raw as *const libc::c_void,
            }))
        }));

        result.unwrap_or(std::ptr::null_mut())
    }
}

fn convert_csr_to_compact_nodes(csr_nodes: &[TTWorkspaceEntryNode]) -> Vec<TTWorkspaceCompactNode> {
    let mut compact_nodes = Vec::with_capacity(csr_nodes.len());
    let mut next_dir_idx = 0u32;

    for node in csr_nodes {
        let is_dir = node.is_dir();
        let dir_idx = if is_dir && node.child_count > 0 && node.first_child_id != u32::MAX {
            let idx = next_dir_idx;
            next_dir_idx += 1;
            idx
        } else {
            0
        };

        let mut flags = 0u16;
        if is_dir {
            flags |= 0x01;
        }
        if (node.flags & 0x02) != 0 {
            flags |= 0x02;
        }
        if (node.flags & 0x04) != 0 {
            flags |= 0x04;
        }

        compact_nodes.push(TTWorkspaceCompactNode {
            name_offset: node.name_offset,
            name_len: node.name_len,
            flags,
            size: node.uncompressed_size,
            mtime: (node.mtime_epoch_s.max(0)) as u32,
            dir_children_idx: dir_idx,
        });
    }
    compact_nodes
}

unsafe fn populate_compact_catalog_mapping(
    ptr: *mut u8,
    total_size: usize,
    nodes_offset: usize,
    compact_nodes: &[TTWorkspaceCompactNode],
    arena_offset: usize,
    arena_bytes: &[u8],
) {
    let nodes_bytes = std::mem::size_of_val(compact_nodes);
    let arena_len = arena_bytes.len();
    let header = TTWorkspaceSnapshotHeader {
        magic: ttzip_types::snapshot::TT_SNAPSHOT_MAGIC,
        abi_major: ttzip_types::snapshot::TT_ABI_VERSION_MAJOR,
        abi_minor: ttzip_types::snapshot::TT_ABI_VERSION_MINOR,
        header_size: 128,
        section_header_size: 32,
        section_count: 2,
        root_node_id: 0,
        total_mapping_size: total_size as u64,
        generation: 1,
        snapshot_sequence: 1,
        section_headers_offset: 128,
        feature_flags: TT_FEATURE_FLAG_COMPACT_NODES,
        reserved_flags: 0,
        snapshot_checksum: 0,
        _reserved: [0u8; 56],
    };

    unsafe {
        std::ptr::write(ptr as *mut TTWorkspaceSnapshotHeader, header);

        let sec_nodes = ttzip_types::snapshot::TTWorkspaceSectionEntry {
            section_type: TT_SECTION_VFS_NODES,
            flags: ttzip_types::snapshot::TT_SECTION_FLAG_ALIGNED_64
                | ttzip_types::snapshot::TT_SECTION_FLAG_READ_ONLY,
            offset: nodes_offset as u64,
            size_bytes: nodes_bytes as u64,
            entry_count: compact_nodes.len() as u64,
        };
        let sec_arena = ttzip_types::snapshot::TTWorkspaceSectionEntry {
            section_type: TT_SECTION_STRING_ARENA,
            flags: ttzip_types::snapshot::TT_SECTION_FLAG_READ_ONLY,
            offset: arena_offset as u64,
            size_bytes: arena_len as u64,
            entry_count: arena_len as u64,
        };

        let sec_ptr = ptr.add(128) as *mut ttzip_types::snapshot::TTWorkspaceSectionEntry;
        std::ptr::write(sec_ptr, sec_nodes);
        std::ptr::write(sec_ptr.add(1), sec_arena);

        if !compact_nodes.is_empty() {
            std::ptr::copy_nonoverlapping(
                compact_nodes.as_ptr() as *const u8,
                ptr.add(nodes_offset),
                nodes_bytes,
            );
        }
        if !arena_bytes.is_empty() {
            std::ptr::copy_nonoverlapping(arena_bytes.as_ptr(), ptr.add(arena_offset), arena_len);
        }
    }
}

/// Builds an in-process 24-byte compact node snapshot (`TT_FEATURE_FLAG_COMPACT_NODES`); only section 1
/// (`TTWorkspaceCompactNode[]`) and section 2 (`TT_SECTION_STRING_ARENA`) are serialized into the Mach VM mapping.
/// # Safety
/// Same pointer, lifetime, and single-owner release preconditions as [`ttzip_vfs_open_snapshot`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ttzip_vfs_open_compact_catalog(
    archive_path: *const c_char,
    password: *const c_char,
    out_view: *mut TTWorkspaceVfsView,
) -> *mut ttvfs_lease_s {
    unsafe {
        let result = catch_unwind(AssertUnwindSafe(|| {
            if archive_path.is_null() || out_view.is_null() {
                return std::ptr::null_mut();
            }
            let Some(table) = inspect_archive_table(archive_path, password) else {
                return std::ptr::null_mut();
            };

            let (csr_nodes, arena) = VfsSnapshotBuilder::build(&table);
            let arena_bytes = arena.into_bytes();
            let compact_nodes = convert_csr_to_compact_nodes(&csr_nodes);

            let header_size = 128usize;
            let sections_size =
                2 * std::mem::size_of::<ttzip_types::snapshot::TTWorkspaceSectionEntry>();
            let nodes_offset = (header_size + sections_size + 63) & !63;
            let nodes_bytes = std::mem::size_of_val(compact_nodes.as_slice());
            let arena_offset = (nodes_offset + nodes_bytes + 63) & !63;
            let arena_len = arena_bytes.len();
            let total_size = (arena_offset + arena_len + 16383) & !16383;

            let Ok(alloc) = MachVmAllocation::allocate(total_size) else {
                return std::ptr::null_mut();
            };
            let ptr = alloc.as_ptr() as *mut u8;
            populate_compact_catalog_mapping(
                ptr,
                total_size,
                nodes_offset,
                &compact_nodes,
                arena_offset,
                &arena_bytes,
            );

            let alloc_raw = Arc::into_raw(Arc::new(alloc));
            (*out_view).header = ptr as *const TTWorkspaceSnapshotHeader;
            (*out_view).nodes = ptr.add(nodes_offset) as *const TTWorkspaceEntryNode;
            (*out_view).node_count = compact_nodes.len() as u64;
            (*out_view).string_arena = ptr.add(arena_offset) as *const libc::c_char;
            (*out_view).string_arena_len = arena_len as u64;
            (*out_view).generation = 1;

            Box::into_raw(Box::new(ttvfs_lease_s {
                mapping: ptr,
                size: total_size,
                ref_count: AtomicU64::new(1),
                alloc_ptr: alloc_raw as *const libc::c_void,
            }))
        }));

        result.unwrap_or(std::ptr::null_mut())
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn mach_task_self() -> u32;
    fn mach_vm_deallocate(target: u32, address: u64, size: u64) -> i32;
}

/// Releases one owning reference to a snapshot lease, deallocating the Mach VM mapping and boxed lease at `ref_count == 0`.
/// # Safety
/// `lease` must be null or a `Box`-allocated `ttvfs_lease_s` with at least one live reference; `alloc_ptr` must come from
/// `Arc::into_raw::<MachVmAllocation>` if non-null, or `mapping`/`size` must describe a Mach VM allocation owned by `lease`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ttzip_vfs_release_snapshot(lease: *mut ttvfs_lease_s) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if lease.is_null() {
            return;
        }
        let l = unsafe { &*lease };
        if l.ref_count.fetch_sub(1, Ordering::Release) == 1 {
            std::sync::atomic::fence(Ordering::Acquire);
            if !l.alloc_ptr.is_null() {
                drop(unsafe { Arc::from_raw(l.alloc_ptr as *const MachVmAllocation) });
            } else if !l.mapping.is_null() && l.size > 0 {
                #[cfg(target_os = "macos")]
                {
                    let _ = unsafe {
                        mach_vm_deallocate(mach_task_self(), l.mapping as u64, l.size as u64)
                    };
                }
            }
            drop(unsafe { Box::from_raw(lease) });
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttzip_vfs_release_snapshot_null_safe() {
        unsafe {
            ttzip_vfs_release_snapshot(std::ptr::null_mut());
        }
    }

    #[test]
    fn test_ttzip_vfs_release_snapshot_lifecycle() {
        let lease = Box::into_raw(Box::new(ttvfs_lease_s {
            mapping: std::ptr::null_mut(),
            size: 0,
            ref_count: AtomicU64::new(2),
            alloc_ptr: std::ptr::null(),
        }));
        unsafe {
            ttzip_vfs_release_snapshot(lease);
            assert_eq!((*lease).ref_count.load(Ordering::Acquire), 1);
            ttzip_vfs_release_snapshot(lease);
        }
    }
}
