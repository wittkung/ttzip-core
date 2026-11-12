// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Lock-free Store-mode multi-core parallel ZIP stream writer.
//!
//! Uses physical APFS disk extent preallocation and multi-threaded `pwrite`
//! for zero-contention parallel packaging directly to disk.

use super::types::{unix_to_dos_time, ZipCreateReport};
use crate::crypto::crc32::crc32_fast;
use crate::fs::apfs::apfs_preallocate;
use crate::types::{TTZipCreateOptions, TTZipStatus};
use crate::zip::extra::ZipExtraFields;
use crate::zip::parser::{
    MAGIC_CDFH, MAGIC_EOCD, MAGIC_LFH, MAGIC_ZIP64_EOCD, MAGIC_ZIP64_LOCATOR,
};
use std::fs::{self, OpenOptions};
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use ttzip_types::scheduler::LocalWorkStealingPool;

/// Item planned for parallel store writing.
#[derive(Debug, Clone)]
struct StorePlanItem {
    src_path: Option<PathBuf>,
    rel_path: String,
    size: u64,
    mtime_secs: u32,
    mode: u32,
    is_directory: bool,
    lfh_offset: u64,
    lfh_header_size: usize,
    total_lfh_size: u64,
}

/// Recursively scans file tree and prepares store plan items.
fn collect_store_plan_items(
    base_src: &Path,
    rel_prefix: &str,
    out: &mut Vec<StorePlanItem>,
) -> Result<(), TTZipStatus> {
    let meta = fs::symlink_metadata(base_src).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let is_dir = meta.is_dir();
    let size = if is_dir { 0 } else { meta.len() };
    let mtime_secs = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);
    let mode = meta.mode();

    let mut rel = rel_prefix.to_string();
    if is_dir && !rel.is_empty() && !rel.ends_with('/') {
        rel.push('/');
    }

    if !rel.is_empty() {
        out.push(StorePlanItem {
            src_path: if is_dir { None } else { Some(base_src.to_path_buf()) },
            rel_path: rel,
            size,
            mtime_secs,
            mode,
            is_directory: is_dir,
            lfh_offset: 0,
            lfh_header_size: 0,
            total_lfh_size: 0,
        });
    }

    if is_dir {
        let entries = fs::read_dir(base_src).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let child_rel = if rel_prefix.is_empty() {
                name.to_string()
            } else {
                format!("{}/{}", rel_prefix.trim_end_matches('/'), name)
            };
            collect_store_plan_items(&path, &child_rel, out)?;
        }
    }
    Ok(())
}

/// Creates a ZIP archive using multi-threaded lock-free Store mode.
pub fn create_zip_store_parallel(
    dest_path: &Path,
    source_paths: &[PathBuf],
    options: &TTZipCreateOptions,
) -> Result<ZipCreateReport, TTZipStatus> {
    let start_time = std::time::Instant::now();

    let mut plan_items = Vec::new();
    for src in source_paths {
        if !src.exists() {
            return Err(TTZipStatus::ErrFileNotFound);
        }
        let fname = src.file_name().unwrap_or_default().to_string_lossy();
        collect_store_plan_items(src, &fname, &mut plan_items)?;
    }

    let (cd_offset, cd_size, is_zip64_required, total_file_size) =
        compute_store_layout(&mut plan_items);

    let file = open_and_preallocate_store_file(dest_path, total_file_size)?;
    let fd = file.as_raw_fd();

    let crcs = write_store_entries_parallel(fd, &plan_items, options.thread_budget as usize)?;
    let cd_bytes = build_store_central_directory_and_eocd(
        &plan_items,
        &crcs,
        cd_offset,
        cd_size,
        is_zip64_required,
    );
    pwrite_exact_slice(fd, &cd_bytes, cd_offset)?;

    let total_bytes: u64 = plan_items.iter().map(|it| it.size).sum();

    Ok(ZipCreateReport {
        total_entries: plan_items.len(),
        total_uncompressed_bytes: total_bytes,
        total_compressed_bytes: total_bytes,
        duration_ms: start_time.elapsed().as_millis() as u64,
    })
}

fn compute_store_layout(plan_items: &mut [StorePlanItem]) -> (u64, u64, bool, u64) {
    let mut current_offset: u64 = 0;
    for item in plan_items.iter_mut() {
        item.lfh_offset = current_offset;
        let name_bytes = item.rel_path.as_bytes();
        let extra_bytes = build_store_lfh_extra(item);
        item.lfh_header_size = 30 + name_bytes.len() + extra_bytes.len();
        item.total_lfh_size = (item.lfh_header_size as u64) + item.size;
        current_offset += item.total_lfh_size;
    }

    let cd_offset = current_offset;
    let mut cd_size: u64 = 0;
    for item in plan_items.iter() {
        let use_zip64 = item.size >= 0xFFFFFFFF || item.lfh_offset >= 0xFFFFFFFF;
        let extra_len = if use_zip64 { 9 + 28 } else { 9 };
        cd_size += (46 + item.rel_path.len() + extra_len) as u64;
    }

    let is_zip64_required =
        plan_items.len() >= 0xFFFF || cd_size >= 0xFFFFFFFF || cd_offset >= 0xFFFFFFFF;
    let eocd64_size = if is_zip64_required { 56 + 20 } else { 0 };
    let total_file_size = cd_offset + cd_size + eocd64_size + 22;
    (cd_offset, cd_size, is_zip64_required, total_file_size)
}

