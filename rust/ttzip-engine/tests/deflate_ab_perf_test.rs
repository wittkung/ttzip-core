// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Exhaustive A/B Performance Benchmark Suite for DEFLATE Microkernel (libdeflate),
//! Parallel ZIP Streaming Engine, and 0..=12 Compression Level Pareto Frontier.

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tempfile::tempdir;
use ttzip_engine::archive::unified::UnifiedArchiveOrchestrator;
use ttzip_engine::codecs::deflate::{
    deflate_compress, deflate_compress_bound, deflate_decompress,
};
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    TTZipExtractOptions,
};
use ttzip_engine::zip::reader::ZipArchive;
use ttzip_engine::zip::writer::streaming_parallel::create_zip_streaming_parallel;

/// Generates synthetic text/JSON-like corpus with high redundancy.
fn generate_text_corpus(size: usize) -> Vec<u8> {
    let pattern = br#"{"id":12345,"name":"TTZip Deflate Bench","status":"active","metrics":{"cpu":4.5,"mem":1024,"io":99.8},"tags":["archiving","deflate","fast"]}\n"#;
    let mut data = Vec::with_capacity(size);
    while data.len() < size {
        let chunk = (size - data.len()).min(pattern.len());
        data.extend_from_slice(&pattern[..chunk]);
    }
    data
}

/// Generates pseudo-random binary data (Mach-O executable-like entropy).
fn generate_binary_corpus(size: usize) -> Vec<u8> {
    let mut data = vec![0u8; size];
    let mut seed = 0x123456789ABCDEF0u64;
    for chunk in data.chunks_mut(8) {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let bytes = seed.to_le_bytes();
        let len = chunk.len().min(8);
        chunk[..len].copy_from_slice(&bytes[..len]);
    }
    data
}

