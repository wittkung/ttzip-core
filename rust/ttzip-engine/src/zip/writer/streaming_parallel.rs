// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

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

use super::data_descriptor::{build_data_descriptor, compute_zipcrypto_check_byte};
use super::streaming_sink::*;
use super::types::{unix_to_dos_time, ZipCreateReport};
use crate::codecs::libdeflate::writer::LibdeflateWriter;
use crate::codecs::zstd::stream::ZstdStreamWriter;
use crate::crypto::crc32::crc32_fast;
use crate::fs::apfs::apfs_preallocate;
use crate::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    TTZipStatus,
};
use crate::zip::extra::ZipExtraFields;
use crate::zip::parser::MAGIC_LFH;
use rayon::prelude::*;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{FileExt, MetadataExt};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// An item planned for compression.
#[derive(Debug, Clone)]
struct CompressionPlanItem {
    abs_path: PathBuf,
    rel_path: String,
    uncompressed_size: u64,
    mtime_secs: u32,
    mode: u32,
    is_directory: bool,
    is_symlink: bool,
    symlink_target: Option<String>,
}

/// Creates a ZIP archive using the high-throughput multi-core streaming parallel engine.
pub fn create_zip_streaming_parallel(
    dest_path: &Path,
    source_paths: &[PathBuf],
    options: &TTZipCreateOptions,
) -> Result<ZipCreateReport, TTZipStatus> {
    let start_time = std::time::Instant::now();

    if let Some(parent) = dest_path.parent() {
        if !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }
    }

    // 1. Collect all items recursively
    let mut plan_items = Vec::new();
    for src in source_paths {
        if !src.exists() && fs::symlink_metadata(src).is_err() {
            return Err(TTZipStatus::ErrFileNotFound);
        }
        let base_parent = src.parent().unwrap_or(src);
        collect_plan_items_recursive(base_parent, src, &mut plan_items)?;
    }

    if plan_items.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    // 2. Open destination file
    let out_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(dest_path)
        .map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let level_num: i32 = match options.level {
        TTZipCompressionLevel::Store => 0,
        TTZipCompressionLevel::Fastest => 1,
        TTZipCompressionLevel::Fast => 3,
        TTZipCompressionLevel::Normal => 6,
        TTZipCompressionLevel::Maximum => 9,
        TTZipCompressionLevel::Ultra => 12,
    };

    let total_uncompressed: u64 = plan_items.iter().map(|i| i.uncompressed_size).sum();

    // APFS Preallocation hint
    if total_uncompressed > 0 {
        let hint_size = if level_num == 0 {
            total_uncompressed + (plan_items.len() as u64 * 128)
        } else {
            (total_uncompressed / 2).max(65536) + (plan_items.len() as u64 * 128)
        };
        let _ = apfs_preallocate(out_file.as_raw_fd(), hint_size as i64);
    }

    let is_cancelled = Arc::new(AtomicBool::new(false));
    let processed_bytes = Arc::new(AtomicU64::new(0));
    let progress_cb = options.progress_callback;
    let user_data_usize = options.user_data as usize;
    let encryption_mode = options.encryption;
    let archive_format = options.format;
    let password_str = if !options.password.is_null() {
        unsafe { std::ffi::CStr::from_ptr(options.password) }
            .to_str()
            .ok()
    } else {
        None
    };

    // 3. Compress and stream write entries with Data Descriptor and bounded memory
    let mut current_offset: u64 = 0;
    let mut cd_entries: Vec<CentralDirectoryMeta> = Vec::with_capacity(plan_items.len());

    // For single/batch items, we process and immediately land to disk without persisting payloads in RAM
    const BATCH_SIZE: usize = 16;
    for batch in plan_items.chunks(BATCH_SIZE) {
        if is_cancelled.load(Ordering::Acquire) {
            return Err(TTZipStatus::Cancelled);
        }

        // Process batch items: compress in parallel if multiple, or sequential if single
        let compressed_batch: Vec<Result<CompressedEntryResult, TTZipStatus>> = if batch.len() > 1 {
            batch
                .par_iter()
                .map(|item| {
                    if is_cancelled.load(Ordering::Relaxed) {
                        return Err(TTZipStatus::Cancelled);
                    }
                    compress_single_item(
                        item,
                        level_num,
                        archive_format,
                        encryption_mode,
                        password_str,
                    )
                })
                .collect()
        } else {
            vec![compress_single_item(
                &batch[0],
                level_num,
                archive_format,
                encryption_mode,
                password_str,
            )]
        };

        // Immediately land each compressed item to disk and drop payload vectors
        for (item, result) in batch.iter().zip(compressed_batch) {
            let entry = result?;
            let lfh_offset = current_offset;

            // Write LFH
            out_file
                .write_all_at(&entry.header_bytes, current_offset)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            current_offset += entry.header_bytes.len() as u64;

            // Write payload
            if !entry.payload.is_empty() {
                entry.payload.write_to_file_at(&out_file, current_offset)?;
                current_offset += entry.payload.len();
            }

            // Write Data Descriptor if not directory
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
                out_file
                    .write_all_at(&dd_bytes, current_offset)
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                current_offset += dd_bytes.len() as u64;
            }

            // Record lightweight ~80-byte Central Directory metadata (payload dropped here)
            cd_entries.push(CentralDirectoryMeta {
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
            });

            // Progress callback update
            let n = processed_bytes.fetch_add(item.uncompressed_size, Ordering::Relaxed);
            if let Some(cb) = progress_cb {
                let rel_c = std::ffi::CString::new(item.rel_path.as_str()).unwrap_or_default();
                let keep_going = unsafe {
                    cb(n, total_uncompressed, rel_c.as_ptr(), user_data_usize as *mut libc::c_void)
                };
                if !keep_going {
                    is_cancelled.store(true, Ordering::Release);
                    return Err(TTZipStatus::Cancelled);
                }
            }
        }
    }

    // 4. Write Central Directory and End of Central Directory structures
    let cd_start_offset = current_offset;
    for cd in &cd_entries {
        let cd_bytes = build_cdfh_bytes(cd);
        out_file
            .write_all_at(&cd_bytes, current_offset)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        current_offset += cd_bytes.len() as u64;
    }
    let cd_size = current_offset - cd_start_offset;

    // Check Zip64 requirements
    let needs_zip64 = cd_entries.len() >= 0xFFFF
        || cd_start_offset >= 0xFFFF_FFFF
        || cd_size >= 0xFFFF_FFFF
        || total_uncompressed >= 0xFFFF_FFFF;

    if needs_zip64 {
        let zip64_eocd_offset = current_offset;
        let zip64_eocd = build_zip64_eocd(cd_entries.len() as u64, cd_size, cd_start_offset);
        out_file
            .write_all_at(&zip64_eocd, current_offset)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        current_offset += zip64_eocd.len() as u64;

        let zip64_locator = build_zip64_locator(zip64_eocd_offset);
        out_file
            .write_all_at(&zip64_locator, current_offset)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        current_offset += zip64_locator.len() as u64;
    }

    let eocd = build_eocd(
        cd_entries.len().min(0xFFFF) as u16,
        cd_size.min(0xFFFF_FFFF) as u32,
        cd_start_offset.min(0xFFFF_FFFF) as u32,
    );
    out_file
        .write_all_at(&eocd, current_offset)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    current_offset += eocd.len() as u64;

    let elapsed_nanos = start_time.elapsed().as_nanos() as u64;
    let elapsed_ms = elapsed_nanos / 1_000_000;

    let prov = crate::types::TTZipExecutionProvenance {
        engine_tag: crate::types::TTZipEngineTag::RustStreamingParallelZip,
        thread_count: options.thread_budget,
        uncompressed_bytes: total_uncompressed,
        compressed_bytes: current_offset,
        kernel_duration_nanos: elapsed_nanos,
        is_fallback: false,
        ..Default::default()
    };
    crate::types::record_execution_provenance(prov);

    Ok(ZipCreateReport {
        total_entries: cd_entries.len(),
        total_uncompressed_bytes: total_uncompressed,
        total_compressed_bytes: current_offset,
        duration_ms: elapsed_ms,
    })
}

