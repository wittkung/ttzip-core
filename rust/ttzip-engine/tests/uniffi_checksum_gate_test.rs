// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Mozilla UniFFI API Checksum & Signature Gate Integration Test.
//!
//! Verifies ABI contract stability, symbol checksum parity, and aggregate
//! SHA-256 fingerprint consistency across all exported UniFFI functions and methods.
//! This gate prevents silent interface drift between Rust core logic and multi-host
//! foreign bindings (macOS Swift 6, Windows WinUI 3 C#/C++, and Linux Qt 6/Slint C++).

use sha2::{Digest, Sha256};
use ttzip_engine::uniffi_api::archive::{
    estimate_shannon_entropy, recommend_codec, sniff_format_buffer,
};
use ttzip_engine::uniffi_api::preset::validate_archive_params;
use ttzip_engine::uniffi_api::types::TTZipArchiveFormat;

// UniFFI Scaffolding C-ABI declarations exported by ttzip_engine.
extern "C" {
    fn ffi_ttzip_engine_uniffi_contract_version() -> u32;

    // Core facade functions
    fn uniffi_ttzip_engine_checksum_func_create_archive_advanced() -> u16;
    fn uniffi_ttzip_engine_checksum_func_create_archive_stream() -> u16;
    fn uniffi_ttzip_engine_checksum_func_detect_archive_format() -> u16;
    fn uniffi_ttzip_engine_checksum_func_estimate_shannon_entropy() -> u16;
    fn uniffi_ttzip_engine_checksum_func_extract_archive_stream() -> u16;
    fn uniffi_ttzip_engine_checksum_func_sweep_stale_staging() -> u16;
    fn uniffi_ttzip_engine_checksum_func_extract_single_entry_stream() -> u16;
    fn uniffi_ttzip_engine_checksum_func_extract_single_entry_stream_guarded() -> u16;
    fn uniffi_ttzip_engine_checksum_func_in_place_mutate_archive() -> u16;
    fn uniffi_ttzip_engine_checksum_func_inspect_archive_entries() -> u16;
    fn uniffi_ttzip_engine_checksum_func_load_archive_vfs_tree() -> u16;
    fn uniffi_ttzip_engine_checksum_func_open_vfs_session() -> u16;
    fn uniffi_ttzip_engine_checksum_func_open_virtual_file_stream() -> u16;
    fn uniffi_ttzip_engine_checksum_func_recommend_codec() -> u16;
    fn uniffi_ttzip_engine_checksum_func_recover_archive_password() -> u16;
    fn uniffi_ttzip_engine_checksum_func_repair_archive_file() -> u16;
    fn uniffi_ttzip_engine_checksum_func_sniff_format_buffer() -> u16;
    fn uniffi_ttzip_engine_checksum_func_sniff_format_file() -> u16;
    fn uniffi_ttzip_engine_checksum_func_validate_archive_params() -> u16;
    fn uniffi_ttzip_engine_checksum_func_verify_archive_integrity() -> u16;

    // VfsSession methods
    fn uniffi_ttzip_engine_checksum_method_vfssession_get_children_paged() -> u16;
    fn uniffi_ttzip_engine_checksum_method_vfssession_get_node_detail() -> u16;
    fn uniffi_ttzip_engine_checksum_method_vfssession_resolve_path() -> u16;
    fn uniffi_ttzip_engine_checksum_method_vfssession_search_top_k() -> u16;
    fn uniffi_ttzip_engine_checksum_method_vfssession_total_nodes() -> u16;
}

const EXPECTED_CONTRACT_VERSION: u32 = 26;

/// Canonical exported symbol checksum expectations table.
struct SymbolChecksumEntry {
    name: &'static str,
    checksum_fn: unsafe extern "C" fn() -> u16,
    expected_value: u16,
}

