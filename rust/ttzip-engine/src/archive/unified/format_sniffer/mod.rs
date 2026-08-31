// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-allocation 3-state format sniffer matrix covering 50+ archive,
//! disk image, virtual container, and compression formats.
//!
//! Supports:
//! 1. Three-state evaluation: `No`, `Yes { confidence, format }`, `NeedMore { required_bytes }`.
//! 2. Fixed offset matching and superblock inspection (e.g. APFS, HFS+, EXT2/3/4, FAT, NTFS).
//! 3. Sliding window signature matching (`kFindSignature`) for SFX / PE wrapped archives.
//! 4. Backward trailer scanning (`kBackwardOpen`) for DMG (`koly`), VHD, and ZIP EOCD.

pub mod bidders;
pub mod formats;
pub mod registry;
pub mod rules;

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

pub use bidders::{ArchiveFormatBidder, BidScore, SeekableStream};
pub use formats::ArchiveFormat;
pub use registry::{BidResult, FormatBidderRegistry};
pub use rules::{SniffAnchor, SniffRule, SNIFF_RULES};

/// Three-state outcome of archive format sniffing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SniffResult {
    /// Provided buffer does not match this format.
    No,
    /// Format confidently identified with confidence level [0..100].
    Yes {
        confidence: u8,
        format: ArchiveFormat,
    },
    /// Buffer is too small or truncated; need more bytes to determine format.
    NeedMore {
        required_bytes: usize,
    },
}

impl SniffResult {
    /// Returns true if this is a positive match.
    #[inline]
    #[must_use]
    pub const fn is_yes(self) -> bool {
        matches!(self, Self::Yes { .. })
    }

    /// Returns true if format does not match.
    #[inline]
    #[must_use]
    pub const fn is_no(self) -> bool {
        matches!(self, Self::No)
    }

    /// Returns true if more bytes are required.
    #[inline]
    #[must_use]
    pub const fn is_need_more(self) -> bool {
        matches!(self, Self::NeedMore { .. })
    }

    /// Returns matched format if `Yes`, otherwise `ArchiveFormat::Unknown`.
    #[inline]
    #[must_use]
    pub const fn format(self) -> ArchiveFormat {
        match self {
            Self::Yes { format, .. } => format,
            _ => ArchiveFormat::Unknown,
        }
    }
}

/// Zero-allocation 3-state format sniffer.
pub struct FormatSniffer;

impl FormatSniffer {
    /// Evaluates the input buffer against 50+ archive & disk image signatures.
    ///
    /// Returns `SniffResult::Yes` on positive match, `SniffResult::NeedMore` if
    /// buffer is too short, or `SniffResult::No` if non-matching.
    pub fn sniff(buffer: &[u8]) -> SniffResult {
        Self::sniff_with_hint(buffer, None)
    }

    /// Evaluates buffer with optional filename extension hint.
    pub fn sniff_with_hint(buffer: &[u8], filename_hint: Option<&str>) -> SniffResult {
        if buffer.is_empty() {
            return SniffResult::NeedMore { required_bytes: 4 };
        }

        let mut max_needed = 0;

        // 1. Direct Rule Evaluation across Master Matrix
        for rule in SNIFF_RULES {
            match Self::evaluate_rule(rule, buffer) {
                SniffResult::Yes { confidence, format } => {
                    // Refine format with compound hints or sub-types if appropriate
                    let final_format = Self::refine_format(format, buffer, filename_hint);
                    return SniffResult::Yes {
                        confidence,
                        format: final_format,
                    };
                }
                SniffResult::NeedMore { required_bytes } => {
                    if required_bytes > max_needed {
                        max_needed = required_bytes;
                    }
                }
                SniffResult::No => {}
            }
        }

        // 2. Specialized Heuristic Probes
        if let Some(res) = Self::probe_fat_filesystem(buffer) {
            return res;
        }
        if let Some(res) = Self::probe_nsis_installer(buffer) {
            return res;
        }
        if let Some(res) = Self::probe_brotli_stream(buffer) {
            return res;
        }
        if let Some(res) = Self::probe_vmdk_text_descriptor(buffer) {
            return res;
        }
        if let Some(res) = Self::probe_sfx_embedded_archive(buffer) {
            return res;
        }
        if let Some(res) = Self::probe_trailing_zip(buffer) {
            return res;
        }

        // 3. If any signature could potentially match if more bytes arrive
        if buffer.len() < 16 {
            return SniffResult::NeedMore {
                required_bytes: if max_needed > 0 { max_needed } else { 16 },
            };
        }

        SniffResult::No
    }

