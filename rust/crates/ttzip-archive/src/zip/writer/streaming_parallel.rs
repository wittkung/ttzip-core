// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! High-throughput multi-core streaming parallel ZIP writer with PKWARE Data Descriptors.
//!
//! Features:
//! - PKWARE ZIP Data Descriptor (General Purpose Bit Flag 0x0808, Bit 3 = 1) streaming.
//! - Constant 64KB bounded-memory streaming writer for uncompressed (Store) payloads.
//! - Single-allocation zero-redundant copy Deflate compression via thread-local `libdeflate`.
//! - Instant disk landing with ~80-byte `CentralDirectoryMeta` in-memory footprint.
//! - Full Zip64 automatic promotion for archives >4GB or catalogs >65535 entries.
//! - 16-byte standard and 24-byte Zip64 Data Descriptor tail blocks.
//! - Cooperative async cancellation token check (<10ms abort latency).
//! - Adaptive Dynamic Batching for small files (<4KB) via `parallel_batch`.

use super::data_descriptor::build_data_descriptor;
use super::parallel_batch::*;
use super::streaming_sink::*;
use super::types::ZipCreateReport;
use crate::types::{TTZipCompressionLevel, TTZipCreateOptions, TTZipStatus};
use std::path::{Path, PathBuf};
use ttzip_types::options::ProgressAtomicSheet;

/// Creates a ZIP archive using the high-throughput multi-core streaming parallel engine.
pub fn create_zip_streaming_parallel(
    dest_path: &Path,
    source_paths: &[PathBuf],
    options: &TTZipCreateOptions,
) -> Result<ZipCreateReport, TTZipStatus> {
    create_zip_streaming_parallel_with_sheet(dest_path, source_paths, options, None)
}

