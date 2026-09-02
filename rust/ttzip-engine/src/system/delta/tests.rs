// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit test suite for TTZip delta patching, bsdiff/bspatch microkernel,
//! format serialization, and boundary fault tolerance.

use super::*;
use crate::system::delta::bsdiff::BsDiffControl;
use crate::system::delta::types::{DeltaCommand, DeltaError, DeltaFormat};

#[test]
fn test_empty_data_diff_and_patch() {
    // 1. Both empty
    let old_empty = b"";
    let new_empty = b"";
    let patch = TTZipDeltaEngine::create_patch(old_empty, new_empty).expect("Patch creation failed");
    let result = TTZipDeltaEngine::apply_patch(old_empty, &patch).expect("Patch application failed");
    assert_eq!(result, new_empty);

    // 2. Old has content, new is empty
    let old_data = b"Some existing file contents";
    let patch = TTZipDeltaEngine::create_patch(old_data, new_empty).expect("Patch creation failed");
    let result = TTZipDeltaEngine::apply_patch(old_data, &patch).expect("Patch application failed");
    assert_eq!(result, new_empty);

    // 3. Old is empty, new has content
    let new_data = b"Brand new file contents created from nothing";
    let patch = TTZipDeltaEngine::create_patch(old_empty, new_data).expect("Patch creation failed");
    let result = TTZipDeltaEngine::apply_patch(old_empty, &patch).expect("Patch application failed");
    assert_eq!(result, new_data);
}

#[test]
fn test_homomorphic_data_zero_diff() {
    let payloads: Vec<Vec<u8>> = vec![
        b"A".to_vec(),
        b"Hello, TTZip delta patching microkernel!".to_vec(),
        vec![0x42; 1024],
        (0..8192).map(|i| (i % 251) as u8).collect(),
    ];

    for data in payloads {
        let patch = TTZipDeltaEngine::create_patch(&data, &data).expect("Homomorphic patch failed");
        let (reconstructed, telemetry) = TTZipDeltaEngine::apply_patch_with_result(&data, &patch)
            .expect("Homomorphic apply failed");

        assert_eq!(reconstructed, data);
        assert_eq!(telemetry.bytes_out, data.len());
        assert!(!telemetry.sha256_hex.is_empty());
    }
}

#[test]
fn test_macho_simulated_instruction_displacement() {
    // Simulating ARM64 / Mach-O binary section with localized pointer updates and rebased addresses
    let mut old_bin: Vec<u8> = vec![0x90u8; 4096];
    for i in (0..4096).step_by(16) {
        // Mock instruction sequence: ADRP + LDR + BL + NOP
        old_bin[i] = 0x00;
        old_bin[i + 1] = 0x00;
        old_bin[i + 2] = 0x00;
        old_bin[i + 3] = 0x90; // ADRP

        old_bin[i + 4] = 0x00;
        old_bin[i + 5] = 0x04;
        old_bin[i + 6] = 0x40;
        old_bin[i + 7] = 0xF9; // LDR

        old_bin[i + 8] = 0x20;
        old_bin[i + 9] = 0x00;
        old_bin[i + 10] = 0x80;
        old_bin[i + 11] = 0x52; // MOV

        old_bin[i + 12] = 0xC0;
        old_bin[i + 13] = 0x03;
        old_bin[i + 14] = 0x5F;
        old_bin[i + 15] = 0xD6; // RET
    }

    let mut new_bin: Vec<u8> = old_bin.clone();
    // Simulate rebase offset drift in 5% of instructions and a string table insertion
    for i in (0..4096).step_by(64) {
        let b1 = new_bin[i + 1];
        new_bin[i + 1] = b1.wrapping_add(0x04);
        let b9 = new_bin[i + 9];
        new_bin[i + 9] = b9.wrapping_add(0x10);
    }
    new_bin.extend_from_slice(b"__cstring:com.wittkung.ttzip.delta.symbol_table_v2\0");

    let patch = TTZipDeltaEngine::create_patch(&old_bin, &new_bin).expect("Mach-O diff failed");
    let (reconstructed, telemetry) = TTZipDeltaEngine::apply_patch_with_result(&old_bin, &patch)
        .expect("Mach-O patch apply failed");

    assert_eq!(reconstructed, new_bin);
    assert_eq!(telemetry.bytes_out, new_bin.len());
    // Delta patch must be significantly smaller than full new binary
    assert!(patch.len() < new_bin.len());
}

#[test]
fn test_delta_commands_serialization_and_cloning() {
    let old_data = b"Original baseline data string for TTZip delta engine testing";
    let new_data = b"Original baseline modified string with cloned sections and new capabilities";

    let commands = vec![
        DeltaCommand::Extract {
            offset: 0,
            length: 18,
        },
        DeltaCommand::Clone {
            source_offset: 0,
            target_offset: 100,
            length: 8,
        },
        DeltaCommand::ModifyPermissions { mode: 0o755 },
        DeltaCommand::Delete {
            offset: 18,
            length: 4,
        },
        DeltaCommand::BinaryDiff {
            diff_len: 20,
            extra_len: 10,
            seek_offset: 5,
        },
    ];

    let patch_bytes = TTZipDeltaEngine::create_patch_with_commands(old_data, new_data, &commands)
        .expect("Patch with commands failed");

    let archive = TTZipDeltaArchive::deserialize(&patch_bytes).expect("Deserialize archive failed");
    let decompressed_cmds = archive
        .decompress_commands()
        .expect("Decompress commands failed");

    assert_eq!(decompressed_cmds, commands);

    let reconstructed = TTZipDeltaEngine::apply_patch(old_data, &patch_bytes)
        .expect("Apply patch failed");
    assert_eq!(reconstructed, new_data);
}

