// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Parallel batch execution and adaptive dynamic batching engine for streaming ZIP compression.
//!
//! Provides:
//! - Small-file dynamic batch aggregation (<4KB threshold) reducing Rayon dispatch overhead.
//! - Memory-bounded batch sizing with byte budget limits (4MB default).
//! - Parallel execution via Rayon work-stealing thread pools with graceful sequential fallback.
//! - Filesystem scanning and plan item collection for explicit and recursive paths.

use super::data_descriptor::compute_zipcrypto_check_byte;
use super::streaming_sink::*;
use super::types::unix_to_dos_time;
use crate::codecs::libdeflate::writer::LibdeflateWriter;
use crate::codecs::zstd::stream::ZstdStreamWriter;
use crate::crypto::crc32::crc32_fast;
use ttzip_types::options::ProgressAtomicSheet;
use crate::types::{
    TTZipArchiveFormat, TTZipEncryptionMethod, TTZipStatus,
};
use crate::zip::extra::ZipExtraFields;
use crate::zip::parser::MAGIC_LFH;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

/// File size threshold (4KB) below which an entry is classified as a small file.
pub const SMALL_FILE_THRESHOLD_BYTES: u64 = 4096;

/// Default batch size for medium to large entries (>= 4KB) to strictly bound resident memory.
pub const DEFAULT_BATCH_SIZE: usize = 16;

/// Maximum batch size for high-frequency small entries (< 4KB) to minimize Rayon scheduling overhead.
pub const MAX_SMALL_FILE_BATCH_SIZE: usize = 256;

/// Maximum cumulative uncompressed byte budget per batch (4MB) to prevent transient memory spikes.
pub const BATCH_BYTE_BUDGET: u64 = 4 * 1024 * 1024;

/// An item planned for compression within a batch.
#[derive(Debug, Clone)]
pub struct CompressionPlanItem {
    pub abs_path: PathBuf,
    pub rel_path: String,
    pub uncompressed_size: u64,
    pub mtime_secs: u32,
    pub mode: u32,
    pub is_directory: bool,
    pub is_symlink: bool,
    pub symlink_target: Option<String>,
}

/// Result of compressing a single entry, holding its payload and metadata for disk landing.
pub struct CompressedEntryResult {
    pub rel_path: String,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub compression_method: u16,
    pub actual_method: u16,
    pub is_encrypted: bool,
    pub mtime_secs: u32,
    pub mode: u32,
    pub is_directory: bool,
    pub header_bytes: Vec<u8>,
    pub payload: EntryPayload,
}

/// Extracts an adaptively sized batch of items from a plan item iterator.
///
/// For small files (< 4KB), directories, and symlinks, aggregates up to `MAX_SMALL_FILE_BATCH_SIZE` (256)
/// items into a single parallel batch or until reaching `BATCH_BYTE_BUDGET` (4MB), drastically reducing
/// thread pool dispatch and synchronization overhead.
/// For larger files (>= 4KB), restricts the batch to `DEFAULT_BATCH_SIZE` (16) items to strictly bound
/// intermediate resident memory.
pub fn take_adaptive_batch<I>(iter: &mut std::iter::Peekable<I>) -> Vec<CompressionPlanItem>
where
    I: Iterator<Item = CompressionPlanItem>,
{
    let mut batch: Vec<CompressionPlanItem> = Vec::new();
    let mut total_bytes: u64 = 0;

    while let Some(peek) = iter.peek() {
        let is_small = peek.is_directory
            || peek.is_symlink
            || peek.uncompressed_size < SMALL_FILE_THRESHOLD_BYTES;

        let max_items = if is_small {
            MAX_SMALL_FILE_BATCH_SIZE
        } else {
            DEFAULT_BATCH_SIZE
        };

        if !batch.is_empty() {
            if batch.len() >= max_items || total_bytes.saturating_add(peek.uncompressed_size) > BATCH_BYTE_BUDGET {
                break;
            }
            let batch_is_small = batch.iter().all(|i| {
                i.is_directory || i.is_symlink || i.uncompressed_size < SMALL_FILE_THRESHOLD_BYTES
            });
            if batch_is_small && !is_small && batch.len() >= DEFAULT_BATCH_SIZE {
                break;
            }
        }

        let item = iter.next().unwrap();
        total_bytes = total_bytes.saturating_add(item.uncompressed_size);
        batch.push(item);

        if (!is_small && batch.len() >= DEFAULT_BATCH_SIZE)
            || (is_small && batch.len() >= MAX_SMALL_FILE_BATCH_SIZE)
            || total_bytes >= BATCH_BYTE_BUDGET
        {
            break;
        }
    }

    batch
}

