// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use ttzip_engine::fs::vfs::node::VfsEntry;
use ttzip_engine::fs::vfs::search::{search_vfs_tree_zero_alloc, TTZipVfsMatchDto};
use ttzip_engine::fs::vfs::VfsTree;

pub struct TrackingAllocator {
    underlying: System,
    total_alloc_count: AtomicUsize,
    total_alloc_bytes: AtomicUsize,
}

impl Default for TrackingAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl TrackingAllocator {
    pub const fn new() -> Self {
        Self {
            underlying: System,
            total_alloc_count: AtomicUsize::new(0),
            total_alloc_bytes: AtomicUsize::new(0),
        }
    }
}

thread_local! {
    static IS_TRACKING_THREAD: Cell<bool> = const { Cell::new(false) };
    static THREAD_ALLOC_COUNT: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if IS_TRACKING_THREAD.with(|t| t.get()) {
            THREAD_ALLOC_COUNT.with(|c| c.set(c.get() + 1));
            self.total_alloc_count.fetch_add(1, Ordering::Relaxed);
            self.total_alloc_bytes.fetch_add(layout.size(), Ordering::Relaxed);
        }
        self.underlying.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.underlying.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if IS_TRACKING_THREAD.with(|t| t.get()) {
            THREAD_ALLOC_COUNT.with(|c| c.set(c.get() + 1));
            self.total_alloc_count.fetch_add(1, Ordering::Relaxed);
            self.total_alloc_bytes.fetch_add(new_size, Ordering::Relaxed);
        }
        self.underlying.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator::new();

pub fn assert_zero_alloc<R, F: FnOnce() -> R>(f: F) -> R {
    IS_TRACKING_THREAD.with(|t| t.set(true));
    THREAD_ALLOC_COUNT.with(|c| c.set(0));

    let result = f();

    let count = THREAD_ALLOC_COUNT.with(|c| c.get());
    IS_TRACKING_THREAD.with(|t| t.set(false));

    assert_eq!(
        count, 0,
        "Zero-Heap Allocation Invariant Violated! Captured {} allocations in search critical section.",
        count
    );
    result
}

#[test]
fn test_vfs_tree_100k_nodes_zero_heap_allocation_search() {
    const NODE_COUNT: usize = 100_000;
    let mut entries = Vec::with_capacity(NODE_COUNT);
    for i in 0..NODE_COUNT {
        entries.push(VfsEntry {
            path: format!("root/dir_{}/sub_{}/file_{}.dat", i / 1000, i % 1000, i),
            uncompressed_size: 1024,
            compressed_size: 512,
            crc32: 0x12345678,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        });
    }
    let tree = VfsTree::build_from_entries(&entries, "ProjectRoot");
    assert_eq!(tree.total_entries, NODE_COUNT);

    let mut result_slots = [TTZipVfsMatchDto {
        struct_size: std::mem::size_of::<TTZipVfsMatchDto>() as u32,
        abi_version: ttzip_engine::types::TTZIP_ABI_VERSION_2,
        name: std::ptr::null(),
        name_len: 0,
        path: std::ptr::null(),
        path_len: 0,
        uncompressed_size: 0,
        compressed_size: 0,
        crc32: 0,
        score: 0,
        is_directory: false,
        is_encrypted: false,
    }; 64];

    let match_count = assert_zero_alloc(|| {
        search_vfs_tree_zero_alloc(&tree.root, "file_99999", &mut result_slots)
    });

    assert!(match_count >= 1);
    let top_match = &result_slots[0];
    let name_slice = unsafe { std::slice::from_raw_parts(top_match.name as *const u8, top_match.name_len) };
    let name_str = std::str::from_utf8(name_slice).unwrap();
    assert_eq!(name_str, "file_99999.dat");
    println!("✓ 100,000 VFS Nodes Searched with EXACTLY 0 Heap Allocations. Matched: {}", match_count);
}
