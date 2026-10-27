// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Bounded-memory streaming sinks, spilled payload management, and Central Directory serialization.

use super::types::unix_to_dos_time;
use crate::types::TTZipStatus;
use crate::zip::extra::ZipExtraFields;
use crate::zip::parser::{
    MAGIC_CDFH, MAGIC_EOCD, MAGIC_ZIP64_EOCD, MAGIC_ZIP64_LOCATOR,
};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const STREAM_CHUNK_SIZE: usize = 64 * 1024;
pub const MAX_IN_MEMORY_PAYLOAD: usize = 60 * 1024 * 1024;

#[derive(Debug)]
pub struct SpillFile {
    pub path: PathBuf,
    pub file: File,
    pub len: u64,
}

impl Drop for SpillFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Debug, Clone)]
pub enum EntryPayload {
    Memory(Vec<u8>),
    Spill(Arc<SpillFile>),
    OriginalFile { path: PathBuf, len: u64 },
}

impl EntryPayload {
    #[inline]
    pub fn len(&self) -> u64 {
        match self {
            Self::Memory(v) => v.len() as u64,
            Self::Spill(spill) => spill.len,
            Self::OriginalFile { len, .. } => *len,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn write_to_file_at(&self, out_file: &File, offset: u64) -> Result<(), TTZipStatus> {
        match self {
            Self::Memory(v) => {
                if !v.is_empty() {
                    out_file
                        .write_all_at(v, offset)
                        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                }
                Ok(())
            }
            Self::Spill(spill) => stream_file_at(&spill.path, out_file, offset),
            Self::OriginalFile { path, .. } => stream_file_at(path, out_file, offset),
        }
    }
}

pub fn stream_file_at(src: &Path, dst: &File, mut offset: u64) -> Result<(), TTZipStatus> {
    let mut f = File::open(src).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut buf = [0u8; STREAM_CHUNK_SIZE];
    loop {
        let n = f.read(&mut buf).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        if n == 0 {
            break;
        }
        dst.write_all_at(&buf[..n], offset)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        offset += n as u64;
    }
    Ok(())
}

pub struct BoundedSink {
    memory_buf: Vec<u8>,
    spill: Option<SpillFile>,
    pub total_written: u64,
}

impl BoundedSink {
    pub fn new() -> Self {
        Self {
            memory_buf: Vec::with_capacity(STREAM_CHUNK_SIZE),
            spill: None,
            total_written: 0,
        }
    }

    pub fn into_payload(self) -> EntryPayload {
        if let Some(spill) = self.spill {
            EntryPayload::Spill(Arc::new(spill))
        } else {
            EntryPayload::Memory(self.memory_buf)
        }
    }
}

impl Default for BoundedSink {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for BoundedSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if let Some(ref mut spill) = self.spill {
            spill.file.write_all(buf)?;
            spill.len += buf.len() as u64;
            self.total_written += buf.len() as u64;
            return Ok(buf.len());
        }
        if self.memory_buf.len() + buf.len() > MAX_IN_MEMORY_PAYLOAD {
            let temp_path = std::env::temp_dir().join(format!(
                "ttzip_spill_{}_{}_{}.tmp",
                std::process::id(),
                unsafe { libc::arc4random() },
                self.total_written
            ));
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .read(true)
                .open(&temp_path)?;
            file.write_all(&self.memory_buf)?;
            file.write_all(buf)?;
            let total = (self.memory_buf.len() + buf.len()) as u64;
            self.memory_buf = Vec::new();
            self.memory_buf.shrink_to_fit();
            self.spill = Some(SpillFile {
                path: temp_path,
                file,
                len: total,
            });
            self.total_written += buf.len() as u64;
            return Ok(buf.len());
        }
        self.memory_buf.extend_from_slice(buf);
        self.total_written += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(ref mut spill) = self.spill {
            spill.file.flush()?;
        }
        Ok(())
    }
}

/// Compact ~80-byte metadata retained in memory for Central Directory construction.
#[derive(Debug, Clone)]
pub struct CentralDirectoryMeta {
    pub rel_path: String,
    pub lfh_offset: u64,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub compression_method: u16,
    pub actual_method: u16,
    pub is_encrypted: bool,
    pub mtime_secs: u32,
    pub mode: u32,
    pub is_directory: bool,
}

pub fn build_cdfh_bytes(cd: &CentralDirectoryMeta) -> Vec<u8> {
    let (dos_time, dos_date) = unix_to_dos_time(cd.mtime_secs);
    let name_bytes = cd.rel_path.as_bytes();

    let is_zip64 = cd.uncompressed_size >= 0xFFFF_FFFF
        || cd.compressed_size >= 0xFFFF_FFFF
        || cd.lfh_offset >= 0xFFFF_FFFF;

    let mut extra_fields = if is_zip64 {
        ZipExtraFields::build_zip64_extra(
            Some(cd.uncompressed_size),
            Some(cd.compressed_size),
            Some(cd.lfh_offset),
        )
    } else {
        Vec::new()
    };

    if cd.is_encrypted && cd.compression_method == 99 {
        extra_fields.extend_from_slice(&ZipExtraFields::build_winzip_aes_extra(cd.actual_method));
    }
    extra_fields.extend_from_slice(&ZipExtraFields::build_extended_timestamp(cd.mtime_secs));

    let flag = if cd.is_directory {
        0x0800u16
    } else if cd.is_encrypted {
        0x0809u16
    } else {
        0x0808u16
    };

    let mut buf = Vec::with_capacity(46 + name_bytes.len() + extra_fields.len());
    buf.extend_from_slice(&MAGIC_CDFH.to_le_bytes());
    buf.extend_from_slice(&0x031Eu16.to_le_bytes());
    buf.extend_from_slice(&(if is_zip64 || cd.is_encrypted { 45u16 } else { 20u16 }).to_le_bytes());
    buf.extend_from_slice(&flag.to_le_bytes());
    buf.extend_from_slice(&cd.compression_method.to_le_bytes());
    buf.extend_from_slice(&dos_time.to_le_bytes());
    buf.extend_from_slice(&dos_date.to_le_bytes());
    buf.extend_from_slice(
        &(if cd.is_encrypted && cd.compression_method == 99 {
            0u32
        } else {
            cd.crc32
        })
        .to_le_bytes(),
    );
    buf.extend_from_slice(
        &(if is_zip64 {
            0xFFFF_FFFFu32
        } else {
            cd.compressed_size as u32
        })
        .to_le_bytes(),
    );
    buf.extend_from_slice(
        &(if is_zip64 {
            0xFFFF_FFFFu32
        } else {
            cd.uncompressed_size as u32
        })
        .to_le_bytes(),
    );
    buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(&(extra_fields.len() as u16).to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    let external_attr = (cd.mode << 16) | if cd.is_directory { 0x10 } else { 0x20 };
    buf.extend_from_slice(&external_attr.to_le_bytes());
    buf.extend_from_slice(
        &(if is_zip64 {
            0xFFFF_FFFFu32
        } else {
            cd.lfh_offset as u32
        })
        .to_le_bytes(),
    );
    buf.extend_from_slice(name_bytes);
    buf.extend_from_slice(&extra_fields);
    buf
}

pub fn build_zip64_eocd(total_entries: u64, cd_size: u64, cd_offset: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(56);
    buf.extend_from_slice(&MAGIC_ZIP64_EOCD.to_le_bytes());
    buf.extend_from_slice(&44u64.to_le_bytes());
    buf.extend_from_slice(&45u16.to_le_bytes());
    buf.extend_from_slice(&45u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&total_entries.to_le_bytes());
    buf.extend_from_slice(&total_entries.to_le_bytes());
    buf.extend_from_slice(&cd_size.to_le_bytes());
    buf.extend_from_slice(&cd_offset.to_le_bytes());
    buf
}

pub fn build_zip64_locator(zip64_eocd_offset: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(20);
    buf.extend_from_slice(&MAGIC_ZIP64_LOCATOR.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&zip64_eocd_offset.to_le_bytes());
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf
}

pub fn build_eocd(entries_count: u16, cd_size: u32, cd_offset: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(22);
    buf.extend_from_slice(&MAGIC_EOCD.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&entries_count.to_le_bytes());
    buf.extend_from_slice(&entries_count.to_le_bytes());
    buf.extend_from_slice(&cd_size.to_le_bytes());
    buf.extend_from_slice(&cd_offset.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf
}

/// Unified output sink for single-file (`pwrite`) or single-pass split-volume (`SplitVolumeWriter`) ZIP streams.
pub enum ZipOutputSink {
    Single {
        file: File,
        current_offset: u64,
        abort_guard: Option<crate::fs::FileAbortGuard>,
    },
    Split {
        writer: crate::archive::split::SplitVolumeWriter,
        committed: bool,
    },
}

impl ZipOutputSink {
    /// Opens a single-file or split-volume ZIP output sink.
    pub fn open(
        dest_path: &Path,
        split_volume_size_bytes: u64,
        prealloc_hint: u64,
    ) -> Result<Self, TTZipStatus> {
        if let Some(parent) = dest_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            let _ = fs::create_dir_all(parent);
        }
        if split_volume_size_bytes > 0 {
            let (base_path, scheme) =
                if dest_path.extension().and_then(|e| e.to_str()) == Some("001") {
                    (
                        dest_path.with_extension(""),
                        crate::archive::split::VolumeNamingScheme::NumberedExtension,
                    )
                } else {
                    (
                        dest_path.to_path_buf(),
                        crate::archive::split::VolumeNamingScheme::PkzipSpanned,
                    )
                };
            let writer = crate::archive::split::SplitVolumeWriter::new(
                &base_path,
                split_volume_size_bytes,
                scheme,
            )
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
            Ok(Self::Split {
                writer,
                committed: false,
            })
        } else {
            let abort_guard = crate::fs::FileAbortGuard::new(dest_path);
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(dest_path)
                .map_err(|_| TTZipStatus::ErrOpenFailed)?;
            if prealloc_hint > 0 {
                use std::os::unix::io::AsRawFd;
                let _ = crate::fs::apfs::apfs_preallocate(file.as_raw_fd(), prealloc_hint as i64);
            }
            Ok(Self::Single {
                file,
                current_offset: 0,
                abort_guard: Some(abort_guard),
            })
        }
    }

    /// Current virtual offset across the output stream.
    #[inline]
    pub fn current_offset(&self) -> u64 {
        match self {
            Self::Single { current_offset, .. } => *current_offset,
            Self::Split { writer, .. } => writer.total_bytes(),
        }
    }

    /// Appends a slice of bytes sequentially to the output sink.
    pub fn write_bytes(&mut self, buf: &[u8]) -> Result<(), TTZipStatus> {
        if buf.is_empty() {
            return Ok(());
        }
        match self {
            Self::Single {
                file,
                current_offset,
                ..
            } => {
                file.write_all_at(buf, *current_offset)
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                *current_offset += buf.len() as u64;
                Ok(())
            }
            Self::Split { writer, .. } => writer
                .write_all(buf)
                .map_err(|_| TTZipStatus::ErrCompressionFailed),
        }
    }

    /// Streams an `EntryPayload` sequentially into the output sink.
    pub fn write_payload(&mut self, payload: &EntryPayload) -> Result<(), TTZipStatus> {
        match self {
            Self::Single {
                file,
                current_offset,
                ..
            } => {
                if !payload.is_empty() {
                    payload.write_to_file_at(file, *current_offset)?;
                    *current_offset += payload.len();
                }
                Ok(())
            }
            Self::Split { writer, .. } => match payload {
                EntryPayload::Memory(v) => writer
                    .write_all(v)
                    .map_err(|_| TTZipStatus::ErrCompressionFailed),
                EntryPayload::Spill(spill) => stream_file_to_writer(&spill.path, writer),
                EntryPayload::OriginalFile { path, .. } => stream_file_to_writer(path, writer),
            },
        }
    }

    /// Commits the output sink and returns the final total compressed byte count.
    pub fn commit(mut self) -> Result<u64, TTZipStatus> {
        match &mut self {
            Self::Single {
                current_offset,
                abort_guard,
                ..
            } => {
                if let Some(guard) = abort_guard.take() {
                    guard.commit();
                }
                Ok(*current_offset)
            }
            Self::Split { writer, committed } => {
                let total = writer.total_bytes();
                writer
                    .close()
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                *committed = true;
                Ok(total)
            }
        }
    }
}

impl Drop for ZipOutputSink {
    fn drop(&mut self) {
        if let Self::Split { writer, committed } = self {
            if !*committed {
                writer.cancel_and_cleanup();
            }
        }
    }
}

fn stream_file_to_writer<W: Write>(src: &Path, dst: &mut W) -> Result<(), TTZipStatus> {
    let mut f = File::open(src).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut buf = [0u8; STREAM_CHUNK_SIZE];
    loop {
        let n = f.read(&mut buf).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        if n == 0 {
            break;
        }
        dst.write_all(&buf[..n])
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    }
    Ok(())
}