    /// Evaluates format from a physical file stream.
    pub fn sniff_file<P: AsRef<Path>>(path: P) -> io::Result<SniffResult> {
        let path_ref = path.as_ref();
        let filename_hint = path_ref.file_name().and_then(|n| n.to_str());

        let mut file = File::open(path_ref)?;
        let file_len = file.metadata()?.len() as usize;

        if file_len == 0 {
            return Ok(SniffResult::No);
        }

        // Read up to 64 KB prefix
        let prefix_len = file_len.min(65536);
        let mut prefix_buf = vec![0u8; prefix_len];
        file.read_exact(&mut prefix_buf)?;

        // Fast head sniffing
        let head_res = Self::sniff_with_hint(&prefix_buf, filename_hint);
        if head_res.is_yes() {
            return Ok(head_res);
        }

        // If file is large enough, read tail 1024 bytes for DMG/VHD/ZIP EOCD
        if file_len >= 512 {
            let tail_len = file_len.min(1024);
            let mut tail_buf = vec![0u8; tail_len];
            file.seek(SeekFrom::End(-(tail_len as i64)))?;
            file.read_exact(&mut tail_buf)?;

            // Tail check DMG 'koly'
            if tail_len >= 512 && &tail_buf[tail_len - 512..tail_len - 508] == b"koly" {
                return Ok(SniffResult::Yes {
                    confidence: 100,
                    format: ArchiveFormat::Dmg,
                });
            }

            // Tail check VHD 'conectix'
            if tail_len >= 512 && &tail_buf[tail_len - 512..tail_len - 504] == b"conectix" {
                return Ok(SniffResult::Yes {
                    confidence: 95,
                    format: ArchiveFormat::Vhd,
                });
            }

            // Tail check ZIP EOCD (PK\x05\x06)
            if let Some(zip_res) = Self::probe_trailing_zip(&tail_buf) {
                if zip_res.is_yes() {
                    return Ok(zip_res);
                }
            }
        }

        Ok(head_res)
    }

    /// Evaluates a single static sniff rule against `buffer`.
    #[inline]
    fn evaluate_rule(rule: &SniffRule, buffer: &[u8]) -> SniffResult {
        let magic_len = rule.magic.len();

        let offset = match rule.anchor {
            SniffAnchor::Head(off) => off,
            SniffAnchor::Sector(sec) => sec * 2048,
            SniffAnchor::TarOffset(off) => off,
            SniffAnchor::Tail(tail_dist) => {
                if buffer.len() < tail_dist {
                    return SniffResult::NeedMore {
                        required_bytes: tail_dist,
                    };
                }
                buffer.len() - tail_dist
            }
        };

        let target_end = offset + magic_len;

        if buffer.len() < target_end || buffer.len() < rule.min_total_size {
            // Check if buffer is a partial prefix of the required magic
            if offset < buffer.len() {
                let available_magic = &buffer[offset..];
                if rule.magic.starts_with(available_magic) {
                    return SniffResult::NeedMore {
                        required_bytes: rule.min_total_size.max(target_end),
                    };
                }
            }
            if buffer.len() < target_end {
                return SniffResult::No;
            }
        }

        if let Some(slice) = buffer.get(offset..target_end) {
            if slice == rule.magic {
                return SniffResult::Yes {
                    confidence: rule.confidence,
                    format: rule.format,
                };
            }
        }

        SniffResult::No
    }

    /// Refines ArchiveFormat using file name hints or secondary headers (e.g. Tar.Gz, Zip64).
    fn refine_format(format: ArchiveFormat, buffer: &[u8], filename_hint: Option<&str>) -> ArchiveFormat {
        match format {
            ArchiveFormat::Zip => {
                // Check if Zip64 Locator exists
                if buffer.windows(4).any(|w| w == [0x50, 0x4B, 0x06, 0x06] || w == [0x50, 0x4B, 0x06, 0x07]) {
                    return ArchiveFormat::Zip64;
                }
                ArchiveFormat::Zip
            }
            ArchiveFormat::Ar => {
                // Check if Debian Package (contains "debian-binary")
                if buffer.len() >= 24 && &buffer[8..21] == b"debian-binary" {
                    return ArchiveFormat::Deb;
                }
                ArchiveFormat::Ar
            }
            ArchiveFormat::PeExe => {
                // Scan sliding window (kFindSignature) for embedded archive
                if let Some(embedded) = Self::scan_sfx_window(buffer) {
                    return embedded;
                }
                ArchiveFormat::PeExe
            }
            _ => {
                if let Some(hint) = filename_hint {
                    let lower = hint.to_lowercase();
                    if (format == ArchiveFormat::Iso || format == ArchiveFormat::Udf) && lower.ends_with(".udf") {
                        return ArchiveFormat::Udf;
                    }
                }
                format
            }
        }
    }

