// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive non-destructive inspection submodule for Unified Orchestrator.

use std::ffi::CString;
use std::path::Path;

use super::zip_central_directory::{
    inspect_zip_central_directory_file, inspect_zip_central_directory_reader,
};

use crate::ffi::archive_ffi::guards::{archive_entry_pathname_safe, ArchiveReadGuard};
use crate::ffi::archive_ffi::sys::*;
use crate::types::{TTZipEntryMetadata, TTZipInspectCallback, TTZipStatus};
use libc::{c_void, mode_t};

/// Borrowed, zero-heap archive entry metadata slice with lossless raw path retention.
#[derive(Debug, Clone, Copy)]
pub struct ArchiveEntryRef<'a> {
    pub path: &'a str,
    pub raw_path: &'a [u8],
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime_epoch_secs: i64,
    pub mode: u32,
    pub is_directory: bool,
    pub is_encrypted: bool,
    pub compression_method: u16,
    pub detected_encoding: Option<&'a str>,
}

impl<'a> ArchiveEntryRef<'a> {
    /// Returns the pristine raw binary path bytes without lossy string conversion.
    #[inline]
    pub fn raw_bytes(&self) -> &'a [u8] {
        self.raw_path
    }

    /// Checks whether the raw path constitutes valid UTF-8.
    #[inline]
    pub fn is_utf8(&self) -> bool {
        std::str::from_utf8(self.raw_path).is_ok()
    }

    /// Returns `true` if this archive entry is encrypted.
    #[inline]
    #[must_use]
    pub const fn is_encrypted(&self) -> bool {
        self.is_encrypted
    }

    /// Returns `true` if this archive entry represents a directory.
    #[inline]
    #[must_use]
    pub const fn is_directory(&self) -> bool {
        self.is_directory
    }

    /// Returns `true` if this entry requires solid decompression context.
    #[inline]
    #[must_use]
    pub const fn is_solid(&self) -> bool {
        false
    }

    /// Returns `true` if this entry supports single-pass linear streaming extraction.
    #[inline]
    #[must_use]
    pub const fn supports_streaming(&self) -> bool {
        true
    }
}

