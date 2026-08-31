// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Native APFS and Linux physical hole punching and TAR sparse stream generator.
//!
//! Implements hardware-level sparse extent discovery (`lseek(SEEK_DATA/SEEK_HOLE)` on Linux
//! and APFS block scans on macOS), GNU Sparse 0.1/1.0 streaming serialization, and
//! zero-loss physical hole restoration to eradicate dense zero-filling.

use super::{SparseExtent, SparseMap, TarSparseFormat};
use crate::archive::tar::pax::build_pax_payload;
use crate::tar::header::TarHeader;
use crate::tar::types::{TarEntryType, BLOCK_SIZE};

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

#[cfg(target_os = "macos")]
const SEEK_DATA: libc::c_int = 4;
#[cfg(target_os = "macos")]
const SEEK_HOLE: libc::c_int = 3;
#[cfg(target_os = "macos")]
pub const F_PUNCHHOLE: libc::c_int = 99;

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "android",
    target_os = "solaris"
))]
use libc::{SEEK_DATA, SEEK_HOLE};

/// macOS kernel `fpunchhole_t` structure for `F_PUNCHHOLE` fcntl command.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct fpunchhole_t {
    pub fp_flags: u32,
    pub reserved: u32,
    pub fp_offset: i64,
    pub fp_length: i64,
}

/// Helper to coalesce contiguous or overlapping sparse extents in-place.
fn coalesce_extents(extents: &mut Vec<SparseExtent>) {
    extents.retain(|e| !e.is_empty());
    if extents.len() <= 1 {
        return;
    }

    extents.sort_unstable_by_key(|e| (e.offset, e.numbytes));
    let mut merged = Vec::with_capacity(extents.len());
    let mut current = extents[0];

    for next in extents.iter().skip(1) {
        if current.offset <= next.end_offset() && next.offset <= current.end_offset() {
            let start = current.offset.min(next.offset);
            let end = current.end_offset().max(next.end_offset());
            current = SparseExtent::new(start, end.saturating_sub(start));
        } else {
            merged.push(current);
            current = *next;
        }
    }
    merged.push(current);
    *extents = merged;
}

/// Detects non-zero data extents of a file using OS-native syscalls or fast block scanning.
///
/// On Linux/FreeBSD, leverages `lseek(SEEK_DATA/SEEK_HOLE)`. On macOS APFS and other platforms,
/// falls back to fast 16KB/64KB SIMD-aligned zero-block scanning.
pub fn detect_file_sparse_extents(file: &File, size: u64) -> io::Result<Vec<SparseExtent>> {
    if size == 0 {
        return Ok(Vec::new());
    }

    #[cfg(unix)]
    {
        let fd = file.as_raw_fd();

        #[cfg(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "android",
            target_os = "solaris"
        ))]
        {
            if let Ok(extents) = detect_sparse_extents_lseek(fd, size) {
                return Ok(extents);
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(extents) = detect_sparse_extents_lseek(fd, size) {
                if !extents.is_empty() {
                    return Ok(extents);
                }
            }
        }
    }

    // Universal fallback: scan seekable clone with 16KB Apple Silicon page-aligned buffer
    let mut dup_file = file.try_clone()?;
    let fs_extents =
        crate::fs::sparse::detect_sparse_extents_from_reader(&mut dup_file, size, 16384)?;
    let mut extents: Vec<SparseExtent> = fs_extents
        .into_iter()
        .map(|e| SparseExtent::new(e.offset, e.length))
        .collect();
    coalesce_extents(&mut extents);
    Ok(extents)
}

/// Helper using POSIX `lseek(SEEK_DATA/SEEK_HOLE)` to discover physical data ranges.
#[cfg(unix)]
fn detect_sparse_extents_lseek(
    fd: std::os::unix::io::RawFd,
    size: u64,
) -> io::Result<Vec<SparseExtent>> {
    let mut extents = Vec::new();
    let mut offset: libc::off_t = 0;
    let end: libc::off_t = size as libc::off_t;

    while offset < end {
        let data_pos = unsafe { libc::lseek(fd, offset, SEEK_DATA) };
        if data_pos < 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ENXIO) {
                // No more data blocks in file
                break;
            }
            return Err(err);
        }
        if data_pos >= end {
            break;
        }

        let hole_pos = unsafe { libc::lseek(fd, data_pos, SEEK_HOLE) };
        if hole_pos < 0 {
            return Err(io::Error::last_os_error());
        }

        let extent_start = data_pos as u64;
        let extent_end = (hole_pos as u64).min(size);
        if extent_end > extent_start {
            extents.push(SparseExtent::new(extent_start, extent_end - extent_start));
        }

        if hole_pos >= end || hole_pos <= data_pos {
            break;
        }
        offset = hole_pos;
    }

    coalesce_extents(&mut extents);
    Ok(extents)
}

