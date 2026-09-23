// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

use std::ffi::{CStr, CString};
use std::fs;
use std::path::Path;
use libc::c_void;

use crate::archive::tar::reader::TarArchive;
use crate::ffi::archive_ffi::guards::ArchiveReadGuard;
use crate::ffi::archive_ffi::sys::*;
use crate::sevenz::decoder::archive::SevenZArchive;
use crate::types::TTZipStatus;
use crate::zip::reader::ZipArchive;

/// Extracts a single entry directly into an in-memory buffer without full archive extraction.
pub fn extract_single_entry_memory(
    archive_path: &Path,
    entry_path: Option<&str>,
    entry_index: i64,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    if !archive_path.exists() {
        return Err(TTZipStatus::ErrFileNotFound);
    }

    let source = crate::archive::source::open_archive_source(archive_path)?;

    if let Some(mapped) = source.as_slice() {
        // 1. Fast Path: ZIP Random Seek Table (O(1) Central Directory)
        if is_zip_magic(mapped) {
            let zip = ZipArchive::open_slice(mapped)?;

            let target_idx = if entry_index >= 0 {
                entry_index as usize
            } else if let Some(target_p) = entry_path {
                zip.entries()
                    .iter()
                    .position(|e| e.rel_path == target_p || e.rel_path.trim_start_matches("./") == target_p)
                    .ok_or(TTZipStatus::ErrFileNotFound)?
            } else {
                return Err(TTZipStatus::ErrInvalidParam);
            };

            return zip.extract_entry_bytes(target_idx, password);
        }

        // 2. Fast Path: 7z Solid Stream Random Seek Table
        if is_sevenz_magic(mapped) {
            let sevenz = SevenZArchive::open_slice(mapped)?;

            let target_idx = if entry_index >= 0 {
                entry_index as usize
            } else if let Some(target_p) = entry_path {
                sevenz
                    .seek_index()
                    .get_by_path(target_p)
                    .map(|loc| loc.file_index)
                    .ok_or(TTZipStatus::ErrFileNotFound)?
            } else {
                return Err(TTZipStatus::ErrInvalidParam);
            };

            return sevenz.extract_entry_bytes_stream(target_idx, password);
        }

        // 3. Fast Path: Pure Rust Uncompressed TAR Subslice
        if is_tar_magic(mapped) {
            let tar = TarArchive::open_slice(mapped)?;

            let target_idx = if entry_index >= 0 {
                entry_index as usize
            } else if let Some(target_p) = entry_path {
                tar.entries()
                    .iter()
                    .position(|e| e.path.as_ref() == target_p || e.path.as_ref().trim_start_matches("./") == target_p)
                    .ok_or(TTZipStatus::ErrFileNotFound)?
            } else {
                return Err(TTZipStatus::ErrInvalidParam);
            };

            return tar.extract_entry_bytes(target_idx).map(|slice| slice.to_vec());
        }
    }

    // 4. General Streaming Path: TAR.GZ / TAR.XZ / TAR.ZST / RAR / ISO (Libarchive Stream-Discarding)
    let arch_c = CString::new(archive_path.to_str().ok_or(TTZipStatus::ErrInvalidParam)?)
        .map_err(|_| TTZipStatus::ErrInvalidParam)?;

    unsafe {
        let a = archive_read_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _guard = ArchiveReadGuard(a);

        archive_read_support_format_all(a);
        archive_read_support_format_raw(a);
        archive_read_support_filter_all(a);

        if let Some(pwd) = password {
            if let Ok(c_pwd) = CString::new(pwd) {
                archive_read_add_passphrase(a, c_pwd.as_ptr());
            }
        }

        if archive_read_open_filename(a, arch_c.as_ptr(), 65536) != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        let mut entry: *mut c_void = std::ptr::null_mut();
        let mut curr_idx: i64 = 0;

        while archive_read_next_header(a, &mut entry) == 0 {
            if entry.is_null() {
                break;
            }

            let raw_path = archive_entry_pathname(entry);
            if raw_path.is_null() {
                archive_read_data_skip(a);
                curr_idx += 1;
                continue;
            }

            let entry_rel_str = match CStr::from_ptr(raw_path).to_str() {
                Ok(s) => s,
                Err(_) => {
                    archive_read_data_skip(a);
                    curr_idx += 1;
                    continue;
                }
            };

            let is_match = if entry_index >= 0 {
                curr_idx == entry_index
            } else if let Some(target_p) = entry_path {
                entry_rel_str == target_p || entry_rel_str.trim_start_matches("./") == target_p
            } else {
                false
            };

            if is_match {
                const MAX_IN_MEMORY_DECOMPRESS_BYTES: usize = 64 * 1024 * 1024;
                let size = archive_entry_size(entry).max(0) as usize;
                if size > MAX_IN_MEMORY_DECOMPRESS_BYTES {
                    return Err(TTZipStatus::ErrOutOfMemory);
                }
                let mut payload = Vec::with_capacity(size.min(10 * 1024 * 1024));
                let mut chunk = [0u8; 65536];

                loop {
                    let r = archive_read_data(a, chunk.as_mut_ptr() as *mut c_void, chunk.len());
                    if r < 0 {
                        return Err(TTZipStatus::ErrExtractionFailed);
                    }
                    if r == 0 {
                        break;
                    }
                    if payload.len() + (r as usize) > MAX_IN_MEMORY_DECOMPRESS_BYTES {
                        return Err(TTZipStatus::ErrOutOfMemory);
                    }
                    payload.extend_from_slice(&chunk[..r as usize]);
                }
                return Ok(payload);
            }

            archive_read_data_skip(a);
            curr_idx += 1;
        }

        Err(TTZipStatus::ErrFileNotFound)
    }
}

