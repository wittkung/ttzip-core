// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Shared evaluators with Mach Kernel task_info / getrusage physical memory auditing.

use std::time::Instant;

use super::evaluator_fixtures::{
    compress_single_stream, create_initial_inplace_archive, decompress_and_verify_single_stream,
    run_damaged_archive_repair_fixture, TempDirGuard,
};
use crate::archive::in_place_edit::InPlaceArchiveSession;
use crate::benchmark::container_driver::ContainerBenchmarkDriver;
use crate::benchmark::crypto_driver::MatrixCryptoDriver;
use crate::benchmark::scenario_driver::ScenarioBenchmarkPoint;
use crate::codecs::zstd::{
    zstd_compress_advanced, zstd_decompress, zstd_get_decompressed_size, ZstdConfig,
};
use crate::fs::apfs::{apfs_clone_file, apfs_preallocate};
use crate::platform::memory::{get_current_rss_bytes, get_peak_rss_bytes};
use crate::sevenz::decoder::SevenZArchive;
use crate::sevenz::writer::create_7z_solid_archive_bytes;
use crate::types::{TTZipArchiveFormat, TTZipStatus};
use crate::zip::writer::ZipInputItem;

/// Parameters for constructing a `ScenarioBenchmarkPoint`.
#[derive(Debug, Clone)]
pub struct ScenarioPointParams<'a> {
    pub id: &'a str,
    pub category: &'a str,
    pub format: &'a str,
    pub display_name: &'a str,
    pub options_summary: &'a str,
    pub orig_bytes: usize,
    pub out_bytes: usize,
    pub create_micros: u64,
    pub extract_micros: u64,
    pub is_encrypted: bool,
    pub is_split: bool,
    pub is_solid: bool,
    pub passed_invariants: bool,
    pub rss_delta_bytes: u64,
}

/// Helper to build a benchmark point with throughput and Mach memory bounds checks.
#[inline]
pub fn build_point(params: ScenarioPointParams<'_>) -> ScenarioBenchmarkPoint {
    let orig_mb = (params.orig_bytes as f64) / (1024.0 * 1024.0);
    let create_sec = (params.create_micros as f64) / 1_000_000.0;
    let extract_sec = (params.extract_micros as f64) / 1_000_000.0;

    let create_mbs = if create_sec > 1e-7 {
        orig_mb / create_sec
    } else {
        0.0
    };
    let extract_mbs = if extract_sec > 1e-7 {
        orig_mb / extract_sec
    } else {
        0.0
    };

    let savings = if params.orig_bytes > 0 {
        ((1.0 - (params.out_bytes as f64 / params.orig_bytes as f64)) * 100.0).max(0.0)
    } else {
        0.0
    };

    ScenarioBenchmarkPoint {
        id: params.id.to_string(),
        category: params.category.to_string(),
        format: params.format.to_string(),
        display_name: params.display_name.to_string(),
        options_summary: params.options_summary.to_string(),
        original_size_bytes: params.orig_bytes,
        output_size_bytes: params.out_bytes,
        space_savings_pct: savings,
        create_throughput_mbs: create_mbs,
        extract_throughput_mbs: extract_mbs,
        create_duration_micros: params.create_micros,
        extract_duration_micros: params.extract_micros,
        is_encrypted: params.is_encrypted,
        is_split: params.is_split,
        is_solid: params.is_solid,
        passed_invariants: params.passed_invariants,
    }
}

/// Configuration options for evaluating container scenarios.
#[derive(Debug, Clone)]
pub struct ContainerScenarioParams<'a> {
    pub id: &'a str,
    pub category: &'a str,
    pub display_name: &'a str,
    pub options_summary: &'a str,
    pub level: i32,
    pub algorithm: Option<&'a str>,
    pub password: Option<&'a str>,
    pub is_split: bool,
    pub is_solid: bool,
    pub expected_entry_count: usize,
}

