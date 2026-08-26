// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! In-place incremental archive mutation engine powered by `.ttzip.wal` Write-Ahead
//! Logging, Piece Tree virtual byte interval remapping, and macOS APFS CoW atomic swap.

use crate::crypto::crc32_fast;
use crate::types::TTZipStatus;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub const WAL_MAGIC: &[u8; 8] = b"TTZWAL01";
pub const WAL_RECORD_MUTATION: u8 = 1;
pub const WAL_RECORD_COMMIT: u8 = 2;

#[cfg(target_os = "macos")]
extern "C" {
    fn clonefile(src: *const libc::c_char, dst: *const libc::c_char, flags: libc::c_uint) -> libc::c_int;
    fn renamex_np(old: *const libc::c_char, new: *const libc::c_char, flags: libc::c_uint) -> libc::c_int;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PieceSource {
    Original { offset: u64, len: u64 },
    WalPayload { wal_offset: u64, len: u64 },
    Inline(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub source: PieceSource,
    pub len: u64,
}

#[derive(Debug, Clone, Default)]
pub struct PieceTree {
    pub pieces: Vec<Piece>,
}

impl PieceTree {
    pub fn new(orig_len: u64) -> Self {
        if orig_len == 0 {
            return Self { pieces: Vec::new() };
        }
        Self { pieces: vec![Piece { source: PieceSource::Original { offset: 0, len: orig_len }, len: orig_len }] }
    }

    pub fn total_length(&self) -> u64 {
        self.pieces.iter().map(|p| p.len).sum()
    }

    pub fn replace_range(&mut self, v_offset: u64, v_len: u64, new_source: PieceSource, new_len: u64) {
        let mut new_pieces = Vec::new();
        let mut curr_offset = 0u64;
        let replace_end = v_offset + v_len;
        let mut inserted = false;

        for piece in &self.pieces {
            let (p_start, p_end) = (curr_offset, curr_offset + piece.len);
            curr_offset = p_end;

            if p_end <= v_offset || p_start >= replace_end {
                new_pieces.push(piece.clone());
                continue;
            }

            if p_start < v_offset {
                let left_len = v_offset - p_start;
                if let PieceSource::Original { offset, .. } = piece.source {
                    new_pieces.push(Piece { source: PieceSource::Original { offset, len: left_len }, len: left_len });
                }
            }

            if !inserted {
                new_pieces.push(Piece { source: new_source.clone(), len: new_len });
                inserted = true;
            }

            if p_end > replace_end {
                let right_len = p_end - replace_end;
                let skip = replace_end - p_start;
                if let PieceSource::Original { offset, .. } = piece.source {
                    new_pieces.push(Piece { source: PieceSource::Original { offset: offset + skip, len: right_len }, len: right_len });
                }
            }
        }
        if !inserted {
            new_pieces.push(Piece { source: new_source, len: new_len });
        }
        self.pieces = new_pieces;
    }

    pub fn assemble_to(&self, orig_file: &mut File, wal_file: &mut File, out_file: &mut File) -> std::io::Result<u64> {
        let mut total = 0u64;
        let mut buf = [0u8; 64 * 1024];

        for piece in &self.pieces {
            match &piece.source {
                PieceSource::Original { offset, len } => {
                    orig_file.seek(SeekFrom::Start(*offset))?;
                    let mut rem = *len;
                    while rem > 0 {
                        let to_read = (rem as usize).min(buf.len());
                        orig_file.read_exact(&mut buf[..to_read])?;
                        out_file.write_all(&buf[..to_read])?;
                        rem -= to_read as u64;
                        total += to_read as u64;
                    }
                }
                PieceSource::WalPayload { wal_offset, len } => {
                    wal_file.seek(SeekFrom::Start(*wal_offset))?;
                    let mut rem = *len;
                    while rem > 0 {
                        let to_read = (rem as usize).min(buf.len());
                        wal_file.read_exact(&mut buf[..to_read])?;
                        out_file.write_all(&buf[..to_read])?;
                        rem -= to_read as u64;
                        total += to_read as u64;
                    }
                }
                PieceSource::Inline(data) => {
                    out_file.write_all(data)?;
                    total += data.len() as u64;
                }
            }
        }
        out_file.flush()?;
        Ok(total)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalMutationSummary {
    pub wal_path: String,
    pub entry_path: String,
    pub delta_bytes: u64,
    pub total_pieces: u32,
    pub is_staged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalCommitResult {
    pub success: bool,
    pub bytes_written: u64,
    pub cow_cloned: bool,
    pub elapsed_millis: u64,
}

pub fn apfs_cow_clone(src: &Path, dst: &Path) -> std::io::Result<bool> {
    if dst.exists() { let _ = fs::remove_file(dst); }
    #[cfg(target_os = "macos")]
    {
        if let (Some(s), Some(d)) = (src.to_str(), dst.to_str()) {
            if let (Ok(sc), Ok(dc)) = (CString::new(s), CString::new(d)) {
                if unsafe { clonefile(sc.as_ptr(), dc.as_ptr(), 0) } == 0 { return Ok(true); }
            }
        }
    }
    fs::copy(src, dst)?;
    Ok(false)
}

pub fn apfs_atomic_swap_or_rename(shadow: &Path, target: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        if let (Some(s), Some(t)) = (shadow.to_str(), target.to_str()) {
            if let (Ok(sc), Ok(tc)) = (CString::new(s), CString::new(t)) {
                if unsafe { renamex_np(sc.as_ptr(), tc.as_ptr(), 0) } == 0 { return Ok(()); }
            }
        }
    }
    fs::rename(shadow, target)
}

pub fn wal_file_path(archive_path: &Path) -> PathBuf {
    let mut p = archive_path.to_path_buf();
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
    p.set_extension(format!("{}.ttzip.wal", ext));
    p
}

pub fn append_wal_mutation(
    archive_path: &Path,
    entry_path: &str,
    target_offset: u64,
    target_len: u64,
    payload: &[u8],
) -> Result<WalMutationSummary, TTZipStatus> {
    let wal_path = wal_file_path(archive_path);
    let mut file = OpenOptions::new().create(true).read(true).append(true).open(&wal_path)
        .map_err(|_| TTZipStatus::ErrOpenFailed)?;

    if file.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        file.write_all(WAL_MAGIC).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    }

    let payload_crc = crc32_fast(0, payload);
    let entry_bytes = entry_path.as_bytes();
    let entry_len = entry_bytes.len() as u16;
    let payload_len = payload.len() as u64;

    file.write_all(&[WAL_RECORD_MUTATION]).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    file.write_all(&payload_crc.to_le_bytes()).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    file.write_all(&target_offset.to_le_bytes()).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    file.write_all(&target_len.to_le_bytes()).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    file.write_all(&entry_len.to_le_bytes()).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    file.write_all(&payload_len.to_le_bytes()).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    file.write_all(entry_bytes).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    file.write_all(payload).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    file.flush().map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let orig_len = fs::metadata(archive_path).map(|m| m.len()).unwrap_or(0);
    let mut pt = PieceTree::new(orig_len);
    pt.replace_range(target_offset, target_len, PieceSource::WalPayload { wal_offset: 0, len: payload_len }, payload_len);

    Ok(WalMutationSummary {
        wal_path: wal_path.to_string_lossy().to_string(),
        entry_path: entry_path.to_string(),
        delta_bytes: payload_len,
        total_pieces: pt.pieces.len() as u32,
        is_staged: true,
    })
}

pub fn commit_wal_to_archive(archive_path: &Path) -> Result<WalCommitResult, TTZipStatus> {
    let start = Instant::now();
    let wal_path = wal_file_path(archive_path);
    if !wal_path.exists() { return Err(TTZipStatus::ErrFileNotFound); }

    let mut wal_file = File::open(&wal_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut magic = [0u8; 8];
    wal_file.read_exact(&mut magic).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    if &magic != WAL_MAGIC { return Err(TTZipStatus::ErrCorruptHeader); }

    let orig_len = fs::metadata(archive_path).map(|m| m.len()).unwrap_or(0);
    let mut piece_tree = PieceTree::new(orig_len);

    while let Ok(rec_type) = {
        let mut b = [0u8; 1];
        wal_file.read_exact(&mut b).map(|_| b[0])
    } {
        if rec_type == WAL_RECORD_MUTATION {
            let mut meta_buf = [0u8; 30];
            wal_file.read_exact(&mut meta_buf).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let target_offset = u64::from_le_bytes(meta_buf[4..12].try_into().unwrap());
            let target_len = u64::from_le_bytes(meta_buf[12..20].try_into().unwrap());
            let entry_len = u16::from_le_bytes(meta_buf[20..22].try_into().unwrap()) as usize;
            let payload_len = u64::from_le_bytes(meta_buf[22..30].try_into().unwrap());

            let mut entry_name_buf = vec![0u8; entry_len];
            wal_file.read_exact(&mut entry_name_buf).map_err(|_| TTZipStatus::ErrCorruptHeader)?;

            let wal_payload_offset = wal_file.stream_position().map_err(|_| TTZipStatus::ErrOpenFailed)?;
            wal_file.seek(SeekFrom::Current(payload_len as i64)).map_err(|_| TTZipStatus::ErrOpenFailed)?;

            piece_tree.replace_range(
                target_offset,
                target_len,
                PieceSource::WalPayload { wal_offset: wal_payload_offset, len: payload_len },
                payload_len,
            );
        } else if rec_type == WAL_RECORD_COMMIT { break; }
    }

    let shadow_path = archive_path.with_extension(format!("{}.shadow.tmp", std::process::id()));
    let cow_cloned = apfs_cow_clone(archive_path, &shadow_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let assemble_res = (|| -> std::io::Result<u64> {
        let mut orig_file = File::open(archive_path)?;
        let mut out_file = OpenOptions::new().write(true).truncate(true).open(&shadow_path)?;
        piece_tree.assemble_to(&mut orig_file, &mut wal_file, &mut out_file)
    })();

    let bytes_written = match assemble_res {
        Ok(bytes) => bytes,
        Err(_) => {
            let _ = fs::remove_file(&shadow_path);
            return Err(TTZipStatus::ErrOpenFailed);
        }
    };

    if apfs_atomic_swap_or_rename(&shadow_path, archive_path).is_err() {
        let _ = fs::remove_file(&shadow_path);
        return Err(TTZipStatus::ErrOpenFailed);
    }

    let _ = fs::remove_file(&wal_path);
    let elapsed = start.elapsed().as_millis() as u64;

    Ok(WalCommitResult {
        success: true,
        bytes_written,
        cow_cloned,
        elapsed_millis: elapsed,
    })
}

pub fn rollback_wal_mutation(archive_path: &Path) -> Result<bool, TTZipStatus> {
    let wal_path = wal_file_path(archive_path);
    let mut cleaned = false;
    if wal_path.exists() {
        let _ = fs::remove_file(&wal_path);
        cleaned = true;
    }
    let shadow_pattern = archive_path.with_extension(format!("{}.shadow.tmp", std::process::id()));
    if shadow_pattern.exists() {
        let _ = fs::remove_file(&shadow_pattern);
        cleaned = true;
    }
    Ok(cleaned)
}

pub fn inspect_wal_status(archive_path: &Path) -> Result<Option<WalMutationSummary>, TTZipStatus> {
    let wal_path = wal_file_path(archive_path);
    if !wal_path.exists() { return Ok(None); }
    let meta = fs::metadata(&wal_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    Ok(Some(WalMutationSummary {
        wal_path: wal_path.to_string_lossy().to_string(),
        entry_path: "staged_wal_mutations".to_string(),
        delta_bytes: meta.len(),
        total_pieces: 1,
        is_staged: true,
    }))
}