// Preserve existing public paths while the executor owns scheduling responsibilities.
pub use super::batch_execution::{
    execute_batch_parallel, execute_batch_parallel_with_flag,
    execute_batch_parallel_with_options, BatchCompressionOptions,
};

#[inline(always)]
fn record_chunk_progress(
    sheet: Option<&ProgressAtomicSheet>,
    delta: u64,
    start: Option<std::time::Instant>,
) {
    if let Some(s) = sheet {
        let done = s.add_processed(delta);
        let elapsed = start.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
        let instant_bps = if elapsed > 0.001 {
            (done as f64 / elapsed) as u64
        } else {
            0
        };
        s.update_progress(done, instant_bps);
    }
}

fn payload_to_memory_bytes(payload: EntryPayload) -> Result<Vec<u8>, TTZipStatus> {
    match payload {
        EntryPayload::Memory(v) => Ok(v),
        EntryPayload::Spill(spill) => {
            if spill.len > MAX_IN_MEMORY_PAYLOAD as u64 {
                return Err(TTZipStatus::ErrOutOfMemory);
            }
            fs::read(&spill.path).map_err(|_| TTZipStatus::ErrOpenFailed)
        }
        EntryPayload::OriginalFile { path, len } => {
            if len > MAX_IN_MEMORY_PAYLOAD as u64 {
                return Err(TTZipStatus::ErrOutOfMemory);
            }
            fs::read(path).map_err(|_| TTZipStatus::ErrOpenFailed)
        }
    }
}

