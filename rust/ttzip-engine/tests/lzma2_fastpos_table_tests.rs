// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive Verification Test Suite for LZMA/LZMA2 FastPos 4096 Slot Lookup Table.
//!
//! Validates:
//! 1. 64-byte cacheline memory alignment invariants.
//! 2. 100% exact match between short distance table (0..4095) and canonical LZMA slot theory.
//! 3. Equivalence between `get_pos_slot_fast` and `get_pos_slot` for all distances < 4096.
//! 4. Logarithmic calculation accuracy across large distances (64KB, 1MB, 16MB, 64MB, 1GB).
//! 5. Exact slot boundaries for all 64 LZMA position slots (0..63).
//! 6. Numerical safety and absence of overflow/panic on boundary values (0, 1, 4095, 4096, 2^31-1, u32::MAX).

use std::mem::align_of;
use ttzip_engine::codecs::lzma2::fastpos_table::{
    get_pos_slot, get_pos_slot_fast, get_pos_slot_math_spec, FastPosTable, FAST_POS_TABLE,
    FAST_POS_TABLE_SIZE, K_FAST_DIST_BITS,
};

#[test]
fn test_fastpos_constants_and_alignment() {
    assert_eq!(K_FAST_DIST_BITS, 12);
    assert_eq!(FAST_POS_TABLE_SIZE, 4096);
    assert_eq!(align_of::<FastPosTable>(), 64);

    let ptr = FAST_POS_TABLE.0.as_ptr() as usize;
    assert_eq!(
        ptr % 64,
        0,
        "FAST_POS_TABLE must be 64-byte cacheline aligned"
    );
}

#[test]
fn test_short_distances_against_theory_and_math_spec() {
    // Distance 0 -> Slot 0
    assert_eq!(get_pos_slot(0), 0);
    assert_eq!(get_pos_slot_fast(0), 0);
    assert_eq!(FAST_POS_TABLE[0], 0);

    // Distance 1 -> Slot 1
    assert_eq!(get_pos_slot(1), 1);
    assert_eq!(get_pos_slot_fast(1), 1);
    assert_eq!(FAST_POS_TABLE[1], 1);

    // Distance 2 -> Slot 2
    assert_eq!(get_pos_slot(2), 2);
    assert_eq!(get_pos_slot_fast(2), 2);
    assert_eq!(FAST_POS_TABLE[2], 2);

    // Distance 3 -> Slot 3
    assert_eq!(get_pos_slot(3), 3);
    assert_eq!(get_pos_slot_fast(3), 3);
    assert_eq!(FAST_POS_TABLE[3], 3);

    // Distances 4..=5 -> Slot 4
    for dist in 4..=5 {
        assert_eq!(get_pos_slot(dist), 4);
        assert_eq!(get_pos_slot_fast(dist), 4);
        assert_eq!(FAST_POS_TABLE[dist as usize], 4);
    }

    // Distances 6..=7 -> Slot 5
    for dist in 6..=7 {
        assert_eq!(get_pos_slot(dist), 5);
        assert_eq!(get_pos_slot_fast(dist), 5);
        assert_eq!(FAST_POS_TABLE[dist as usize], 5);
    }

    // Distances 8..=11 -> Slot 6
    for dist in 8..=11 {
        assert_eq!(get_pos_slot(dist), 6);
        assert_eq!(get_pos_slot_fast(dist), 6);
        assert_eq!(FAST_POS_TABLE[dist as usize], 6);
    }

    // Distances 12..=15 -> Slot 7
    for dist in 12..=15 {
        assert_eq!(get_pos_slot(dist), 7);
        assert_eq!(get_pos_slot_fast(dist), 7);
        assert_eq!(FAST_POS_TABLE[dist as usize], 7);
    }

    // Exhaustively verify all 4096 entries against mathematical specification
    for dist in 0..(FAST_POS_TABLE_SIZE as u32) {
        let expected_slot = get_pos_slot_math_spec(dist);
        let table_val = FAST_POS_TABLE[dist as usize] as u32;
        let fast_val = get_pos_slot_fast(dist);
        let pos_val = get_pos_slot(dist);

        assert_eq!(
            table_val, expected_slot,
            "Table mismatch at dist {dist}: got {table_val}, expected {expected_slot}"
        );
        assert_eq!(
            fast_val, expected_slot,
            "Fast slot mismatch at dist {dist}: got {fast_val}, expected {expected_slot}"
        );
        assert_eq!(
            pos_val, expected_slot,
            "get_pos_slot mismatch at dist {dist}: got {pos_val}, expected {expected_slot}"
        );
    }
}