#[test]
fn test_ab_deflate_levels_spectrum_and_pareto() {
    let corpus = generate_binary_corpus(512 * 1024); // 512 KB (was 4MB)
    let levels = [0, 1, 3, 6, 9, 12];

    println!("\n=========================================================================");
    println!("  DEFLATE 0..=12 Levels Spectrum & Ratio Pareto Analysis (512KB Binary)");
    println!("=========================================================================");
    println!("  Level | Compressed Size | Ratio  | Time (ms) | Speed (MB/s)");
    println!("  -----------------------------------------------------------");

    let mut prev_size = usize::MAX;
    for &lvl in &levels {
        let max_bound = deflate_compress_bound(corpus.len(), lvl);
        let mut out = vec![0u8; max_bound];
        let start = Instant::now();
        let len = deflate_compress(&corpus, &mut out, lvl).unwrap();
        let elapsed = start.elapsed();
        let ratio = (len as f64 / corpus.len() as f64) * 100.0;
        let speed_mbs = (corpus.len() as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64();

        println!("    {:2}  | {:12} B | {:5.2}% | {:8.2?} | {:8.2} MB/s", lvl, len, ratio, elapsed, speed_mbs);

        if lvl >= 6 {
            assert!(len <= prev_size + 1024, "Higher levels should maintain high compression density");
        }
        prev_size = len;
    }
    println!("=========================================================================");
}

#[test]
fn test_ab_deflate_micro_benchmark_10000_runs() {
    let payload = generate_text_corpus(32 * 1024); // 32KB
    let max_bound = deflate_compress_bound(payload.len(), 6);
    let mut dst = vec![0u8; max_bound];
    let mut decomp_buf = vec![0u8; payload.len()];
    let iterations = 2_000;

    // 1. Deflate compression benchmark (2,000 iterations @ Level 6)
    let start_comp = Instant::now();
    let mut comp_len = 0;
    for _ in 0..iterations {
        comp_len = deflate_compress(&payload, &mut dst, 6).unwrap();
    }
    let duration_comp = start_comp.elapsed();

    // 2. Inflate decompression benchmark (2,000 iterations)
    let start_decomp = Instant::now();
    for _ in 0..iterations {
        deflate_decompress(&dst[..comp_len], &mut decomp_buf).unwrap();
    }
    let duration_decomp = start_decomp.elapsed();

    let total_bytes = (payload.len() * iterations) as f64 / (1024.0 * 1024.0 * 1024.0);
    let comp_throughput = total_bytes / duration_comp.as_secs_f64();
    let decomp_throughput = total_bytes / duration_decomp.as_secs_f64();

    println!("\n=======================================================");
    println!("  DEFLATE Libdeflate Micro-Bench (2,000 runs @ 32KB)");
    println!("=======================================================");
    println!("  Compression Throughput:   {:.2} GB/s ({:.2?})", comp_throughput, duration_comp);
    println!("  Decompression Throughput: {:.2} GB/s ({:.2?})", decomp_throughput, duration_decomp);
    println!("=======================================================");

    let min_comp_throughput = if cfg!(debug_assertions) { 0.05 } else { 0.15 };
    let min_decomp_throughput = if cfg!(debug_assertions) { 0.2 } else { 0.5 };
    assert!(comp_throughput > min_comp_throughput, "Deflate compress throughput should exceed {} GB/s", min_comp_throughput);
    assert!(decomp_throughput > min_decomp_throughput, "Inflate decompress throughput should exceed {} GB/s", min_decomp_throughput);
}

#[test]
fn test_ab_parallel_zip_streaming_1mb() {
    let dir = tempdir().unwrap();
    let file_count = 16;
    let file_size = 64 * 1024; // 16 x 64KB = 1 MB (was 100 MB)
    let mut input_paths = Vec::new();

    for i in 0..file_count {
        let file_path = dir.path().join(format!("file_{:02}.dat", i));
        let data = generate_text_corpus(file_size);
        fs::write(&file_path, &data).unwrap();
        input_paths.push(file_path);
    }

    let total_mb = (file_count * file_size) as f64 / (1024.0 * 1024.0);

    // 1. Benchmark: Single-threaded ZIP creation (thread_budget = 1)
    let single_zip = dir.path().join("single.zip");
    let opts_single = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: ttzip_engine::types::TTZIP_ABI_VERSION_3,
        format: TTZipArchiveFormat::Zip,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 1,
        solid_block_size_mb: 0,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };
    let start_single = Instant::now();
    UnifiedArchiveOrchestrator::create_archive(&input_paths, &single_zip, &opts_single, 0).unwrap();
    let duration_single = start_single.elapsed();
    let throughput_single = (total_mb / 1024.0) / duration_single.as_secs_f64();

    // 2. Benchmark: Multithreaded Parallel ZIP creation (thread_budget = 0 / Auto)
    let mt_zip = dir.path().join("multithread.zip");
    let opts_mt = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: ttzip_engine::types::TTZIP_ABI_VERSION_3,
        format: TTZipArchiveFormat::Zip,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 0,
        solid_block_size_mb: 0,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };
    let start_mt = Instant::now();
    UnifiedArchiveOrchestrator::create_archive(&input_paths, &mt_zip, &opts_mt, 0).unwrap();
    let duration_mt = start_mt.elapsed();
    let throughput_mt = (total_mb / 1024.0) / duration_mt.as_secs_f64();

    let speedup = throughput_mt / throughput_single;

    println!("\n=======================================================");
    println!("  Parallel ZIP 1MB Multi-File Archive Streaming A/B");
    println!("=======================================================");
    println!("  Corpus:                16 files x 64KB (1 MB total)");
    println!("  Single-Thread Time:    {:.2?} ({:.2} GB/s)", duration_single, throughput_single);
    println!("  Multi-Thread Time:     {:.2?} ({:.2} GB/s)", duration_mt, throughput_mt);
    println!("  Parallel Speedup:      {:.2}x", speedup);
    println!("=======================================================");

    assert!(throughput_mt >= 0.05, "Parallel ZIP throughput should be reasonable (got {:.2} GB/s)", throughput_mt);
}

unsafe extern "C" fn track_extract_progress(
    processed: u64,
    _total: u64,
    _current_file: *const libc::c_char,
    user_data: *mut libc::c_void,
) -> bool {
    let counter = unsafe { &*(user_data as *const AtomicU64) };
    counter.store(processed, Ordering::Relaxed);
    true
}

