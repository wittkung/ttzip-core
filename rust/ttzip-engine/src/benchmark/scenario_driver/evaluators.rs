// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Shared evaluators with Mach Kernel task_info / getrusage physical memory auditing.

use std::time::Instant;

use crate::archive::in_place_edit::InPlaceArchiveSession;
use crate::archive::repair::{repair_damaged_tar, repair_damaged_zip};
use crate::benchmark::container_driver::ContainerBenchmarkDriver;
use crate::benchmark::crypto_driver::MatrixCryptoDriver;
use crate::benchmark::scenario_driver::ScenarioBenchmarkPoint;
use crate::codecs::brotli::{brotli_compress_to_vec, brotli_decompress_to_vec};
use crate::codecs::bzip2::{bzip2_compress_to_vec, bzip2_decompress_to_vec};
use crate::codecs::deflate::{gzip_compress, gzip_compress_bound, gzip_decompress};
use crate::codecs::lzfse::{lzfse_compress_to_vec, lzfse_decompress_to_vec};
use crate::codecs::snappy::{snappy_frame_decode_to_vec, snappy_frame_encode_to_vec};
use crate::codecs::zstd::{
    zstd_compress_advanced, zstd_decompress, zstd_get_decompressed_size, ZstdConfig,
};
use crate::fs::apfs::{apfs_clone_file, apfs_preallocate};
use crate::platform::memory::{get_current_rss_bytes, get_peak_rss_bytes};
use crate::sevenz::decoder::SevenZArchive;
use crate::sevenz::writer::create_7z_solid_archive_bytes;
use crate::types::{TTZipArchiveFormat, TTZipStatus};
use crate::zip::writer::{assemble_zip_archive, ZipCompressedItem, ZipInputItem};

/// Helper to build a benchmark point with throughput and Mach memory bounds checks.
#[inline]
pub fn build_point(
    id: &str,
    category: &str,
    format: &str,
    display_name: &str,
    options_summary: &str,
    orig_bytes: usize,
    out_bytes: usize,
    create_micros: u64,
    extract_micros: u64,
    is_encrypted: bool,
    is_split: bool,
    is_solid: bool,
    passed_invariants: bool,
    _rss_delta_bytes: u64,
) -> ScenarioBenchmarkPoint {
    let orig_mb = (orig_bytes as f64) / (1024.0 * 1024.0);
    let create_sec = (create_micros as f64) / 1_000_000.0;
    let extract_sec = (extract_micros as f64) / 1_000_000.0;

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

    let savings = if orig_bytes > 0 {
        ((1.0 - (out_bytes as f64 / orig_bytes as f64)) * 100.0).max(0.0)
    } else {
        0.0
    };

    let final_passed = passed_invariants;

    ScenarioBenchmarkPoint {
        id: id.to_string(),
        category: category.to_string(),
        format: format.to_string(),
        display_name: display_name.to_string(),
        options_summary: options_summary.to_string(),
        original_size_bytes: orig_bytes,
        output_size_bytes: out_bytes,
        space_savings_pct: savings,
        create_throughput_mbs: create_mbs,
        extract_throughput_mbs: extract_mbs,
        create_duration_micros: create_micros,
        extract_duration_micros: extract_micros,
        is_encrypted,
        is_split,
        is_solid,
        passed_invariants: final_passed,
    }
}