fn emit_sevenz_header_entries<F>(
    info: &crate::sevenz::header::SevenZHeaderInfo,
    seek_index: &crate::sevenz::header::SevenZSeekIndex,
    detect_encoding: bool,
    callback: &mut F,
) -> usize
where
    F: FnMut(&ArchiveEntryRef<'_>) -> bool,
{
    let mut count = 0;
    for (idx, file) in info.files.iter().enumerate() {
        let mut detected_charset: Option<String> = None;
        if detect_encoding {
            let path_bytes = file.rel_path.as_bytes();
            if !path_bytes.is_ascii() {
                detected_charset = crate::codecs::chardet::detect_charset(path_bytes);
            }
        }
        let detected_encoding = detected_charset.as_deref();
        let loc = seek_index.entries.get(idx);
        let uncomp_sz = loc.map(|l| l.uncompressed_size).unwrap_or(0);
        let crc_val = loc.and_then(|l| l.crc).unwrap_or(0);

        let entry_ref = ArchiveEntryRef {
            path: file.rel_path.as_str(),
            raw_path: file.rel_path.as_bytes(),
            uncompressed_size: uncomp_sz,
            compressed_size: 0,
            crc32: crc_val,
            mtime_epoch_secs: file.mtime_epoch_secs.unwrap_or(0),
            mode: file.mode,
            is_directory: file.is_directory,
            is_encrypted: info.is_encrypted,
            compression_method: info.primary_method_id as u16,
            detected_encoding,
        };
        count += 1;
        if !callback(&entry_ref) {
            break;
        }
    }
    count
}

fn inspect_special_or_compressed_stream<F>(
    archive_path: &Path,
    volume_chain: &[std::path::PathBuf],
    password: Option<&str>,
    detect_encoding: bool,
    callback: F,
) -> Result<Option<usize>, TTZipStatus>
where
    F: FnMut(&ArchiveEntryRef<'_>) -> bool,
{
    if let Some(transcoded) =
        crate::archive::stream_adapter::try_transcode_special_container(archive_path, volume_chain)?
    {
        if transcoded.entry_count == 0 {
            return Ok(Some(0));
        }
        let stream_reader =
            crate::archive::stream_adapter::read::ArchiveStreamReader::open_sequential(
                transcoded.cursor,
                65536,
            )?;
        let count = unsafe {
            inspect_from_handle(stream_reader.as_raw_archive(), detect_encoding, callback)
        }?;
        return Ok(Some(count));
    }

    match crate::archive::stream_adapter::try_open_resolved_stream(archive_path, volume_chain)? {
        Some(crate::archive::stream_adapter::ResolvedStreamKind::TarContainer(pre)) => {
            let pwd_c = password.and_then(|p| CString::new(p).ok());
            let mut stream_reader =
                crate::archive::stream_adapter::read::ArchiveStreamReader::open_sequential(
                    pre, 65536,
                )?;
            let a = stream_reader.as_raw_archive();
            if let Some(ref p) = pwd_c {
                unsafe {
                    archive_read_add_passphrase(a, p.as_ptr());
                }
            }
            let count = unsafe { inspect_from_handle(a, detect_encoding, callback) }?;
            stream_reader.verify_underlying_eof()?;
            Ok(Some(count))
        }
        Some(crate::archive::stream_adapter::ResolvedStreamKind::SingleFileStream {
            stream,
            entry_name,
            compressed_size,
            mtime_epoch_secs,
            codec,
        }) => {
            let count = crate::archive::stream_adapter::inspect_single_file_stream(
                stream,
                &entry_name,
                compressed_size,
                mtime_epoch_secs,
                codec,
                callback,
            )?;
            Ok(Some(count))
        }
        None => Ok(None),
    }
}

fn inspect_multivolume_archive<F>(
    archive_path: &Path,
    volume_chain: Vec<std::path::PathBuf>,
    password: Option<&str>,
    detect_encoding: bool,
    mut callback: F,
) -> Result<usize, TTZipStatus>
where
    F: FnMut(&ArchiveEntryRef<'_>) -> bool,
{
    use std::io::{Read, Seek, SeekFrom};
    let mut virtual_reader =
        crate::archive::split::VirtualMultiVolumeReader::from_volumes(volume_chain.clone())
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut magic = [0u8; 6];
    let is_sevenz =
        virtual_reader.read_exact(&mut magic).is_ok() && &magic == b"7z\xBC\xAF\x27\x1C";
    let _ = virtual_reader.seek(SeekFrom::Start(0));

    if is_sevenz {
        match crate::sevenz::encrypted_header::EncodedHeaderDecoder::default()
            .decode_from_reader(&mut virtual_reader, password)
        {
            Ok(info) => {
                let seek_index = crate::sevenz::header::SevenZSeekIndex::build(&info);
                return Ok(emit_sevenz_header_entries(
                    &info,
                    &seek_index,
                    detect_encoding,
                    &mut callback,
                ));
            }
            Err(crate::sevenz::dag::SevenZError::BadPassword) => {
                return Err(TTZipStatus::ErrInvalidPassword);
            }
            Err(_) => {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
        }
    }

    let total_len = virtual_reader.total_size();
    if let Ok(count) = inspect_zip_central_directory_reader(
        &mut virtual_reader,
        total_len,
        detect_encoding,
        &mut callback,
    ) {
        return Ok(count);
    }
    let _ = virtual_reader.seek(SeekFrom::Start(0));

    if let Some(count) = inspect_special_or_compressed_stream(
        archive_path,
        &volume_chain,
        password,
        detect_encoding,
        &mut callback,
    )? {
        return Ok(count);
    }

    let pwd_c = password.and_then(|p| CString::new(p).ok());
    let stream_reader =
        crate::archive::stream_adapter::read::ArchiveStreamReader::open_seekable(
            virtual_reader,
            65536,
        )?;
    let a = stream_reader.as_raw_archive();
    if let Some(ref p) = pwd_c {
        unsafe {
            archive_read_add_passphrase(a, p.as_ptr());
        }
    }
    unsafe { inspect_from_handle(a, detect_encoding, callback) }
}

/// Inspects an archive and invokes the callback for every discovered entry metadata item.
///
/// Takes a zero-allocation borrowed `ArchiveEntryRef` slice callback in pure Safe Rust.
pub fn inspect_archive<F>(
    archive_path: &Path,
    password: Option<&str>,
    detect_encoding: bool,
    mut callback: F,
) -> Result<usize, TTZipStatus>
where
    F: FnMut(&ArchiveEntryRef<'_>) -> bool,
{
    let volume_chain = crate::archive::split::detect_volume_chain(archive_path)
        .map_err(|_| TTZipStatus::ErrFileNotFound)?;

    if volume_chain.len() > 1 {
        return inspect_multivolume_archive(
            archive_path,
            volume_chain,
            password,
            detect_encoding,
            callback,
        );
    }

    let single_path = volume_chain.first().map(|p| p.as_path()).unwrap_or(archive_path);
    if !single_path.exists() {
        return Err(TTZipStatus::ErrFileNotFound);
    }

    // Fast path 1: ZIP via bounded pread Central Directory (zero mmap)
    if let Ok(count) =
        inspect_zip_central_directory_file(single_path, detect_encoding, &mut callback)
    {
        return Ok(count);
    }

    // Fast path 2: 7z via memory mapped seek index
    if let Ok(source) = crate::archive::source::open_archive_source(single_path) {
        if let Some(mapped) = source.as_slice() {
            if mapped.starts_with(b"7z\xBC\xAF\x27\x1C") {
                match crate::sevenz::SevenZArchive::open_slice_with_password(mapped, password) {
                    Ok(archive) => {
                        return Ok(emit_sevenz_header_entries(
                            archive.info(),
                            archive.seek_index(),
                            detect_encoding,
                            &mut callback,
                        ));
                    }
                    Err(err)
                        if super::fallback_policy::should_abort_without_fallback(err) =>
                    {
                        return Err(err);
                    }
                    Err(_) => {}
                }
            }
        }
    }

    inspect_archive_via_libarchive(single_path, password, detect_encoding, callback)
}

/// Inspects an archive strictly via the libarchive / stream adapter engine.
pub fn inspect_archive_via_libarchive<F>(
    archive_path: &Path,
    password: Option<&str>,
    detect_encoding: bool,
    mut callback: F,
) -> Result<usize, TTZipStatus>
where
    F: FnMut(&ArchiveEntryRef<'_>) -> bool,
{
    let volume_chain = crate::archive::split::detect_volume_chain(archive_path)
        .unwrap_or_else(|_| vec![archive_path.to_path_buf()]);
    if volume_chain.len() > 1 {
        return inspect_multivolume_archive(
            archive_path,
            volume_chain,
            password,
            detect_encoding,
            callback,
        );
    }

    if let Some(count) = inspect_special_or_compressed_stream(
        archive_path,
        &volume_chain,
        password,
        detect_encoding,
        &mut callback,
    )? {
        return Ok(count);
    }

    let arch_c = CString::new(archive_path.to_str().ok_or(TTZipStatus::ErrInvalidParam)?)
        .map_err(|_| TTZipStatus::ErrInvalidParam)?;
    let pwd_c = password.and_then(|p| CString::new(p).ok());

    unsafe {
        libc::setlocale(libc::LC_ALL, c"en_US.UTF-8".as_ptr());
        let a = archive_read_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _guard = ArchiveReadGuard(a);

        archive_read_support_format_all(a);
        archive_read_support_filter_all(a);

        if let Some(ref p) = pwd_c {
            archive_read_add_passphrase(a, p.as_ptr());
        }

        let open_rc = archive_read_open_filename(a, arch_c.as_ptr(), 65536);
        if open_rc != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        inspect_from_handle(a, detect_encoding, callback)
    }
}

unsafe fn inspect_from_handle<F>(
    a: *mut c_void,
    detect_encoding: bool,
    mut cb: F,
) -> Result<usize, TTZipStatus>
where
    F: FnMut(&ArchiveEntryRef<'_>) -> bool,
{
    let mut entry: *mut c_void = std::ptr::null_mut();
    let mut count = 0;

    loop {
        let rc = archive_read_next_header(a, &mut entry);
        if rc == 1 {
            // ARCHIVE_EOF
            break;
        } else if rc == -10 {
            // ARCHIVE_RETRY
            continue;
        } else if rc != 0 {
            if let Some(err_str) = crate::sys::libarchive::sys::get_archive_error_string(a) {
                let lower = err_str.to_lowercase();
                if lower.contains("passphrase") || lower.contains("password") || lower.contains("encrypted") {
                    return Err(TTZipStatus::ErrInvalidPassword);
                }
            }
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        if entry.is_null() {
            break;
        }
        let path_c_str = match archive_entry_pathname_safe(entry) {
            Some(p) => p,
            None => {
                archive_read_data_skip(a);
                continue;
            }
        };
        let path_bytes = path_c_str.to_bytes();
        if path_bytes.is_empty() {
            archive_read_data_skip(a);
            continue;
        }

        let detected_charset_opt = if detect_encoding && !path_bytes.is_ascii() {
            crate::codecs::chardet::detect_charset(path_bytes)
        } else {
            None
        };
        let detected_encoding = detected_charset_opt.as_deref();

        let uncompressed_size = archive_entry_size(entry).max(0) as u64;
        let mode = archive_entry_mode(entry) as u32;
        let filetype = archive_entry_filetype(entry);
        let is_dir = (filetype & (libc::S_IFMT as mode_t)) == (libc::S_IFDIR as mode_t)
            || (mode & (libc::S_IFMT as u32)) == (libc::S_IFDIR as u32)
            || path_bytes.ends_with(b"/");
        let mtime = archive_entry_mtime(entry) as i64;
        let is_data_enc = archive_entry_is_data_encrypted(entry) != 0;
        let is_meta_enc = archive_entry_is_metadata_encrypted(entry) != 0;

        // Lossless path resolution:
        // Do NOT call `to_string_lossy()`, as that irreversibly destroys non-UTF-8
        // codepages (e.g. GBK, Big5, Shift-JIS, EUC-KR) by replacing bytes with `\u{FFFD}`.
        // 1. If valid UTF-8, borrow the slice directly with zero allocations.
        // 2. If non-UTF-8, transcode using the detected charset or filename remediation.
        // 3. Keep pristine raw binary bytes accessible in `entry_ref.raw_path`.
        let decoded_storage = if std::str::from_utf8(path_bytes).is_err() {
            let transcoded = if let Some(charset) = detected_encoding {
                crate::charset::transcode_to_utf8(path_bytes, charset)
                    .unwrap_or_else(|_| crate::charset::sanitize_filename(path_bytes))
            } else {
                crate::charset::sanitize_filename(path_bytes)
            };
            Some(transcoded)
        } else {
            None
        };

        let path_str: &str = match &decoded_storage {
            Some(s) => s.as_str(),
            None => std::str::from_utf8(path_bytes).unwrap_or_default(),
        };

        let entry_ref = ArchiveEntryRef {
            path: path_str,
            raw_path: path_bytes,
            uncompressed_size,
            compressed_size: 0,
            crc32: 0,
            mtime_epoch_secs: mtime,
            mode,
            is_directory: is_dir,
            is_encrypted: is_data_enc || is_meta_enc,
            compression_method: 0,
            detected_encoding,
        };

        count += 1;
        let should_continue = cb(&entry_ref);
        let skip_rc = archive_read_data_skip(a);
        if skip_rc < 0 && skip_rc != -10 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        if !should_continue {
            break;
        }
    }

    Ok(count)
}

/// Compatibility wrapper bridging the Safe Rust closure-based `inspect_archive`
/// to the legacy C-ABI function pointer callback and `user_data` pointer.
pub fn inspect_archive_c_compat(
    archive_path: &Path,
    password: Option<&str>,
    detect_encoding: bool,
    callback: TTZipInspectCallback,
    user_data: *mut c_void,
) -> Result<usize, TTZipStatus> {
    let cb = match callback {
        Some(f) => f,
        None => return Err(TTZipStatus::ErrInvalidParam),
    };

    inspect_archive(archive_path, password, detect_encoding, |entry_ref| {
        let path_c = match CString::new(entry_ref.path) {
            Ok(c) => c,
            Err(_) => return true,
        };
        let encoding_c = entry_ref.detected_encoding.and_then(|s| CString::new(s).ok());
        let meta = TTZipEntryMetadata {
            struct_size: std::mem::size_of::<TTZipEntryMetadata>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_3,
            path: path_c.as_ptr(),
            uncompressed_size: entry_ref.uncompressed_size,
            compressed_size: entry_ref.compressed_size,
            crc32: entry_ref.crc32,
            mtime_epoch_secs: entry_ref.mtime_epoch_secs,
            mode: entry_ref.mode,
            is_directory: entry_ref.is_directory,
            is_encrypted: entry_ref.is_encrypted,
            compression_method: entry_ref.compression_method,
            detected_encoding: encoding_c.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
        };
        unsafe { cb(&meta, user_data) }
    })
}

use crate::types::unified_node::{UnifiedEntryNode, UnifiedVfsTable, NODE_FLAG_DIRECTORY, NODE_FLAG_ENCRYPTED};

/// Inspects an archive and ingests entries directly into a `UnifiedVfsTable` without intermediate FFI struct allocations.
pub fn inspect_archive_into_vfs(
    archive_path: &Path,
    password: Option<&str>,
    detect_encoding: bool,
    table: &mut UnifiedVfsTable,
) -> Result<usize, TTZipStatus> {
    inspect_archive(archive_path, password, detect_encoding, |entry_ref| {
        let mut flags = 0;
        if entry_ref.is_directory {
            flags |= NODE_FLAG_DIRECTORY;
        }
        if entry_ref.is_encrypted {
            flags |= NODE_FLAG_ENCRYPTED;
        }

        let node = UnifiedEntryNode {
            uncompressed_size: entry_ref.uncompressed_size,
            compressed_size: entry_ref.compressed_size,
            path_offset: 0,
            path_len: 0,
            flags,
            crc32: entry_ref.crc32,
            parent_id: crate::types::unified_node::ROOT_PARENT_ID,
            first_child_id: crate::types::unified_node::NO_NODE,
            next_sibling_id: crate::types::unified_node::NO_NODE,
            mtime_epoch_secs: entry_ref.mtime_epoch_secs as u32,
            _reserved: 0,
        };
        table.add_entry(node, entry_ref.path);
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_entry_ref_lossless_access() {
        let raw_utf8 = b"standard/path/file.txt";
        let entry_ref = ArchiveEntryRef {
            path: "standard/path/file.txt",
            raw_path: raw_utf8,
            uncompressed_size: 100,
            compressed_size: 50,
            crc32: 0x12345678,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
            compression_method: 8,
            detected_encoding: Some("UTF-8"),
        };

        assert_eq!(entry_ref.raw_bytes(), raw_utf8);
        assert!(entry_ref.is_utf8());
        assert_eq!(entry_ref.path, "standard/path/file.txt");
    }

    #[test]
    fn test_non_utf8_encoding_lossless_resolution() {
        // GBK byte sequence representing Chinese characters
        let gbk_vec = vec![0xb2, 0xe2, 0xca, 0xd4, b'.', b't', b'x', b't'];
        let gbk_bytes: &[u8] = &gbk_vec;
        assert!(std::str::from_utf8(gbk_bytes).is_err());

        // Verify that sanitize_filename cleanly transcodes without producing replacement characters (\u{FFFD})
        let transcoded = crate::charset::sanitize_filename(gbk_bytes);
        assert_eq!(transcoded, "\u{6D4B}\u{8BD5}.txt");
        assert!(!transcoded.contains('\u{FFFD}'));

        let entry_ref = ArchiveEntryRef {
            path: &transcoded,
            raw_path: gbk_bytes,
            uncompressed_size: 200,
            compressed_size: 100,
            crc32: 0,
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
            compression_method: 0,
            detected_encoding: Some("GBK"),
        };

        assert_eq!(entry_ref.raw_bytes(), gbk_bytes);
        assert!(!entry_ref.is_utf8());
        assert_eq!(entry_ref.path, "\u{6D4B}\u{8BD5}.txt");
    }
}

