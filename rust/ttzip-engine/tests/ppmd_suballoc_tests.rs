// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use ttzip_engine::codecs::ppmd::{
    PpmdRestoreMethod, PpmdUnitContext as PpmdContext, PpmdUnitState as PpmdState, PpmdVariant,
    SeeEntry, SeeEstimator, SubAllocBumpArena, PPMD_PERIOD_BITS, PPMD_UNIT_SIZE, SEE_NUM_BINS,
    SEE_NUM_CLASSES,
};

#[test]
fn test_ppmd_context_and_state_memory_layout_invariants() {
    assert_eq!(
        std::mem::size_of::<PpmdContext>(),
        12,
        "PpmdContext must be exactly 12 bytes in physical memory"
    );
    assert_eq!(
        std::mem::size_of::<PpmdState>(),
        6,
        "PpmdState must be exactly 6 bytes in physical memory"
    );
    assert_eq!(
        std::mem::align_of::<PpmdState>(),
        1,
        "PpmdState packed alignment must be 1 byte"
    );

    // Verify 2 PpmdState units exactly equal 1 PPMD_UNIT_SIZE (12 bytes)
    assert_eq!(std::mem::size_of::<PpmdState>() * 2, PPMD_UNIT_SIZE);

    // Test binary state embedding in PpmdContext (1-state optimization)
    let mut ctx = PpmdContext::new(42);
    assert_eq!(ctx.suffix_ref, 42);
    assert_eq!(ctx.num_stats, 0);
    assert!(ctx.is_binary());
    assert!(!ctx.is_root());

    let state = PpmdState::new(0x41, 15, 0x1234_5678);
    ctx.num_stats = 1;
    ctx.set_one_state(&state);

    let extracted = ctx.one_state();
    assert_eq!(extracted.symbol(), 0x41);
    assert_eq!(extracted.freq(), 15);
    assert_eq!(extracted.successor_ref(), 0x1234_5678);
}

#[test]
fn test_see_estimator_quantization_and_adaptive_updates() {
    let mut see = SeeEstimator::new();

    // Verify all bins initialized properly
    for class_idx in 0..SEE_NUM_CLASSES {
        for bin_idx in 0..SEE_NUM_BINS {
            let entry = see.get_entry_mut(class_idx, bin_idx);
            assert!(entry.summ > 0);
            assert_eq!(entry.shift, PPMD_PERIOD_BITS - 4);
            assert_eq!(entry.count, 4);
        }
    }

    // Test quantization bin derivation across all 16 combinations
    for non_masked in 1..=4 {
        for diff_stats in 1..=4 {
            for summ_freq in [10u16, 200u16] {
                for num_stats in [1u16, 16u16] {
                    for num_masked in [0usize, 5usize] {
                        for hi_bits in [0u8, 8u8] {
                            let bin = SeeEstimator::quantize_bin(
                                non_masked, diff_stats, summ_freq, num_stats, num_masked, hi_bits,
                            );
                            assert!(bin < SEE_NUM_BINS);
                        }
                    }
                }
            }
        }
    }

    // Test frequency generation and adaptive updating
    let entry = see.get_entry_mut(0, 0);
    let initial_summ = entry.summ;
    let esc_freq = entry.make_esc_freq();
    assert!(esc_freq >= 1);
    assert!(entry.summ < initial_summ);

    // Run updates to simulate count decrement and shift acceleration
    let mut test_entry = SeeEntry::new(100, 3, 2);
    test_entry.update();
    assert_eq!(test_entry.count, 1);
    test_entry.update();
    assert_eq!(test_entry.count, 3 << 3);
    assert_eq!(test_entry.shift, 4);
}

