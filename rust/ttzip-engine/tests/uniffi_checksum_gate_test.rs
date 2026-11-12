// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Mozilla UniFFI API Checksum & Signature Gate Integration Test.
//!
//! Verifies ABI contract stability, symbol checksum parity, and aggregate
//! SHA-256 fingerprint consistency across all exported UniFFI functions and methods.
//! This gate prevents silent interface drift between Rust core logic and multi-host
//! foreign bindings (macOS Swift 6, Windows WinUI 3 C#/C++, and Linux Qt 6/Slint C++).

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use ttzip_engine::uniffi_api::archive::{
    estimate_shannon_entropy, recommend_codec, sniff_format_buffer,
};
use ttzip_engine::uniffi_api::preset::validate_archive_params;
use ttzip_engine::uniffi_api::types::TTZipArchiveFormat;

#[path = "common/uniffi_all_checksums.rs"]
mod uniffi_all_checksums;
use uniffi_all_checksums::*;

const SWIFT_BINDINGS_SRC: &str =
    include_str!("../../../Sources/TTZipCore/Generated/ttzip_engine.swift");
const PYTHON_BINDINGS_SRC: &str =
    include_str!("../../../sdk/python/ttzip/ttzip_engine.py");
const GENERATED_FFI_HEADER_SRC: &str =
    include_str!("../../../Sources/TTZipCore/Generated/ttzip_engineFFI.h");
const ENGINE_FFI_HEADER_SRC: &str =
    include_str!("../../../Sources/TTZipEngineFFI/include/ttzip_engineFFI.h");
const BRIDGE_FFI_HEADER_SRC: &str =
    include_str!("../../../Sources/CTTZipBridge/include/ttzip_engineFFI.h");

// UniFFI Scaffolding C-ABI contract version exported by ttzip_engine.
unsafe extern "C" {
    fn ffi_ttzip_engine_uniffi_contract_version() -> u32;
}

const EXPECTED_CONTRACT_VERSION: u32 = 26;
const EXPECTED_UNIFFI_SYMBOL_COUNT: usize = 359;

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
    SymbolChecksumEntry {
        name: "compute_file_stream_hash",
        checksum_fn: uniffi_ttzip_engine_checksum_func_compute_file_stream_hash,
        expected_value: 21618,
    },
    SymbolChecksumEntry {
        name: "extractionsession_preflight",
        checksum_fn: uniffi_ttzip_engine_checksum_method_extractionsession_preflight,
        expected_value: 51770,
    },
];

const EXPECTED_AGGREGATE_SHA256: &str =
    "b848b930b3f3bf6f16e23bf3824e4ea0f6379e2eec739d9b493ba8bfb3e26be8";

fn live_rust_checksum_map() -> BTreeMap<&'static str, u16> {
    assert_eq!(
        ALL_UNIFFI_ENGINE_CHECKSUMS.len(),
        EXPECTED_UNIFFI_SYMBOL_COUNT,
        "Expected {} entries in ALL_UNIFFI_ENGINE_CHECKSUMS, got {}",
        EXPECTED_UNIFFI_SYMBOL_COUNT,
        ALL_UNIFFI_ENGINE_CHECKSUMS.len()
    );
    let mut map = BTreeMap::new();
    for &(sym, func) in ALL_UNIFFI_ENGINE_CHECKSUMS {
        let prev = map.insert(sym, unsafe { func() });
        assert!(prev.is_none(), "Duplicate UniFFI symbol entry: {}", sym);
    }
    map
}

fn parse_delimited_checksums<'a>(
    src: &'a str,
    prefix: &str,
    suffix: &str,
) -> BTreeMap<&'a str, u16> {
    let mut map = BTreeMap::new();
    for line in src.lines() {
        let Some(rest) = line.trim().strip_prefix(prefix) else {
            continue;
        };
        let Some((sym, rhs)) = rest.split_once("() != ") else {
            continue;
        };
        if !sym.starts_with("uniffi_ttzip_engine_checksum_") {
            continue;
        }
        let Some(num_str) = rhs.strip_suffix(suffix) else {
            continue;
        };
        let value: u16 = num_str
            .parse()
            .expect("UniFFI checksum literal must be a valid u16");
        map.insert(sym, value);
    }
    map
}

fn parse_c_header_checksum_symbols(src: &str) -> BTreeSet<&str> {
    src.lines()
        .filter_map(|line| {
            let sym = line.trim().strip_prefix("uint16_t ")?.strip_suffix("(void")?;
            sym.starts_with("uniffi_ttzip_engine_checksum_").then_some(sym)
        })
        .collect()
}

