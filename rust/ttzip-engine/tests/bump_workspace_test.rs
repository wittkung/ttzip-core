// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Exhaustive integration and invariant tests for `BumpWorkspace` dual-ended arena.

use ttzip_engine::memory::{BumpWorkspace, WorkspaceError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C, align(64))]
struct Aligned64Struct {
    header: u64,
    payload: [u8; 32],
    extra: [u8; 24],
}

#[derive(Debug, Clone, PartialEq, Default)]
struct TestLookupTable {
    id: u32,
    weight: f64,
    entries: [u16; 8],
}

#[test]
fn test_bump_workspace_creation_and_capacity_alignment() {
    let ws = BumpWorkspace::new(1000).expect("create 1000 byte workspace");
    // Aligned to 64 bytes -> 1024
    assert_eq!(ws.total_capacity(), 1024);
    assert_eq!(ws.available_bytes(), 1024);
    assert_eq!(ws.bottom_allocated(), 0);
    assert_eq!(ws.top_allocated(), 0);
    assert!(ws.is_empty());
}

#[test]
fn test_bump_workspace_custom_alignment_and_power_of_two() {
    let ws = BumpWorkspace::with_alignment(2048, 128).expect("create 128-byte aligned workspace");
    assert_eq!(ws.total_capacity(), 2048);

    let err = BumpWorkspace::with_alignment(1024, 7).unwrap_err();
    assert!(matches!(err, WorkspaceError::InvalidAlignment { .. }));
}

#[test]
fn test_bump_workspace_bottom_alloc_64_byte_alignment() {
    let ws = BumpWorkspace::new(4096).expect("create workspace");

    let slice = ws
        .alloc_bottom_aligned::<Aligned64Struct>(4, 64)
        .expect("alloc aligned structs");
    assert_eq!(slice.len(), 4);
    assert_eq!(slice.as_ptr() as usize % 64, 0);

    for (i, item) in slice.iter_mut().enumerate() {
        item.header = (i as u64) + 1;
        item.payload[0] = 0xAA;
    }

    assert_eq!(slice[0].header, 1);
    assert_eq!(slice[3].header, 4);
    assert_eq!(ws.bottom_allocated(), 4 * 64);
}

#[test]
fn test_bump_workspace_dual_ended_collision_oom() {
    let ws = BumpWorkspace::new(1024).expect("create workspace");

    // Allocate 512 bytes bottom
    let _bottom = ws.alloc_bottom::<u8>(512).expect("alloc 512 bottom");
    assert_eq!(ws.bottom_allocated(), 512);
    assert_eq!(ws.available_bytes(), 512);

    // Allocate 256 bytes top
    let _top = ws.alloc_top(256).expect("alloc 256 top");
    assert_eq!(ws.top_allocated(), 256);
    assert_eq!(ws.available_bytes(), 256);

    // Requesting 300 bytes must trigger OutOfMemory
    let oom_err = ws.alloc_top(300).unwrap_err();
    match oom_err {
        WorkspaceError::OutOfMemory {
            requested,
            available,
            total_capacity,
        } => {
            assert_eq!(requested, 300);
            assert_eq!(available, 256);
            assert_eq!(total_capacity, 1024);
        }
        other => panic!("Expected OutOfMemory, got {:?}", other),
    }

    // Requesting exactly 256 bytes must succeed
    let exact_top = ws.alloc_top(256).expect("alloc exact 256 top");
    assert_eq!(exact_top.len(), 256);
    assert_eq!(ws.available_bytes(), 0);
}

#[test]
fn test_bump_workspace_reset_top_retains_static_tables() {
    let mut ws = BumpWorkspace::new(4096).expect("create workspace");

    // Bottom static tables
    let tables = ws
        .alloc_bottom::<TestLookupTable>(10)
        .expect("alloc bottom tables");
    tables[0].id = 42;
    tables[9].id = 99;
    let bottom_used = ws.bottom_allocated();

    for iteration in 0..100 {
        // Ephemeral scratchpad
        let scratch = ws.alloc_top(1024).expect("alloc scratch");
        scratch[0] = (iteration % 255) as u8;
        scratch[1023] = 0xEE;
        assert_eq!(ws.top_allocated(), 1024);

        // Zero-allocation reset of top scratchpad
        ws.reset_top();
        assert_eq!(ws.top_allocated(), 0);
        assert_eq!(ws.bottom_allocated(), bottom_used);
    }

    // Full reset
    ws.reset_all();
    assert_eq!(ws.bottom_allocated(), 0);
    assert_eq!(ws.top_allocated(), 0);
    assert!(ws.is_empty());
}

#[test]
fn test_bump_workspace_zero_sized_allocations() {
    let ws = BumpWorkspace::new(1024).expect("create workspace");
    let empty_bottom = ws.alloc_bottom::<u64>(0).expect("empty bottom");
    assert_eq!(empty_bottom.len(), 0);

    let empty_top = ws.alloc_top(0).expect("empty top");
    assert_eq!(empty_top.len(), 0);

    assert_eq!(ws.bottom_allocated(), 0);
    assert_eq!(ws.top_allocated(), 0);
}

#[test]
fn test_bump_workspace_high_water_mark() {
    let mut ws = BumpWorkspace::new(2048).expect("create workspace");
    assert_eq!(ws.high_water_mark(), 0);

    let _b = ws.alloc_bottom::<u8>(300).expect("alloc 300");
    assert_eq!(ws.high_water_mark(), 300);

    let _t = ws.alloc_top(500).expect("alloc 500");
    assert_eq!(ws.high_water_mark(), 800);

    ws.reset_top();
    assert_eq!(ws.high_water_mark(), 800);

    let _t2 = ws.alloc_top(1000).expect("alloc 1000");
    assert_eq!(ws.high_water_mark(), 1300);
}
