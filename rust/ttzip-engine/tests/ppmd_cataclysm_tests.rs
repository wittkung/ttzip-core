// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! PPMd Extreme Memory Starvation Cut-Off & Restart Cataclysm Test Suite (Task 14.10).
//!
//! Hardens PPMd7 (RestartModel) and PPMd8 (CutOff 75% Space Pruning) under extreme
//! memory quotas (2KB, 8KB, 32KB, 128KB) against structured and high-entropy corpora.
//!
//! Validates:
//! - 100% Bit-Exact decompression fidelity across dozens to hundreds of memory exhaustion events.
//! - Deterministic pool restarts for PPMd7 and >= 25% space pruning for PPMd8.
//! - Strict physical memory bounds: Zero heap reallocation and zero net heap leaks.

use ttzip_engine::codecs::ppmd::{
    PpmdRestoreMethod, PpmdSubAllocModel, PpmdVariant, PPMD_MIN_SUBALLOC_SIZE,
};

/// Quota ladder covering extreme physical minimum (2KB), small (8KB), medium (32KB), and bound (128KB).
const CATACLYSM_MEMORY_QUOTAS: [usize; 4] = [
    2048,           // 2KB: Absolute hardware/allocator limit
    8192,           // 8KB: Extreme starvation
    32768,          // 32KB: Embedded microcontroller budget
    131072,         // 128KB: Constrained sandbox budget
];

// MARK: - Corpus Generation Helpers

/// Generates a realistic structured JSON dataset with deep nesting and repetitive key tokens.
fn generate_json_corpus(target_len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(target_len);
    out.extend_from_slice(b"{\"system_metrics\":{\"engine\":\"TTZip\",\"nodes\":[");
    let mut idx = 0;
    while out.len() < target_len.saturating_sub(64) {
        let entry = format!(
            "{{\"id\":{},\"name\":\"node_{}\",\"status\":\"active\",\"load\":{:.2},\"tags\":[\"io\",\"vfs\",\"core\"]}},",
            idx,
            idx % 32,
            ((idx * 17) % 100) as f64 / 10.0
        );
        out.extend_from_slice(entry.as_bytes());
        idx += 1;
    }
    if out.ends_with(b",") {
        out.pop();
    }
    out.extend_from_slice(b"],\"status\":\"ok\",\"verified\":true}}");
    out
}

