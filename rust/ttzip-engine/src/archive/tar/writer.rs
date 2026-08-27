// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Streaming PAX-format TAR Archive Generator.
//!
//! Generates POSIX.1-2001 PAX compliant archives supporting arbitrary path lengths,
//! large file sizes (>8GB), symlinks, directories, and 512-byte alignment padding.

use super::header::*;
use super::pax::build_pax_payload;
use crate::types::{TTZipCreateOptions, TTZipStatus};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[inline]
fn truncate_to_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        s
    } else {
        let mut end = max_bytes;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}

/// Splits a file path into POSIX USTAR `(prefix, name)` components.
///
/// POSIX.1-1988 USTAR limits filenames to 100 bytes and prefix to 155 bytes,
/// separated by a slash `/` (which is not stored in either field).
/// Total maximum representable path length is 155 + 1 + 100 = 256 bytes.
///
/// Returns:
/// - `Some(("", path))` if `path.len() <= 100` (fits entirely in name field).
/// - `Some((prefix, name))` if path can be split at a `/` such that `prefix.len() <= 155`
///   and `name.len() <= 100` (both non-empty).
/// - `None` if `path` cannot be represented in USTAR format without PAX headers.
pub fn split_ustar_path(path: &str) -> Option<(&str, &str)> {
    let bytes = path.as_bytes();
    if bytes.len() <= 100 {
        return Some(("", path));
    }
    if bytes.len() > 256 {
        return None;
    }

    let min_idx = 1.max(bytes.len().saturating_sub(101));
    let max_idx = 155.min(bytes.len().saturating_sub(2));

    for idx in (min_idx..=max_idx).rev() {
        if bytes[idx] == b'/' {
            let prefix = &path[..idx];
            let name = &path[idx + 1..];
            if !prefix.is_empty() && prefix.len() <= 155 && !name.is_empty() && name.len() <= 100 {
                return Some((prefix, name));
            }
        }
    }

    None
}

/// Streaming PAX format TAR Archive Writer.
pub struct TarWriter<W: Write> {
    inner: W,
    finished: bool,
}

