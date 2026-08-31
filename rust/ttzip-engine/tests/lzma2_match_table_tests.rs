// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive Verification Test Suite for LZMA2 Dual-Mode Match Table.
//!
//! Validates:
//! 1. `BitPackedEntry` 26/6-bit field packing and unpacking fidelity across all extrema.
//! 2. `StructuredMatchEntry` 40-bit (5-byte) packed representation under 128MB, 512MB, 1GB dictionaries.
//! 3. Automatic mode selection (`BitPacked` vs `Structured`) by dictionary threshold (64 MB).
//! 4. Jump-Pointer Chaining across multi-hop links for ultra-long matches (>1000 bytes) and cycle guards.
//! 5. Zero-copy buffer borrowing (`as_output_buffer_mut`, `as_byte_slice`) and boundary safety.

use std::mem::{align_of, size_of};
use ttzip_engine::codecs::lzma2::match_table::{
    BitPackedEntry, MatchTable, MatchTableMode, StructuredMatchEntry, COMPACT_DICT_THRESHOLD,
    MAX_JUMP_CHAIN_HOPS,
};

#[test]
fn test_bit_packed_entry_memory_layout_and_constants() {
    assert_eq!(size_of::<BitPackedEntry>(), 4);
    assert_eq!(align_of::<BitPackedEntry>(), 4);
    assert_eq!(BitPackedEntry::LINK_BITS, 26);
    assert_eq!(BitPackedEntry::LENGTH_BITS, 6);
    assert_eq!(BitPackedEntry::LINK_MASK, 0x03FF_FFFF);
    assert_eq!(BitPackedEntry::MAX_LINK, 67_108_863);
    assert_eq!(BitPackedEntry::LENGTH_SHIFT, 26);
    assert_eq!(BitPackedEntry::LENGTH_MASK, 0x3F);
    assert_eq!(BitPackedEntry::MAX_LENGTH, 63);
    assert_eq!(MAX_JUMP_CHAIN_HOPS, 65536);
}

#[test]
fn test_bit_packed_entry_extrema_and_roundtrip() {
    let test_cases: &[(u32, u32)] = &[
        (0, 0),
        (0, 1),
        (0, 63),
        (1, 0),
        (1, 1),
        (1, 63),
        (4095, 32),
        (65535, 10),
        (1_000_000, 45),
        (16_777_215, 60),
        (67_108_862, 62),
        (67_108_863, 63), // Absolute MAX for 26-bit link and 6-bit len
    ];

    for &(link, len) in test_cases {
        let entry = BitPackedEntry::new(link, len);
        assert_eq!(entry.link(), link, "Link mismatch for ({link}, {len})");
        assert_eq!(entry.length(), len, "Length mismatch for ({link}, {len})");

        // Raw representation check
        let expected_raw = ((len & 0x3F) << 26) | (link & 0x03FF_FFFF);
        assert_eq!(entry.raw(), expected_raw);
        assert_eq!(BitPackedEntry::from_raw(expected_raw), entry);

        // Individual setter tests
        let mut mut_entry = BitPackedEntry::default();
        mut_entry.set_link(link);
        assert_eq!(mut_entry.link(), link);
        assert_eq!(mut_entry.length(), 0);

        mut_entry.set_length(len);
        assert_eq!(mut_entry.link(), link);
        assert_eq!(mut_entry.length(), len);
    }
}

#[test]
fn test_bit_packed_entry_overflow_masking_isolation() {
    // Passing values larger than bit widths must not corrupt neighboring fields
    let oversized_link = 0xFFFF_FFFF; // 32 bits set
    let oversized_len = 0xFF; // 8 bits set

    let entry = BitPackedEntry::new(oversized_link, oversized_len);
    assert_eq!(entry.link(), BitPackedEntry::MAX_LINK);
    assert_eq!(entry.length(), BitPackedEntry::MAX_LENGTH);

    // Verify mutating link does not affect length
    let mut entry2 = BitPackedEntry::new(100, 42);
    entry2.set_link(0xFFFF_FFFF);
    assert_eq!(entry2.length(), 42);
    assert_eq!(entry2.link(), BitPackedEntry::MAX_LINK);

    // Verify mutating length does not affect link
    let mut entry3 = BitPackedEntry::new(54321, 10);
    entry3.set_length(0xFFFF_FFFF);
    assert_eq!(entry3.link(), 54321);
    assert_eq!(entry3.length(), BitPackedEntry::MAX_LENGTH);
}