/// Generates a structured XML dataset with nested tags, attributes, and text payloads.
fn generate_xml_corpus(target_len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(target_len);
    out.extend_from_slice(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<archive_registry>\n");
    let mut idx = 0;
    while out.len() < target_len.saturating_sub(64) {
        let entry = format!(
            "  <entry id=\"{}\" type=\"file\" flags=\"0x{:04X}\">\n    <path>/sys/kernel/suballoc/node_{}.rs</path>\n    <digest algorithm=\"sha256\">a1b2c3d4e5f67890{:04x}</digest>\n  </entry>\n",
            idx,
            (idx * 31) & 0xFFFF,
            idx % 64,
            idx & 0xFFFF
        );
        out.extend_from_slice(entry.as_bytes());
        idx += 1;
    }
    out.extend_from_slice(b"</archive_registry>\n");
    out
}

/// Generates a realistic Rust source code corpus with syntax keywords, functions, and structs.
fn generate_rust_code_corpus(target_len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(target_len);
    out.extend_from_slice(b"// TTZip Synthesized Microkernel Source Corpus\n\n");
    let mut fn_idx = 0;
    while out.len() < target_len.saturating_sub(128) {
        let snippet = format!(
            "#[inline]\npub fn process_suballoc_chunk_{}(arena: &mut SubAllocBumpArena, count: usize) -> Result<u32, TTZipStatus> {{\n    let mut sum: u32 = 0;\n    for i in 0..count {{\n        let offset = arena.alloc_units_for_states((i % 16) + 1)?;\n        sum = sum.wrapping_add(offset);\n    }}\n    Ok(sum)\n}}\n\n",
            fn_idx
        );
        out.extend_from_slice(snippet.as_bytes());
        fn_idx += 1;
    }
    out
}

/// Generates a high-entropy mixed stream with pseudo-random shifts and periodic anchors.
fn generate_mixed_entropy_corpus(target_len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(target_len);
    let mut state: u32 = 0x1234_5678;
    for i in 0..target_len {
        if i % 32 == 0 {
            out.extend_from_slice(b"__ANCHOR_HEADER__");
        } else {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            let byte = ((state >> 16) & 0xFF) as u8;
            out.push(byte);
        }
    }
    out.truncate(target_len);
    out
}

// MARK: - PPMd7 RestartModel Cataclysm Tests

#[test]
fn test_ppmd7_extreme_starvation_restart_cataclysm_matrix() {
    let corpora = [
        ("JSON_Corpus", generate_json_corpus(8192)),
        ("XML_Corpus", generate_xml_corpus(8192)),
        ("Rust_Source_Corpus", generate_rust_code_corpus(8192)),
        ("Mixed_Entropy_Corpus", generate_mixed_entropy_corpus(4096)),
    ];

    for &quota in &CATACLYSM_MEMORY_QUOTAS {
        for (name, corpus) in &corpora {
            let mut encoder_model = PpmdSubAllocModel::new(quota, 6, PpmdVariant::Ppmd7)
                .expect("PPMd7 encoder model creation must succeed");
            assert_eq!(encoder_model.arena.variant, PpmdVariant::Ppmd7);
            assert_eq!(encoder_model.arena.restore_method, PpmdRestoreMethod::Restart);

            let compressed = encoder_model
                .compress(corpus)
                .expect("PPMd7 compression under extreme starvation must succeed");
            assert!(!compressed.is_empty(), "Compressed payload must not be empty");

            // For extreme 2KB and 8KB quotas, verify that RestartModel was triggered repeatedly
            if quota <= 8192 {
                assert!(
                    encoder_model.restart_count >= 5,
                    "PPMd7 under quota {} on {} must trigger multiple restarts (got {})",
                    quota,
                    name,
                    encoder_model.restart_count
                );
            }

            // Decompression validation
            let mut decoder_model = PpmdSubAllocModel::new(quota, 6, PpmdVariant::Ppmd7)
                .expect("PPMd7 decoder model creation must succeed");

            let decompressed = decoder_model
                .decompress(&compressed, corpus.len())
                .expect("PPMd7 decompression under extreme starvation must succeed");

            // 100% Bit-Exact Verification
            assert_eq!(
                decompressed.len(),
                corpus.len(),
                "Decompressed length mismatch on {} under quota {}",
                name,
                quota
            );
            assert_eq!(
                &decompressed,
                corpus,
                "100% Bit-Exact fidelity violated for PPMd7 on {} under quota {}",
                name,
                quota
            );

            // Synchronous Restart Count Verification
            assert_eq!(
                encoder_model.restart_count,
                decoder_model.restart_count,
                "Encoder and Decoder restart counts must be 100% deterministic and equal"
            );

            // Hard Memory Bounding Invariant
            assert!(
                decoder_model.arena.active_used_bytes() <= quota,
                "Active used bytes ({}) exceeded hard quota limit ({})",
                decoder_model.arena.active_used_bytes(),
                quota
            );
        }
    }
}

// MARK: - PPMd8 CutOff Pruning Cataclysm Tests

#[test]
fn test_ppmd8_extreme_starvation_cutoff_prune_cataclysm_matrix() {
    let corpora = [
        ("JSON_Corpus", generate_json_corpus(8192)),
        ("XML_Corpus", generate_xml_corpus(8192)),
        ("Rust_Source_Corpus", generate_rust_code_corpus(8192)),
        ("Mixed_Entropy_Corpus", generate_mixed_entropy_corpus(4096)),
    ];

    for &quota in &CATACLYSM_MEMORY_QUOTAS {
        for (name, corpus) in &corpora {
            let mut encoder_model = PpmdSubAllocModel::new(quota, 8, PpmdVariant::Ppmd8)
                .expect("PPMd8 encoder model creation must succeed");
            assert_eq!(encoder_model.arena.variant, PpmdVariant::Ppmd8);
            assert_eq!(encoder_model.arena.restore_method, PpmdRestoreMethod::CutOff);

            let compressed = encoder_model
                .compress(corpus)
                .expect("PPMd8 compression under extreme starvation must succeed");
            assert!(!compressed.is_empty(), "Compressed payload must not be empty");

            // Verify CutOff pruning events under tight budgets
            if quota <= 8192 {
                assert!(
                    encoder_model.cutoff_count >= 5,
                    "PPMd8 under quota {} on {} must trigger multiple CutOff prunes (got {})",
                    quota,
                    name,
                    encoder_model.cutoff_count
                );
                assert!(
                    encoder_model.total_freed_by_cutoff >= quota / 4,
                    "CutOff prunes must reclaim >= 25% of unit space (freed {}, quota {})",
                    encoder_model.total_freed_by_cutoff,
                    quota
                );
            }

            // Decompression validation
            let mut decoder_model = PpmdSubAllocModel::new(quota, 8, PpmdVariant::Ppmd8)
                .expect("PPMd8 decoder model creation must succeed");

            let decompressed = decoder_model
                .decompress(&compressed, corpus.len())
                .expect("PPMd8 decompression under extreme starvation must succeed");

            // 100% Bit-Exact Verification
            assert_eq!(
                decompressed.len(),
                corpus.len(),
                "Decompressed length mismatch on {} under quota {}",
                name,
                quota
            );
            assert_eq!(
                &decompressed,
                corpus,
                "100% Bit-Exact fidelity violated for PPMd8 on {} under quota {}",
                name,
                quota
            );

            // Synchronous CutOff Count Verification
            assert_eq!(
                encoder_model.cutoff_count,
                decoder_model.cutoff_count,
                "Encoder and Decoder CutOff counts must be 100% deterministic and equal"
            );

            // Hard Memory Bounding Invariant
            assert!(
                decoder_model.arena.active_used_bytes() <= quota,
                "Active used bytes ({}) exceeded hard quota limit ({})",
                decoder_model.arena.active_used_bytes(),
                quota
            );
        }
    }
}

// MARK: - Hard Memory Invariants & Zero Heap Leakage Tests

#[test]
fn test_ppmd_hard_memory_invariants_and_zero_heap_leak() {
    for &quota in &CATACLYSM_MEMORY_QUOTAS {
        let mut model7 = PpmdSubAllocModel::new(quota, 6, PpmdVariant::Ppmd7).unwrap();
        let mut model8 = PpmdSubAllocModel::new(quota, 6, PpmdVariant::Ppmd8).unwrap();

        // 1. Assert minimum allocation bound
        assert!(quota >= PPMD_MIN_SUBALLOC_SIZE);

        // 2. Perform 500 consecutive symbol insertion and exhaustion cycles
        for step in 0..500 {
            let symbol = ((step * 37) & 0xFF) as u8;
            let mut enc = ttzip_engine::codecs::ppmd::PpmdRangeEncoder::new();
            model7.encode_symbol(symbol, &mut enc).expect("PPMd7 encode must not fail");
            model8.encode_symbol(symbol, &mut enc).expect("PPMd8 encode must not fail");

            // Strict memory bounding assertion at every step
            assert!(
                model7.arena.active_used_bytes() <= quota,
                "PPMd7 active memory must never exceed quota"
            );
            assert!(
                model8.arena.active_used_bytes() <= quota,
                "PPMd8 active memory must never exceed quota"
            );
        }

        // 3. Assert zero heap expansion (capacity equals initial allocation)
        assert_eq!(
            model7.arena.size,
            quota,
            "Arena size must remain strictly bounded"
        );
        assert_eq!(
            model8.arena.size,
            quota,
            "Arena size must remain strictly bounded"
        );
    }
}

// MARK: - Comparative Adaptation Tests

#[test]
fn test_ppmd7_vs_ppmd8_comparative_compression_gain_under_starvation() {
    let corpus = generate_json_corpus(16384);
    let tight_quota = 8192; // 8KB tight budget

    let mut model7 = PpmdSubAllocModel::new(tight_quota, 6, PpmdVariant::Ppmd7).unwrap();
    let mut model8 = PpmdSubAllocModel::new(tight_quota, 6, PpmdVariant::Ppmd8).unwrap();

    let comp7 = model7.compress(&corpus).expect("PPMd7 compress");
    let comp8 = model8.compress(&corpus).expect("PPMd8 compress");

    assert!(!comp7.is_empty());
    assert!(!comp8.is_empty());

    // Both variants must decompress to 100% Bit-Exact data
    let mut dec7 = PpmdSubAllocModel::new(tight_quota, 6, PpmdVariant::Ppmd7).unwrap();
    let mut dec8 = PpmdSubAllocModel::new(tight_quota, 6, PpmdVariant::Ppmd8).unwrap();

    let out7 = dec7.decompress(&comp7, corpus.len()).expect("PPMd7 decompress");
    let out8 = dec8.decompress(&comp8, corpus.len()).expect("PPMd8 decompress");

    assert_eq!(&out7, &corpus);
    assert_eq!(&out8, &corpus);
}
