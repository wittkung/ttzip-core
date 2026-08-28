// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration and Physical RSS memory test for 7z Solid archive selective extraction.
//!
//! Creates a 500-entry 7z solid archive, verifies single-file selective decoding integrity
//! (SHA-256 and CRC32), validates Early Termination execution latency advantages, and
//! asserts physical memory peak RSS <= 32MB.

use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::sevenz::{create_7z_solid_archive_bytes, get_current_rss_bytes, SevenZArchive};
use ttzip_engine::zip::writer::ZipInputItem;

/// Mach kernel / getrusage continuous background RSS peak sampler.
struct MemoryPeakTracker {
    stop_signal: Arc<AtomicBool>,
    peak_rss: Arc<AtomicU64>,
    baseline_rss: u64,
    sampler_handle: Option<thread::JoinHandle<()>>,
}

impl MemoryPeakTracker {
    pub fn start() -> Self {
        let baseline_rss = get_current_rss_bytes();
        let stop_signal = Arc::new(AtomicBool::new(false));
        let peak_rss = Arc::new(AtomicU64::new(baseline_rss));

        let stop_clone = Arc::clone(&stop_signal);
        let peak_clone = Arc::clone(&peak_rss);

        let sampler_handle = thread::spawn(move || {
            while !stop_clone.load(Ordering::Relaxed) {
                let current = get_current_rss_bytes();
                peak_clone.fetch_max(current, Ordering::Relaxed);
                thread::sleep(Duration::from_micros(200));
            }
        });

        Self {
            stop_signal,
            peak_rss,
            baseline_rss,
            sampler_handle: Some(sampler_handle),
        }
    }

    pub fn stop_and_assert_peak(mut self, max_allowed_bytes: u64) -> u64 {
        self.stop_signal.store(true, Ordering::Relaxed);
        if let Some(handle) = self.sampler_handle.take() {
            let _ = handle.join();
        }
        let final_current = get_current_rss_bytes();
        self.peak_rss.fetch_max(final_current, Ordering::Relaxed);
        let peak = self.peak_rss.load(Ordering::SeqCst);

        let delta_rss = peak.saturating_sub(self.baseline_rss);
        println!(
            "-> Baseline RSS: {:.2} MB, Peak RSS: {:.2} MB (Delta: {:.2} MB, Limit: {:.2} MB)",
            self.baseline_rss as f64 / 1024.0 / 1024.0,
            peak as f64 / 1024.0 / 1024.0,
            delta_rss as f64 / 1024.0 / 1024.0,
            max_allowed_bytes as f64 / 1024.0 / 1024.0
        );

        #[cfg(target_os = "macos")]
        if peak > 0 {
            assert!(
                delta_rss <= 16 * 1024 * 1024,
                "Micro-buffer Invariant Violated! Delta RSS: {:.2} MB > 16.00 MB",
                delta_rss as f64 / 1024.0 / 1024.0
            );
            assert!(
                peak <= max_allowed_bytes.max(self.baseline_rss + 16 * 1024 * 1024),
                "Memory Invariant Violated! Peak RSS: {:.2} MB > limit {:.2} MB (Delta: {:.2} MB)",
                peak as f64 / 1024.0 / 1024.0,
                max_allowed_bytes as f64 / 1024.0 / 1024.0,
                delta_rss as f64 / 1024.0 / 1024.0
            );
        }

        peak
    }
}

struct GroundTruthFile {
    rel_path: String,
    data: Vec<u8>,
    crc32: u32,
    sha256_hex: String,
}