static CANONICAL_SYMBOL_CHECKSUMS: &[SymbolChecksumEntry] = &[
    SymbolChecksumEntry {
        name: "create_archive_advanced",
        checksum_fn: uniffi_ttzip_engine_checksum_func_create_archive_advanced,
        expected_value: 44444,
    },
    SymbolChecksumEntry {
        name: "create_archive_stream",
        checksum_fn: uniffi_ttzip_engine_checksum_func_create_archive_stream,
        expected_value: 6966,
    },
    SymbolChecksumEntry {
        name: "detect_archive_format",
        checksum_fn: uniffi_ttzip_engine_checksum_func_detect_archive_format,
        expected_value: 2812,
    },
    SymbolChecksumEntry {
        name: "estimate_shannon_entropy",
        checksum_fn: uniffi_ttzip_engine_checksum_func_estimate_shannon_entropy,
        expected_value: 5862,
    },
    SymbolChecksumEntry {
        name: "extract_archive_stream",
        checksum_fn: uniffi_ttzip_engine_checksum_func_extract_archive_stream,
        expected_value: 15312,
    },
    SymbolChecksumEntry {
        name: "sweep_stale_staging",
        checksum_fn: uniffi_ttzip_engine_checksum_func_sweep_stale_staging,
        expected_value: 45473,
    },
    SymbolChecksumEntry {
        name: "extract_single_entry_stream",
        checksum_fn: uniffi_ttzip_engine_checksum_func_extract_single_entry_stream,
        expected_value: 57921,
    },
    SymbolChecksumEntry {
        name: "extract_single_entry_stream_guarded",
        checksum_fn: uniffi_ttzip_engine_checksum_func_extract_single_entry_stream_guarded,
        expected_value: 48102,
    },
    SymbolChecksumEntry {
        name: "in_place_mutate_archive",
        checksum_fn: uniffi_ttzip_engine_checksum_func_in_place_mutate_archive,
        expected_value: 28841,
    },
    SymbolChecksumEntry {
        name: "inspect_archive_entries",
        checksum_fn: uniffi_ttzip_engine_checksum_func_inspect_archive_entries,
        expected_value: 63804,
    },
    SymbolChecksumEntry {
        name: "load_archive_vfs_tree",
        checksum_fn: uniffi_ttzip_engine_checksum_func_load_archive_vfs_tree,
        expected_value: 44693,
    },
    SymbolChecksumEntry {
        name: "open_vfs_session",
        checksum_fn: uniffi_ttzip_engine_checksum_func_open_vfs_session,
        expected_value: 51358,
    },
    SymbolChecksumEntry {
        name: "open_virtual_file_stream",
        checksum_fn: uniffi_ttzip_engine_checksum_func_open_virtual_file_stream,
        expected_value: 21449,
    },
    SymbolChecksumEntry {
        name: "recommend_codec",
        checksum_fn: uniffi_ttzip_engine_checksum_func_recommend_codec,
        expected_value: 1005,
    },
    SymbolChecksumEntry {
        name: "recover_archive_password",
        checksum_fn: uniffi_ttzip_engine_checksum_func_recover_archive_password,
        expected_value: 41198,
    },
    SymbolChecksumEntry {
        name: "repair_archive_file",
        checksum_fn: uniffi_ttzip_engine_checksum_func_repair_archive_file,
        expected_value: 26144,
    },
    SymbolChecksumEntry {
        name: "sniff_format_buffer",
        checksum_fn: uniffi_ttzip_engine_checksum_func_sniff_format_buffer,
        expected_value: 26304,
    },
    SymbolChecksumEntry {
        name: "sniff_format_file",
        checksum_fn: uniffi_ttzip_engine_checksum_func_sniff_format_file,
        expected_value: 63931,
    },
    SymbolChecksumEntry {
        name: "validate_archive_params",
        checksum_fn: uniffi_ttzip_engine_checksum_func_validate_archive_params,
        expected_value: 5518,
    },
    SymbolChecksumEntry {
        name: "verify_archive_integrity",
        checksum_fn: uniffi_ttzip_engine_checksum_func_verify_archive_integrity,
        expected_value: 56393,
    },
    SymbolChecksumEntry {
        name: "vfssession_get_children_paged",
        checksum_fn: uniffi_ttzip_engine_checksum_method_vfssession_get_children_paged,
        expected_value: 4085,
    },
    SymbolChecksumEntry {
        name: "vfssession_get_node_detail",
        checksum_fn: uniffi_ttzip_engine_checksum_method_vfssession_get_node_detail,
        expected_value: 25579,
    },
    SymbolChecksumEntry {
        name: "vfssession_resolve_path",
        checksum_fn: uniffi_ttzip_engine_checksum_method_vfssession_resolve_path,
        expected_value: 42545,
    },
    SymbolChecksumEntry {
        name: "vfssession_search_top_k",
        checksum_fn: uniffi_ttzip_engine_checksum_method_vfssession_search_top_k,
        expected_value: 47926,
    },
    SymbolChecksumEntry {
        name: "vfssession_total_nodes",
        checksum_fn: uniffi_ttzip_engine_checksum_method_vfssession_total_nodes,
        expected_value: 46657,
    },
];