/// Compresses a single plan item into a local file header and bounded payload.
/// Periodically polls cancellation probe and updates atomic progress sheet on each 64KB chunk.
pub fn compress_single_item(
    item: CompressionPlanItem,
    level: i32,
    format: TTZipArchiveFormat,
    encryption: TTZipEncryptionMethod,
    password: Option<&str>,
    sheet: Option<&ProgressAtomicSheet>,
    start_time: Option<std::time::Instant>,
) -> Result<CompressedEntryResult, TTZipStatus> {
    if let Some(s) = sheet {
        if s.is_cancelled() {
            return Err(TTZipStatus::Cancelled);
        }
    }

    let CompressionPlanItem {
        abs_path,
        rel_path,
        uncompressed_size: _,
        mtime_secs,
        mode,
        is_directory,
        is_symlink,
        symlink_target,
    } = item;

    if is_directory {
        let (dos_time, dos_date) = unix_to_dos_time(mtime_secs);
        let name_bytes = rel_path.as_bytes();
        let ts_extra = ZipExtraFields::build_extended_timestamp(mtime_secs);
        let mut header = Vec::with_capacity(30 + name_bytes.len() + ts_extra.len());
        for part in [
            &MAGIC_LFH.to_le_bytes()[..],
            &20u16.to_le_bytes(),
            &0x0800u16.to_le_bytes(),
            &0u16.to_le_bytes(),
            &dos_time.to_le_bytes(),
            &dos_date.to_le_bytes(),
            &[0u8; 12],
            &(name_bytes.len() as u16).to_le_bytes(),
            &(ts_extra.len() as u16).to_le_bytes(),
            name_bytes,
            &ts_extra,
        ] {
            header.extend_from_slice(part);
        }

        return Ok(CompressedEntryResult {
            rel_path,
            uncompressed_size: 0,
            compressed_size: 0,
            crc32: 0,
            compression_method: 0,
            actual_method: 0,
            is_encrypted: false,
            mtime_secs,
            mode,
            is_directory: true,
            header_bytes: header,
            payload: EntryPayload::Memory(Vec::new()),
        });
    }

    let (actual_method, uncompressed_size, crc, payload) = if is_symlink {
        let sym_bytes = symlink_target.unwrap_or_default().into_bytes();
        let len = sym_bytes.len() as u64;
        let c = crc32_fast(0, &sym_bytes);
        record_chunk_progress(sheet, len, start_time);
        (0u16, len, c, EntryPayload::Memory(sym_bytes))
    } else {
        if let Some(s) = sheet {
            if s.is_cancelled() {
                return Err(TTZipStatus::Cancelled);
            }
        }
        let mut file = File::open(&abs_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        let meta = file.metadata().map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let file_len = meta.len();

        let use_zstd = format == TTZipArchiveFormat::Zstd;
        let target_method = if level == 0 || file_len == 0 {
            0u16
        } else if use_zstd {
            93u16
        } else {
            8u16
        };

        let mut read_buf = [0u8; STREAM_CHUNK_SIZE];
        let mut computed_crc = 0u32;
        let mut total_read = 0u64;

        enum TargetWriter {
            Store,
            Zstd(ZstdStreamWriter<BoundedSink>),
            Deflate(LibdeflateWriter<BoundedSink>),
        }

        let mut target_writer = match target_method {
            0 => TargetWriter::Store,
            93 => TargetWriter::Zstd(ZstdStreamWriter::with_level(BoundedSink::new(), level.min(19))?),
            _ => TargetWriter::Deflate(LibdeflateWriter::new_raw(BoundedSink::new(), level.min(12))?),
        };

        loop {
            if let Some(s) = sheet {
                if s.is_cancelled() {
                    return Err(TTZipStatus::Cancelled);
                }
            }
            let n = file.read(&mut read_buf).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            if n == 0 {
                break;
            }
            let chunk = &read_buf[..n];
            computed_crc = crc32_fast(computed_crc, chunk);
            total_read += n as u64;
            match &mut target_writer {
                TargetWriter::Store => {}
                TargetWriter::Zstd(w) => {
                    w.write_all(chunk).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                }
                TargetWriter::Deflate(w) => {
                    w.write_all(chunk).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                }
            }
            record_chunk_progress(sheet, n as u64, start_time);
        }

        match target_writer {
            TargetWriter::Store => (
                0u16,
                total_read,
                computed_crc,
                EntryPayload::OriginalFile {
                    path: abs_path,
                    len: total_read,
                },
            ),
            TargetWriter::Zstd(w) => {
                let sink = w.finish()?;
                if sink.total_written < total_read {
                    (93u16, total_read, computed_crc, sink.into_payload())
                } else {
                    (
                        0u16,
                        total_read,
                        computed_crc,
                        EntryPayload::OriginalFile {
                            path: abs_path,
                            len: total_read,
                        },
                    )
                }
            }
            TargetWriter::Deflate(w) => {
                let sink = w.finish().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                if sink.total_written < total_read {
                    (8u16, total_read, computed_crc, sink.into_payload())
                } else {
                    (
                        0u16,
                        total_read,
                        computed_crc,
                        EntryPayload::OriginalFile {
                            path: abs_path,
                            len: total_read,
                        },
                    )
                }
            }
        }
    };

    if let Some(s) = sheet {
        if s.is_cancelled() {
            return Err(TTZipStatus::Cancelled);
        }
    }

    let (comp_method, is_encrypted, final_payload) = match encryption {
        TTZipEncryptionMethod::Aes256 => {
            if let Some(s) = sheet {
                if s.is_cancelled() {
                    return Err(TTZipStatus::Cancelled);
                }
            }
            let pass = password.ok_or(TTZipStatus::ErrInvalidPassword)?;
            let raw_bytes = payload_to_memory_bytes(payload)?;
            let mut salt = [0u8; 16];
            unsafe {
                libc::arc4random_buf(salt.as_mut_ptr() as *mut libc::c_void, 16);
            }
            let mut enc_payload = Vec::new();
            crate::crypto::sha1::winzip_aes256_encrypt_and_tag(
                pass,
                &salt,
                &raw_bytes,
                &mut enc_payload,
            )?;
            (99u16, true, EntryPayload::Memory(enc_payload))
        }
        TTZipEncryptionMethod::ZipCrypto => {
            if let Some(s) = sheet {
                if s.is_cancelled() {
                    return Err(TTZipStatus::Cancelled);
                }
            }
            let pass = password.ok_or(TTZipStatus::ErrInvalidPassword)?;
            let raw_bytes = payload_to_memory_bytes(payload)?;
            let mut enc_payload = Vec::with_capacity(12 + raw_bytes.len());
            let mut header = [0u8; 12];
            unsafe {
                libc::arc4random_buf(header.as_mut_ptr() as *mut libc::c_void, 11);
            }
            let (dos_time, _) = unix_to_dos_time(mtime_secs);
            header[11] = compute_zipcrypto_check_byte(crc, dos_time, true);
            let mut keys =
                crate::crypto::zipcrypto::ZipCryptoKeys::from_password(pass.as_bytes());
            keys.encrypt_slice(&mut header);
            enc_payload.extend_from_slice(&header);
            let mut body = raw_bytes;
            keys.encrypt_slice(&mut body);
            enc_payload.extend_from_slice(&body);
            (actual_method, true, EntryPayload::Memory(enc_payload))
        }
        _ => (actual_method, false, payload),
    };

    let compressed_size = final_payload.len();
    let (dos_time, dos_date) = unix_to_dos_time(mtime_secs);
    let name_bytes = rel_path.as_bytes();

    let is_zip64 = uncompressed_size >= 0xFFFF_FFFF || compressed_size >= 0xFFFF_FFFF;
    let mut extra_fields = Vec::new();
    if is_zip64 {
        extra_fields.extend_from_slice(&ZipExtraFields::build_zip64_extra(
            Some(uncompressed_size),
            Some(compressed_size),
            None,
        ));
    }
    if is_encrypted && comp_method == 99 {
        extra_fields.extend_from_slice(&ZipExtraFields::build_winzip_aes_extra(actual_method));
    }
    extra_fields.extend_from_slice(&ZipExtraFields::build_extended_timestamp(mtime_secs));

    let flag = if is_encrypted { 0x0809u16 } else { 0x0808u16 };
    let ver = if is_zip64 || is_encrypted { 45u16 } else { 20u16 };
    let mut header = Vec::with_capacity(30 + name_bytes.len() + extra_fields.len());
    for part in [
        &MAGIC_LFH.to_le_bytes()[..],
        &ver.to_le_bytes(),
        &flag.to_le_bytes(),
        &comp_method.to_le_bytes(),
        &dos_time.to_le_bytes(),
        &dos_date.to_le_bytes(),
        &[0u8; 12],
        &(name_bytes.len() as u16).to_le_bytes(),
        &(extra_fields.len() as u16).to_le_bytes(),
        name_bytes,
        &extra_fields,
    ] {
        header.extend_from_slice(part);
    }

    Ok(CompressedEntryResult {
        rel_path,
        uncompressed_size,
        compressed_size,
        crc32: crc,
        compression_method: comp_method,
        actual_method,
        is_encrypted,
        mtime_secs,
        mode,
        is_directory: false,
        header_bytes: header,
        payload: final_payload,
    })
}

/// Recursively traverses input source paths and aggregates compression plan items.
pub fn collect_source_plan_items(source_paths: &[PathBuf]) -> Result<Vec<CompressionPlanItem>, TTZipStatus> {
    let mut plan_items = Vec::new();
    for src in source_paths {
        if !src.exists() && fs::symlink_metadata(src).is_err() {
            return Err(TTZipStatus::ErrFileNotFound);
        }
        let base_parent = src.parent().unwrap_or(src);
        collect_plan_items_recursive(base_parent, src, &mut plan_items)?;
    }
    Ok(plan_items)
}

/// Collects compression plan items under a directory tree using an explicit heap worklist (O(1) stack).
pub fn collect_plan_items_recursive(
    base_parent: &Path,
    start: &Path,
    out: &mut Vec<CompressionPlanItem>,
) -> Result<(), TTZipStatus> {
    let mut stack: Vec<PathBuf> = vec![start.to_path_buf()];

    while let Some(current) = stack.pop() {
        let meta = fs::symlink_metadata(&current).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        let is_dir = meta.is_dir();
        let is_symlink = meta.file_type().is_symlink();
        let size = if is_dir { 0 } else { meta.len() };
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as u32)
            .unwrap_or(0);

        #[cfg(unix)]
        let mode = meta.mode();
        #[cfg(not(unix))]
        let mode = if is_dir { 0o755 } else { 0o644 };

        let rel_prefix = current.strip_prefix(base_parent).unwrap_or(&current);
        let mut rel = rel_prefix.to_string_lossy().to_string();
        if is_dir && !rel.ends_with('/') {
            rel.push('/');
        }

        let symlink_target = if is_symlink {
            fs::read_link(&current).ok().map(|p| p.to_string_lossy().to_string())
        } else {
            None
        };

        if !rel.is_empty() {
            out.push(CompressionPlanItem {
                abs_path: current.clone(),
                rel_path: rel,
                uncompressed_size: size,
                mtime_secs: mtime,
                mode: if mode != 0 { mode } else if is_dir { 0o755 } else { 0o644 },
                is_directory: is_dir,
                is_symlink,
                symlink_target,
            });
        }

        if is_dir && !is_symlink {
            let mut children = Vec::new();
            for entry in fs::read_dir(&current).map_err(|_| TTZipStatus::ErrOpenFailed)? {
                let entry = entry.map_err(|_| TTZipStatus::ErrOpenFailed)?;
                children.push(entry.path());
            }
            children.sort_unstable_by(|a, b| b.cmp(a));
            stack.extend(children);
        }
    }
    Ok(())
}