#[test]
fn test_suballoc_bump_arena_continuous_allocation_and_freelist_cycling() {
    let memory_sizes = [2048, 4096, 65536, 1024 * 1024];

    for &size in &memory_sizes {
        let mut arena = SubAllocBumpArena::new(size, PpmdVariant::Ppmd7)
            .expect("Arena initialization must succeed for valid size");

        assert_eq!(arena.size, size);
        assert!(arena.root_context_ref > 0);

        // Read and verify root Order-0 context
        let root = arena
            .read_context(arena.root_context_ref)
            .expect("Root context must be readable");
        assert_eq!(root.num_stats, 256);
        assert_eq!(root.summ_freq, 257);
        assert_eq!(root.suffix_ref, 0);

        // Verify root states array
        let state_0 = arena
            .read_state(root.stats_ref, 0)
            .expect("State 0 must be readable");
        assert_eq!(state_0.symbol(), 0);
        assert_eq!(state_0.freq(), 1);

        let state_255 = arena
            .read_state(root.stats_ref, 255)
            .expect("State 255 must be readable");
        assert_eq!(state_255.symbol(), 255);
        assert_eq!(state_255.freq(), 1);

        // Allocate several contexts
        let mut allocated_ctxs = Vec::new();
        for _ in 0..10 {
            if let Ok(ctx_ref) = arena.alloc_context() {
                allocated_ctxs.push(ctx_ref);
                let ctx = PpmdContext::new(arena.root_context_ref);
                arena.write_context(ctx_ref, &ctx).unwrap();
            }
        }
        assert!(!allocated_ctxs.is_empty());

        // Allocate state blocks of different sizes
        let mut state_blocks = Vec::new();
        for num_states in [2, 4, 8, 16] {
            if let Ok(block_ref) = arena.alloc_units_for_states(num_states) {
                state_blocks.push((block_ref, num_states.div_ceil(2)));
            }
        }

        // Free state blocks and verify recycling from FreeLists
        for &(block_ref, nu) in &state_blocks {
            arena.free_units(block_ref, nu);
        }

        // Reallocate should reuse freed units from FreeLists
        for &(orig_ref, nu) in &state_blocks {
            let indx = arena.units_to_indx[(nu - 1).min(127)] as usize;
            if let Ok(realloc_ref) = arena.alloc_units(indx) {
                assert!(realloc_ref > 0);
                assert_eq!(realloc_ref, orig_ref);
            }
        }
    }
}

#[test]
fn test_suballoc_bump_arena_split_and_glue_defragmentation() {
    let mut arena = SubAllocBumpArena::new(65536, PpmdVariant::Ppmd7).unwrap();

    // Allocate 4 blocks of index 5 (8 units each = 96 bytes)
    let b1 = arena.alloc_units(5).unwrap();
    let b2 = arena.alloc_units(5).unwrap();
    let b3 = arena.alloc_units(5).unwrap();
    let b4 = arena.alloc_units(5).unwrap();

    // Free all 4 blocks
    arena.free_units(b1, 8);
    arena.free_units(b2, 8);
    arena.free_units(b3, 8);
    arena.free_units(b4, 8);

    // Trigger Glue defragmentation
    arena.glue_free_blocks();

    // After gluing adjacent 8-unit blocks (8 + 8 + 8 + 8 = 32 units),
    // we should be able to allocate a large 32-unit block (index 13) directly from free list!
    let indx_32 = arena.units_to_indx[31] as usize;
    let b_large = arena.alloc_units(indx_32);
    assert!(b_large.is_ok(), "Coalesced large block allocation must succeed");
}

#[test]
fn test_suballoc_memory_exhaustion_and_restart_recovery() {
    // Smallest 2KB arena
    let mut arena = SubAllocBumpArena::new(2048, PpmdVariant::Ppmd7).unwrap();

    // Allocate until exhaustion
    let mut alloc_count = 0;
    while arena.alloc_context().is_ok() {
        alloc_count += 1;
        if alloc_count > 1000 {
            break;
        }
    }

    // Allocation should now fail with OutOfMemory
    let failed_alloc = arena.alloc_units_for_states(64);
    assert!(failed_alloc.is_err());

    // Trigger RestartModel
    arena.restart_model().expect("RestartModel must reinitialize cleanly");

    // Allocation must succeed again after restart
    let new_ctx = arena.alloc_context();
    assert!(new_ctx.is_ok(), "Allocation after RestartModel must succeed");
}

#[test]
fn test_suballoc_memory_exhaustion_and_cutoff_prune_recovery() {
    let mut arena = SubAllocBumpArena::new(8192, PpmdVariant::Ppmd8).unwrap();
    assert_eq!(arena.restore_method, PpmdRestoreMethod::CutOff);

    // Build context chain
    let mut prev = arena.root_context_ref;
    for i in 0..10 {
        if let Ok(c) = arena.alloc_context() {
            if let Ok(states) = arena.alloc_units_for_states(4) {
                let ctx = PpmdContext::new_full(4, 10, states, prev);
                for s in 0..4 {
                    let st = PpmdState::new(s as u8, 2, if i < 8 { c } else { 0 });
                    arena.write_state(states, s, &st).unwrap();
                }
                arena.write_context(c, &ctx).unwrap();
                prev = c;
            }
        }
    }

    // Run CutOff tree prune
    let pruned = arena.cutoff_prune(arena.root_context_ref, 6);
    assert!(pruned.is_ok());

    // Context allocation remains viable
    let next_ctx = arena.alloc_context();
    assert!(next_ctx.is_ok());
}