/// Explicitly punches a physical hole in a file on supported filesystems (APFS / ext4 / XFS).
pub fn punch_file_hole(file: &File, offset: u64, length: u64) -> io::Result<()> {
    if length == 0 {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let punch = fpunchhole_t {
            fp_flags: 0,
            reserved: 0,
            fp_offset: offset as i64,
            fp_length: length as i64,
        };
        let ret = unsafe { libc::fcntl(file.as_raw_fd(), F_PUNCHHOLE, &punch) };
        if ret == 0 {
            return Ok(());
        }
    }

    #[cfg(target_os = "linux")]
    {
        let ret = unsafe {
            libc::fallocate(
                file.as_raw_fd(),
                libc::FALLOC_FL_PUNCH_HOLE | libc::FALLOC_FL_KEEP_SIZE,
                offset as libc::off_t,
                length as libc::off_t,
            )
        };
        if ret == 0 {
            return Ok(());
        }
    }

    Ok(())
}

/// Serializes a sparse file into a TAR stream using GNU Sparse 0.1 or GNU Sparse 1.0 format.
///
/// Returns the total number of bytes written to `writer` (headers, metadata, non-zero data, padding).
pub fn write_sparse_file_to_tar<W: Write>(
    writer: &mut W,
    file: &mut File,
    path: &str,
    mode: TarSparseFormat,
) -> io::Result<u64> {
    let normalized_path = path.replace('\\', "/");
    let metadata = file.metadata()?;
    let real_size = metadata.len();

    let extents = detect_file_sparse_extents(file, real_size)?;
    let sparse_map = SparseMap::new(real_size, extents);
    let total_data_bytes = sparse_map.total_data_bytes();

    let file_mode = {
        #[cfg(unix)]
        {
            metadata.permissions().mode() & 0o777
        }
        #[cfg(not(unix))]
        {
            0o644
        }
    };

    let mtime = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut total_written: u64 = 0;

    match mode {
        TarSparseFormat::Gnu0_1 => {
            let map_str = sparse_map.to_gnu_0_1_map_string();
            let real_size_str = real_size.to_string();
            let num_blocks_str = sparse_map.extents.len().to_string();

            let mut pax_records: Vec<(&str, &str)> = vec![
                ("GNU.sparse.major", "0"),
                ("GNU.sparse.minor", "1"),
                ("GNU.sparse.name", &normalized_path),
                ("GNU.sparse.realsize", &real_size_str),
                ("GNU.sparse.size", &real_size_str),
                ("GNU.sparse.numblocks", &num_blocks_str),
                ("GNU.sparse.map", &map_str),
            ];

            if normalized_path.len() > 100 {
                pax_records.push(("path", &normalized_path));
            }

            let pax_payload = build_pax_payload(&pax_records);
            let pax_filename = format!(
                "PaxHeaders.0/{}",
                normalized_path.rsplit('/').next().unwrap_or("sparse_entry")
            );

            // Write PAX header block
            let mut pax_hdr = TarHeader::new();
            pax_hdr.set_name(&pax_filename);
            pax_hdr.set_mode(0o644);
            pax_hdr.set_size(pax_payload.len() as u64);
            pax_hdr.set_mtime(mtime);
            pax_hdr.set_entry_type(TarEntryType::XHeader);
            pax_hdr.set_ustar_magic();
            pax_hdr.update_checksum();

            writer.write_all(pax_hdr.as_bytes())?;
            total_written += BLOCK_SIZE as u64;

            // Write PAX payload with 512-byte padding
            writer.write_all(&pax_payload)?;
            total_written += pax_payload.len() as u64;

            let pax_pad = (BLOCK_SIZE - (pax_payload.len() % BLOCK_SIZE)) % BLOCK_SIZE;
            if pax_pad > 0 {
                let pad_zeros = [0u8; BLOCK_SIZE];
                writer.write_all(&pad_zeros[..pax_pad])?;
                total_written += pax_pad as u64;
            }

            // Write main sparse header block
            let mut main_hdr = TarHeader::new();
            main_hdr.set_name(&normalized_path);
            main_hdr.set_mode(file_mode);
            main_hdr.set_size(total_data_bytes);
            main_hdr.set_mtime(mtime);
            main_hdr.set_entry_type(TarEntryType::GNUSparse);
            main_hdr.set_ustar_magic();
            main_hdr.update_checksum();

            writer.write_all(main_hdr.as_bytes())?;
            total_written += BLOCK_SIZE as u64;

            // Stream non-zero data extents
            let streamed_bytes = stream_extents_to_writer(writer, file, &sparse_map.extents)?;
            total_written += streamed_bytes;

            // Trailing block padding for total data
            let data_pad = (BLOCK_SIZE - ((total_data_bytes as usize) % BLOCK_SIZE)) % BLOCK_SIZE;
            if data_pad > 0 {
                let pad_zeros = [0u8; BLOCK_SIZE];
                writer.write_all(&pad_zeros[..data_pad])?;
                total_written += data_pad as u64;
            }
        }
        TarSparseFormat::Gnu1_0 => {
            let real_size_str = real_size.to_string();

            let mut pax_records: Vec<(&str, &str)> = vec![
                ("GNU.sparse.major", "1"),
                ("GNU.sparse.minor", "0"),
                ("GNU.sparse.name", &normalized_path),
                ("GNU.sparse.realsize", &real_size_str),
            ];

            if normalized_path.len() > 100 {
                pax_records.push(("path", &normalized_path));
            }

            let pax_payload = build_pax_payload(&pax_records);
            let pax_filename = format!(
                "PaxHeaders.0/{}",
                normalized_path.rsplit('/').next().unwrap_or("sparse_entry")
            );

            // Write PAX header block
            let mut pax_hdr = TarHeader::new();
            pax_hdr.set_name(&pax_filename);
            pax_hdr.set_mode(0o644);
            pax_hdr.set_size(pax_payload.len() as u64);
            pax_hdr.set_mtime(mtime);
            pax_hdr.set_entry_type(TarEntryType::XHeader);
            pax_hdr.set_ustar_magic();
            pax_hdr.update_checksum();

            writer.write_all(pax_hdr.as_bytes())?;
            total_written += BLOCK_SIZE as u64;

            // Write PAX payload with padding
            writer.write_all(&pax_payload)?;
            total_written += pax_payload.len() as u64;

            let pax_pad = (BLOCK_SIZE - (pax_payload.len() % BLOCK_SIZE)) % BLOCK_SIZE;
            if pax_pad > 0 {
                let pad_zeros = [0u8; BLOCK_SIZE];
                writer.write_all(&pad_zeros[..pax_pad])?;
                total_written += pax_pad as u64;
            }

            // Generate GNU 1.0 map block
            let map_block = sparse_map.to_gnu_1_0_map_block();
            let total_entry_size = map_block.len() as u64 + total_data_bytes;

            // Write main sparse header block
            let mut main_hdr = TarHeader::new();
            main_hdr.set_name(&normalized_path);
            main_hdr.set_mode(file_mode);
            main_hdr.set_size(total_entry_size);
            main_hdr.set_mtime(mtime);
            main_hdr.set_entry_type(TarEntryType::GNUSparse);
            main_hdr.set_ustar_magic();
            main_hdr.update_checksum();

            writer.write_all(main_hdr.as_bytes())?;
            total_written += BLOCK_SIZE as u64;

            // Write map block
            writer.write_all(&map_block)?;
            total_written += map_block.len() as u64;

            // Stream non-zero data extents
            let streamed_bytes = stream_extents_to_writer(writer, file, &sparse_map.extents)?;
            total_written += streamed_bytes;

            // Trailing block padding for total data
            let data_pad = (BLOCK_SIZE - ((total_data_bytes as usize) % BLOCK_SIZE)) % BLOCK_SIZE;
            if data_pad > 0 {
                let pad_zeros = [0u8; BLOCK_SIZE];
                writer.write_all(&pad_zeros[..data_pad])?;
                total_written += data_pad as u64;
            }
        }
    }

    Ok(total_written)
}