#[test]
fn test_corrupted_patch_fault_tolerance() {
    let old_data = b"Standard baseline data";
    let new_data = b"Upgraded target data with modifications";

    let valid_patch = TTZipDeltaEngine::create_patch(old_data, new_data).unwrap();

    // 1. Invalid Magic
    let mut corrupt_magic = valid_patch.clone();
    corrupt_magic[0] = b'B';
    corrupt_magic[1] = b'A';
    corrupt_magic[2] = b'D';
    corrupt_magic[3] = b'!';
    assert!(matches!(
        TTZipDeltaEngine::apply_patch(old_data, &corrupt_magic),
        Err(DeltaError::CorruptedPatch(_)) | Err(DeltaError::InvalidMagic(_))
    ));

    // 2. Truncated patch
    let truncated = &valid_patch[..12];
    assert!(matches!(
        TTZipDeltaEngine::apply_patch(old_data, truncated),
        Err(DeltaError::TruncatedData { .. })
    ));

    // 3. Corrupted Payload / CRC Mismatch
    let mut corrupt_payload = valid_patch.clone();
    let mid = corrupt_payload.len() / 2;
    corrupt_payload[mid] ^= 0xFF;
    assert!(matches!(
        TTZipDeltaEngine::apply_patch(old_data, &corrupt_payload),
        Err(DeltaError::CorruptedPatch(_))
    ));

    // 4. Source Data Mismatch
    let wrong_old = b"Completely different source baseline data";
    assert!(matches!(
        TTZipDeltaEngine::apply_patch(wrong_old, &valid_patch),
        Err(DeltaError::SourceHashMismatch { .. })
    ));
}

#[test]
fn test_out_of_bounds_seek_and_overflow_protection() {
    let old_data = b"Short buffer";
    let controls = vec![BsDiffControl::new(20, 0, 0)]; // diff_len exceeds old_data.len()
    let diff_data = vec![0u8; 20];
    let extra_data = Vec::new();

    let result = TTZipBsPatch::apply_streams(old_data, 20, &controls, &diff_data, &extra_data);
    assert!(matches!(result, Err(DeltaError::OutOfBoundsSeek { .. })));

    // Negative out of bounds seek
    let controls_neg = vec![
        BsDiffControl::new(4, 0, -100), // seek offset goes negative
    ];
    let diff_data = vec![0u8; 4];
    let result = TTZipBsPatch::apply_streams(old_data, 4, &controls_neg, &diff_data, &extra_data);
    assert!(matches!(result, Err(DeltaError::OutOfBoundsSeek { .. })));

    // Target buffer overflow
    let controls_overflow = vec![BsDiffControl::new(10, 5, 0)];
    let diff_data = vec![0u8; 10];
    let extra_data = vec![0u8; 5];
    let result = TTZipBsPatch::apply_streams(old_data, 8, &controls_overflow, &diff_data, &extra_data);
    assert!(matches!(result, Err(DeltaError::TargetBufferOverflow { .. })));
}

#[test]
fn test_delta_format_detection_and_magic() {
    assert_eq!(DeltaFormat::from_magic(b"BSDIFF40"), DeltaFormat::Bsdiff40);
    assert_eq!(DeltaFormat::from_magic(b"BSDIFN40"), DeltaFormat::Bsdifn40);
    assert_eq!(DeltaFormat::from_magic(b"SPK3"), DeltaFormat::Spk3);
    assert_eq!(DeltaFormat::from_magic(b"SPK4"), DeltaFormat::Spk4);
    assert_eq!(DeltaFormat::from_magic(b"spk!"), DeltaFormat::Spk4);
    assert_eq!(DeltaFormat::from_magic(b"INVALID"), DeltaFormat::Unknown);
    assert_eq!(DeltaFormat::from_magic(b""), DeltaFormat::Unknown);

    assert_eq!(DeltaFormat::Spk4.magic_bytes(), b"spk!");
}

#[test]
fn test_large_payload_multi_block_diff() {
    let mut old_large = Vec::with_capacity(32768);
    let mut new_large = Vec::with_capacity(32768);

    for i in 0..32768 {
        old_large.push((i * 17 % 256) as u8);
        // Introduce intermittent modifications
        if i % 300 == 0 {
            new_large.push(0xFF);
        } else {
            new_large.push((i * 17 % 256) as u8);
        }
    }

    let patch = TTZipDeltaEngine::create_patch(&old_large, &new_large).expect("Large diff failed");
    let (reconstructed, telemetry) = TTZipDeltaEngine::apply_patch_with_result(&old_large, &patch)
        .expect("Large patch apply failed");

    assert_eq!(reconstructed, new_large);
    assert_eq!(telemetry.bytes_out, new_large.len());
    assert_eq!(telemetry.bytes_out, 32768);
}