impl<W: Write> TarWriter<W> {
    /// Creates a new streaming TAR writer wrapping any `std::io::Write` sink.
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            finished: false,
        }
    }

    /// Consumes the TAR writer and returns the underlying writer.
    pub fn into_inner(self) -> W {
        self.inner
    }

    /// Appends a regular file entry to the TAR archive.
    pub fn append_file(
        &mut self,
        rel_path: &str,
        data: &[u8],
        mode: u32,
        mtime: i64,
    ) -> Result<(), TTZipStatus> {
        self.append_custom_entry(rel_path, data, mode, mtime, TYPE_REGULAR, None, None)
    }

    /// Appends a file from an existing `Read` stream with a known size using 64KB buffering.
    pub fn append_file_stream<R: Read>(
        &mut self,
        rel_path: &str,
        reader: &mut R,
        size: u64,
        mode: u32,
        mtime: i64,
    ) -> Result<(), TTZipStatus> {
        let normalized_path = rel_path.replace('\\', "/");
        let path_len = normalized_path.len();
        let ustar_split = split_ustar_path(&normalized_path);

        let needs_pax = ustar_split.is_none()
            || size >= 0o77777777777
            || mtime > 0o77777777777;

        if needs_pax {
            let mut pax_records = Vec::new();
            if path_len > 100 {
                pax_records.push(("path", normalized_path.as_str()));
            }
            let size_str;
            if size >= 0o77777777777 {
                size_str = size.to_string();
                pax_records.push(("size", &size_str));
            }
            let mtime_str;
            if mtime > 0o77777777777 {
                mtime_str = mtime.to_string();
                pax_records.push(("mtime", &mtime_str));
            }

            let pax_payload = build_pax_payload(&pax_records);
            let pax_filename = format!("PaxHeaders.0/{}", normalized_path.rsplit('/').next().unwrap_or("entry"));
            let pax_header = TarHeader {
                name: pax_filename,
                mode: 0o644,
                uid: 0,
                gid: 0,
                size: pax_payload.len() as u64,
                mtime: mtime.max(0),
                chksum: 0,
                typeflag: TYPE_PAX_EXT_HEADER,
                linkname: String::new(),
                magic: *MAGIC_USTAR,
                version: *VERSION_USTAR,
                uname: "root".to_string(),
                gname: "root".to_string(),
                devmajor: 0,
                devminor: 0,
                prefix: String::new(),
            };

            let pax_block = build_tar_header_block(&pax_header);
            self.inner
                .write_all(&pax_block)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            self.inner
                .write_all(&pax_payload)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

            let pax_pad = (TAR_BLOCK_SIZE - (pax_payload.len() % TAR_BLOCK_SIZE)) % TAR_BLOCK_SIZE;
            if pax_pad > 0 {
                self.inner
                    .write_all(&vec![0u8; pax_pad])
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            }
        }

        let (prefix, name) = match ustar_split {
            Some((pfx, nm)) => (pfx.to_string(), nm.to_string()),
            None => (
                String::new(),
                truncate_to_char_boundary(&normalized_path, 100).to_string(),
            ),
        };

        let header = TarHeader {
            name,
            mode: if mode != 0 { mode } else { 0o644 },
            uid: 0,
            gid: 0,
            size,
            mtime: mtime.max(0),
            chksum: 0,
            typeflag: TYPE_REGULAR,
            linkname: String::new(),
            magic: *MAGIC_USTAR,
            version: *VERSION_USTAR,
            uname: String::new(),
            gname: String::new(),
            devmajor: 0,
            devminor: 0,
            prefix,
        };

        let block = build_tar_header_block(&header);
        self.inner
            .write_all(&block)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

        if size > 0 {
            let mut buf = vec![0u8; 64 * 1024];
            let mut remaining = size;
            while remaining > 0 {
                let to_read = (remaining as usize).min(buf.len());
                let n = reader
                    .read(&mut buf[..to_read])
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                if n == 0 {
                    break;
                }
                self.inner
                    .write_all(&buf[..n])
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                remaining = remaining.saturating_sub(n as u64);
            }

            let pad = (TAR_BLOCK_SIZE - ((size as usize) % TAR_BLOCK_SIZE)) % TAR_BLOCK_SIZE;
            if pad > 0 {
                self.inner
                    .write_all(&vec![0u8; pad])
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            }
        }

        Ok(())
    }

    /// Appends a file, directory, or symlink from disk to the archive using streaming I/O.
    pub fn append_file_from_disk(
        &mut self,
        disk_path: &Path,
        rel_path: &str,
    ) -> Result<u64, TTZipStatus> {
        let meta = fs::symlink_metadata(disk_path)
            .map_err(|_| TTZipStatus::ErrFileNotFound)?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let filetype = meta.file_type();

        if filetype.is_symlink() {
            let target = fs::read_link(disk_path)
                .map_err(|_| TTZipStatus::ErrFileNotFound)?;
            self.append_symlink(rel_path, &target.to_string_lossy(), 0o777, mtime)?;
            Ok(0)
        } else if filetype.is_dir() {
            let mode = (meta.permissions().mode() & 0o777) as u32;
            self.append_dir(rel_path, mode, mtime)?;
            Ok(0)
        } else {
            let size = meta.len();
            let mode = (meta.permissions().mode() & 0o777) as u32;
            let mut file = File::open(disk_path)
                .map_err(|_| TTZipStatus::ErrFileNotFound)?;
            self.append_file_stream(rel_path, &mut file, size, mode, mtime)?;
            Ok(size)
        }
    }

    /// Appends a directory entry to the TAR archive.
    pub fn append_dir(&mut self, rel_path: &str, mode: u32, mtime: i64) -> Result<(), TTZipStatus> {
        let mut dir_path = rel_path.to_string();
        if !dir_path.ends_with('/') {
            dir_path.push('/');
        }
        self.append_custom_entry(&dir_path, &[], mode, mtime, TYPE_DIRECTORY, None, None)
    }

    /// Appends a symbolic link entry to the TAR archive.
    pub fn append_symlink(
        &mut self,
        rel_path: &str,
        target: &str,
        mode: u32,
        mtime: i64,
    ) -> Result<(), TTZipStatus> {
        self.append_custom_entry(rel_path, &[], mode, mtime, TYPE_SYMLINK, Some(target), None)
    }

    /// Appends an entry with custom typeflag, link target, and optional PAX extra records.
    #[allow(clippy::too_many_arguments)]
    pub fn append_custom_entry(
        &mut self,
        rel_path: &str,
        data: &[u8],
        mode: u32,
        mtime: i64,
        typeflag: u8,
        link_target: Option<&str>,
        pax_extra: Option<&HashMap<String, String>>,
    ) -> Result<(), TTZipStatus> {
        let normalized_path = rel_path.replace('\\', "/");
        let path_len = normalized_path.len();
        let size = data.len() as u64;
        let link_len = link_target.map(|s| s.len()).unwrap_or(0);

        let ustar_split = split_ustar_path(&normalized_path);

        // Check if PAX Extended Header is necessary
        let needs_pax = ustar_split.is_none()
            || link_len > 100
            || size >= 0o77777777777 // 8GB octal limit
            || mtime > 0o77777777777
            || pax_extra.is_some();

        if needs_pax {
            let mut pax_records = Vec::new();
            if path_len > 100 {
                pax_records.push(("path", normalized_path.as_str()));
            }
            if let Some(target) = link_target {
                if link_len > 100 {
                    pax_records.push(("linkpath", target));
                }
            }
            let size_str;
            if size >= 0o77777777777 {
                size_str = size.to_string();
                pax_records.push(("size", &size_str));
            }
            let mtime_str;
            if mtime > 0o77777777777 {
                mtime_str = mtime.to_string();
                pax_records.push(("mtime", &mtime_str));
            }

            if let Some(extra) = pax_extra {
                for (k, v) in extra {
                    pax_records.push((k.as_str(), v.as_str()));
                }
            }

            let pax_payload = build_pax_payload(&pax_records);
            let pax_filename = format!("PaxHeaders.0/{}", normalized_path.rsplit('/').next().unwrap_or("entry"));
            let pax_header = TarHeader {
                name: pax_filename,
                mode: 0o644,
                uid: 0,
                gid: 0,
                size: pax_payload.len() as u64,
                mtime: mtime.max(0),
                chksum: 0,
                typeflag: TYPE_PAX_EXT_HEADER,
                linkname: String::new(),
                magic: *MAGIC_USTAR,
                version: *VERSION_USTAR,
                uname: "root".to_string(),
                gname: "root".to_string(),
                devmajor: 0,
                devminor: 0,
                prefix: String::new(),
            };

            let pax_block = build_tar_header_block(&pax_header);
            self.inner
                .write_all(&pax_block)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            self.inner
                .write_all(&pax_payload)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

            let pax_pad = (TAR_BLOCK_SIZE - (pax_payload.len() % TAR_BLOCK_SIZE)) % TAR_BLOCK_SIZE;
            if pax_pad > 0 {
                self.inner
                    .write_all(&vec![0u8; pax_pad])
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            }
        }

        // Standard ustar Header
        let (prefix, name) = match ustar_split {
            Some((pfx, nm)) => (pfx.to_string(), nm.to_string()),
            None => (
                String::new(),
                truncate_to_char_boundary(&normalized_path, 100).to_string(),
            ),
        };

        let short_link = link_target
            .map(|s| truncate_to_char_boundary(s, 100).to_string())
            .unwrap_or_default();

        let header = TarHeader {
            name,
            mode: if mode != 0 { mode } else if typeflag == TYPE_DIRECTORY { 0o755 } else { 0o644 },
            uid: 0,
            gid: 0,
            size,
            mtime: mtime.max(0),
            chksum: 0,
            typeflag,
            linkname: short_link,
            magic: *MAGIC_USTAR,
            version: *VERSION_USTAR,
            uname: String::new(),
            gname: String::new(),
            devmajor: 0,
            devminor: 0,
            prefix,
        };

        let block = build_tar_header_block(&header);
        self.inner
            .write_all(&block)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

        // Write payload
        if !data.is_empty() {
            self.inner
                .write_all(data)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

            let pad = (TAR_BLOCK_SIZE - (data.len() % TAR_BLOCK_SIZE)) % TAR_BLOCK_SIZE;
            if pad > 0 {
                self.inner
                    .write_all(&vec![0u8; pad])
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            }
        }

        Ok(())
    }

    /// Finishes the archive by writing the standard two 512-byte zero blocks.
    pub fn finish(&mut self) -> Result<(), TTZipStatus> {
        if self.finished {
            return Ok(());
        }
        let zero_blocks = [0u8; TAR_BLOCK_SIZE * 2];
        self.inner
            .write_all(&zero_blocks)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        self.inner
            .flush()
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        self.finished = true;
        Ok(())
    }
}