/// Batch selective extraction to destination directory.
/// Single-pass O(N) stream scan with HashSet lookup and direct-to-disk write.
pub fn extract_selected_entries(
    archive_path: &Path,
    target_paths: &[String],
    destination_dir: &Path,
    options: &crate::types::TTZipExtractOptions,
) -> Result<usize, TTZipStatus> {
    use std::collections::HashSet;
    use std::io::Write;
    use std::ffi::c_void;
    use std::os::unix::io::AsRawFd;
    use crate::ffi::archive_ffi::sys::*;
    use crate::fs::apfs::apfs_preallocate;
    use crate::fs::safe_extract::sanitize_and_validate_path;
    use libc::mode_t;

    if !archive_path.exists() {
        return Err(TTZipStatus::ErrFileNotFound);
    }
    if target_paths.is_empty() {
        return Ok(0);
    }

    if !options.dry_run && !destination_dir.exists() {
        let _ = fs::create_dir_all(destination_dir);
    }

    let target_set: HashSet<&str> = target_paths.iter().map(|s| s.as_str()).collect();
    let mut extracted_count = 0;
    let mut total_processed: u64 = 0;

    let arch_c = CString::new(archive_path.to_str().ok_or(TTZipStatus::ErrInvalidParam)?)
        .map_err(|_| TTZipStatus::ErrInvalidParam)?;

    unsafe {
        let a = archive_read_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _guard = ArchiveReadGuard(a);

        archive_read_support_format_all(a);
        archive_read_support_format_raw(a);
        archive_read_support_filter_all(a);

        if !options.password.is_null() {
            if let Ok(p_str) = CStr::from_ptr(options.password).to_str() {
                if !p_str.is_empty() {
                    archive_read_add_passphrase(a, options.password);
                }
            }
        }

        if archive_read_open_filename(a, arch_c.as_ptr(), 65536) != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        let mut entry: *mut c_void = std::ptr::null_mut();

        while archive_read_next_header(a, &mut entry) == 0 {
            if entry.is_null() {
                break;
            }

            let raw_path = archive_entry_pathname(entry);
            if raw_path.is_null() {
                archive_read_data_skip(a);
                continue;
            }

            // Cancellation check on each archive header traversal
            if let Some(cb) = options.progress_callback {
                if !cb(total_processed, total_processed, raw_path, options.user_data) {
                    return Err(TTZipStatus::Cancelled);
                }
            }

            let entry_rel_str = match CStr::from_ptr(raw_path).to_str() {
                Ok(s) => s,
                Err(_) => {
                    archive_read_data_skip(a);
                    continue;
                }
            };

            let clean_rel = entry_rel_str.trim_start_matches("./");
            let is_match = target_set.contains(entry_rel_str) || target_set.contains(clean_rel);

            if is_match {
                let file_type = archive_entry_filetype(entry);
                let mode = archive_entry_mode(entry) as u32;
                let is_dir = (file_type & (libc::S_IFMT as mode_t)) == (libc::S_IFDIR as mode_t)
                    || (mode & (libc::S_IFMT as u32)) == (libc::S_IFDIR as u32)
                    || entry_rel_str.ends_with('/');

                let sanitized = sanitize_and_validate_path(destination_dir, entry_rel_str)?;

                if is_dir {
                    if !options.dry_run {
                        fs::create_dir_all(&sanitized).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                    }
                    extracted_count += 1;
                } else {
                    if let Some(parent) = sanitized.parent() {
                        if !parent.exists() && !options.dry_run {
                            let _ = fs::create_dir_all(parent);
                        }
                    }

                    if !options.dry_run {
                        let mut out_file = fs::OpenOptions::new()
                            .create(true)
                            .write(true)
                            .truncate(true)
                            .open(&sanitized)
                            .map_err(|_| TTZipStatus::ErrExtractionFailed)?;

                        let size = archive_entry_size(entry).max(0) as u64;
                        if size > 0 {
                            let _ = apfs_preallocate(out_file.as_raw_fd(), size as i64);
                        }

                        let mut chunk = [0u8; 65536];
                        loop {
                            let r = archive_read_data(a, chunk.as_mut_ptr() as *mut c_void, chunk.len());
                            if r < 0 {
                                drop(out_file);
                                let _ = fs::remove_file(&sanitized);
                                return Err(TTZipStatus::ErrExtractionFailed);
                            }
                            if r == 0 {
                                break;
                            }
                            if out_file.write_all(&chunk[..r as usize]).is_err() {
                                drop(out_file);
                                let _ = fs::remove_file(&sanitized);
                                return Err(TTZipStatus::ErrExtractionFailed);
                            }
                            total_processed = total_processed.saturating_add(r as u64);

                            // Cancellation check within 64KB block streaming loop
                            if let Some(cb) = options.progress_callback {
                                if !cb(total_processed, total_processed, raw_path, options.user_data) {
                                    drop(out_file);
                                    let _ = fs::remove_file(&sanitized);
                                    return Err(TTZipStatus::Cancelled);
                                }
                            }
                        }
                        if out_file.flush().is_err() {
                            drop(out_file);
                            let _ = fs::remove_file(&sanitized);
                            return Err(TTZipStatus::ErrExtractionFailed);
                        }
                    } else {
                        archive_read_data_skip(a);
                    }
                    extracted_count += 1;
                }
            } else {
                archive_read_data_skip(a);
            }
        }

        Ok(extracted_count)
    }
}