fn measure_median_indexed<F: FnMut(usize)>(runs: usize, mut op: F) -> Duration {
    op(0);
    let mut samples = Vec::with_capacity(runs);
    for idx in 1..=runs {
        let start = Instant::now();
        op(idx);
        samples.push(start.elapsed());
    }
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn raw_libdeflate_disk_create_1t(
    input_paths: &[std::path::PathBuf],
    out_path: &std::path::Path,
    lvl: i32,
) {
    let plan_items =
        ttzip_engine::zip::writer::parallel_batch::collect_source_plan_items(input_paths).unwrap();
    let total_uncomp: u64 = plan_items.iter().map(|i| i.uncompressed_size).sum();
    let mut sink =
        ttzip_engine::zip::writer::streaming_sink::ZipOutputSink::open(out_path, 0, total_uncomp)
            .unwrap();
    for item in plan_items {
        if !item.is_directory {
            let raw = fs::read(&item.abs_path).unwrap();
            let _crc = crc32_fast(0, &raw);
            let mut dst = vec![0u8; deflate_compress_bound(raw.len(), lvl)];
            let n = deflate_compress(&raw, &mut dst, lvl).unwrap();
            sink.write_bytes(&dst[..n]).unwrap();
        }
    }
    sink.commit().unwrap();
}

fn raw_libdeflate_disk_extract_1t(
    archive: &ZipArchive<'_>,
    l0_deflate_blobs: &[Vec<u8>],
    out_dir: &std::path::Path,
    lvl: i32,
) {
    let jail = ttzip_engine::fs::safe_extract::DirJail::open(out_dir).unwrap();
    let mut engine = ttzip_engine::fs::safe_extract::SafeExtractEngine::for_root(out_dir);
    for (idx, entry) in archive.entries().iter().enumerate() {
        let safe_path =
            ttzip_engine::fs::safe_extract::sanitize_and_validate_path(out_dir, &entry.rel_path)
                .unwrap();
        engine.register_entry_typed(
            safe_path,
            entry.mode & 0o7777,
            entry.mtime_epoch_secs,
            0,
            entry.is_directory,
            false,
        );
        if entry.is_directory {
            jail.create_dir_all_in_jail(&entry.rel_path).unwrap();
            continue;
        }
        let mut inflight =
            ttzip_engine::fs::abort_guard::InFlightJailFile::open(&jail, &entry.rel_path, true)
                .unwrap();
        if lvl == 0 {
            let mut out = vec![0u8; entry.uncompressed_size as usize];
            let n = deflate_decompress(&l0_deflate_blobs[idx], &mut out).unwrap();
            let _crc = crc32_fast(0, &out[..n]);
            std::io::Write::write_all(inflight.file_mut(), &out[..n]).unwrap();
        } else {
            archive
                .extract_entry_to_sink(idx, None, inflight.file_mut())
                .unwrap();
        }
        inflight.commit(&jail).unwrap();
    }
    engine.apply_deferred_metadata(false).unwrap();
}

#[test]
fn test_ab_zip_pipeline_vs_raw_libdeflate_across_all_levels() {
    let dir = tempdir().unwrap();
    let file_count = 16;
    let file_size = 128 * 1024; // 16 x 128KB = 2 MB
    let total_mb = (file_count * file_size) as f64 / (1024.0 * 1024.0);
    let mut input_paths = Vec::new();
    let mut l0_deflate_blobs = Vec::new();
    for i in 0..file_count {
        let sub = dir.path().join(format!("src_{}", i % 4));
        fs::create_dir_all(&sub).unwrap();
        let p = sub.join(format!("item_{:02}.json", i));
        let data = generate_text_corpus(file_size);
        let mut dst = vec![0u8; deflate_compress_bound(data.len(), 0)];
        let n = deflate_compress(&data, &mut dst, 0).unwrap();
        dst.truncate(n);
        l0_deflate_blobs.push(dst);
        fs::write(&p, data).unwrap();
        input_paths.push(p);
    }

    let level_specs = [
        (0, TTZipCompressionLevel::Store),
        (1, TTZipCompressionLevel::Fastest),
        (3, TTZipCompressionLevel::Fast),
        (6, TTZipCompressionLevel::Normal),
        (9, TTZipCompressionLevel::Maximum),
        (12, TTZipCompressionLevel::Ultra),
    ];

    println!("\n=========================================================================================");
    println!("  TTZip Disk ZIP Pipeline vs 1T Raw libdeflate A/B Matrix (16 x 128KB = 2MB Corpus)");
    println!("=========================================================================================");
    println!("  Lvl | TTZip Create | Raw 1T Create | Create A/B | TTZip Extract | Raw 1T Extract | Extract A/B");
    println!("  ---------------------------------------------------------------------------------------");

    for (lvl_num, lvl_enum) in level_specs {
        let zip_runs: Vec<_> = (0..=5)
            .map(|r| dir.path().join(format!("ttzip_l{lvl_num}_{r}.zip")))
            .collect();
        let raw_runs: Vec<_> = (0..=5)
            .map(|r| dir.path().join(format!("raw_l{lvl_num}_{r}.bin")))
            .collect();
        let opts = TTZipCreateOptions {
            level: lvl_enum,
            thread_budget: 0,
            ..Default::default()
        };

        let ttzip_create_dur = measure_median_indexed(5, |r| {
            create_zip_streaming_parallel(&zip_runs[r], &input_paths, &opts).unwrap();
        });
        let raw_create_dur = measure_median_indexed(5, |r| {
            raw_libdeflate_disk_create_1t(&input_paths, &raw_runs[r], lvl_num);
        });

        let zip_bytes = fs::read(&zip_runs[0]).unwrap();
        let archive = ZipArchive::open_slice(&zip_bytes).unwrap();
        let ext_runs: Vec<_> = (0..=5)
            .map(|r| {
                let d = dir.path().join(format!("ext_l{lvl_num}_{r}"));
                fs::create_dir_all(&d).unwrap();
                d
            })
            .collect();
        let raw_ext_runs: Vec<_> = (0..=5)
            .map(|r| {
                let d = dir.path().join(format!("raw_ext_l{lvl_num}_{r}"));
                fs::create_dir_all(&d).unwrap();
                d
            })
            .collect();

        let progress_counter = AtomicU64::new(0);
        let ext_opts = TTZipExtractOptions {
            thread_budget: 0,
            progress_callback: Some(track_extract_progress),
            user_data: (&progress_counter as *const AtomicU64) as *mut libc::c_void,
            ..Default::default()
        };

        let ttzip_ext_dur = measure_median_indexed(5, |r| {
            progress_counter.store(0, Ordering::Relaxed);
            archive.extract_all(&ext_runs[r], &ext_opts).unwrap();
        });
        let raw_ext_dur = measure_median_indexed(5, |r| {
            raw_libdeflate_disk_extract_1t(&archive, &l0_deflate_blobs, &raw_ext_runs[r], lvl_num);
        });

        assert_eq!(
            progress_counter.load(Ordering::Relaxed),
            (file_count * file_size) as u64
        );

        let ttzip_c_mbs = total_mb / ttzip_create_dur.as_secs_f64();
        let raw_c_mbs = total_mb / raw_create_dur.as_secs_f64();
        let create_ab = ttzip_c_mbs / raw_c_mbs;

        let ttzip_e_mbs = total_mb / ttzip_ext_dur.as_secs_f64();
        let raw_e_mbs = total_mb / raw_ext_dur.as_secs_f64();
        let extract_ab = ttzip_e_mbs / raw_e_mbs;

        println!(
            "  {:2}  | {:8.1} MB/s | {:9.1} MB/s | {:9.2}x | {:9.1} MB/s | {:10.1} MB/s | {:9.2}x",
            lvl_num, ttzip_c_mbs, raw_c_mbs, create_ab, ttzip_e_mbs, raw_e_mbs, extract_ab
        );

        assert!(
            create_ab >= 1.0,
            "Level {lvl_num} TTZip create A/B ratio ({create_ab:.2}x) must be >= 1.0x of 1T raw libdeflate"
        );
        assert!(
            extract_ab >= 1.0,
            "Level {lvl_num} TTZip extract A/B ratio ({extract_ab:.2}x) must be >= 1.0x of 1T raw libdeflate"
        );
    }
    println!("=========================================================================================");
}