#[test]
fn test_structured_match_entry_memory_layout_and_constants() {
    assert_eq!(size_of::<StructuredMatchEntry>(), 5);
    assert_eq!(StructuredMatchEntry::ENTRY_SIZE, 5);
    assert_eq!(StructuredMatchEntry::MAX_LINK, u32::MAX);
    assert_eq!(StructuredMatchEntry::MAX_LENGTH, 255);
}

#[test]
fn test_structured_match_entry_large_dictionaries() {
    let test_cases: &[(u32, u8)] = &[
        (0, 0),
        (1, 1),
        (64 * 1024 * 1024, 64),      // 64 MB
        (128 * 1024 * 1024, 128),    // 128 MB
        (256 * 1024 * 1024, 200),    // 256 MB
        (512 * 1024 * 1024, 250),    // 512 MB
        (1024 * 1024 * 1024, 255),   // 1 GB
        (u32::MAX, 255),             // 4 GB boundary
    ];

    for &(link, len) in test_cases {
        let mut entry = StructuredMatchEntry::new(link, len);
        assert_eq!(entry.link(), link, "Structured link mismatch for {link}");
        assert_eq!(entry.length(), len as u32);
        assert_eq!(entry.length_u8(), len);

        // Mutating fields
        entry.set_link(link.wrapping_add(1));
        assert_eq!(entry.link(), link.wrapping_add(1));

        entry.set_length(len.wrapping_add(1));
        assert_eq!(entry.length_u8(), len.wrapping_add(1));

        entry.set(link, len);
        assert_eq!(entry.link(), link);
        assert_eq!(entry.length_u8(), len);
    }
}

#[test]
fn test_match_table_auto_mode_selection_and_properties() {
    // 16 MB -> BitPacked (4 bytes/entry)
    let table_16m = MatchTable::new(16 * 1024 * 1024, 1000);
    assert_eq!(table_16m.mode(), MatchTableMode::BitPacked);
    assert_eq!(table_16m.entry_size_bytes(), 4);
    assert_eq!(table_16m.len(), 1000);
    assert_eq!(table_16m.memory_bytes(), 4000);
    assert!(!table_16m.is_empty());

    // 64 MB (threshold boundary) -> BitPacked
    let table_64m = MatchTable::new(COMPACT_DICT_THRESHOLD, 500);
    assert_eq!(table_64m.mode(), MatchTableMode::BitPacked);
    assert_eq!(table_64m.entry_size_bytes(), 4);
    assert_eq!(table_64m.memory_bytes(), 2000);

    // 128 MB -> Structured (5 bytes/entry)
    let table_128m = MatchTable::new(128 * 1024 * 1024, 1000);
    assert_eq!(table_128m.mode(), MatchTableMode::Structured);
    assert_eq!(table_128m.entry_size_bytes(), 5);
    assert_eq!(table_128m.memory_bytes(), 5000);

    // 1 GB -> Structured (5 bytes/entry)
    let table_1g = MatchTable::new(1024 * 1024 * 1024, 200);
    assert_eq!(table_1g.mode(), MatchTableMode::Structured);
    assert_eq!(table_1g.entry_size_bytes(), 5);
    assert_eq!(table_1g.memory_bytes(), 1000);

    // Empty table check
    let empty_table = MatchTable::new_bit_packed(0);
    assert_eq!(empty_table.len(), 0);
    assert!(empty_table.is_empty());
}

#[test]
fn test_match_table_bit_packed_jump_pointer_chaining_long_match() {
    let mut table = MatchTable::new_bit_packed(30);

    // Construct a multi-hop chain where match length exceeds 1000 bytes:
    // 16 hops of MAX_LENGTH (63) + 1 final hop of 15 = 16 * 63 + 15 = 1023 bytes.
    // Positions: 0 -> 1 -> 2 -> ... -> 16
    for i in 0..16 {
        table.set_match(i, (i + 1) as u32, BitPackedEntry::MAX_LENGTH);
    }
    // Final terminal node at pos 16
    table.set_match(16, 9999, 15);

    // Querying from pos 0 should automatically follow the chain and accumulate 1023 bytes
    let (final_link, total_len) = table.get_match(0);
    assert_eq!(total_len, 1023, "Accumulated length must be 1023 bytes");
    assert_eq!(final_link, 9999, "Final link must point to terminal target");

    // Querying from intermediate pos 10: (16 - 10) * 63 + 15 = 6 * 63 + 15 = 393 bytes
    let (inter_link, inter_len) = table.get_match(10);
    assert_eq!(inter_len, 393);
    assert_eq!(inter_link, 9999);

    // Querying terminal pos 16 directly
    let (term_link, term_len) = table.get_match(16);
    assert_eq!(term_len, 15);
    assert_eq!(term_link, 9999);

    // Raw entry check must not perform chaining
    let raw_0 = table.get_raw_entry(0).expect("raw entry exists");
    assert_eq!(raw_0, (1, 63));
}