const EXPECTED_AGGREGATE_SHA256: &str =
    "858deb19bc7ed47c0f667367b5f955610b452750e3ad08f20d5c2c820931bf79";

#[test]
fn test_uniffi_contract_version() {
    let actual_version = unsafe { ffi_ttzip_engine_uniffi_contract_version() };
    assert_eq!(
        actual_version, EXPECTED_CONTRACT_VERSION,
        "UniFFI contract version mismatch: expected {}, got {}",
        EXPECTED_CONTRACT_VERSION, actual_version
    );
}

#[test]
fn test_core_facade_function_checksums() {
    let mut hasher = Sha256::new();

    for entry in CANONICAL_SYMBOL_CHECKSUMS {
        let actual = unsafe { (entry.checksum_fn)() };
        assert_eq!(
            actual, entry.expected_value,
            "Checksum drift detected in UniFFI symbol '{}': expected {}, got {}. \
             Update bindings and downstream host contracts if this signature change was intentional.",
            entry.name, entry.expected_value, actual
        );

        let line = format!("{}:{}\n", entry.name, actual);
        hasher.update(line.as_bytes());
    }

    let digest = hasher.finalize();
    let actual_hex = hex::encode(digest);
    assert_eq!(
        actual_hex, EXPECTED_AGGREGATE_SHA256,
        "Aggregate SHA-256 fingerprint mismatch for UniFFI symbols table: expected {}, got {}",
        EXPECTED_AGGREGATE_SHA256, actual_hex
    );
}

#[test]
fn test_stateless_facade_functional_contracts() {
    // 1. Sniff format buffer
    let zip_magic = b"PK\x03\x04\x14\x00\x00\x00\x00\x00";
    let meta = sniff_format_buffer(zip_magic.to_vec(), Some("archive.zip".to_string()));
    assert!(meta.is_archive, "Expected sniffed buffer to be recognized as an archive");
    assert!(meta.format_name.contains("ZIP Archive"), "Expected ZIP Archive: got {}", meta.format_name);

    // 2. Estimate Shannon entropy
    let uniform_zeros = vec![0u8; 1024];
    let zero_entropy = estimate_shannon_entropy(uniform_zeros);
    assert_eq!(zero_entropy, 0.0, "Zero-filled buffer must have exactly 0.0 entropy");

    let pseudo_random: Vec<u8> = (0..=255).cycle().take(2560).collect();
    let high_entropy = estimate_shannon_entropy(pseudo_random);
    assert!(high_entropy > 7.9, "Uniform byte sequence must have near-maximal entropy: got {}", high_entropy);

    // 3. Recommend codec algorithm
    let cold_storage_rec = recommend_codec(vec![0xAA; 512], 2);
    assert!(!cold_storage_rec.is_empty(), "Codec recommendation must not be empty");

    // 4. Validate create archive params
    let validation = validate_archive_params(TTZipArchiveFormat::Zip, 6, true, true);
    assert!(validation.is_ok(), "Normal ZIP creation parameters must validate successfully");
}