/// Helper to recursively collect files and directories for TAR creation.
fn collect_tar_entries_recursive(
    root: &Path,
    current: &Path,
    out: &mut Vec<(PathBuf, String)>,
) -> std::io::Result<()> {
    let rel_prefix = current.strip_prefix(root).unwrap_or(current);
    let rel_str = rel_prefix.to_string_lossy().to_string();

    if !rel_str.is_empty() {
        out.push((current.to_path_buf(), rel_str));
    }

    if let Ok(meta) = fs::symlink_metadata(current) {
        if meta.is_dir() && !meta.file_type().is_symlink() {
            for entry in fs::read_dir(current)? {
                let entry = entry?;
                collect_tar_entries_recursive(root, &entry.path(), out)?;
            }
        }
    }
    Ok(())
}

/// Recursively writes source paths to any `Write` sink as a standard PAX/USTAR TAR stream.
pub fn write_tar_to_writer<W: Write>(
    source_paths: &[PathBuf],
    writer: W,
    options: &TTZipCreateOptions,
) -> Result<(), TTZipStatus> {
    if source_paths.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let mut tar_writer = TarWriter::new(writer);
    let mut entries_to_write = Vec::new();
    for src_path in source_paths {
        if !src_path.exists() && fs::symlink_metadata(src_path).is_err() {
            return Err(TTZipStatus::ErrFileNotFound);
        }
        let base_parent = src_path.parent().unwrap_or(src_path);
        collect_tar_entries_recursive(base_parent, src_path, &mut entries_to_write)
            .map_err(|_| TTZipStatus::ErrFileNotFound)?;
    }

    let mut processed_bytes: u64 = 0;
    for (abs_path, rel_name) in entries_to_write {
        let meta = match fs::symlink_metadata(&abs_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let filetype = meta.file_type();

        if filetype.is_symlink() {
            if let Ok(target) = fs::read_link(&abs_path) {
                tar_writer.append_symlink(&rel_name, &target.to_string_lossy(), 0o777, mtime)?;
            }
        } else if filetype.is_dir() {
            let mode = (meta.permissions().mode() & 0o777) as u32;
            tar_writer.append_dir(&rel_name, mode, mtime)?;
        } else {
            let size = meta.len();
            let mode = (meta.permissions().mode() & 0o777) as u32;
            let mut file = File::open(&abs_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;

            let normalized_path = rel_name.replace('\\', "/");
            let path_len = normalized_path.len();
            let ustar_split = split_ustar_path(&normalized_path);

            let needs_pax = ustar_split.is_none()
                || size >= 0o77777777777
                || mtime > 0o77777777777;

            if needs_pax {
                let mut pax_records = Vec::new();
                if path_len > 100 {
                    pax_records.push(("path", normalized_path.as_str()));
                }
                let size_str;
                if size >= 0o77777777777 {
                    size_str = size.to_string();
                    pax_records.push(("size", &size_str));
                }
                let mtime_str;
                if mtime > 0o77777777777 {
                    mtime_str = mtime.to_string();
                    pax_records.push(("mtime", &mtime_str));
                }

                let pax_payload = build_pax_payload(&pax_records);
                let pax_filename = format!(
                    "PaxHeaders.0/{}",
                    normalized_path.rsplit('/').next().unwrap_or("entry")
                );
                let pax_header = TarHeader {
                    name: pax_filename,
                    mode: 0o644,
                    uid: 0,
                    gid: 0,
                    size: pax_payload.len() as u64,
                    mtime: mtime.max(0),
                    chksum: 0,
                    typeflag: TYPE_PAX_EXT_HEADER,
                    linkname: String::new(),
                    magic: *MAGIC_USTAR,
                    version: *VERSION_USTAR,
                    uname: "root".to_string(),
                    gname: "root".to_string(),
                    devmajor: 0,
                    devminor: 0,
                    prefix: String::new(),
                };

                let pax_block = build_tar_header_block(&pax_header);
                tar_writer
                    .inner
                    .write_all(&pax_block)
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                tar_writer
                    .inner
                    .write_all(&pax_payload)
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

                let pax_pad =
                    (TAR_BLOCK_SIZE - (pax_payload.len() % TAR_BLOCK_SIZE)) % TAR_BLOCK_SIZE;
                if pax_pad > 0 {
                    tar_writer
                        .inner
                        .write_all(&vec![0u8; pax_pad])
                        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                }
            }

            let (prefix, name) = match ustar_split {
                Some((pfx, nm)) => (pfx.to_string(), nm.to_string()),
                None => (
                    String::new(),
                    truncate_to_char_boundary(&normalized_path, 100).to_string(),
                ),
            };

            let header = TarHeader {
                name,
                mode: if mode != 0 { mode } else { 0o644 },
                uid: 0,
                gid: 0,
                size,
                mtime: mtime.max(0),
                chksum: 0,
                typeflag: TYPE_REGULAR,
                linkname: String::new(),
                magic: *MAGIC_USTAR,
                version: *VERSION_USTAR,
                uname: String::new(),
                gname: String::new(),
                devmajor: 0,
                devminor: 0,
                prefix,
            };

            let block = build_tar_header_block(&header);
            tar_writer
                .inner
                .write_all(&block)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

            if size > 0 {
                let mut buf = vec![0u8; 64 * 1024];
                let mut remaining = size;
                while remaining > 0 {
                    let to_read = (remaining as usize).min(buf.len());
                    let n = file
                        .read(&mut buf[..to_read])
                        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                    if n == 0 {
                        break;
                    }
                    tar_writer
                        .inner
                        .write_all(&buf[..n])
                        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                    remaining = remaining.saturating_sub(n as u64);
                    processed_bytes = processed_bytes.saturating_add(n as u64);

                    if let Some(cb) = options.progress_callback {
                        let rel_c = std::ffi::CString::new(rel_name.as_str()).unwrap_or_default();
                        let should_continue = unsafe {
                            cb(processed_bytes, processed_bytes, rel_c.as_ptr(), options.user_data)
                        };
                        if !should_continue {
                            return Err(TTZipStatus::Cancelled);
                        }
                    }
                }

                let pad = (TAR_BLOCK_SIZE - ((size as usize) % TAR_BLOCK_SIZE)) % TAR_BLOCK_SIZE;
                if pad > 0 {
                    tar_writer
                        .inner
                        .write_all(&vec![0u8; pad])
                        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                }
            }
        }

        if let Some(cb) = options.progress_callback {
            let rel_c = std::ffi::CString::new(rel_name.as_str()).unwrap_or_default();
            let should_continue = unsafe {
                cb(processed_bytes, processed_bytes, rel_c.as_ptr(), options.user_data)
            };
            if !should_continue {
                return Err(TTZipStatus::Cancelled);
            }
        }
    }

    tar_writer.finish()?;
    Ok(())
}