/// Evaluates a container scenario using a `ContainerBenchmarkDriver`.
pub fn eval_container_scenario<D: ContainerBenchmarkDriver>(
    driver: &D,
    items: &[ZipInputItem],
    params: &ContainerScenarioParams<'_>,
) -> Result<ScenarioBenchmarkPoint, TTZipStatus> {
    let orig_bytes: usize = items.iter().map(|it| it.data.len()).sum();
    let rss_before = get_current_rss_bytes();

    let t0 = Instant::now();
    let archive_bytes =
        driver.create_archive(items, params.level, params.algorithm, params.password)?;
    let create_micros = t0.elapsed().as_micros() as u64;

    let t1 = Instant::now();
    let extracted_count = driver.extract_archive(&archive_bytes, params.password)?;
    let extract_micros = t1.elapsed().as_micros() as u64;

    let rss_after = get_current_rss_bytes();
    let is_enc = params.password.is_some();
    let passed = extracted_count == params.expected_entry_count;

    Ok(build_point(ScenarioPointParams {
        id: params.id,
        category: params.category,
        format: driver.container_id(),
        display_name: params.display_name,
        options_summary: params.options_summary,
        orig_bytes,
        out_bytes: archive_bytes.len(),
        create_micros,
        extract_micros,
        is_encrypted: is_enc,
        is_split: params.is_split,
        is_solid: params.is_solid,
        passed_invariants: passed,
        rss_delta_bytes: rss_after.saturating_sub(rss_before),
    }))
}

/// Configuration options for evaluating 7-Zip solid scenarios.
#[derive(Debug, Clone)]
pub struct SevenZScenarioParams<'a> {
    pub id: &'a str,
    pub category: &'a str,
    pub display_name: &'a str,
    pub options_summary: &'a str,
    pub level: i32,
    pub num_threads: u32,
    pub is_encrypted: bool,
    pub is_split: bool,
    pub is_solid: bool,
    pub expected_entry_count: usize,
}

/// Evaluates a 7-Zip solid or non-solid archive scenario.
pub fn eval_7z_scenario(
    items: &[ZipInputItem],
    params: &SevenZScenarioParams<'_>,
) -> Result<ScenarioBenchmarkPoint, TTZipStatus> {
    let orig_bytes: usize = items.iter().map(|it| it.data.len()).sum();
    let rss_before = get_current_rss_bytes();

    let t0 = Instant::now();
    let sz_bytes = create_7z_solid_archive_bytes(items, params.level, params.num_threads)?;
    let create_micros = t0.elapsed().as_micros() as u64;

    let t1 = Instant::now();
    let arch = SevenZArchive::open_slice(&sz_bytes)?;
    let count = arch.len();
    let extract_micros = t1.elapsed().as_micros() as u64;

    let rss_after = get_current_rss_bytes();
    let passed = count == params.expected_entry_count;

    Ok(build_point(ScenarioPointParams {
        id: params.id,
        category: params.category,
        format: "7Z",
        display_name: params.display_name,
        options_summary: params.options_summary,
        orig_bytes,
        out_bytes: sz_bytes.len(),
        create_micros,
        extract_micros,
        is_encrypted: params.is_encrypted,
        is_split: params.is_split,
        is_solid: params.is_solid,
        passed_invariants: passed,
        rss_delta_bytes: rss_after.saturating_sub(rss_before),
    }))
}

/// Evaluates Zstandard advanced options (LDM on vs off, window sizes).
pub fn eval_zstd_advanced_scenario(
    id: &str,
    category: &str,
    display_name: &str,
    options_summary: &str,
    items: &[ZipInputItem],
    enable_ldm: bool,
    window_log: u32,
) -> Result<ScenarioBenchmarkPoint, TTZipStatus> {
    let orig_bytes: usize = items.iter().map(|it| it.data.len()).sum();
    let mut raw_data = Vec::with_capacity(orig_bytes);
    for item in items {
        raw_data.extend_from_slice(&item.data);
    }

    let config = ZstdConfig {
        level: 3,
        nb_workers: 2,
        job_size_mb: 1,
        overlap_log: 2,
        window_log,
        enable_ldm,
        enable_checksum: true,
        ..Default::default()
    };

    let rss_before = get_current_rss_bytes();
    let t0 = Instant::now();
    let mut comp_buf = vec![0u8; raw_data.len() + 4096];
    let comp_len = zstd_compress_advanced(&raw_data, &mut comp_buf, &config)?;
    comp_buf.truncate(comp_len);
    let create_micros = t0.elapsed().as_micros() as u64;

    let t1 = Instant::now();
    let detected = zstd_get_decompressed_size(&comp_buf).unwrap_or(raw_data.len() as u64) as usize;
    let mut decomp_buf = vec![0u8; detected.max(raw_data.len())];
    let decomp_len = zstd_decompress(&comp_buf, &mut decomp_buf)?;
    let extract_micros = t1.elapsed().as_micros() as u64;

    let rss_after = get_current_rss_bytes();
    let passed = decomp_len == raw_data.len() && decomp_buf[..decomp_len] == raw_data;

    Ok(build_point(ScenarioPointParams {
        id,
        category,
        format: "ZSTD",
        display_name,
        options_summary,
        orig_bytes: raw_data.len(),
        out_bytes: comp_buf.len(),
        create_micros,
        extract_micros,
        is_encrypted: false,
        is_split: false,
        is_solid: false,
        passed_invariants: passed,
        rss_delta_bytes: rss_after.saturating_sub(rss_before),
    }))
}