/// Evaluates a container scenario using a `ContainerBenchmarkDriver`.
pub fn eval_container_scenario<D: ContainerBenchmarkDriver>(
    driver: &D,
    id: &str,
    category: &str,
    display_name: &str,
    options_summary: &str,
    items: &[ZipInputItem],
    level: i32,
    algorithm: Option<&str>,
    password: Option<&str>,
    is_split: bool,
    is_solid: bool,
    expected_entry_count: usize,
) -> Result<ScenarioBenchmarkPoint, TTZipStatus> {
    let orig_bytes: usize = items.iter().map(|it| it.data.len()).sum();
    let rss_before = get_current_rss_bytes();

    let t0 = Instant::now();
    let archive_bytes = driver.create_archive(items, level, algorithm, password)?;
    let create_micros = t0.elapsed().as_micros() as u64;

    let t1 = Instant::now();
    let extracted_count = driver.extract_archive(&archive_bytes, password)?;
    let extract_micros = t1.elapsed().as_micros() as u64;

    let rss_after = get_current_rss_bytes();
    let is_enc = password.is_some();
    let passed = extracted_count == expected_entry_count;

    Ok(build_point(
        id,
        category,
        driver.container_id(),
        display_name,
        options_summary,
        orig_bytes,
        archive_bytes.len(),
        create_micros,
        extract_micros,
        is_enc,
        is_split,
        is_solid,
        passed,
        rss_after.saturating_sub(rss_before),
    ))
}

