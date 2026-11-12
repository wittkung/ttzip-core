// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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

pub use super::gnu::split_ustar_path;

const ZERO_BLOCK: [u8; TAR_BLOCK_SIZE] = [0u8; TAR_BLOCK_SIZE];

/// Custom entry specification for appending specialized TAR records (symlink, PAX, directory).
#[derive(Debug, Clone, Copy)]
pub struct TarCustomEntryParams<'a> {
    pub rel_path: &'a str,
    pub data: &'a [u8],
    pub mode: u32,
    pub mtime: i64,
    pub typeflag: u8,
    pub link_target: Option<&'a str>,
    pub pax_extra: Option<&'a HashMap<String, String>>,
}

/// Parameters for constructing a USTAR header and optional PAX extended block.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TarHeaderBuildParams<'a> {
    pub normalized_path: &'a str,
    pub size: u64,
    pub mode: u32,
    pub mtime: i64,
    pub typeflag: u8,
    pub link_target: Option<&'a str>,
    pub pax_extra: Option<&'a HashMap<String, String>>,
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
        self.append_custom_entry(TarCustomEntryParams {
            rel_path,
            data,
            mode,
            mtime,
            typeflag: TYPE_REGULAR,
            link_target: None,
            pax_extra: None,
        })
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
        let block = self.write_pax_and_build_ustar_header(TarHeaderBuildParams {
            normalized_path: &normalized_path,
            size,
            mode,
            mtime,
            typeflag: TYPE_REGULAR,
            link_target: None,
            pax_extra: None,
        })?;
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
            self.write_block_padding(size as usize)?;
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
        self.append_custom_entry(TarCustomEntryParams {
            rel_path: &dir_path,
            data: &[],
            mode,
            mtime,
            typeflag: TYPE_DIRECTORY,
            link_target: None,
            pax_extra: None,
        })
    }

    /// Appends a symbolic link entry to the TAR archive.
    pub fn append_symlink(
        &mut self,
        rel_path: &str,
        target: &str,
        mode: u32,
        mtime: i64,
    ) -> Result<(), TTZipStatus> {
        self.append_custom_entry(TarCustomEntryParams {
            rel_path,
            data: &[],
            mode,
            mtime,
            typeflag: TYPE_SYMLINK,
            link_target: Some(target),
            pax_extra: None,
        })
    }

    /// Appends an entry with custom typeflag, link target, and optional PAX extra records.
    pub fn append_custom_entry(
        &mut self,
        params: TarCustomEntryParams<'_>,
    ) -> Result<(), TTZipStatus> {
        let normalized_path = params.rel_path.replace('\\', "/");
        let size = params.data.len() as u64;
        let block = self.write_pax_and_build_ustar_header(TarHeaderBuildParams {
            normalized_path: &normalized_path,
            size,
            mode: params.mode,
            mtime: params.mtime,
            typeflag: params.typeflag,
            link_target: params.link_target,
            pax_extra: params.pax_extra,
        })?;
        self.inner
            .write_all(&block)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

        if !params.data.is_empty() {
            self.inner
                .write_all(params.data)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            self.write_block_padding(params.data.len())?;
        }

        Ok(())
    }

    pub(crate) fn write_pax_and_build_ustar_header(
        &mut self,
        params: TarHeaderBuildParams<'_>,
    ) -> Result<[u8; TAR_BLOCK_SIZE], TTZipStatus> {
        let TarHeaderBuildParams {
            normalized_path,
            size,
            mode,
            mtime,
            typeflag,
            link_target,
            pax_extra,
        } = params;
        let ustar_split = split_ustar_path(normalized_path);
        let link_len = link_target.map(|s| s.len()).unwrap_or(0);
        let needs_pax = ustar_split.is_none()
            || link_len > 100
            || size >= 0o77777777777
            || mtime > 0o77777777777
            || pax_extra.is_some();

        if needs_pax {
            self.write_pax_extended_block(normalized_path, size, mtime, link_target, pax_extra)?;
        }

        let (prefix, name) = match ustar_split {
            Some((pfx, nm)) => (pfx.to_string(), nm.to_string()),
            None => (
                String::new(),
                truncate_to_char_boundary(normalized_path, 100).to_string(),
            ),
        };
        let short_link = link_target
            .map(|s| truncate_to_char_boundary(s, 100).to_string())
            .unwrap_or_default();
        let default_mode = if typeflag == TYPE_DIRECTORY { 0o755 } else { 0o644 };

        let header = TarHeader {
            name,
            mode: if mode != 0 { mode } else { default_mode },
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
        Ok(build_tar_header_block(&header))
    }

    fn write_pax_extended_block(
        &mut self,
        normalized_path: &str,
        size: u64,
        mtime: i64,
        link_target: Option<&str>,
        pax_extra: Option<&HashMap<String, String>>,
    ) -> Result<(), TTZipStatus> {
        let mut pax_records = Vec::new();
        if normalized_path.len() > 100 {
            pax_records.push(("path", normalized_path));
        }
        if let Some(target) = link_target.filter(|t| t.len() > 100) {
            pax_records.push(("linkpath", target));
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
        self.inner
            .write_all(&pax_block)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        self.inner
            .write_all(&pax_payload)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        self.write_block_padding(pax_payload.len())
    }

    fn write_block_padding(&mut self, len: usize) -> Result<(), TTZipStatus> {
        let pad = (TAR_BLOCK_SIZE - (len % TAR_BLOCK_SIZE)) % TAR_BLOCK_SIZE;
        if pad > 0 {
            self.inner
                .write_all(&ZERO_BLOCK[..pad])
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
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

/// Helper to recursively collect files and directories for TAR creation using an explicit heap worklist (O(1) stack).
fn collect_tar_entries_recursive(
    root: &Path,
    start: &Path,
    out: &mut Vec<(PathBuf, String)>,
) -> std::io::Result<()> {
    let mut stack: Vec<PathBuf> = vec![start.to_path_buf()];

    while let Some(current) = stack.pop() {
        let rel_prefix = current.strip_prefix(root).unwrap_or(&current);
        let rel_str = rel_prefix.to_string_lossy().to_string();

        if !rel_str.is_empty() {
            out.push((current.clone(), rel_str));
        }

        if let Ok(meta) = fs::symlink_metadata(&current) {
            if meta.is_dir() && !meta.file_type().is_symlink() {
                let mut children = Vec::new();
                for entry in fs::read_dir(&current)? {
                    children.push(entry?.path());
                }
                children.sort_unstable_by(|a, b| b.cmp(a));
                stack.extend(children);
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
) -> Result<W, TTZipStatus> {
    if source_paths.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let mut entries_to_write = Vec::new();
    for src_path in source_paths {
        if !src_path.exists() && fs::symlink_metadata(src_path).is_err() {
            return Err(TTZipStatus::ErrFileNotFound);
        }
        let base_parent = src_path.parent().unwrap_or(src_path);
        collect_tar_entries_recursive(base_parent, src_path, &mut entries_to_write)
            .map_err(|_| TTZipStatus::ErrFileNotFound)?;
    }

    write_tar_entries_to_writer(&entries_to_write, writer, options)
}

/// Writes pre-collected `(disk_path, archive_path)` entries to any `Write` sink as a PAX/USTAR TAR stream.
pub fn write_tar_entries_to_writer<W: Write>(
    entries: &[(PathBuf, String)],
    writer: W,
    options: &TTZipCreateOptions,
) -> Result<W, TTZipStatus> {
    if entries.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let resolved: Vec<(&Path, &str, fs::Metadata)> = entries
        .iter()
        .filter_map(|(p, r)| fs::symlink_metadata(p).ok().map(|m| (p.as_path(), r.as_str(), m)))
        .collect();
    let total_bytes: u64 = resolved
        .iter()
        .filter(|(_, _, m)| m.is_file())
        .map(|(_, _, m)| m.len())
        .sum();

    let mut tar_writer = TarWriter::new(writer);
    let mut processed_bytes: u64 = 0;
    let mut io_buf = vec![0u8; 64 * 1024 + TAR_BLOCK_SIZE * 2];

    for &(abs_path, rel_name, ref meta) in &resolved {
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let filetype = meta.file_type();

        if filetype.is_symlink() {
            if let Ok(target) = fs::read_link(abs_path) {
                tar_writer.append_symlink(rel_name, &target.to_string_lossy(), 0o777, mtime)?;
            }
        } else if filetype.is_dir() {
            let mode = meta.permissions().mode() & 0o777;
            tar_writer.append_dir(rel_name, mode, mtime)?;
        } else {
            let size = meta.len();
            let mode = meta.permissions().mode() & 0o777;
            let mut stream_state = TarDiskStreamState {
                io_buf: &mut io_buf,
                processed_bytes: &mut processed_bytes,
                total_bytes,
                options,
            };
            append_tar_regular_file_from_disk(
                &mut tar_writer,
                TarDiskRegularEntry {
                    abs_path,
                    rel_name,
                    size,
                    mode,
                    mtime,
                },
                &mut stream_state,
            )?;
        }

        if let Some(cb) = options.progress_callback {
            let rel_c = std::ffi::CString::new(rel_name).unwrap_or_default();
            let should_continue = unsafe {
                cb(processed_bytes, total_bytes.max(processed_bytes), rel_c.as_ptr(), options.user_data)
            };
            if !should_continue {
                return Err(TTZipStatus::Cancelled);
            }
        }
    }

    tar_writer.finish()?;
    Ok(tar_writer.into_inner())
}

struct TarDiskRegularEntry<'a> {
    abs_path: &'a Path,
    rel_name: &'a str,
    size: u64,
    mode: u32,
    mtime: i64,
}

struct TarDiskStreamState<'a, 'b> {
    io_buf: &'a mut [u8],
    processed_bytes: &'a mut u64,
    total_bytes: u64,
    options: &'b TTZipCreateOptions,
}

fn append_tar_regular_file_from_disk<W: Write>(
    tar_writer: &mut TarWriter<W>,
    entry: TarDiskRegularEntry<'_>,
    state: &mut TarDiskStreamState<'_, '_>,
) -> Result<(), TTZipStatus> {
    let mut file = File::open(entry.abs_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let normalized_path = entry.rel_name.replace('\\', "/");
    let block = tar_writer.write_pax_and_build_ustar_header(TarHeaderBuildParams {
        normalized_path: &normalized_path,
        size: entry.size,
        mode: entry.mode,
        mtime: entry.mtime,
        typeflag: TYPE_REGULAR,
        link_target: None,
        pax_extra: None,
    })?;

    if entry.size == 0 {
        tar_writer
            .inner
            .write_all(&block)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    } else {
        state.io_buf[..TAR_BLOCK_SIZE].copy_from_slice(&block);
        stream_tar_file_payload(
            tar_writer,
            &mut file,
            entry.rel_name,
            entry.size,
            state,
        )?;
    }
    Ok(())
}

fn stream_tar_file_payload<W: Write>(
    tar_writer: &mut TarWriter<W>,
    file: &mut File,
    rel_name: &str,
    size: u64,
    state: &mut TarDiskStreamState<'_, '_>,
) -> Result<(), TTZipStatus> {
    let pad = (TAR_BLOCK_SIZE - ((size as usize) % TAR_BLOCK_SIZE)) % TAR_BLOCK_SIZE;
    let max_chunk = state.io_buf.len().saturating_sub(TAR_BLOCK_SIZE * 2).max(4096);
    let mut prefix_len = TAR_BLOCK_SIZE;
    let mut remaining = size;
    while remaining > 0 {
        let to_read = (remaining as usize).min(max_chunk);
        let n = file
            .read(&mut state.io_buf[prefix_len..prefix_len + to_read])
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        if n == 0 {
            break;
        }
        remaining = remaining.saturating_sub(n as u64);
        let mut write_end = prefix_len + n;
        if remaining == 0 && pad > 0 {
            state.io_buf[write_end..write_end + pad].fill(0);
            write_end += pad;
        }
        tar_writer
            .inner
            .write_all(&state.io_buf[..write_end])
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        prefix_len = 0;
        *state.processed_bytes = state.processed_bytes.saturating_add(n as u64);

        if let Some(cb) = state.options.progress_callback {
            let rel_c = std::ffi::CString::new(rel_name).unwrap_or_default();
            let should_continue = unsafe {
                cb(
                    *state.processed_bytes,
                    state.total_bytes.max(*state.processed_bytes),
                    rel_c.as_ptr(),
                    state.options.user_data,
                )
            };
            if !should_continue {
                return Err(TTZipStatus::Cancelled);
            }
        }
    }
    if remaining > 0 {
        let mut write_end = prefix_len;
        if pad > 0 {
            state.io_buf[write_end..write_end + pad].fill(0);
            write_end += pad;
        }
        if write_end > 0 {
            tar_writer
                .inner
                .write_all(&state.io_buf[..write_end])
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_collect_tar_entries_deep_worklist_iteration() {
        let temp = tempdir().expect("tempdir");
        let mut curr = temp.path().to_path_buf();
        for i in 0..50 {
            curr = curr.join(format!("level_{}", i));
            fs::create_dir(&curr).expect("create_dir");
        }
        let leaf_file = curr.join("leaf.txt");
        fs::write(&leaf_file, b"deep_content").expect("write");

        let mut out = Vec::new();
        collect_tar_entries_recursive(temp.path(), temp.path(), &mut out).expect("collect");

        assert_eq!(out.len(), 51);
        assert!(out.iter().any(|(_, rel)| rel.ends_with("leaf.txt")));
    }
}