#[inline]
fn is_zip_magic(mapped: &[u8]) -> bool {
    if mapped.len() < 22 {
        return false;
    }
    if mapped.starts_with(b"PK\x03\x04") || mapped.starts_with(b"PK\x05\x06") {
        return true;
    }
    let search_window = mapped.len().min(65535 + 22);
    let tail = &mapped[mapped.len() - search_window..];
    tail.windows(4).rev().any(|w| w == b"PK\x05\x06")
}

#[inline]
fn is_sevenz_magic(mapped: &[u8]) -> bool {
    mapped.len() >= 6 && &mapped[0..6] == b"7z\xBC\xAF\x27\x1C"
}

#[inline]
fn is_tar_magic(mapped: &[u8]) -> bool {
    mapped.len() >= 262 && &mapped[257..262] == b"ustar"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tempfile::tempdir;
    use crate::archive::unified::UnifiedArchiveOrchestrator;
    use crate::types::{TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod};

    #[test]
    fn test_extract_selected_entries_cancellation_during_chunk_read() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("big_file.bin");
        // 256 KB file, ensuring multiple 64KB chunk reads
        let data = vec![0xA5u8; 256 * 1024];
        fs::write(&src_file, &data).unwrap();

        let archive_path = dir.path().join("test_extract_cancel.tar");
        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            format: TTZipArchiveFormat::Tar,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        UnifiedArchiveOrchestrator::create_archive(
            &[src_file],
            &archive_path,
            &create_opt,
            0,
        ).expect("Failed to create test archive");

        let dest_dir = dir.path().join("extracted");
        fs::create_dir_all(&dest_dir).unwrap();

        static CHUNK_CANCEL_TRIGGERED: AtomicBool = AtomicBool::new(false);
        CHUNK_CANCEL_TRIGGERED.store(false, Ordering::SeqCst);

        unsafe extern "C" fn cancel_on_chunk_cb(
            proc: u64,
            _tot: u64,
            _entry: *const libc::c_char,
            _user_data: *mut c_void,
        ) -> bool {
            if proc > 0 {
                CHUNK_CANCEL_TRIGGERED.store(true, Ordering::SeqCst);
                false
            } else {
                true
            }
        }

        let extract_opt = crate::types::TTZipExtractOptions {
            progress_callback: Some(cancel_on_chunk_cb),
            ..Default::default()
        };

        let res = extract_selected_entries(
            &archive_path,
            &["big_file.bin".to_string()],
            &dest_dir,
            &extract_opt,
        );

        assert_eq!(res, Err(TTZipStatus::Cancelled));
        assert!(CHUNK_CANCEL_TRIGGERED.load(Ordering::SeqCst));

        // Ensure the truncated file was completely destroyed upon cancellation
        let target_extracted_path = dest_dir.join("big_file.bin");
        assert!(!target_extracted_path.exists(), "Truncated file must be destroyed upon cancellation");
    }

    #[test]
    fn test_extract_selected_entries_cancellation_on_header() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("small_file.bin");
        fs::write(&src_file, b"content").unwrap();

        let archive_path = dir.path().join("test_extract_cancel_header.tar");
        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            format: TTZipArchiveFormat::Tar,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        UnifiedArchiveOrchestrator::create_archive(
            &[src_file],
            &archive_path,
            &create_opt,
            0,
        ).expect("Failed to create test archive");

        let dest_dir = dir.path().join("extracted_header");
        fs::create_dir_all(&dest_dir).unwrap();

        unsafe extern "C" fn cancel_immediate_cb(
            _proc: u64,
            _tot: u64,
            _entry: *const libc::c_char,
            _user_data: *mut c_void,
        ) -> bool {
            false
        }

        let extract_opt = crate::types::TTZipExtractOptions {
            progress_callback: Some(cancel_immediate_cb),
            ..Default::default()
        };

        let res = extract_selected_entries(
            &archive_path,
            &["small_file.bin".to_string()],
            &dest_dir,
            &extract_opt,
        );

        assert_eq!(res, Err(TTZipStatus::Cancelled));
    }

    #[test]
    fn test_create_archive_cancellation_during_chunk_write() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("big_file_for_create.bin");
        // 256 KB file, ensuring multiple 64KB chunk reads during creation
        let data = vec![0x3Cu8; 256 * 1024];
        fs::write(&src_file, &data).unwrap();

        let archive_path = dir.path().join("test_create_cancel.tar.gz");

        static CREATE_CHUNK_CANCEL: AtomicBool = AtomicBool::new(false);
        CREATE_CHUNK_CANCEL.store(false, Ordering::SeqCst);

        unsafe extern "C" fn cancel_after_first_chunk(
            proc: u64,
            _tot: u64,
            _entry: *const libc::c_char,
            _user_data: *mut c_void,
        ) -> bool {
            if proc >= 65536 {
                CREATE_CHUNK_CANCEL.store(true, Ordering::SeqCst);
                false
            } else {
                true
            }
        }

        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            format: TTZipArchiveFormat::TarGz,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: Some(cancel_after_first_chunk),
            user_data: std::ptr::null_mut(),
        };

        let res = UnifiedArchiveOrchestrator::create_archive(
            &[src_file],
            &archive_path,
            &create_opt,
            0,
        );

        assert_eq!(res, Err(TTZipStatus::Cancelled));
        assert!(CREATE_CHUNK_CANCEL.load(Ordering::SeqCst));
    }

    #[test]
    fn test_extract_single_memory_budget_limit() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("budget_test.bin");
        let data = b"selective in-memory extraction payload".to_vec();
        fs::write(&src_file, &data).unwrap();

        let archive_path = dir.path().join("test_budget.tar.gz");
        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            format: TTZipArchiveFormat::TarGz,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        UnifiedArchiveOrchestrator::create_archive(
            &[src_file],
            &archive_path,
            &create_opt,
            0,
        ).expect("Failed to create test archive");

        let res = extract_single_entry_memory(&archive_path, Some("budget_test.bin"), -1, None);
        assert_eq!(res.unwrap(), data);
    }
}



