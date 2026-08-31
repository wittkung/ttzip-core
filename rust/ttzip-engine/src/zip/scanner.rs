// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-Performance Reverse Sliding Window EOCD Scanner & SFX Preamble Resolver.
//!
//! Provides zero-copy backward scanning for the ZIP End of Central Directory (EOCD)
//! record, hardware SIMD vectorization (ARM NEON / x86 SSE2/AVX2), spoofed EOCD signature
//! rejection inside comments, and self-extracting archive (SFX) / Shebang offset adaptation.

use std::io::{Read, Seek, SeekFrom};

// MARK: - Constants

/// Minimum physical byte size of a ZIP End of Central Directory (EOCD) record.
pub const EOCD_MIN_SIZE: usize = 22;

/// Maximum allowable length of a ZIP archive comment in bytes (`u16::MAX`).
pub const MAX_COMMENT_LEN: usize = 65535;

/// Maximum search window backwards from stream end (`22 + 65535 = 65557`).
pub const MAX_EOCD_SEARCH_WINDOW: usize = 65557;

/// Signature for End of Central Directory record (`PK\x05\x06`).
pub const MAGIC_EOCD: u32 = 0x06054B50;

/// Signature for Central Directory File Header (`PK\x01\x02`).
pub const MAGIC_CDFH: u32 = 0x02014B50;

/// Signature for Zip64 End of Central Directory Locator (`PK\x06\x07`).
pub const MAGIC_ZIP64_LOCATOR: u32 = 0x07064B50;

/// Signature for Zip64 End of Central Directory record (`PK\x06\x06`).
pub const MAGIC_ZIP64_EOCD: u32 = 0x06064B50;

// MARK: - Data Models

/// Metadata resolved from the ZIP End of Central Directory (EOCD) record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CentralDirectoryEndInfo {
    /// Absolute byte offset of the standard EOCD record in the stream.
    pub eocd_offset: u64,
    /// Detected SFX/preamble archive offset (0 for standard archives).
    pub archive_offset: u64,
    /// Central Directory start offset (relative to `archive_offset` or absolute).
    pub cd_offset: u64,
    /// Total byte size of the Central Directory records.
    pub cd_size: u64,
    /// Total number of entries in the Central Directory.
    pub total_entries: u64,
    /// Raw archive comment bytes.
    pub comment: Vec<u8>,
}

/// Error classifications during ZIP scanning and parsing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ZipEngineError {
    #[error("Archive size ({actual} bytes) is smaller than minimum EOCD size ({required} bytes)")]
    FileTooSmall { required: usize, actual: usize },

    #[error("End of Central Directory (EOCD) signature not found within search window")]
    EocdNotFound,

    #[error("Invalid comment length: declared {declared} bytes, but file only has {available} bytes after EOCD")]
    InvalidCommentLength { declared: usize, available: usize },

    #[error("Corrupted ZIP header or central directory structure: {0}")]
    CorruptedHeader(String),

    #[error("Central directory offset ({offset}) or size ({size}) exceeds file boundary ({file_len})")]
    InvalidCentralDirectoryBoundary { offset: u64, size: u64, file_len: u64 },

    #[error("I/O error during scanning: {0}")]
    Io(String),

    #[error("Status error: {0}")]
    Status(crate::types::TTZipStatus),
}

impl From<std::io::Error> for ZipEngineError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<crate::types::TTZipStatus> for ZipEngineError {
    fn from(status: crate::types::TTZipStatus) -> Self {
        Self::Status(status)
    }
}

impl From<ZipEngineError> for crate::types::TTZipStatus {
    fn from(err: ZipEngineError) -> Self {
        match err {
            ZipEngineError::FileTooSmall { .. } => crate::types::TTZipStatus::ErrCorruptHeader,
            ZipEngineError::EocdNotFound => crate::types::TTZipStatus::ErrCorruptHeader,
            ZipEngineError::InvalidCommentLength { .. } => crate::types::TTZipStatus::ErrCorruptHeader,
            ZipEngineError::CorruptedHeader(_) => crate::types::TTZipStatus::ErrCorruptHeader,
            ZipEngineError::InvalidCentralDirectoryBoundary { .. } => {
                crate::types::TTZipStatus::ErrInvalidOffset
            }
            ZipEngineError::Io(_) => crate::types::TTZipStatus::ErrOpenFailed,
            ZipEngineError::Status(s) => s,
        }
    }
}

// MARK: - Binary Primitive Readers

#[inline(always)]
fn read_u16_le(slice: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([slice[offset], slice[offset + 1]])
}

#[inline(always)]
fn read_u32_le(slice: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        slice[offset],
        slice[offset + 1],
        slice[offset + 2],
        slice[offset + 3],
    ])
}