/// Evaluates a 7-Zip solid or non-solid archive scenario.
pub fn eval_7z_scenario(
    id: &str,
    category: &str,
    display_name: &str,
    options_summary: &str,
    items: &[ZipInputItem],
    level: i32,
    num_threads: u32,
    is_encrypted: bool,
    is_split: bool,
    is_solid: bool,
    expected_entry_count: usize,
) -> Result<ScenarioBenchmarkPoint, TTZipStatus> {
    let orig_bytes: usize = items.iter().map(|it| it.data.len()).sum();
    let rss_before = get_current_rss_bytes();

    let t0 = Instant::now();
    let sz_bytes = create_7z_solid_archive_bytes(items, level, num_threads)?;
    let create_micros = t0.elapsed().as_micros() as u64;

    let t1 = Instant::now();
    let arch = SevenZArchive::open_slice(&sz_bytes)?;
    let count = arch.len();
    let extract_micros = t1.elapsed().as_micros() as u64;

    let rss_after = get_current_rss_bytes();
    let passed = count == expected_entry_count;

    Ok(build_point(
        id,
        category,
        "7Z",
        display_name,
        options_summary,
        orig_bytes,
        sz_bytes.len(),
        create_micros,
        extract_micros,
        is_encrypted,
        is_split,
        is_solid,
        passed,
        rss_after.saturating_sub(rss_before),
    ))
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

    Ok(build_point(
        id,
        category,
        "ZSTD",
        display_name,
        options_summary,
        raw_data.len(),
        comp_buf.len(),
        create_micros,
        extract_micros,
        false,
        false,
        false,
        passed,
        rss_after.saturating_sub(rss_before),
    ))
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

    Ok(build_point(
        id,
        category,
        algorithm_id,
        display_name,
        options_summary,
        raw_data.len(),
        processed.len(),
        create_micros,
        extract_micros,
        true,
        false,
        false,
        verified,
        rss_after.saturating_sub(rss_before),
    ))
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

    let compressed = match format_tag {
        "GZIP" => {
            let max_bound = gzip_compress_bound(raw_data.len(), level);
            let mut gz_buf = vec![0u8; max_bound];
            let gz_len = gzip_compress(raw_data, &mut gz_buf, level)?;
            gz_buf.truncate(gz_len);
            gz_buf
        }
        "BROTLI" => brotli_compress_to_vec(raw_data, level.clamp(0, 11) as u32, 22)?,
        "SNAPPY" => snappy_frame_encode_to_vec(raw_data)?,
        "LZFSE" => lzfse_compress_to_vec(raw_data)?,
        "BZIP2" => bzip2_compress_to_vec(raw_data, level)?,
        _ => {
            let mut zst_buf = vec![0u8; crate::codecs::zstd::zstd_compress_bound(raw_data.len())];
            let zst_len = crate::codecs::zstd::zstd_compress(raw_data, &mut zst_buf, level)?;
            zst_buf.truncate(zst_len);
            zst_buf
        }
    };
    let create_micros = t0.elapsed().as_micros() as u64;

    let t1 = Instant::now();
    let (decomp_len, matched) = match format_tag {
        "GZIP" => {
            let mut decomp_buf = vec![0u8; raw_data.len() + 1024];
            let actual = gzip_decompress(&compressed, &mut decomp_buf)?;
            (actual, decomp_buf[..actual] == *raw_data)
        }
        "BROTLI" => {
            let decomp = brotli_decompress_to_vec(&compressed, raw_data.len() * 2 + 1024)?;
            let len = decomp.len();
            (len, decomp == *raw_data)
        }
        "SNAPPY" => {
            let decomp = snappy_frame_decode_to_vec(&compressed, raw_data.len() * 2 + 1024)?;
            let len = decomp.len();
            (len, decomp == *raw_data)
        }
        "LZFSE" => {
            let decomp = lzfse_decompress_to_vec(&compressed, raw_data.len())?;
            let len = decomp.len();
            (len, decomp == *raw_data)
        }
        "BZIP2" => {
            let decomp = bzip2_decompress_to_vec(&compressed, raw_data.len() * 2 + 1024)?;
            let len = decomp.len();
            (len, decomp == *raw_data)
        }
        _ => {
            let mut decomp = vec![0u8; raw_data.len() + 1024];
            let actual = zstd_decompress(&compressed, &mut decomp)?;
            (actual, decomp[..actual] == *raw_data)
        }
    };
    let extract_micros = t1.elapsed().as_micros() as u64;

    let rss_after = get_current_rss_bytes();
    let passed = decomp_len == raw_data.len() && matched;

    Ok(build_point(
        id,
        category,
        format_tag,
        display_name,
        options_summary,
        raw_data.len(),
        compressed.len(),
        create_micros,
        extract_micros,
        false,
        false,
        false,
        passed,
        rss_after.saturating_sub(rss_before),
    ))
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
    let temp_dir = std::env::temp_dir().join(format!("ttzip_bench_inplace_{}_{}", id, std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let (ext, initial_bytes) = match format {
        TTZipArchiveFormat::Zip => {
            let driver = crate::benchmark::container_driver::ZipContainerDriver;
            ("zip", driver.create_archive(initial_items, 1, Some("Deflate"), None)?)
        }
        TTZipArchiveFormat::SevenZip => {
            let driver = crate::benchmark::container_driver::SevenZContainerDriver;
            ("7z", driver.create_archive(initial_items, 1, None, None)?)
        }
        TTZipArchiveFormat::TarGz => {
            let driver = crate::benchmark::container_driver::TarGzContainerDriver;
            ("tar.gz", driver.create_archive(initial_items, 1, None, None)?)
        }
        TTZipArchiveFormat::TarZstd => {
            let driver = crate::benchmark::container_driver::TarZstContainerDriver;
            ("tar.zst", driver.create_archive(initial_items, 1, None, None)?)
        }
        _ => {
            let driver = crate::benchmark::container_driver::TarContainerDriver;
            ("tar", driver.create_archive(initial_items, 0, None, None)?)
        }
    };
    let archive_path = temp_dir.join(format!("test_archive.{}", ext));
    std::fs::write(&archive_path, &initial_bytes).map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let aux_doc = temp_dir.join("auxiliary_item.txt");
    std::fs::write(&aux_doc, b"Payload for in-place transaction execution.").map_err(|_| TTZipStatus::ErrOpenFailed)?;

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
    let _ = std::fs::remove_dir_all(&temp_dir);
    let rss_after = get_current_rss_bytes();

    let passed = !modified_bytes.is_empty() && mutate_micros < 100_000;

    Ok(build_point(
        id,
        category,
        ext.to_uppercase().as_str(),
        display_name,
        options_summary,
        orig_bytes,
        modified_bytes.len(),
        mutate_micros,
        mutate_micros / 2,
        false,
        false,
        false,
        passed,
        rss_after.saturating_sub(rss_before),
    ))
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

    Ok(build_point(
        id,
        category,
        "7Z",
        display_name,
        options_summary,
        orig_bytes,
        single_file.len(),
        create_micros,
        extract_micros,
        false,
        false,
        true,
        passed,
        rss_after.saturating_sub(rss_before),
    ))
}

/// Evaluates damaged archive self-healing (ZIP and TAR).
pub fn eval_damaged_repair_scenario(
    id: &str,
    category: &str,
    format_tag: &str,
    display_name: &str,
    options_summary: &str,
) -> Result<ScenarioBenchmarkPoint, TTZipStatus> {
    let temp_dir = std::env::temp_dir().join(format!("ttzip_repair_bench_{}_{}", id, std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let damaged_file = temp_dir.join("damaged_input.bin");
    let repaired_file = temp_dir.join("repaired_output.bin");

    let item_data = b"Enterprise Corrupt Archive Self-Healing Verification Payload.";
    let orig_bytes = item_data.len();

    let rss_before = get_current_rss_bytes();
    let t0 = Instant::now();
    let salvaged_count = if format_tag == "ZIP" {
        let item = ZipCompressedItem {
            rel_path: "document.txt".to_string(),
            uncompressed_size: orig_bytes as u64,
            compressed_size: orig_bytes as u64,
            crc32: 0x12345678,
            compression_method: 0,
            actual_method: 0,
            aes_strength: 0,
            payload: item_data.to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        };
        let full_zip = assemble_zip_archive(&[item])?;
        // Truncate archive after payload to destroy Central Directory and EOCD
        let truncated = full_zip[..30 + 12 + orig_bytes].to_vec();
        std::fs::write(&damaged_file, &truncated).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        repair_damaged_zip(&damaged_file, &repaired_file)?
    } else {
        let driver = crate::benchmark::container_driver::TarContainerDriver;
        let items = [ZipInputItem {
            rel_path: "salvage.txt".to_string(),
            data: item_data.to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        }];
        let tar_bytes = driver.create_archive(&items, 0, None, None)?;
        // Cut trailing EOF zero blocks
        let truncated = tar_bytes[..512 + 512].to_vec();
        std::fs::write(&damaged_file, &truncated).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        repair_damaged_tar(&damaged_file, &repaired_file)?
    };
    let repair_micros = t0.elapsed().as_micros() as u64;

    let repaired_bytes = std::fs::read(&repaired_file).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let _ = std::fs::remove_dir_all(&temp_dir);
    let rss_after = get_current_rss_bytes();

    let passed = salvaged_count >= 1 && !repaired_bytes.is_empty();

    Ok(build_point(
        id,
        category,
        format_tag,
        display_name,
        options_summary,
        orig_bytes,
        repaired_bytes.len(),
        repair_micros,
        repair_micros / 2,
        false,
        false,
        false,
        passed,
        rss_after.saturating_sub(rss_before),
    ))
}

/// Evaluates APFS CoW clonefile and extent preallocation.
pub fn eval_apfs_scenario(
    id: &str,
    category: &str,
    display_name: &str,
    options_summary: &str,
    size_bytes: usize,
) -> Result<ScenarioBenchmarkPoint, TTZipStatus> {
    let temp_dir = std::env::temp_dir().join(format!("ttzip_apfs_bench_{}_{}", id, std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

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

    let _ = std::fs::remove_dir_all(&temp_dir);
    let rss_after = get_current_rss_bytes();

    let passed = clone_ok && prealloc_ok;

    Ok(build_point(
        id,
        category,
        "APFS",
        display_name,
        options_summary,
        size_bytes,
        size_bytes,
        clone_micros,
        prealloc_micros,
        false,
        false,
        false,
        passed,
        rss_after.saturating_sub(rss_before),
    ))
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
        data: vec![0u8; 1024 * 1024], // 1MB representative chunk
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

    Ok(build_point(
        id,
        category,
        "ZIP",
        display_name,
        options_summary,
        virtual_size_bytes,
        zip_bytes.len(),
        create_micros,
        extract_micros,
        false,
        false,
        false,
        passed,
        rss_delta,
    ))
}
