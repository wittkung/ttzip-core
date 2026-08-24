// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use ttzip_engine::archive::source::{open_archive_source, StorageMedium};
use ttzip_engine::zip::reader::ZipArchive;

/// Darwin Mach kernel task_info FFI binding
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
struct MachTaskBasicInfo {
    pub virtual_size: u64,
    pub resident_size: u64,
    pub resident_size_max: u64,
    pub user_time_sec: i32,
    pub user_time_usec: i32,
    pub system_time_sec: i32,
    pub system_time_usec: i32,
    pub policy: i32,
    pub suspend_count: i32,
}

const MACH_TASK_BASIC_INFO: u32 = 20;
const MACH_TASK_BASIC_INFO_COUNT: u32 = (std::mem::size_of::<MachTaskBasicInfo>() / std::mem::size_of::<i32>()) as u32;

extern "C" {
    fn mach_task_self() -> u32;
    fn task_info(
        target_task: u32,
        flavor: u32,
        task_info_out: *mut MachTaskBasicInfo,
        task_info_outCnt: *mut u32,
    ) -> i32;
}

pub fn get_current_rss_bytes() -> u64 {
    unsafe {
        let mut info: MachTaskBasicInfo = std::mem::zeroed();
        let mut count = MACH_TASK_BASIC_INFO_COUNT;
        let kret = task_info(mach_task_self(), MACH_TASK_BASIC_INFO, &mut info, &mut count);
        if kret == 0 {
            info.resident_size
        } else {
            let mut usage: libc::rusage = std::mem::zeroed();
            libc::getrusage(libc::RUSAGE_SELF, &mut usage);
            usage.ru_maxrss as u64
        }
    }
}

pub struct MemoryPeakTracker {
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
                thread::sleep(Duration::from_micros(500));
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
        assert!(
            peak <= max_allowed_bytes,
            "Memory Invariant Violated! Peak RSS: {:.2} MB > limit {:.2} MB (Delta: {:.2} MB)",
            peak as f64 / 1024.0 / 1024.0,
            max_allowed_bytes as f64 / 1024.0 / 1024.0,
            delta_rss as f64 / 1024.0 / 1024.0
        );
        peak
    }
}

pub struct ApfsSparseZipFixture {
    pub file_path: PathBuf,
    pub logical_size: u64,
    pub entry_name: String,
}

impl ApfsSparseZipFixture {
    pub fn create_50gb_zip(dir: &Path) -> Self {
        let file_path = dir.join("synthetic_50gb_sparse.zip");
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&file_path)
            .expect("Failed to create sparse test file");

        let entry_name = "sparse_payload.bin".to_string();
        let payload_size: u64 = 50 * 1024 * 1024 * 1024; // 50 GiB

        // A. Local File Header
        let mut lfh = Vec::with_capacity(64);
        lfh.extend_from_slice(&0x04034b50u32.to_le_bytes());
        lfh.extend_from_slice(&45u16.to_le_bytes()); // ZIP64 version 4.5
        lfh.extend_from_slice(&0x0000u16.to_le_bytes());
        lfh.extend_from_slice(&0x0000u16.to_le_bytes()); // Store
        lfh.extend_from_slice(&0x0000u16.to_le_bytes());
        lfh.extend_from_slice(&0x0000u16.to_le_bytes());
        lfh.extend_from_slice(&0x00000000u32.to_le_bytes());
        lfh.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        lfh.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        lfh.extend_from_slice(&(entry_name.len() as u16).to_le_bytes());
        lfh.extend_from_slice(&20u16.to_le_bytes());
        lfh.extend_from_slice(entry_name.as_bytes());

        // Zip64 Extra Field in LFH
        lfh.extend_from_slice(&0x0001u16.to_le_bytes());
        lfh.extend_from_slice(&16u16.to_le_bytes());
        lfh.extend_from_slice(&payload_size.to_le_bytes());
        lfh.extend_from_slice(&payload_size.to_le_bytes());

        file.write_all(&lfh).expect("Failed to write LFH");
        let lfh_len = lfh.len() as u64;

        // B. APFS hole seek (50GB virtual span)
        file.seek(SeekFrom::Current(payload_size as i64))
            .expect("Failed to seek 50GB sparse hole");

        let cd_offset = lfh_len + payload_size;

        // C. Central Directory Header
        let mut cdh = Vec::with_capacity(128);
        cdh.extend_from_slice(&0x02014b50u32.to_le_bytes());
        cdh.extend_from_slice(&45u16.to_le_bytes());
        cdh.extend_from_slice(&45u16.to_le_bytes());
        cdh.extend_from_slice(&0x0000u16.to_le_bytes());
        cdh.extend_from_slice(&0x0000u16.to_le_bytes());
        cdh.extend_from_slice(&0x0000u16.to_le_bytes());
        cdh.extend_from_slice(&0x0000u16.to_le_bytes());
        cdh.extend_from_slice(&0x00000000u32.to_le_bytes());
        cdh.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        cdh.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        cdh.extend_from_slice(&(entry_name.len() as u16).to_le_bytes());
        cdh.extend_from_slice(&28u16.to_le_bytes());
        cdh.extend_from_slice(&0u16.to_le_bytes());
        cdh.extend_from_slice(&0u16.to_le_bytes());
        cdh.extend_from_slice(&0u16.to_le_bytes());
        cdh.extend_from_slice(&0u32.to_le_bytes());
        cdh.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        cdh.extend_from_slice(entry_name.as_bytes());