fn build_store_lfh_extra(item: &StorePlanItem) -> Vec<u8> {
    let use_zip64 = item.size >= 0xFFFFFFFF || item.lfh_offset >= 0xFFFFFFFF;
    let mut extra_bytes = Vec::new();
    if use_zip64 {
        extra_bytes.extend_from_slice(&ZipExtraFields::build_zip64_extra(
            Some(item.size),
            Some(item.size),
            None,
        ));
    }
    extra_bytes.extend_from_slice(&ZipExtraFields::build_extended_timestamp(item.mtime_secs));
    extra_bytes
}

fn open_and_preallocate_store_file(
    dest_path: &Path,
    total_file_size: u64,
) -> Result<fs::File, TTZipStatus> {
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    }

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(dest_path)
        .map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let fd = file.as_raw_fd();
    let _ = apfs_preallocate(fd, total_file_size as i64);
    unsafe {
        libc::ftruncate(fd, total_file_size as libc::off_t);
    }
    Ok(file)
}

const STORE_STREAM_CHUNK_SIZE: usize = 256 * 1024;

fn pwrite_exact_slice(fd: i32, data: &[u8], mut offset: u64) -> Result<(), TTZipStatus> {
    let mut remaining = data;
    while !remaining.is_empty() {
        let written = unsafe {
            libc::pwrite(
                fd,
                remaining.as_ptr() as *const libc::c_void,
                remaining.len(),
                offset as libc::off_t,
            )
        };
        if written <= 0 {
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        let n = written as usize;
        remaining = &remaining[n..];
        offset += n as u64;
    }
    Ok(())
}

fn stream_store_entry_payload(
    fd: i32,
    src: &Path,
    expected_size: u64,
    mut payload_offset: u64,
) -> Result<u32, TTZipStatus> {
    use std::io::Read;

    let mut file = fs::File::open(src).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    if expected_size == 0 {
        return Ok(0);
    }
    let buf_cap = (expected_size as usize).clamp(1, STORE_STREAM_CHUNK_SIZE);
    let mut buf = vec![0u8; buf_cap];
    let mut remaining = expected_size;
    let mut crc = 0u32;

    while remaining > 0 {
        let to_read = (remaining as usize).min(buf.len());
        let n = file
            .read(&mut buf[..to_read])
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        if n == 0 {
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        let chunk = &buf[..n];
        crc = crc32_fast(crc, chunk);
        pwrite_exact_slice(fd, chunk, payload_offset)?;
        payload_offset += n as u64;
        remaining -= n as u64;
    }
    Ok(crc)
}

fn write_single_store_entry(fd: i32, item: &StorePlanItem) -> Result<u32, TTZipStatus> {
    let (dos_time, dos_date) = unix_to_dos_time(item.mtime_secs);
    let name_bytes = item.rel_path.as_bytes();
    let use_zip64 = item.size >= 0xFFFFFFFF || item.lfh_offset >= 0xFFFFFFFF;
    let extra_bytes = build_store_lfh_extra(item);
    let payload_offset = item.lfh_offset + (item.lfh_header_size as u64);

    let crc32 = if let Some(ref src) = item.src_path {
        stream_store_entry_payload(fd, src, item.size, payload_offset)?
    } else {
        0
    };

    let uncomp_size_field = if item.size >= 0xFFFFFFFF { 0xFFFFFFFFu32 } else { item.size as u32 };
    let version_needed = if use_zip64 { 45u16 } else { 10u16 };

    let mut header = Vec::with_capacity(item.lfh_header_size);
    header.extend_from_slice(&MAGIC_LFH.to_le_bytes());
    header.extend_from_slice(&version_needed.to_le_bytes());
    header.extend_from_slice(&0x0800u16.to_le_bytes());
    header.extend_from_slice(&0u16.to_le_bytes());
    header.extend_from_slice(&dos_time.to_le_bytes());
    header.extend_from_slice(&dos_date.to_le_bytes());
    header.extend_from_slice(&crc32.to_le_bytes());
    header.extend_from_slice(&uncomp_size_field.to_le_bytes());
    header.extend_from_slice(&uncomp_size_field.to_le_bytes());
    header.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    header.extend_from_slice(&(extra_bytes.len() as u16).to_le_bytes());
    header.extend_from_slice(name_bytes);
    header.extend_from_slice(&extra_bytes);

    pwrite_exact_slice(fd, &header, item.lfh_offset)?;
    Ok(crc32)
}

fn write_store_entries_parallel(
    fd: i32,
    plan_items: &[StorePlanItem],
    raw_thread_budget: usize,
) -> Result<Vec<u32>, TTZipStatus> {
    let total_items = plan_items.len();
    let thread_budget = raw_thread_budget.clamp(1, 64).min(total_items).max(1);

    if thread_budget <= 1 || total_items <= 1 {
        return plan_items
            .iter()
            .map(|item| write_single_store_entry(fd, item))
            .collect();
    }

    let chunk_size = total_items.div_ceil(thread_budget);
    let chunks: Vec<_> = plan_items.chunks(chunk_size).collect();
    let num_chunks = chunks.len();
    let chunk_results: parking_lot::Mutex<Vec<Option<Vec<u32>>>> =
        parking_lot::Mutex::new((0..num_chunks).map(|_| None).collect());
    let first_error: parking_lot::Mutex<Option<TTZipStatus>> = parking_lot::Mutex::new(None);
    let pool = LocalWorkStealingPool::with_threads(thread_budget);

    pool.scope(|s| {
        for (chunk_idx, chunk) in chunks.into_iter().enumerate() {
            let results_ref = &chunk_results;
            let err_ref = &first_error;
            s.spawn(move || {
                process_store_chunk(fd, chunk_idx, chunk, results_ref, err_ref);
            });
        }
    });

    if let Some(err) = first_error.into_inner() {
        return Err(err);
    }

    let mut results_vec = Vec::with_capacity(total_items);
    for chunk_opt in chunk_results.into_inner() {
        results_vec.extend(chunk_opt.ok_or(TTZipStatus::ErrCompressionFailed)?);
    }
    Ok(results_vec)
}

fn process_store_chunk(
    fd: i32,
    chunk_idx: usize,
    chunk: &[StorePlanItem],
    results_ref: &parking_lot::Mutex<Vec<Option<Vec<u32>>>>,
    err_ref: &parking_lot::Mutex<Option<TTZipStatus>>,
) {
    let mut local_res = Vec::with_capacity(chunk.len());
    for item in chunk {
        if err_ref.lock().is_some() {
            return;
        }
        match write_single_store_entry(fd, item) {
            Ok(crc) => local_res.push(crc),
            Err(e) => {
                let mut guard = err_ref.lock();
                if guard.is_none() {
                    *guard = Some(e);
                }
                return;
            }
        }
    }
    let mut guard = results_ref.lock();
    guard[chunk_idx] = Some(local_res);
}

fn build_store_central_directory_and_eocd(
    plan_items: &[StorePlanItem],
    crcs: &[u32],
    cd_offset: u64,
    cd_size: u64,
    is_zip64_required: bool,
) -> Vec<u8> {
    let mut cd_bytes = Vec::with_capacity(cd_size as usize + 128);
    for (i, item) in plan_items.iter().enumerate() {
        append_store_cdfh_record(&mut cd_bytes, item, crcs[i]);
    }

    let num_entries = plan_items.len() as u64;
    if is_zip64_required {
        let z64_eocd_pos = cd_offset + (cd_bytes.len() as u64);
        cd_bytes.extend_from_slice(&MAGIC_ZIP64_EOCD.to_le_bytes());
        cd_bytes.extend_from_slice(&44u64.to_le_bytes());
        cd_bytes.extend_from_slice(&45u16.to_le_bytes());
        cd_bytes.extend_from_slice(&45u16.to_le_bytes());
        cd_bytes.extend_from_slice(&0u32.to_le_bytes());
        cd_bytes.extend_from_slice(&0u32.to_le_bytes());
        cd_bytes.extend_from_slice(&num_entries.to_le_bytes());
        cd_bytes.extend_from_slice(&num_entries.to_le_bytes());
        cd_bytes.extend_from_slice(&cd_size.to_le_bytes());
        cd_bytes.extend_from_slice(&cd_offset.to_le_bytes());

        cd_bytes.extend_from_slice(&MAGIC_ZIP64_LOCATOR.to_le_bytes());
        cd_bytes.extend_from_slice(&0u32.to_le_bytes());
        cd_bytes.extend_from_slice(&z64_eocd_pos.to_le_bytes());
        cd_bytes.extend_from_slice(&1u32.to_le_bytes());
    }

    let entries_field = if num_entries >= 0xFFFF { 0xFFFFu16 } else { num_entries as u16 };
    let cd_size_field = if cd_size >= 0xFFFFFFFF { 0xFFFFFFFFu32 } else { cd_size as u32 };
    let cd_offset_field = if cd_offset >= 0xFFFFFFFF { 0xFFFFFFFFu32 } else { cd_offset as u32 };

    cd_bytes.extend_from_slice(&MAGIC_EOCD.to_le_bytes());
    cd_bytes.extend_from_slice(&0u16.to_le_bytes());
    cd_bytes.extend_from_slice(&0u16.to_le_bytes());
    cd_bytes.extend_from_slice(&entries_field.to_le_bytes());
    cd_bytes.extend_from_slice(&entries_field.to_le_bytes());
    cd_bytes.extend_from_slice(&cd_size_field.to_le_bytes());
    cd_bytes.extend_from_slice(&cd_offset_field.to_le_bytes());
    cd_bytes.extend_from_slice(&0u16.to_le_bytes());
    cd_bytes
}

fn append_store_cdfh_record(cd_bytes: &mut Vec<u8>, item: &StorePlanItem, crc32: u32) {
    let (dos_time, dos_date) = unix_to_dos_time(item.mtime_secs);
    let name_bytes = item.rel_path.as_bytes();
    let use_zip64 = item.size >= 0xFFFFFFFF || item.lfh_offset >= 0xFFFFFFFF;

    let mut extra_bytes = Vec::new();
    if use_zip64 {
        let u_sz = (item.size >= 0xFFFFFFFF).then_some(item.size);
        let off = (item.lfh_offset >= 0xFFFFFFFF).then_some(item.lfh_offset);
        extra_bytes.extend_from_slice(&ZipExtraFields::build_zip64_extra(u_sz, u_sz, off));
    }
    extra_bytes.extend_from_slice(&ZipExtraFields::build_extended_timestamp(item.mtime_secs));

    let version_made_by = 0x031Eu16;
    let version_needed = if use_zip64 { 45u16 } else { 10u16 };
    let uncomp_size_field = if item.size >= 0xFFFFFFFF { 0xFFFFFFFFu32 } else { item.size as u32 };
    let lfh_offset_field = if item.lfh_offset >= 0xFFFFFFFF { 0xFFFFFFFFu32 } else { item.lfh_offset as u32 };
    let ext_attr = (item.mode << 16) | if item.is_directory { 0x10 } else { 0 };

    cd_bytes.extend_from_slice(&MAGIC_CDFH.to_le_bytes());
    cd_bytes.extend_from_slice(&version_made_by.to_le_bytes());
    cd_bytes.extend_from_slice(&version_needed.to_le_bytes());
    cd_bytes.extend_from_slice(&0x0800u16.to_le_bytes());
    cd_bytes.extend_from_slice(&0u16.to_le_bytes());
    cd_bytes.extend_from_slice(&dos_time.to_le_bytes());
    cd_bytes.extend_from_slice(&dos_date.to_le_bytes());
    cd_bytes.extend_from_slice(&crc32.to_le_bytes());
    cd_bytes.extend_from_slice(&uncomp_size_field.to_le_bytes());
    cd_bytes.extend_from_slice(&uncomp_size_field.to_le_bytes());
    cd_bytes.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    cd_bytes.extend_from_slice(&(extra_bytes.len() as u16).to_le_bytes());
    cd_bytes.extend_from_slice(&0u16.to_le_bytes());
    cd_bytes.extend_from_slice(&0u16.to_le_bytes());
    cd_bytes.extend_from_slice(&0u16.to_le_bytes());
    cd_bytes.extend_from_slice(&ext_attr.to_le_bytes());
    cd_bytes.extend_from_slice(&lfh_offset_field.to_le_bytes());
    cd_bytes.extend_from_slice(name_bytes);
    cd_bytes.extend_from_slice(&extra_bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_parallel_streamed_multi_chunk_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src_file = dir.path().join("large_store.bin");
        let payload: Vec<u8> = (0..(600 * 1024)).map(|i| ((i * 31 + 7) & 0xFF) as u8).collect();
        fs::write(&src_file, &payload).expect("write src");

        let zip_path = dir.path().join("out_store.zip");
        let opts = TTZipCreateOptions {
            thread_budget: 2,
            ..Default::default()
        };
        let report =
            create_zip_store_parallel(&zip_path, &[src_file], &opts).expect("store parallel");
        assert_eq!(report.total_entries, 1);
        assert_eq!(report.total_uncompressed_bytes, payload.len() as u64);

        let zip_bytes = fs::read(&zip_path).expect("read zip");
        let archive = crate::zip::reader::ZipArchive::open_slice(&zip_bytes).expect("open zip");
        assert_eq!(archive.len(), 1);
        let extracted = archive.extract_entry_bytes(0, None).expect("extract");
        assert_eq!(extracted, payload);
    }
}