#[test]
fn test_match_table_structured_jump_pointer_chaining_long_match() {
    let mut table = MatchTable::new_structured(20);

    // Construct a multi-hop chain in Structured mode (>1000 bytes):
    // 4 hops of MAX_LENGTH (255) + 1 final hop of 50 = 4 * 255 + 50 = 1070 bytes.
    // Positions: 0 -> 1 -> 2 -> 3 -> 4
    for i in 0..4 {
        table.set_match(i, (i + 1) as u32, StructuredMatchEntry::MAX_LENGTH);
    }
    // Terminal node at pos 4
    table.set_match(4, 88888, 50);

    let (final_link, total_len) = table.get_match(0);
    assert_eq!(total_len, 1070, "Accumulated length must be 1070 bytes");
    assert_eq!(final_link, 88888, "Final link must point to terminal target");

    // Intermediate pos 2: (4 - 2) * 255 + 50 = 560 bytes
    let (mid_link, mid_len) = table.get_match(2);
    assert_eq!(mid_len, 560);
    assert_eq!(mid_link, 88888);
}

#[test]
fn test_match_table_jump_pointer_cycle_and_out_of_bounds_guard() {
    let mut table = MatchTable::new_bit_packed(10);

    // Self-loop case: pos 0 has MAX_LENGTH (63) and links to itself (pos 0)
    table.set_match(0, 0, 63);
    let (link0, len0) = table.get_match(0);
    assert_eq!(len0, 63, "Self loop must terminate gracefully after 1 hop");
    assert_eq!(link0, 0);

    // Out-of-bounds link: pos 1 has MAX_LENGTH (63) and links to pos 999 (out of table)
    table.set_match(1, 999, 63);
    let (link1, len1) = table.get_match(1);
    assert_eq!(len1, 63, "Out of bounds jump must terminate safely");
    assert_eq!(link1, 999);

    // Querying out of bounds position directly
    assert_eq!(table.get_match(100), (0, 0));
    assert_eq!(table.get_raw_entry(100), None);
}

#[test]
fn test_match_table_zero_copy_buffer_borrowing_and_mutation() {
    let num_entries = 100;
    let mut table = MatchTable::new_bit_packed(num_entries);

    // Full buffer slice checks
    assert_eq!(table.as_byte_slice().len(), num_entries * 4);
    assert_eq!(table.as_byte_slice_mut().len(), num_entries * 4);

    // Sub-slice borrowing via as_output_buffer_mut
    let offset_pos = 10;
    let out_buf = table.as_output_buffer_mut(offset_pos);
    assert_eq!(out_buf.len(), (num_entries - offset_pos) * 4);

    // Mutate byte slice directly (simulate fast hardware scanner writing packed match)
    // Write link=12345, len=40 into entry 10 (bytes 0..4 of out_buf)
    let packed_val = BitPackedEntry::new(12345, 40).raw();
    let bytes = packed_val.to_le_bytes();
    out_buf[0..4].copy_from_slice(&bytes);

    // Verify table reflects the direct byte write without copying
    assert_eq!(table.get_match(10), (12345, 40));
    assert_eq!(table.get_raw_entry(10), Some((12345, 40)));

    // Boundary tests for output buffer
    assert_eq!(table.as_output_buffer_mut(num_entries).len(), 0);
    assert_eq!(table.as_output_buffer_mut(num_entries + 50).len(), 0);

    // Structured mode zero-copy test
    let mut struct_table = MatchTable::new_structured(50);
    assert_eq!(struct_table.as_byte_slice().len(), 50 * 5);
    let s_buf = struct_table.as_output_buffer_mut(5);
    assert_eq!(s_buf.len(), (50 - 5) * 5);

    // Clear test
    table.clear();
    assert_eq!(table.get_match(10), (0, 0));
}