        // Zip64 Extra in CD
        cdh.extend_from_slice(&0x0001u16.to_le_bytes());
        cdh.extend_from_slice(&24u16.to_le_bytes());
        cdh.extend_from_slice(&payload_size.to_le_bytes());
        cdh.extend_from_slice(&payload_size.to_le_bytes());
        cdh.extend_from_slice(&0u64.to_le_bytes());

        file.write_all(&cdh).expect("Failed to write CDH");
        let cd_len = cdh.len() as u64;

        // D. Zip64 EOCD Record
        let zip64_eocd_offset = cd_offset + cd_len;
        let mut z64_eocd = Vec::with_capacity(64);
        z64_eocd.extend_from_slice(&0x06064b50u32.to_le_bytes());
        z64_eocd.extend_from_slice(&44u64.to_le_bytes());
        z64_eocd.extend_from_slice(&45u16.to_le_bytes());
        z64_eocd.extend_from_slice(&45u16.to_le_bytes());
        z64_eocd.extend_from_slice(&0u32.to_le_bytes());
        z64_eocd.extend_from_slice(&0u32.to_le_bytes());
        z64_eocd.extend_from_slice(&1u64.to_le_bytes());
        z64_eocd.extend_from_slice(&1u64.to_le_bytes());
        z64_eocd.extend_from_slice(&cd_len.to_le_bytes());
        z64_eocd.extend_from_slice(&cd_offset.to_le_bytes());
        file.write_all(&z64_eocd).expect("Failed to write Zip64 EOCD");

        // E. Zip64 Locator
        let mut z64_loc = Vec::with_capacity(20);
        z64_loc.extend_from_slice(&0x07064b50u32.to_le_bytes());
        z64_loc.extend_from_slice(&0u32.to_le_bytes());
        z64_loc.extend_from_slice(&zip64_eocd_offset.to_le_bytes());
        z64_loc.extend_from_slice(&1u32.to_le_bytes());
        file.write_all(&z64_loc).expect("Failed to write Zip64 Locator");

        // F. Standard EOCD
        let mut eocd = Vec::with_capacity(22);
        eocd.extend_from_slice(&0x06054b50u32.to_le_bytes());
        eocd.extend_from_slice(&0u16.to_le_bytes());
        eocd.extend_from_slice(&0u16.to_le_bytes());
        eocd.extend_from_slice(&0xFFFFu16.to_le_bytes());
        eocd.extend_from_slice(&0xFFFFu16.to_le_bytes());
        eocd.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        eocd.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        eocd.extend_from_slice(&0u16.to_le_bytes());
        file.write_all(&eocd).expect("Failed to write EOCD");
        file.flush().expect("Failed to flush sparse archive");

        let total_logical_len = file.metadata().unwrap().len();
        Self {
            file_path,
            logical_size: total_logical_len,
            entry_name,
        }
    }
}

#[test]
fn test_50gb_sparse_mmap_and_bounded_rss_invariant() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let fixture = ApfsSparseZipFixture::create_50gb_zip(temp_dir.path());
    assert!(fixture.logical_size >= 50 * 1024 * 1024 * 1024);

    let tracker = MemoryPeakTracker::start();

    let source = open_archive_source(&fixture.file_path).expect("open_archive_source failed");
    assert_eq!(source.medium(), StorageMedium::LocalFastApfs);
    assert_eq!(source.len(), fixture.logical_size);

    let slice = source.as_slice().expect("MmapSource must expose memory slice");
    let archive = ZipArchive::open_slice(slice).expect("Failed to parse 50GB Zip64 archive");
    assert_eq!(archive.entries().len(), 1);
    assert_eq!(archive.entries()[0].rel_path, fixture.entry_name);
    assert_eq!(archive.entries()[0].uncompressed_size, 50 * 1024 * 1024 * 1024);

    let mut chunk = vec![0u8; 1024 * 1024];
    let bytes_read = source.read_at(&mut chunk, 10 * 1024 * 1024 * 1024).expect("read_at sparse area failed");
    assert_eq!(bytes_read, 1024 * 1024);
    assert!(chunk.iter().all(|&b| b == 0));

    // Hard assert: Peak RSS during entire 50GB operation < 32MB (generous for test runner baseline)
    let max_allowed_rss = 32 * 1024 * 1024;
    let recorded_peak = tracker.stop_and_assert_peak(max_allowed_rss);
    println!("✓ 50GB Sparse Archive Peak RSS: {:.2} MB (Limit: 32.00 MB)", recorded_peak as f64 / 1024.0 / 1024.0);
}