/// Creates a single-file or split-volume ZIP archive using the multi-core streaming parallel engine.
pub fn create_zip_streaming_parallel_split(
    dest_path: &Path,
    source_paths: &[PathBuf],
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<ZipCreateReport, TTZipStatus> {
    let start_time = std::time::Instant::now();
    let plan_items = collect_source_plan_items(source_paths)?;
    if plan_items.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    compress_and_write_plan_items(
        dest_path,
        plan_items,
        options,
        split_volume_size_bytes,
        start_time,
        None,
    )
}

/// Creates a ZIP archive using the high-throughput streaming parallel engine with an atomic progress sheet.
pub fn create_zip_streaming_parallel_with_sheet(
    dest_path: &Path,
    source_paths: &[PathBuf],
    options: &TTZipCreateOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<ZipCreateReport, TTZipStatus> {
    let start_time = std::time::Instant::now();
    let plan_items = collect_source_plan_items(source_paths)?;
    if plan_items.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    compress_and_write_plan_items(dest_path, plan_items, options, 0, start_time, sheet)
}

/// Creates a ZIP archive from explicitly planned entries using the streaming parallel engine.
pub fn create_zip_streaming_parallel_explicit(
    dest_path: &Path,
    planned_entries: &[(PathBuf, String)],
    options: &TTZipCreateOptions,
) -> Result<ZipCreateReport, TTZipStatus> {
    create_zip_streaming_parallel_explicit_with_sheet(dest_path, planned_entries, options, None)
}

/// Creates a single-file or split-volume ZIP archive from explicitly planned entries.
pub fn create_zip_streaming_parallel_explicit_split(
    dest_path: &Path,
    planned_entries: &[(PathBuf, String)],
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<ZipCreateReport, TTZipStatus> {
    let start_time = std::time::Instant::now();
    let plan_items = build_explicit_plan_items(planned_entries)?;
    if plan_items.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    compress_and_write_plan_items(
        dest_path,
        plan_items,
        options,
        split_volume_size_bytes,
        start_time,
        None,
    )
}

/// Creates a ZIP archive from explicit entries using the streaming parallel engine with an atomic progress sheet.
pub fn create_zip_streaming_parallel_explicit_with_sheet(
    dest_path: &Path,
    planned_entries: &[(PathBuf, String)],
    options: &TTZipCreateOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<ZipCreateReport, TTZipStatus> {
    let start_time = std::time::Instant::now();
    let plan_items = build_explicit_plan_items(planned_entries)?;
    if plan_items.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    compress_and_write_plan_items(dest_path, plan_items, options, 0, start_time, sheet)
}

fn resolve_compression_level_num(level: TTZipCompressionLevel) -> i32 {
    match level {
        TTZipCompressionLevel::Store => 0,
        TTZipCompressionLevel::Fastest => 1,
        TTZipCompressionLevel::Fast => 3,
        TTZipCompressionLevel::Normal => 6,
        TTZipCompressionLevel::Maximum => 9,
        TTZipCompressionLevel::Ultra => 12,
    }
}

fn write_compressed_entry_to_sink(
    sink: &mut ZipOutputSink,
    entry: CompressedEntryResult,
) -> Result<CentralDirectoryMeta, TTZipStatus> {
    let lfh_offset = sink.current_offset();
    sink.write_bytes(&entry.header_bytes)?;
    if !entry.payload.is_empty() {
        sink.write_payload(&entry.payload)?;
    }
    if !entry.is_directory {
        let is_zip64 = entry.uncompressed_size >= 0xFFFF_FFFF
            || entry.compressed_size >= 0xFFFF_FFFF
            || lfh_offset >= 0xFFFF_FFFF;
        let dd_bytes = build_data_descriptor(
            entry.crc32,
            entry.compressed_size,
            entry.uncompressed_size,
            is_zip64,
        );
        sink.write_bytes(&dd_bytes)?;
    }
    Ok(CentralDirectoryMeta {
        rel_path: entry.rel_path,
        lfh_offset,
        uncompressed_size: entry.uncompressed_size,
        compressed_size: entry.compressed_size,
        crc32: entry.crc32,
        compression_method: entry.compression_method,
        actual_method: entry.actual_method,
        is_encrypted: entry.is_encrypted,
        mtime_secs: entry.mtime_secs,
        mode: entry.mode,
        is_directory: entry.is_directory,
    })
}

fn write_central_directory_tail_to_sink(
    sink: &mut ZipOutputSink,
    cd_entries: &[CentralDirectoryMeta],
    total_uncompressed: u64,
) -> Result<(), TTZipStatus> {
    let cd_start_offset = sink.current_offset();
    for cd in cd_entries {
        let cd_bytes = build_cdfh_bytes(cd);
        sink.write_bytes(&cd_bytes)?;
    }
    let cd_size = sink.current_offset() - cd_start_offset;

    let needs_zip64 = cd_entries.len() >= 0xFFFF
        || cd_start_offset >= 0xFFFF_FFFF
        || cd_size >= 0xFFFF_FFFF
        || total_uncompressed >= 0xFFFF_FFFF;

    if needs_zip64 {
        let zip64_eocd_offset = sink.current_offset();
        let zip64_eocd = build_zip64_eocd(cd_entries.len() as u64, cd_size, cd_start_offset);
        sink.write_bytes(&zip64_eocd)?;
        let zip64_locator = build_zip64_locator(zip64_eocd_offset);
        sink.write_bytes(&zip64_locator)?;
    }

    let eocd = build_eocd(
        cd_entries.len().min(0xFFFF) as u16,
        cd_size.min(0xFFFF_FFFF) as u32,
        cd_start_offset.min(0xFFFF_FFFF) as u32,
    );
    sink.write_bytes(&eocd)
}

fn compress_and_write_plan_items(
    dest_path: &Path,
    plan_items: Vec<CompressionPlanItem>,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    start_time: std::time::Instant,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<ZipCreateReport, TTZipStatus> {
    let level_num = resolve_compression_level_num(options.level);
    let total_uncompressed: u64 = plan_items.iter().map(|i| i.uncompressed_size).sum();
    let prealloc_hint = if total_uncompressed > 0 {
        if level_num == 0 {
            total_uncompressed + (plan_items.len() as u64 * 128)
        } else {
            (total_uncompressed / 2).max(65536) + (plan_items.len() as u64 * 128)
        }
    } else {
        0
    };
    let mut sink = ZipOutputSink::open(dest_path, split_volume_size_bytes, prealloc_hint)?;

    let local_sheet = ProgressAtomicSheet::with_total(total_uncompressed);
    let active_sheet = sheet.unwrap_or(&local_sheet);
    active_sheet.set_total(total_uncompressed);

    let progress_cb = options.progress_callback;
    let user_data_usize = options.user_data as usize;
    let password_str = if !options.password.is_null() {
        unsafe { std::ffi::CStr::from_ptr(options.password) }
            .to_str()
            .ok()
    } else {
        None
    };

    let mut cd_entries: Vec<CentralDirectoryMeta> = Vec::with_capacity(plan_items.len());
    let mut plan_items_iter = plan_items.into_iter().peekable();

    loop {
        if active_sheet.is_cancelled() {
            return Err(TTZipStatus::Cancelled);
        }
        let batch = take_adaptive_batch(&mut plan_items_iter);
        if batch.is_empty() {
            break;
        }
        let compressed_batch = execute_batch_parallel_with_options(
            batch,
            BatchCompressionOptions {
                level: level_num,
                format: options.format,
                encryption: options.encryption,
                password: password_str,
                sheet: Some(active_sheet),
                thread_budget: options.thread_budget,
            },
        );

        for result in compressed_batch {
            if active_sheet.is_cancelled() {
                return Err(TTZipStatus::Cancelled);
            }
            let meta = write_compressed_entry_to_sink(&mut sink, result?)?;
            let current_processed = active_sheet.read_snapshot().processed_bytes;
            let elapsed_secs = start_time.elapsed().as_secs_f64();
            let instant_bps = if elapsed_secs > 0.001 {
                (current_processed as f64 / elapsed_secs) as u64
            } else {
                0
            };
            active_sheet.update_progress(current_processed, instant_bps);
            if let Some(cb) = progress_cb {
                let rel_c = std::ffi::CString::new(meta.rel_path.as_str()).unwrap_or_default();
                let keep_going = unsafe {
                    cb(
                        current_processed,
                        total_uncompressed,
                        rel_c.as_ptr(),
                        user_data_usize as *mut libc::c_void,
                    )
                };
                if !keep_going {
                    active_sheet.cancel();
                    return Err(TTZipStatus::Cancelled);
                }
            }
            cd_entries.push(meta);
        }
    }

    write_central_directory_tail_to_sink(&mut sink, &cd_entries, total_uncompressed)?;
    let final_compressed_bytes = sink.commit()?;
    let elapsed_nanos = start_time.elapsed().as_nanos() as u64;
    let prov = crate::types::TTZipExecutionProvenance {
        engine_tag: crate::types::TTZipEngineTag::RustStreamingParallelZip,
        thread_count: options.thread_budget,
        uncompressed_bytes: total_uncompressed,
        compressed_bytes: final_compressed_bytes,
        kernel_duration_nanos: elapsed_nanos,
        is_fallback: false,
        ..Default::default()
    };
    crate::types::record_execution_provenance(prov);
    active_sheet.mark_finished();

    Ok(ZipCreateReport {
        total_entries: cd_entries.len(),
        total_uncompressed_bytes: total_uncompressed,
        total_compressed_bytes: final_compressed_bytes,
        duration_ms: elapsed_nanos / 1_000_000,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use crate::crypto::crc32::crc32_fast;
    use crate::zip::reader::ZipArchive;
    use crate::zip::writer::data_descriptor::MAGIC_DATA_DESCRIPTOR;

    #[test]
    fn test_build_data_descriptor_standard_and_zip64() {
        let crc = 0x12345678;
        let comp = 1024u64;
        let uncomp = 2048u64;

        // Standard 16-byte Data Descriptor
        let dd16 = build_data_descriptor(crc, comp, uncomp, false);
        assert_eq!(dd16.len(), 16);
        assert_eq!(&dd16[0..4], &MAGIC_DATA_DESCRIPTOR.to_le_bytes());
        assert_eq!(&dd16[4..8], &crc.to_le_bytes());
        assert_eq!(&dd16[8..12], &1024u32.to_le_bytes());
        assert_eq!(&dd16[12..16], &2048u32.to_le_bytes());

        // Zip64 24-byte Data Descriptor
        let zcomp = 0x1_0000_0000u64;
        let zuncomp = 0x2_0000_0000u64;
        let dd24 = build_data_descriptor(crc, zcomp, zuncomp, true);
        assert_eq!(dd24.len(), 24);
        assert_eq!(&dd24[0..4], &MAGIC_DATA_DESCRIPTOR.to_le_bytes());
        assert_eq!(&dd24[4..8], &crc.to_le_bytes());
        assert_eq!(&dd24[8..16], &zcomp.to_le_bytes());
        assert_eq!(&dd24[16..24], &zuncomp.to_le_bytes());
    }

    #[test]
    fn test_streaming_parallel_data_descriptor_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src_dir = temp_dir.path().join("src");
        let out_zip = temp_dir.path().join("out.zip");

        fs::create_dir_all(src_dir.join("sub")).unwrap();
        fs::write(src_dir.join("hello.txt"), b"Streaming Data Descriptor Test Payload!").unwrap();
        fs::write(src_dir.join("sub/binary.dat"), vec![0xABu8; 8192]).unwrap();

        let opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_3,
            format: crate::types::TTZipArchiveFormat::Zip,
            level: TTZipCompressionLevel::Normal,
            encryption: crate::types::TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 2,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        let report = create_zip_streaming_parallel(&out_zip, std::slice::from_ref(&src_dir), &opt).unwrap();
        assert_eq!(report.total_entries, 4); // root dir + sub dir + 2 files

        let zip_bytes = fs::read(&out_zip).unwrap();
        let archive = ZipArchive::open_slice(&zip_bytes).unwrap();
        assert_eq!(archive.len(), 4);

        // Verify LFH has Bit 3 flag (0x0808) and zeros for CRC/sizes
        let entry_txt = archive.entries().iter().find(|e| e.rel_path.ends_with("hello.txt")).unwrap();
        assert_eq!(entry_txt.flag & 0x0008, 0x0008);

        let lfh_off = entry_txt.lfh_offset as usize;
        let lfh_flag = u16::from_le_bytes(zip_bytes[lfh_off + 6..lfh_off + 8].try_into().unwrap());
        assert_eq!(lfh_flag, 0x0808);
        let lfh_crc = u32::from_le_bytes(zip_bytes[lfh_off + 14..lfh_off + 18].try_into().unwrap());
        assert_eq!(lfh_crc, 0);
        let lfh_csize = u32::from_le_bytes(zip_bytes[lfh_off + 18..lfh_off + 22].try_into().unwrap());
        assert_eq!(lfh_csize, 0);

        // Verify Central Directory has valid CRC & sizes
        assert_eq!(entry_txt.uncompressed_size, b"Streaming Data Descriptor Test Payload!".len() as u64);
        assert_eq!(entry_txt.crc32, crc32_fast(0, b"Streaming Data Descriptor Test Payload!"));

        // Verify extraction
        let idx = archive.entries().iter().position(|e| e.rel_path.ends_with("hello.txt")).unwrap();
        let data = archive.extract_entry_bytes(idx, None).unwrap();
        assert_eq!(data, b"Streaming Data Descriptor Test Payload!");

        let idx_bin = archive.entries().iter().position(|e| e.rel_path.ends_with("binary.dat")).unwrap();
        let bin_data = archive.extract_entry_bytes(idx_bin, None).unwrap();
        assert_eq!(bin_data, vec![0xABu8; 8192]);
    }

    #[test]
    fn test_streaming_parallel_dynamic_batching_many_small_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src_dir = temp_dir.path().join("small_files");
        let out_zip = temp_dir.path().join("small_out.zip");

        fs::create_dir_all(&src_dir).unwrap();
        // Create 300 tiny files (<100 bytes each) to trigger dynamic batch aggregation
        for i in 0..300 {
            fs::write(src_dir.join(format!("file_{:03}.txt", i)), format!("Payload {}", i)).unwrap();
        }

        let opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_3,
            format: crate::types::TTZipArchiveFormat::Zip,
            level: TTZipCompressionLevel::Fastest,
            encryption: crate::types::TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 4,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        let report = create_zip_streaming_parallel(&out_zip, std::slice::from_ref(&src_dir), &opt).unwrap();
        assert_eq!(report.total_entries, 301); // 1 root dir + 300 files

        let zip_bytes = fs::read(&out_zip).unwrap();
        let archive = ZipArchive::open_slice(&zip_bytes).unwrap();
        assert_eq!(archive.len(), 301);

        // Verify sample file extraction
        let idx = archive.entries().iter().position(|e| e.rel_path.ends_with("file_123.txt")).unwrap();
        let data = archive.extract_entry_bytes(idx, None).unwrap();
        assert_eq!(data, b"Payload 123");
    }

    #[test]
    fn test_streaming_parallel_cancellation_with_sheet() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src_dir = temp_dir.path().join("cancel_src");
        let out_zip = temp_dir.path().join("cancel_out.zip");

        fs::create_dir_all(&src_dir).unwrap();
        for i in 0..10 {
            fs::write(src_dir.join(format!("chunk_{}.dat", i)), vec![0x33u8; 64 * 1024]).unwrap();
        }

        let opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_3,
            format: crate::types::TTZipArchiveFormat::Zip,
            level: TTZipCompressionLevel::Fast,
            encryption: crate::types::TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 2,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        let sheet = ProgressAtomicSheet::new();
        sheet.cancel();

        let res = create_zip_streaming_parallel_with_sheet(
            &out_zip,
            std::slice::from_ref(&src_dir),
            &opt,
            Some(&sheet),
        );

        assert_eq!(res.err(), Some(TTZipStatus::Cancelled));
        assert!(!out_zip.exists(), "Aborted file must be cleaned up");
    }

    #[test]
    fn test_streaming_parallel_progress_sheet_lifecycle() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src_dir = temp_dir.path().join("sheet_src");
        let out_zip = temp_dir.path().join("sheet_out.zip");

        fs::create_dir_all(&src_dir).unwrap();
        let payload = vec![0x77u8; 128 * 1024];
        fs::write(src_dir.join("large_payload.bin"), &payload).unwrap();

        let opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_3,
            format: crate::types::TTZipArchiveFormat::Zip,
            level: TTZipCompressionLevel::Fastest,
            encryption: crate::types::TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 2,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        let sheet = ProgressAtomicSheet::new();

        let report = create_zip_streaming_parallel_with_sheet(
            &out_zip,
            std::slice::from_ref(&src_dir),
            &opt,
            Some(&sheet),
        )
        .unwrap();

        assert_eq!(report.total_entries, 2);
        let snap = sheet.read_snapshot();
        assert!(snap.is_finished);
        assert!(!snap.is_cancelled);
        assert!(snap.processed_bytes >= payload.len() as u64);
        assert_eq!(snap.total_bytes, payload.len() as u64);
    }
}