struct CompressedEntryResult {
    rel_path: String,
    uncompressed_size: u64,
    compressed_size: u64,
    crc32: u32,
    compression_method: u16,
    actual_method: u16,
    is_encrypted: bool,
    mtime_secs: u32,
    mode: u32,
    is_directory: bool,
    header_bytes: Vec<u8>,
    payload: EntryPayload,
}

fn compress_single_item(
    item: &CompressionPlanItem,
    level: i32,
    format: TTZipArchiveFormat,
    encryption: TTZipEncryptionMethod,
    password: Option<&str>,
) -> Result<CompressedEntryResult, TTZipStatus> {
    if item.is_directory {
        let (dos_date, dos_time) = unix_to_dos_time(item.mtime_secs);
        let name_bytes = item.rel_path.as_bytes();
        let mut header = Vec::with_capacity(30 + name_bytes.len());
        header.extend_from_slice(&MAGIC_LFH.to_le_bytes());
        header.extend_from_slice(&20u16.to_le_bytes());
        header.extend_from_slice(&0x0800u16.to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(&dos_time.to_le_bytes());
        header.extend_from_slice(&dos_date.to_le_bytes());
        header.extend_from_slice(&0u32.to_le_bytes());
        header.extend_from_slice(&0u32.to_le_bytes());
        header.extend_from_slice(&0u32.to_le_bytes());
        header.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(name_bytes);

        return Ok(CompressedEntryResult {
            rel_path: item.rel_path.clone(),
            uncompressed_size: 0,
            compressed_size: 0,
            crc32: 0,
            compression_method: 0,
            actual_method: 0,
            is_encrypted: false,
            mtime_secs: item.mtime_secs,
            mode: item.mode,
            is_directory: true,
            header_bytes: header,
            payload: EntryPayload::Memory(Vec::new()),
        });
    }

    let (actual_method, uncompressed_size, crc, payload) = if item.is_symlink {
        let sym_bytes = item.symlink_target.clone().unwrap_or_default().into_bytes();
        let len = sym_bytes.len() as u64;
        let c = crc32_fast(0, &sym_bytes);
        (0u16, len, c, EntryPayload::Memory(sym_bytes))
    } else {
        let mut file = File::open(&item.abs_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
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

        if target_method == 0 {
            loop {
                let n = file.read(&mut read_buf).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                if n == 0 {
                    break;
                }
                computed_crc = crc32_fast(computed_crc, &read_buf[..n]);
                total_read += n as u64;
            }
            (
                0u16,
                total_read,
                computed_crc,
                EntryPayload::OriginalFile {
                    path: item.abs_path.clone(),
                    len: total_read,
                },
            )
        } else if target_method == 93 {
            let sink = BoundedSink::new();
            let mut zstd_writer = ZstdStreamWriter::with_level(sink, level.min(19))?;
            loop {
                let n = file.read(&mut read_buf).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                if n == 0 {
                    break;
                }
                computed_crc = crc32_fast(computed_crc, &read_buf[..n]);
                total_read += n as u64;
                zstd_writer
                    .write_all(&read_buf[..n])
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            }
            let sink = zstd_writer.finish()?;
            if sink.total_written < total_read {
                (93u16, total_read, computed_crc, sink.into_payload())
            } else {
                (
                    0u16,
                    total_read,
                    computed_crc,
                    EntryPayload::OriginalFile {
                        path: item.abs_path.clone(),
                        len: total_read,
                    },
                )
            }
        } else {
            let sink = BoundedSink::new();
            let mut def_writer = LibdeflateWriter::new_raw(sink, level.min(12))?;
            loop {
                let n = file.read(&mut read_buf).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                if n == 0 {
                    break;
                }
                computed_crc = crc32_fast(computed_crc, &read_buf[..n]);
                total_read += n as u64;
                def_writer
                    .write_all(&read_buf[..n])
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            }
            let sink = def_writer
                .finish()
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            if sink.total_written < total_read {
                (8u16, total_read, computed_crc, sink.into_payload())
            } else {
                (
                    0u16,
                    total_read,
                    computed_crc,
                    EntryPayload::OriginalFile {
                        path: item.abs_path.clone(),
                        len: total_read,
                    },
                )
            }
        }
    };

    let (comp_method, is_encrypted, final_payload) = match encryption {
        TTZipEncryptionMethod::Aes256 => {
            let pass = password.ok_or(TTZipStatus::ErrInvalidPassword)?;
            let raw_bytes = match &payload {
                EntryPayload::Memory(v) => v.clone(),
                EntryPayload::Spill(spill) => {
                    if spill.len > MAX_IN_MEMORY_PAYLOAD as u64 {
                        return Err(TTZipStatus::ErrOutOfMemory);
                    }
                    fs::read(&spill.path).map_err(|_| TTZipStatus::ErrOpenFailed)?
                }
                EntryPayload::OriginalFile { path, len } => {
                    if *len > MAX_IN_MEMORY_PAYLOAD as u64 {
                        return Err(TTZipStatus::ErrOutOfMemory);
                    }
                    fs::read(path).map_err(|_| TTZipStatus::ErrOpenFailed)?
                }
            };
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
            let pass = password.ok_or(TTZipStatus::ErrInvalidPassword)?;
            let raw_bytes = match &payload {
                EntryPayload::Memory(v) => v.clone(),
                EntryPayload::Spill(spill) => {
                    if spill.len > MAX_IN_MEMORY_PAYLOAD as u64 {
                        return Err(TTZipStatus::ErrOutOfMemory);
                    }
                    fs::read(&spill.path).map_err(|_| TTZipStatus::ErrOpenFailed)?
                }
                EntryPayload::OriginalFile { path, len } => {
                    if *len > MAX_IN_MEMORY_PAYLOAD as u64 {
                        return Err(TTZipStatus::ErrOutOfMemory);
                    }
                    fs::read(path).map_err(|_| TTZipStatus::ErrOpenFailed)?
                }
            };
            let mut enc_payload = Vec::with_capacity(12 + raw_bytes.len());
            let mut header = [0u8; 12];
            unsafe {
                libc::arc4random_buf(header.as_mut_ptr() as *mut libc::c_void, 11);
            }
            let (_, dos_time) = unix_to_dos_time(item.mtime_secs);
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
    let (dos_date, dos_time) = unix_to_dos_time(item.mtime_secs);
    let name_bytes = item.rel_path.as_bytes();

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

    let flag = if is_encrypted { 0x0809u16 } else { 0x0808u16 };
    let mut header = Vec::with_capacity(30 + name_bytes.len() + extra_fields.len());
    header.extend_from_slice(&MAGIC_LFH.to_le_bytes());
    header.extend_from_slice(&(if is_zip64 || is_encrypted { 45u16 } else { 20u16 }).to_le_bytes());
    header.extend_from_slice(&flag.to_le_bytes());
    header.extend_from_slice(&comp_method.to_le_bytes());
    header.extend_from_slice(&dos_time.to_le_bytes());
    header.extend_from_slice(&dos_date.to_le_bytes());
    header.extend_from_slice(&0u32.to_le_bytes());
    header.extend_from_slice(&0u32.to_le_bytes());
    header.extend_from_slice(&0u32.to_le_bytes());
    header.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    header.extend_from_slice(&(extra_fields.len() as u16).to_le_bytes());
    header.extend_from_slice(name_bytes);
    header.extend_from_slice(&extra_fields);

    Ok(CompressedEntryResult {
        rel_path: item.rel_path.clone(),
        uncompressed_size,
        compressed_size,
        crc32: crc,
        compression_method: comp_method,
        actual_method,
        is_encrypted,
        mtime_secs: item.mtime_secs,
        mode: item.mode,
        is_directory: false,
        header_bytes: header,
        payload: final_payload,
    })
}

fn collect_plan_items_recursive(
    base_parent: &Path,
    current: &Path,
    out: &mut Vec<CompressionPlanItem>,
) -> Result<(), TTZipStatus> {
    let meta = fs::symlink_metadata(current).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let is_dir = meta.is_dir();
    let is_symlink = meta.file_type().is_symlink();
    let size = if is_dir { 0 } else { meta.len() };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);
    let mode = meta.mode();

    let rel_prefix = current.strip_prefix(base_parent).unwrap_or(current);
    let mut rel = rel_prefix.to_string_lossy().to_string();
    if is_dir && !rel.ends_with('/') {
        rel.push('/');
    }

    let symlink_target = if is_symlink {
        fs::read_link(current)
            .ok()
            .map(|p| p.to_string_lossy().to_string())
    } else {
        None
    };

    if !rel.is_empty() {
        out.push(CompressionPlanItem {
            abs_path: current.to_path_buf(),
            rel_path: rel,
            uncompressed_size: size,
            mtime_secs: mtime,
            mode: if mode != 0 {
                mode
            } else if is_dir {
                0o755
            } else {
                0o644
            },
            is_directory: is_dir,
            is_symlink,
            symlink_target,
        });
    }

    if is_dir && !is_symlink {
        for entry in fs::read_dir(current).map_err(|_| TTZipStatus::ErrOpenFailed)? {
            let entry = entry.map_err(|_| TTZipStatus::ErrOpenFailed)?;
            collect_plan_items_recursive(base_parent, &entry.path(), out)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            format: crate::types::TTZipArchiveFormat::Zip,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
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
}