fn assert_checksum_map_parity(
    label: &str,
    rust_map: &BTreeMap<&str, u16>,
    binding_map: &BTreeMap<&str, u16>,
) {
    let missing: Vec<&str> = rust_map
        .keys()
        .copied()
        .filter(|k| !binding_map.contains_key(k))
        .collect();
    let stale: Vec<&str> = binding_map
        .keys()
        .copied()
        .filter(|k| !rust_map.contains_key(k))
        .collect();
    let mismatches: Vec<String> = rust_map
        .iter()
        .filter_map(|(&sym, &rust_val)| {
            let &binding_val = binding_map.get(sym)?;
            (binding_val != rust_val)
                .then(|| format!("{}: {}={} != rust={}", sym, label, binding_val, rust_val))
        })
        .collect();

    assert!(
        binding_map.len() == EXPECTED_UNIFFI_SYMBOL_COUNT
            && missing.is_empty()
            && stale.is_empty()
            && mismatches.is_empty(),
        "{} UniFFI checksum parity failure (total={}, expected={}): missing={} {:?}, stale={} {:?}, mismatches={} {:?}",
        label,
        binding_map.len(),
        EXPECTED_UNIFFI_SYMBOL_COUNT,
        missing.len(),
        missing,
        stale.len(),
        stale,
        mismatches.len(),
        mismatches
    );
}

#[test]
fn test_swift_generated_bindings_checksum_parity() {
    let rust_map = live_rust_checksum_map();
    let swift_map = parse_delimited_checksums(SWIFT_BINDINGS_SRC, "if (", ") {");
    assert_checksum_map_parity("ttzip_engine.swift", &rust_map, &swift_map);
}

#[test]
fn test_python_sdk_bindings_checksum_parity() {
    let rust_map = live_rust_checksum_map();
    let python_map = parse_delimited_checksums(PYTHON_BINDINGS_SRC, "if lib.", ":");
    assert_checksum_map_parity("ttzip_engine.py", &rust_map, &python_map);
}

#[test]
fn test_c_ffi_headers_checksum_symbol_set_parity() {
    let rust_map = live_rust_checksum_map();
    let expected_set: BTreeSet<&str> = rust_map.keys().copied().collect();
    for (label, src) in [
        ("Sources/TTZipCore/Generated/ttzip_engineFFI.h", GENERATED_FFI_HEADER_SRC),
        ("Sources/TTZipEngineFFI/include/ttzip_engineFFI.h", ENGINE_FFI_HEADER_SRC),
        ("Sources/CTTZipBridge/include/ttzip_engineFFI.h", BRIDGE_FFI_HEADER_SRC),
    ] {
        let header_set = parse_c_header_checksum_symbols(src);
        assert_eq!(
            header_set.len(),
            EXPECTED_UNIFFI_SYMBOL_COUNT,
            "Expected {} checksum declarations in {}, got {}",
            EXPECTED_UNIFFI_SYMBOL_COUNT,
            label,
            header_set.len()
        );
        assert_eq!(
            header_set, expected_set,
            "C FFI header checksum symbol set mismatch in {}",
            label
        );
    }
}

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

#[test]
fn test_u1_create_transaction_incomplete_uniffi_schema_surface() {
    use ttzip_engine::uniffi_api::types::{
        TTZipError, UniFFIArchiveCleanupDisposition, UniFFICreateBackupRecoveryState,
        UniFFICreateTransactionFailureReport, UniFFIDescriptorReleaseStatus,
        UniFFIResidualPathRole, UniFFIStageCleanupFailureItem, UniFFIStageFileFinishDiagnostic,
    };

    let report = UniFFICreateTransactionFailureReport {
        primary_status_code: -23,
        was_primary_cancelled: true,
        is_cancellation_requested: true,
        stage_dir: "/tmp/.ttzip_create_txn_1".to_string(),
        stage_dir_preserved: true,
        unconfirmed_release_count: 1,
        cleanup_failures: vec![UniFFIStageCleanupFailureItem {
            role: UniFFIResidualPathRole::StagedArtifact,
            disposition: UniFFIArchiveCleanupDisposition::DescriptorReleaseUnconfirmed,
            path: None,
            original_staged_path: Some("/tmp/.ttzip_create_txn_1/out.zip".to_string()),
            os_error_code: Some(4),
            message: "close(2) returned EINTR".to_string(),
        }],
        finish_diagnostics: vec![UniFFIStageFileFinishDiagnostic {
            role: UniFFIResidualPathRole::StagedArtifact,
            diagnostic_path: "/tmp/.ttzip_create_txn_1/out.zip".to_string(),
            expected_dev: 1,
            expected_ino: 2,
            primary_status_code: -15,
            prior_write_or_encoder_error: None,
            userspace_flush_os_error: None,
            userspace_flush_detail: None,
            sync_os_error: None,
            sync_detail: None,
            close_os_error: Some(4),
            close_detail: Some("Interrupted system call".to_string()),
            release_status: UniFFIDescriptorReleaseStatus::ReleaseUnconfirmed,
        }],
        recovery_state: UniFFICreateBackupRecoveryState::NotApplicable,
        backup_dir: None,
        restored_targets: vec![],
        rollback_failed_entries: vec![],
        backup_dir_cleanup_error: None,
        summary_message: "incomplete".to_string(),
    };
    let err = TTZipError::CreateTransactionIncomplete {
        report: Box::new(report),
        summary_message: "incomplete".to_string(),
    };
    assert!(err.to_string().contains("incomplete"));
}