#[inline(always)]
fn read_u64_le(slice: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        slice[offset],
        slice[offset + 1],
        slice[offset + 2],
        slice[offset + 3],
        slice[offset + 4],
        slice[offset + 5],
        slice[offset + 6],
        slice[offset + 7],
    ])
}

// MARK: - Reverse SIMD & Scalar Matchers

/// Locates all `PK\x05\x06` (`0x06054B50`) candidate offsets in reverse order (high to low).
pub fn find_eocd_candidate_offsets(window: &[u8]) -> Vec<usize> {
    if window.len() < EOCD_MIN_SIZE {
        return Vec::new();
    }
    let max_pos = window.len() - EOCD_MIN_SIZE;
    let mut candidates = Vec::with_capacity(4);

    #[cfg(target_arch = "aarch64")]
    unsafe {
        find_eocd_candidates_neon(window, max_pos, &mut candidates);
    }
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    unsafe {
        find_eocd_candidates_sse2(window, max_pos, &mut candidates);
    }
    #[cfg(not(any(target_arch = "aarch64", all(target_arch = "x86_64", target_feature = "sse2"))))]
    {
        find_eocd_candidates_scalar(window, max_pos, &mut candidates);
    }

    candidates
}

#[inline]
#[allow(dead_code)]
pub fn find_eocd_candidates_scalar(window: &[u8], max_pos: usize, out: &mut Vec<usize>) {
    let mut pos = max_pos as isize;
    while pos >= 0 {
        let p = pos as usize;
        if window[p] == b'P'
            && p + 4 <= window.len()
            && window[p + 1] == b'K'
            && window[p + 2] == 0x05
            && window[p + 3] == 0x06
        {
            out.push(p);
        }
        pos -= 1;
    }
}