/// Normalizes explicit path pairs into validated compression plan items.
pub fn build_explicit_plan_items(
    planned_entries: &[(PathBuf, String)],
) -> Result<Vec<CompressionPlanItem>, TTZipStatus> {
    let mut plan_items = Vec::with_capacity(planned_entries.len());
    for (abs_path, rel_path) in planned_entries {
        if let Ok(meta) = fs::symlink_metadata(abs_path) {
            let is_symlink = meta.file_type().is_symlink();
            let is_directory = meta.is_dir() && !is_symlink;
            let uncompressed_size = if is_directory || is_symlink { 0 } else { meta.len() };
            let mtime_secs = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as u32)
                .unwrap_or(0);
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                let m = meta.permissions().mode();
                if is_directory && (m & 0o170000) == 0 { m | 0o040755 } else { m }
            };
            #[cfg(not(unix))]
            let mode = if is_directory { 0o040755 } else { 0o100644 };
            let symlink_target = if is_symlink {
                fs::read_link(abs_path).ok().map(|p| p.to_string_lossy().into_owned())
            } else {
                None
            };
            let final_rel = crate::fs::safe_extract::validate_and_normalize_zip_rel_path(rel_path, is_directory)?;
            plan_items.push(CompressionPlanItem {
                abs_path: abs_path.clone(),
                rel_path: final_rel,
                uncompressed_size,
                mtime_secs,
                mode,
                is_directory,
                is_symlink,
                symlink_target,
            });
        }
    }
    Ok(plan_items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dummy_item(size: u64, is_dir: bool) -> CompressionPlanItem {
        CompressionPlanItem {
            abs_path: PathBuf::from("dummy"),
            rel_path: "dummy".to_string(),
            uncompressed_size: size,
            mtime_secs: 0,
            mode: 0o644,
            is_directory: is_dir,
            is_symlink: false,
            symlink_target: None,
        }
    }

    #[test]
    fn test_take_adaptive_batch_small_files_expands_to_256() {
        let items: Vec<_> = (0..300).map(|_| make_dummy_item(100, false)).collect();
        let mut iter = items.into_iter().peekable();

        let batch1 = take_adaptive_batch(&mut iter);
        assert_eq!(batch1.len(), MAX_SMALL_FILE_BATCH_SIZE);

        let batch2 = take_adaptive_batch(&mut iter);
        assert_eq!(batch2.len(), 300 - MAX_SMALL_FILE_BATCH_SIZE);
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_take_adaptive_batch_large_files_bounded_to_16() {
        let items: Vec<_> = (0..50).map(|_| make_dummy_item(10 * 1024, false)).collect();
        let mut iter = items.into_iter().peekable();

        let batch1 = take_adaptive_batch(&mut iter);
        assert_eq!(batch1.len(), DEFAULT_BATCH_SIZE);

        let batch2 = take_adaptive_batch(&mut iter);
        assert_eq!(batch2.len(), DEFAULT_BATCH_SIZE);
    }

    #[test]
    fn test_take_adaptive_batch_byte_budget_enforcement() {
        // Two 3MB items should not be packed into a single 4MB budget batch
        let items = vec![
            make_dummy_item(3 * 1024 * 1024, false),
            make_dummy_item(3 * 1024 * 1024, false),
        ];
        let mut iter = items.into_iter().peekable();

        let batch1 = take_adaptive_batch(&mut iter);
        assert_eq!(batch1.len(), 1);

        let batch2 = take_adaptive_batch(&mut iter);
        assert_eq!(batch2.len(), 1);
    }

    fn make_dummy_file_item(path: PathBuf, name: &str, size: u64) -> CompressionPlanItem {
        CompressionPlanItem {
            abs_path: path,
            rel_path: name.to_string(),
            uncompressed_size: size,
            mtime_secs: 0,
            mode: 0o644,
            is_directory: false,
            is_symlink: false,
            symlink_target: None,
        }
    }

    #[test]
    fn test_execute_batch_parallel_cancellation_probe() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("cancel_test.bin");
        fs::write(&file_path, vec![0x55u8; 128 * 1024]).unwrap();

        let item = make_dummy_file_item(file_path, "cancel_test.bin", 128 * 1024);
        let sheet = ProgressAtomicSheet::new();
        sheet.cancel();

        let results = execute_batch_parallel(
            vec![item], 6, TTZipArchiveFormat::Zip, TTZipEncryptionMethod::None, None, Some(&sheet),
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].as_ref().err(), Some(&TTZipStatus::Cancelled));
    }

    #[test]
    fn test_execute_batch_parallel_progress_sheet_tracking() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("progress_test.bin");
        let payload = vec![0xAAu8; 256 * 1024];
        fs::write(&file_path, &payload).unwrap();

        let item = make_dummy_file_item(file_path, "progress_test.bin", payload.len() as u64);
        let sheet = ProgressAtomicSheet::with_total(payload.len() as u64);

        let results = execute_batch_parallel(
            vec![item], 1, TTZipArchiveFormat::Zip, TTZipEncryptionMethod::None, None, Some(&sheet),
        );
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());

        let snapshot = sheet.read_snapshot();
        assert_eq!(snapshot.processed_bytes, payload.len() as u64);
        assert_eq!(snapshot.total_bytes, payload.len() as u64);
        assert!(!snapshot.is_cancelled);
    }
}