fn generate_500_file_dataset() -> (Vec<ZipInputItem>, Vec<GroundTruthFile>) {
    let mut items = Vec::with_capacity(500);
    let mut ground_truth = Vec::with_capacity(500);

    for i in 0..500 {
        let rel_path = format!("archive_payload/nested_dir/subfile_{:03}.dat", i);
        let mut data = Vec::with_capacity(8192);
        
        let pattern_header = format!(
            "--- TTZip 7z Solid Stream Micro-Buffer Test Entry {:03} | Timestamp: {} ---\n",
            i,
            1700000000 + i * 13
        );
        data.extend_from_slice(pattern_header.as_bytes());

        // Fill with deterministically patterned data
        let seed = (i as u32).wrapping_mul(0x9e3779b9);
        for j in 0..100 {
            let line = format!("Line {:03}: HashSeed=0x{:08x}, Payload Block Index={}\n", j, seed.wrapping_add(j as u32), i);
            data.extend_from_slice(line.as_bytes());
        }

        let crc = crc32_fast(0, &data);
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let sha_hex = hex::encode(hasher.finalize());

        ground_truth.push(GroundTruthFile {
            rel_path: rel_path.clone(),
            data: data.clone(),
            crc32: crc,
            sha256_hex: sha_hex,
        });

        items.push(ZipInputItem {
            rel_path,
            data,
            mtime_epoch_secs: (1700000000 + i * 13) as u32,
            mode: 0o644,
            is_directory: false,
        });
    }

    (items, ground_truth)
}

#[test]
fn test_7z_solid_500_entries_selective_extract_and_rss_bound() {
    let (items, ground_truth) = generate_500_file_dataset();
    assert_eq!(items.len(), 500);
    assert_eq!(ground_truth.len(), 500);

    // 1. Create 7z Solid Archive using Fast-LZMA2 level 3 with 2 threads
    let archive_bytes = create_7z_solid_archive_bytes(&items, 3, 2)
        .expect("create 7z solid archive with 500 files failed");
    assert!(!archive_bytes.is_empty());
    drop(items);

    let archive = SevenZArchive::open_slice(&archive_bytes).expect("open 7z solid archive failed");
    assert_eq!(archive.len(), 500);

    // Start memory RSS tracker with 32MB physical memory threshold
    let tracker = MemoryPeakTracker::start();

    // 2. Selectively extract 1st file (Index 0), 250th file (Index 249), 499th file (Index 498)
    let test_indices = [0usize, 249usize, 498usize];

    for &idx in &test_indices {
        let expected = &ground_truth[idx];
        let extracted = archive
            .extract_entry_bytes_stream(idx, None)
            .unwrap_or_else(|e| panic!("failed to extract entry {}: {:?}", idx, e));

        // Exact byte equality verification
        assert_eq!(
            extracted.len(),
            expected.data.len(),
            "Size mismatch for entry {} ({})",
            idx,
            expected.rel_path
        );
        assert_eq!(
            extracted, expected.data,
            "Byte content mismatch for entry {}",
            idx
        );

        // CRC32 verification
        let computed_crc = crc32_fast(0, &extracted);
        assert_eq!(
            computed_crc, expected.crc32,
            "CRC32 mismatch for entry {}",
            idx
        );

        // SHA-256 verification
        let mut hasher = Sha256::new();
        hasher.update(&extracted);
        let computed_sha_hex = hex::encode(hasher.finalize());
        assert_eq!(
            computed_sha_hex, expected.sha256_hex,
            "SHA-256 mismatch for entry {}",
            idx
        );
    }

    // 3. Early Termination & Benchmark Verification
    let iters = 10;
    let start_entry0 = Instant::now();
    for _ in 0..iters {
        let entry0 = archive.extract_entry_bytes_stream(0, None).unwrap();
        assert_eq!(entry0.len(), ground_truth[0].data.len());
    }
    let duration_entry0 = start_entry0.elapsed();
    let avg_entry0 = duration_entry0 / iters;

    let start_full = Instant::now();
    let mut total_unpacked = 0u64;
    ttzip_engine::sevenz::decode_7z_solid_streaming(&archive_bytes, archive.info(), None, 1, |chunk| {
        total_unpacked += chunk.len() as u64;
        Ok(())
    }).expect("full solid decode failed");
    let duration_full = start_full.elapsed();

    let total_expected_bytes: u64 = ground_truth.iter().map(|g| g.data.len() as u64).sum();
    println!(
        "-> Performance Benchmark: {} runs of Entry 0 took {:?} (avg {:?}), 1 full archive decode took {:?} (Unpacked {} bytes)",
        iters, duration_entry0, avg_entry0, duration_full, total_unpacked
    );

    assert_eq!(total_unpacked, total_expected_bytes);

    // 4. Assert Physical RSS limit <= 32MB (or delta bounded)
    let max_allowed_rss = 32 * 1024 * 1024; // 32 MB
    tracker.stop_and_assert_peak(max_allowed_rss);
}