#[cfg(target_arch = "aarch64")]
unsafe fn find_eocd_candidates_neon(window: &[u8], max_pos: usize, out: &mut Vec<usize>) {
    use core::arch::aarch64::*;
    let target = vdupq_n_u8(b'P');
    let mut pos = max_pos as isize;

    while pos >= 15 {
        let start = (pos - 15) as usize;
        let chunk = vld1q_u8(window.as_ptr().add(start));
        let cmp = vceqq_u8(chunk, target);
        let max_val = vmaxvq_u8(cmp);
        if max_val != 0 {
            for idx in (start..=pos as usize).rev() {
                if idx + 4 <= window.len()
                    && *window.get_unchecked(idx) == b'P'
                    && *window.get_unchecked(idx + 1) == b'K'
                    && *window.get_unchecked(idx + 2) == 0x05
                    && *window.get_unchecked(idx + 3) == 0x06
                {
                    out.push(idx);
                }
            }
        }
        pos -= 16;
    }

    while pos >= 0 {
        let p = pos as usize;
        if p + 4 <= window.len()
            && window[p] == b'P'
            && window[p + 1] == b'K'
            && window[p + 2] == 0x05
            && window[p + 3] == 0x06
        {
            out.push(p);
        }
        pos -= 1;
    }
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
unsafe fn find_eocd_candidates_sse2(window: &[u8], max_pos: usize, out: &mut Vec<usize>) {
    use core::arch::x86_64::*;
    let target = _mm_set1_epi8(b'P' as i8);
    let mut pos = max_pos as isize;

    while pos >= 15 {
        let start = (pos - 15) as usize;
        let chunk = _mm_loadu_si128(window.as_ptr().add(start) as *const __m128i);
        let cmp = _mm_cmpeq_epi8(chunk, target);
        let mask = _mm_movemask_epi8(cmp);
        if mask != 0 {
            for idx in (start..=pos as usize).rev() {
                if idx + 4 <= window.len()
                    && *window.get_unchecked(idx) == b'P'
                    && *window.get_unchecked(idx + 1) == b'K'
                    && *window.get_unchecked(idx + 2) == 0x05
                    && *window.get_unchecked(idx + 3) == 0x06
                {
                    out.push(idx);
                }
            }
        }
        pos -= 16;
    }

    while pos >= 0 {
        let p = pos as usize;
        if p + 4 <= window.len()
            && window[p] == b'P'
            && window[p + 1] == b'K'
            && window[p + 2] == 0x05
            && window[p + 3] == 0x06
        {
            out.push(p);
        }
        pos -= 1;
    }
}

// MARK: - EocdScanner Core Engine

/// High-performance reverse sliding window scanner for ZIP EOCD records.
pub struct EocdScanner;

impl EocdScanner {
    /// Scans an in-memory byte slice for the ZIP End of Central Directory record.
    pub fn scan_slice(bytes: &[u8]) -> Result<CentralDirectoryEndInfo, ZipEngineError> {
        let file_len = bytes.len() as u64;
        if file_len < EOCD_MIN_SIZE as u64 {
            return Err(ZipEngineError::FileTooSmall {
                required: EOCD_MIN_SIZE,
                actual: bytes.len(),
            });
        }

        let search_window_len = (file_len.min(MAX_EOCD_SEARCH_WINDOW as u64)) as usize;
        let window_start = (file_len as usize) - search_window_len;
        let window = &bytes[window_start..];

        let candidates = find_eocd_candidate_offsets(window);
        if candidates.is_empty() {
            return Err(ZipEngineError::EocdNotFound);
        }

        for &rel_pos in &candidates {
            let abs_eocd_offset = (window_start + rel_pos) as u64;
            if let Ok(info) = Self::validate_and_parse_slice(bytes, abs_eocd_offset, rel_pos, window) {
                return Ok(info);
            }
        }

        Err(ZipEngineError::EocdNotFound)
    }

    /// Scans a `Read + Seek` stream for the ZIP End of Central Directory record.
    pub fn scan<R: Read + Seek>(
        reader: &mut R,
        file_len: u64,
    ) -> Result<CentralDirectoryEndInfo, ZipEngineError> {
        if file_len < EOCD_MIN_SIZE as u64 {
            return Err(ZipEngineError::FileTooSmall {
                required: EOCD_MIN_SIZE,
                actual: file_len as usize,
            });
        }

        let search_window_len = (file_len.min(MAX_EOCD_SEARCH_WINDOW as u64)) as usize;
        let window_start = file_len - search_window_len as u64;

        reader.seek(SeekFrom::Start(window_start))?;
        let mut window = vec![0u8; search_window_len];
        reader.read_exact(&mut window)?;

        let candidates = find_eocd_candidate_offsets(&window);
        if candidates.is_empty() {
            return Err(ZipEngineError::EocdNotFound);
        }

        for &rel_pos in &candidates {
            let abs_eocd_offset = window_start + rel_pos as u64;
            if let Ok(info) =
                Self::validate_and_parse_reader(reader, file_len, abs_eocd_offset, rel_pos, &window)
            {
                return Ok(info);
            }
        }

        Err(ZipEngineError::EocdNotFound)
    }

    fn validate_and_parse_slice(
        bytes: &[u8],
        abs_eocd_offset: u64,
        rel_pos: usize,
        window: &[u8],
    ) -> Result<CentralDirectoryEndInfo, ZipEngineError> {
        let file_len = bytes.len() as u64;
        let mut total_entries = read_u16_le(window, rel_pos + 10) as u64;
        let mut cd_size = read_u32_le(window, rel_pos + 12) as u64;
        let mut cd_offset = read_u32_le(window, rel_pos + 16) as u64;
        let comment_len = read_u16_le(window, rel_pos + 20) as usize;

        // Invariant: EOCD + declared comment must not exceed file boundary
        let eocd_record_end = abs_eocd_offset + (EOCD_MIN_SIZE as u64) + (comment_len as u64);
        if eocd_record_end > file_len {
            return Err(ZipEngineError::InvalidCommentLength {
                declared: comment_len,
                available: (file_len.saturating_sub(abs_eocd_offset + EOCD_MIN_SIZE as u64)) as usize,
            });
        }

        let comment_start = rel_pos + EOCD_MIN_SIZE;
        let comment = window[comment_start..comment_start + comment_len].to_vec();

        // Check for Zip64 EOCD Locator (20 bytes before standard EOCD)
        if abs_eocd_offset >= 20 {
            let locator_pos = (abs_eocd_offset - 20) as usize;
            if read_u32_le(bytes, locator_pos) == MAGIC_ZIP64_LOCATOR {
                let z64_eocd_off = read_u64_le(bytes, locator_pos + 8) as usize;
                if z64_eocd_off + 56 <= abs_eocd_offset as usize
                    && read_u32_le(bytes, z64_eocd_off) == MAGIC_ZIP64_EOCD
                {
                    total_entries = read_u64_le(bytes, z64_eocd_off + 32);
                    cd_size = read_u64_le(bytes, z64_eocd_off + 40);
                    cd_offset = read_u64_le(bytes, z64_eocd_off + 48);
                }
            }
        }

        // SFX / Shebang adaptive offset calculation
        let mut archive_offset = abs_eocd_offset.saturating_sub(cd_size + cd_offset);

        if total_entries > 0 {
            let probed_cdfh_relative = (archive_offset + cd_offset) as usize;
            let probed_cdfh_absolute = cd_offset as usize;

            let relative_has_cdfh = probed_cdfh_relative + 4 <= bytes.len()
                && read_u32_le(bytes, probed_cdfh_relative) == MAGIC_CDFH;
            let absolute_has_cdfh = probed_cdfh_absolute + 4 <= bytes.len()
                && read_u32_le(bytes, probed_cdfh_absolute) == MAGIC_CDFH;

            if relative_has_cdfh {
                // Standard or SFX archive with relative CD offset
            } else if absolute_has_cdfh {
                // SFX archive where CD offset is already written as absolute file offset
                archive_offset = 0;
            } else {
                return Err(ZipEngineError::CorruptedHeader(
                    "Probed CDFH magic mismatch at central directory offset".into(),
                ));
            }
        }

        if archive_offset + cd_offset + cd_size > abs_eocd_offset {
            return Err(ZipEngineError::InvalidCentralDirectoryBoundary {
                offset: archive_offset + cd_offset,
                size: cd_size,
                file_len,
            });
        }

        Ok(CentralDirectoryEndInfo {
            eocd_offset: abs_eocd_offset,
            archive_offset,
            cd_offset,
            cd_size,
            total_entries,
            comment,
        })
    }

    fn validate_and_parse_reader<R: Read + Seek>(
        reader: &mut R,
        file_len: u64,
        abs_eocd_offset: u64,
        rel_pos: usize,
        window: &[u8],
    ) -> Result<CentralDirectoryEndInfo, ZipEngineError> {
        let mut total_entries = read_u16_le(window, rel_pos + 10) as u64;
        let mut cd_size = read_u32_le(window, rel_pos + 12) as u64;
        let mut cd_offset = read_u32_le(window, rel_pos + 16) as u64;
        let comment_len = read_u16_le(window, rel_pos + 20) as usize;

        let eocd_record_end = abs_eocd_offset + (EOCD_MIN_SIZE as u64) + (comment_len as u64);
        if eocd_record_end > file_len {
            return Err(ZipEngineError::InvalidCommentLength {
                declared: comment_len,
                available: (file_len.saturating_sub(abs_eocd_offset + EOCD_MIN_SIZE as u64)) as usize,
            });
        }

        let comment_start = rel_pos + EOCD_MIN_SIZE;
        let comment = window[comment_start..comment_start + comment_len].to_vec();

        // Check for Zip64 locator
        if abs_eocd_offset >= 20 {
            reader.seek(SeekFrom::Start(abs_eocd_offset - 20))?;
            let mut loc_buf = [0u8; 20];
            if reader.read_exact(&mut loc_buf).is_ok() && read_u32_le(&loc_buf, 0) == MAGIC_ZIP64_LOCATOR {
                let z64_eocd_off = read_u64_le(&loc_buf, 8);
                if z64_eocd_off + 56 <= abs_eocd_offset {
                    reader.seek(SeekFrom::Start(z64_eocd_off))?;
                    let mut z64_buf = [0u8; 56];
                    if reader.read_exact(&mut z64_buf).is_ok()
                        && read_u32_le(&z64_buf, 0) == MAGIC_ZIP64_EOCD
                    {
                        total_entries = read_u64_le(&z64_buf, 32);
                        cd_size = read_u64_le(&z64_buf, 40);
                        cd_offset = read_u64_le(&z64_buf, 48);
                    }
                }
            }
        }

        let mut archive_offset = abs_eocd_offset.saturating_sub(cd_size + cd_offset);

        if total_entries > 0 {
            let mut cdfh_buf = [0u8; 4];
            let mut relative_valid = false;
            let probed_relative = archive_offset + cd_offset;

            if probed_relative + 4 <= file_len {
                reader.seek(SeekFrom::Start(probed_relative))?;
                if reader.read_exact(&mut cdfh_buf).is_ok() && read_u32_le(&cdfh_buf, 0) == MAGIC_CDFH {
                    relative_valid = true;
                }
            }

            if !relative_valid {
                let mut absolute_valid = false;
                if cd_offset + 4 <= file_len {
                    reader.seek(SeekFrom::Start(cd_offset))?;
                    if reader.read_exact(&mut cdfh_buf).is_ok() && read_u32_le(&cdfh_buf, 0) == MAGIC_CDFH {
                        absolute_valid = true;
                    }
                }

                if absolute_valid {
                    archive_offset = 0;
                } else {
                    return Err(ZipEngineError::CorruptedHeader(
                        "Probed CDFH magic mismatch at central directory offset".into(),
                    ));
                }
            }
        }

        if archive_offset + cd_offset + cd_size > abs_eocd_offset {
            return Err(ZipEngineError::InvalidCentralDirectoryBoundary {
                offset: archive_offset + cd_offset,
                size: cd_size,
                file_len,
            });
        }

        Ok(CentralDirectoryEndInfo {
            eocd_offset: abs_eocd_offset,
            archive_offset,
            cd_offset,
            cd_size,
            total_entries,
            comment,
        })
    }
}