    /// Probe for FAT12/16/32 and exFAT boot sectors.
    fn probe_fat_filesystem(buffer: &[u8]) -> Option<SniffResult> {
        if buffer.len() < 512 {
            if buffer.len() >= 3 && &buffer[0..3] == b"\xEB\x3C\x90" {
                return Some(SniffResult::NeedMore { required_bytes: 512 });
            }
            return None;
        }

        // FAT boot signature at offset 510-511 must be 0x55, 0xAA
        if buffer[510] == 0x55 && buffer[511] == 0xAA {
            if buffer.len() >= 62 && (&buffer[54..62] == b"FAT12   " || &buffer[54..62] == b"FAT16   ") {
                return Some(SniffResult::Yes {
                    confidence: 95,
                    format: ArchiveFormat::Fat,
                });
            }
            if buffer.len() >= 90 && &buffer[82..90] == b"FAT32   " {
                return Some(SniffResult::Yes {
                    confidence: 95,
                    format: ArchiveFormat::Fat,
                });
            }
        }
        None
    }

    /// Probe for NSIS Nullsoft Installer signature (`\xEF\xBE\xAD\xDE\x4E\x75\x6C\x6C\x73\x6F\x66\x74\x49\x6E\x73\x74`).
    fn probe_nsis_installer(buffer: &[u8]) -> Option<SniffResult> {
        const NSIS_SIG: &[u8] = b"\xEF\xBE\xAD\xDENullsoftInst";
        if buffer.len() < NSIS_SIG.len() {
            return None;
        }

        // Scan sliding window (kFindSignature) up to 64KB
        let scan_limit = buffer.len().min(65536);
        if buffer[..scan_limit].windows(NSIS_SIG.len()).any(|w| w == NSIS_SIG) {
            return Some(SniffResult::Yes {
                confidence: 95,
                format: ArchiveFormat::Nsis,
            });
        }
        None
    }

    /// Probe for VMDK ASCII text descriptor.
    fn probe_vmdk_text_descriptor(buffer: &[u8]) -> Option<SniffResult> {
        if buffer.starts_with(b"# Disk DescriptorFile") || buffer.starts_with(b"# Extent descriptor") {
            return Some(SniffResult::Yes {
                confidence: 95,
                format: ArchiveFormat::Vmdk,
            });
        }
        None
    }

    /// Probe for Brotli compressed stream heuristic.
    fn probe_brotli_stream(buffer: &[u8]) -> Option<SniffResult> {
        if buffer.starts_with(b"\xCE\xB2\xCF\x81") {
            return Some(SniffResult::Yes {
                confidence: 90,
                format: ArchiveFormat::Brotli,
            });
        }
        None
    }

    /// Scanning backwards for ZIP End of Central Directory (`PK\x05\x06`).
    fn probe_trailing_zip(buffer: &[u8]) -> Option<SniffResult> {
        if buffer.len() < 22 {
            return None;
        }

        // Scan backwards within trailing 65KB (standard ZIP comment limit)
        let search_start = buffer.len().saturating_sub(65557);
        let search_slice = &buffer[search_start..];

        for i in (0..=search_slice.len().saturating_sub(22)).rev() {
            if search_slice[i..i + 4] == [0x50, 0x4B, 0x05, 0x06] {
                return Some(SniffResult::Yes {
                    confidence: 90,
                    format: ArchiveFormat::Zip,
                });
            }
        }
        None
    }

    /// Probe for embedded SFX archives in Windows PE headers.
    fn probe_sfx_embedded_archive(buffer: &[u8]) -> Option<SniffResult> {
        if buffer.starts_with(b"MZ") && buffer.len() > 64 {
            if let Some(fmt) = Self::scan_sfx_window(buffer) {
                return Some(SniffResult::Yes {
                    confidence: 85,
                    format: fmt,
                });
            }
        }
        None
    }

    /// Sliding window signature scanner (kFindSignature) for SFX / PE headers.
    fn scan_sfx_window(buffer: &[u8]) -> Option<ArchiveFormat> {
        let scan_limit = buffer.len().min(65536);
        let slice = &buffer[..scan_limit];

        // Search for 7z SFX
        if slice.windows(6).any(|w| w == [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
            return Some(ArchiveFormat::SevenZip);
        }

        // Search for ZIP SFX
        if slice.windows(4).any(|w| w == [0x50, 0x4B, 0x03, 0x04]) {
            return Some(ArchiveFormat::Zip);
        }

        // Search for RAR SFX
        if slice.windows(7).any(|w| w == [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00]) {
            return Some(ArchiveFormat::Rar4);
        }
        if slice.windows(8).any(|w| w == [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00]) {
            return Some(ArchiveFormat::Rar5);
        }

        None
    }

    /// Probe sliding window scanning for embedded signatures across raw slice.
    pub fn sniff_sliding_window(buffer: &[u8], max_window: usize) -> SniffResult {
        let limit = buffer.len().min(max_window);
        let slice = &buffer[..limit];

        for rule in SNIFF_RULES {
            if slice.windows(rule.magic.len()).any(|w| w == rule.magic) {
                return SniffResult::Yes {
                    confidence: rule.confidence.saturating_sub(10), // slight penalty for offset
                    format: rule.format,
                };
            }
        }

        SniffResult::No
    }
}