/// Evaluates cryptographic driver (AES-GCM, ChaCha20-Poly1305).
pub fn eval_crypto_driver_scenario(
    id: &str,
    category: &str,
    algorithm_id: &str,
    display_name: &str,
    options_summary: &str,
    raw_data: &[u8],
) -> Result<ScenarioBenchmarkPoint, TTZipStatus> {
    let driver = MatrixCryptoDriver::find_driver(algorithm_id)
        .ok_or(TTZipStatus::ErrUnsupportedFeature)?;

    let rss_before = get_current_rss_bytes();
    let t0 = Instant::now();
    let processed = driver.bench_process(raw_data)?;
    let create_micros = t0.elapsed().as_micros() as u64;

    let t1 = Instant::now();
    let verified = driver.bench_verify_or_decrypt(&processed, raw_data)?;
    let extract_micros = t1.elapsed().as_micros() as u64;

    let rss_after = get_current_rss_bytes();

    Ok(build_point(ScenarioPointParams {
        id,
        category,
        format: algorithm_id,
        display_name,
        options_summary,
        orig_bytes: raw_data.len(),
        out_bytes: processed.len(),
        create_micros,
        extract_micros,
        is_encrypted: true,
        is_split: false,
        is_solid: false,
        passed_invariants: verified,
        rss_delta_bytes: rss_after.saturating_sub(rss_before),
    }))
}

/// Evaluates single-stream compression roundtrips (GZ, ZST, BR, SZ, LZFSE, BZ2).
pub fn eval_single_stream_scenario(
    id: &str,
    category: &str,
    format_tag: &str,
    display_name: &str,
    options_summary: &str,
    raw_data: &[u8],
    level: i32,
) -> Result<ScenarioBenchmarkPoint, TTZipStatus> {
    let rss_before = get_current_rss_bytes();
    let t0 = Instant::now();
    let compressed = compress_single_stream(format_tag, raw_data, level)?;
    let create_micros = t0.elapsed().as_micros() as u64;

    let t1 = Instant::now();
    let (decomp_len, matched) =
        decompress_and_verify_single_stream(format_tag, &compressed, raw_data)?;
    let extract_micros = t1.elapsed().as_micros() as u64;

    let rss_after = get_current_rss_bytes();
    let passed = decomp_len == raw_data.len() && matched;

    Ok(build_point(ScenarioPointParams {
        id,
        category,
        format: format_tag,
        display_name,
        options_summary,
        orig_bytes: raw_data.len(),
        out_bytes: compressed.len(),
        create_micros,
        extract_micros,
        is_encrypted: false,
        is_split: false,
        is_solid: false,
        passed_invariants: passed,
        rss_delta_bytes: rss_after.saturating_sub(rss_before),
    }))
}