#[test]
fn test_large_distances_against_logarithmic_spec() {
    let test_cases: &[(u32, &'static str)] = &[
        (64 * 1024, "64 KB"),
        (1024 * 1024, "1 MB"),
        (16 * 1024 * 1024, "16 MB"),
        (64 * 1024 * 1024, "64 MB"),
        (1024 * 1024 * 1024, "1 GB"),
    ];

    for &(dist, label) in test_cases {
        let actual = get_pos_slot(dist);
        let expected = get_pos_slot_math_spec(dist);
        assert_eq!(
            actual, expected,
            "Mismatch for {label} (dist={dist}): got {actual}, expected {expected}"
        );

        // Also test immediate neighbors
        if dist > 0 {
            assert_eq!(
                get_pos_slot(dist - 1),
                get_pos_slot_math_spec(dist - 1),
                "Mismatch for {label} - 1 (dist={})",
                dist - 1
            );
        }
        if dist < u32::MAX {
            assert_eq!(
                get_pos_slot(dist + 1),
                get_pos_slot_math_spec(dist + 1),
                "Mismatch for {label} + 1 (dist={})",
                dist + 1
            );
        }
    }
}

#[test]
fn test_all_64_slot_boundaries() {
    // For slots 0..4: base distances are 0, 1, 2, 3
    for s in 0..4u32 {
        assert_eq!(get_pos_slot(s), s);
        assert_eq!(get_pos_slot_math_spec(s), s);
    }

    // For slots 4..64: verify base and end bounds
    for s in 4..64u32 {
        let k = (s >> 1) - 1;
        let base = (2 | (s & 1)) << k;
        let end = if s == 63 {
            u32::MAX
        } else {
            (((2 | (s & 1)) + 1) << k) - 1
        };

        // Base distance of slot
        assert_eq!(
            get_pos_slot(base),
            s,
            "Slot {s} base boundary {base} computed incorrectly"
        );
        assert_eq!(
            get_pos_slot_math_spec(base),
            s,
            "Slot {s} base boundary {base} math spec mismatch"
        );

        // End distance of slot
        assert_eq!(
            get_pos_slot(end),
            s,
            "Slot {s} end boundary {end} computed incorrectly"
        );
        assert_eq!(
            get_pos_slot_math_spec(end),
            s,
            "Slot {s} end boundary {end} math spec mismatch"
        );

        // If not at slot 4, previous distance before base should belong to slot s - 1
        if base > 0 {
            assert_eq!(
                get_pos_slot(base - 1),
                s - 1,
                "Distance {} immediately before slot {s} base should be slot {}",
                base - 1,
                s - 1
            );
        }

        // If not at slot 63, next distance after end should belong to slot s + 1
        if end < u32::MAX {
            assert_eq!(
                get_pos_slot(end + 1),
                s + 1,
                "Distance {} immediately after slot {s} end should be slot {}",
                end + 1,
                s + 1
            );
        }
    }
}

#[test]
fn test_boundary_extrema_and_numerical_safety() {
    let boundary_values: &[u32] = &[
        0,
        1,
        2,
        3,
        4,
        4095,
        4096,
        4097,
        (1 << 16) - 1,
        1 << 16,
        (1 << 20) - 1,
        1 << 20,
        (1 << 30) - 1,
        1 << 30,
        (1 << 31) - 1,
        1 << 31,
        u32::MAX - 1,
        u32::MAX,
    ];

    for &dist in boundary_values {
        let slot = get_pos_slot(dist);
        let math_slot = get_pos_slot_math_spec(dist);
        assert_eq!(
            slot, math_slot,
            "Extremum test failed at dist {dist}: got {slot}, expected {math_slot}"
        );
        assert!(
            slot < 64,
            "Slot {slot} out of valid 6-bit LZMA range [0, 63] at dist {dist}"
        );
    }
}

#[test]
fn test_dense_sample_differential_across_entire_u32_range() {
    // Step through the entire 32-bit range with geometric and pseudo-random spacing
    let mut dist: u64 = 0;
    while dist <= u32::MAX as u64 {
        let d = dist as u32;
        assert_eq!(
            get_pos_slot(d),
            get_pos_slot_math_spec(d),
            "Dense sample differential failed at dist {d}"
        );

        if dist < 10000 {
            dist += 1;
        } else if dist < 1_000_000 {
            dist += 97;
        } else if dist < 100_000_000 {
            dist += 9973;
        } else {
            dist += 1_000_003;
        }
    }
}