/// Helper to copy non-zero extents from `file` to `writer`.
fn stream_extents_to_writer<W: Write>(
    writer: &mut W,
    file: &mut File,
    extents: &[SparseExtent],
) -> io::Result<u64> {
    let mut buffer = vec![0u8; 64 * 1024];
    let mut written: u64 = 0;

    for extent in extents {
        if extent.numbytes == 0 {
            continue;
        }

        file.seek(SeekFrom::Start(extent.offset))?;
        let mut remaining = extent.numbytes;

        while remaining > 0 {
            let to_read = (remaining as usize).min(buffer.len());
            file.read_exact(&mut buffer[..to_read])?;
            writer.write_all(&buffer[..to_read])?;
            remaining -= to_read as u64;
            written += to_read as u64;
        }
    }

    Ok(written)
}

/// Extracts a sparse file from a TAR payload stream directly to disk with physical hole preservation.
///
/// Reconstructs non-zero data regions with exact seeking and anchors trailing holes with `ftruncate`/`set_len`,
/// eliminating the legacy defect where `tar-rs` writes dense zero bytes and destroys file sparsity.
pub fn extract_sparse_file_with_hole_punching<R: Read>(
    reader: &mut R,
    target_path: &Path,
    sparse_map: &SparseMap,
) -> io::Result<u64> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(target_path)?;

    if sparse_map.real_size == 0 {
        return Ok(0);
    }

    let mut buffer = vec![0u8; 64 * 1024];

    // Scheme A: Precise physical seek reconstruction
    for extent in &sparse_map.extents {
        if extent.numbytes == 0 {
            continue;
        }

        file.seek(SeekFrom::Start(extent.offset))?;
        let mut remaining = extent.numbytes;

        while remaining > 0 {
            let to_read = (remaining as usize).min(buffer.len());
            reader.read_exact(&mut buffer[..to_read])?;
            file.write_all(&buffer[..to_read])?;
            remaining -= to_read as u64;
        }
    }

    // Anchor trailing holes and finalize exact logical length
    file.set_len(sparse_map.real_size)?;
    file.flush()?;

    Ok(sparse_map.real_size)
}