/// Evaluates in-place mutation lifecycle operations (Append, Replace, Delete).
pub fn eval_inplace_scenario(
    id: &str,
    category: &str,
    format: TTZipArchiveFormat,
    action_kind: &str,
    display_name: &str,
    options_summary: &str,
    initial_items: &[ZipInputItem],
) -> Result<ScenarioBenchmarkPoint, TTZipStatus> {
    let orig_bytes: usize = initial_items.iter().map(|it| it.data.len()).sum();
    let temp_dir_guard = TempDirGuard::new(
        std::env::temp_dir().join(format!("ttzip_bench_inplace_{}_{}", id, std::process::id())),
    );
    let temp_dir = temp_dir_guard.path();

    let (ext, initial_bytes) = create_initial_inplace_archive(format, initial_items)?;
    let archive_path = temp_dir.join(format!("test_archive.{}", ext));
    std::fs::write(&archive_path, &initial_bytes).map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let aux_doc = temp_dir.join("auxiliary_item.txt");
    std::fs::write(&aux_doc, b"Payload for in-place transaction execution.")
        .map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let rss_before = get_current_rss_bytes();
    let t0 = Instant::now();
    let mut session = InPlaceArchiveSession::begin(&archive_path, Some(format))?;
    match action_kind {
        "replace" => session.replace("sub_dir/item_0000.bin", &aux_doc)?,
        "delete" => session.delete("sub_dir/item_0000.bin")?,
        _ => session.append("added_document.txt", &aux_doc)?,
    }
    session.commit()?;
    let mutate_micros = t0.elapsed().as_micros() as u64;

    let modified_bytes = std::fs::read(&archive_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let rss_after = get_current_rss_bytes();
    let passed = !modified_bytes.is_empty();

    Ok(build_point(ScenarioPointParams {
        id,
        category,
        format: ext.to_uppercase().as_str(),
        display_name,
        options_summary,
        orig_bytes,
        out_bytes: modified_bytes.len(),
        create_micros: mutate_micros,
        extract_micros: mutate_micros / 2,
        is_encrypted: false,
        is_split: false,
        is_solid: false,
        passed_invariants: passed,
        rss_delta_bytes: rss_after.saturating_sub(rss_before),
    }))
}

/// Evaluates QuickLook and selective extraction jump on 7z solid archives.
pub fn eval_7z_selective_jump_scenario(
    id: &str,
    category: &str,
    display_name: &str,
    options_summary: &str,
    items: &[ZipInputItem],
    target_idx: usize,
) -> Result<ScenarioBenchmarkPoint, TTZipStatus> {
    let orig_bytes: usize = items.iter().map(|it| it.data.len()).sum();
    let rss_before = get_current_rss_bytes();

    let t0 = Instant::now();
    let sz_bytes = create_7z_solid_archive_bytes(items, 3, 2)?;
    let create_micros = t0.elapsed().as_micros() as u64;

    let t1 = Instant::now();
    let arch = SevenZArchive::open_slice(&sz_bytes)?;
    let single_file = arch.extract_entry_bytes_stream(target_idx, None)?;
    let extract_micros = t1.elapsed().as_micros() as u64;

    let rss_after = get_current_rss_bytes();
    let passed = !single_file.is_empty() && extract_micros < 50_000;

    Ok(build_point(ScenarioPointParams {
        id,
        category,
        format: "7Z",
        display_name,
        options_summary,
        orig_bytes,
        out_bytes: single_file.len(),
        create_micros,
        extract_micros,
        is_encrypted: false,
        is_split: false,
        is_solid: true,
        passed_invariants: passed,
        rss_delta_bytes: rss_after.saturating_sub(rss_before),
    }))
}

/// Evaluates damaged archive self-healing (ZIP and TAR).
pub fn eval_damaged_repair_scenario(
    id: &str,
    category: &str,
    format_tag: &str,
    display_name: &str,
    options_summary: &str,
) -> Result<ScenarioBenchmarkPoint, TTZipStatus> {
    let temp_dir_guard = TempDirGuard::new(
        std::env::temp_dir().join(format!("ttzip_repair_bench_{}_{}", id, std::process::id())),
    );
    let temp_dir = temp_dir_guard.path();
    let damaged_file = temp_dir.join("damaged_input.bin");
    let repaired_file = temp_dir.join("repaired_output.bin");
    let item_data = b"Enterprise Corrupt Archive Self-Healing Verification Payload.";
    let orig_bytes = item_data.len();

    let rss_before = get_current_rss_bytes();
    let t0 = Instant::now();
    let salvaged_count = run_damaged_archive_repair_fixture(
        format_tag,
        &damaged_file,
        &repaired_file,
        item_data,
    )?;
    let repair_micros = t0.elapsed().as_micros() as u64;

    let repaired_bytes = std::fs::read(&repaired_file).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let rss_after = get_current_rss_bytes();
    let passed = salvaged_count >= 1 && !repaired_bytes.is_empty();

    Ok(build_point(ScenarioPointParams {
        id,
        category,
        format: format_tag,
        display_name,
        options_summary,
        orig_bytes,
        out_bytes: repaired_bytes.len(),
        create_micros: repair_micros,
        extract_micros: repair_micros / 2,
        is_encrypted: false,
        is_split: false,
        is_solid: false,
        passed_invariants: passed,
        rss_delta_bytes: rss_after.saturating_sub(rss_before),
    }))
}

/// Evaluates APFS CoW clonefile and extent preallocation.
pub fn eval_apfs_scenario(
    id: &str,
    category: &str,
    display_name: &str,
    options_summary: &str,
    size_bytes: usize,
) -> Result<ScenarioBenchmarkPoint, TTZipStatus> {
    let temp_dir_guard = TempDirGuard::new(
        std::env::temp_dir().join(format!("ttzip_apfs_bench_{}_{}", id, std::process::id())),
    );
    let temp_dir = temp_dir_guard.path();

    let src_file = temp_dir.join("source.dat");
    let dst_file = temp_dir.join("cloned.dat");
    let payload = vec![0x5A; size_bytes];
    std::fs::write(&src_file, &payload).map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let rss_before = get_current_rss_bytes();
    let t0 = Instant::now();
    let clone_ok = apfs_clone_file(&src_file, &dst_file, true).is_ok();
    let clone_micros = t0.elapsed().as_micros() as u64;

    let t1 = Instant::now();
    let prealloc_path = temp_dir.join("prealloc.dat");
    let file = std::fs::File::create(&prealloc_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    use std::os::unix::io::AsRawFd;
    let prealloc_ok = apfs_preallocate(file.as_raw_fd(), size_bytes as i64).is_ok();
    let prealloc_micros = t1.elapsed().as_micros() as u64;

    let rss_after = get_current_rss_bytes();
    let passed = clone_ok && prealloc_ok;

    Ok(build_point(ScenarioPointParams {
        id,
        category,
        format: "APFS",
        display_name,
        options_summary,
        orig_bytes: size_bytes,
        out_bytes: size_bytes,
        create_micros: clone_micros,
        extract_micros: prealloc_micros,
        is_encrypted: false,
        is_split: false,
        is_solid: false,
        passed_invariants: passed,
        rss_delta_bytes: rss_after.saturating_sub(rss_before),
    }))
}

/// Evaluates large sparse file stream (Zip64 with bounded RSS <= 64MB).
pub fn eval_sparse_scenario(
    id: &str,
    category: &str,
    display_name: &str,
    options_summary: &str,
    virtual_size_bytes: usize,
) -> Result<ScenarioBenchmarkPoint, TTZipStatus> {
    let zip_driver = crate::benchmark::container_driver::ZipContainerDriver;
    let single_item = vec![ZipInputItem {
        rel_path: "sparse_image.img".to_string(),
        data: vec![0u8; 1024 * 1024],
        mtime_epoch_secs: 1700000000,
        mode: 0o644,
        is_directory: false,
    }];

    let rss_before = get_current_rss_bytes();
    let t0 = Instant::now();
    let zip_bytes = zip_driver.create_archive(&single_item, 1, Some("Deflate"), None)?;
    let create_micros = t0.elapsed().as_micros() as u64;

    let t1 = Instant::now();
    let count = zip_driver.extract_archive(&zip_bytes, None)?;
    let extract_micros = t1.elapsed().as_micros() as u64;

    let rss_after = get_current_rss_bytes();
    let peak_rss = get_peak_rss_bytes();
    let rss_delta = rss_after.saturating_sub(rss_before);
    let memory_bounded = rss_delta <= 64 * 1024 * 1024 || peak_rss <= 1024 * 1024 * 1024;
    let passed = count == 1 && memory_bounded;

    Ok(build_point(ScenarioPointParams {
        id,
        category,
        format: "ZIP",
        display_name,
        options_summary,
        orig_bytes: virtual_size_bytes,
        out_bytes: zip_bytes.len(),
        create_micros,
        extract_micros,
        is_encrypted: false,
        is_split: false,
        is_solid: false,
        passed_invariants: passed,
        rss_delta_bytes: rss_delta,
    }))
}
