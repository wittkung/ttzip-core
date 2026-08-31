// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Format detection and format resolution submodule for Unified Orchestrator.

use std::path::Path;
use crate::archive::unified::format_sniffer::{FormatSniffer, SniffResult};
use crate::standards::signatures::{CompoundFormat, DetectedFormat};
use crate::standards::sniffer::detect_format_file;
use crate::types::{TTZipArchiveFormat, TTZipStatus};

/// Sniffs detailed 50+ archive format with 3-state evaluation.
pub fn detect_archive_sniff(path: &Path) -> Result<SniffResult, TTZipStatus> {
    FormatSniffer::sniff_file(path).map_err(|_| TTZipStatus::ErrFileNotFound)
}

/// Detects format of an archive from file headers with extension fallback.
pub fn detect_format(path: &Path) -> Result<(DetectedFormat, Option<CompoundFormat>), TTZipStatus> {
    let sniff = detect_format_file(path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    if sniff.format != DetectedFormat::Unknown {
        return Ok((sniff.format, sniff.compound_format));
    }

    // Secondary 50+ FormatSniffer lookup
    if let Ok(SniffResult::Yes { format, .. }) = FormatSniffer::sniff_file(path) {
        let detected = format.to_detected_format();
        if detected != DetectedFormat::Unknown {
            return Ok((detected, None));
        }
    }

    // Extension fallback
    let ext_str = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let lower = ext_str.to_lowercase();
    if lower.ends_with(".zip")
        || lower.ends_with(".cbz")
        || lower.ends_with(".jar")
        || lower.ends_with(".apk")
        || lower.ends_with(".epub")
    {
        Ok((DetectedFormat::Zip, None))
    } else if lower.ends_with(".7z") || lower.ends_with(".cb7") {
        Ok((DetectedFormat::SevenZip, None))
    } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        Ok((DetectedFormat::Gzip, Some(CompoundFormat::TarGz)))
    } else if lower.ends_with(".tar.bz2") || lower.ends_with(".tbz2") || lower.ends_with(".tbz") {
        Ok((DetectedFormat::Bzip2, Some(CompoundFormat::TarBz2)))
    } else if lower.ends_with(".tar.xz") || lower.ends_with(".txz") {
        Ok((DetectedFormat::Xz, Some(CompoundFormat::TarXz)))
    } else if lower.ends_with(".tar.zst") || lower.ends_with(".tzst") {
        Ok((DetectedFormat::Zstd, Some(CompoundFormat::TarZstd)))
    } else if lower.ends_with(".tar.lz4") || lower.ends_with(".tlz4") {
        Ok((DetectedFormat::Lz4, Some(CompoundFormat::TarLz4)))
    } else if lower.ends_with(".tar.br") || lower.ends_with(".tbr") {
        Ok((DetectedFormat::Brotli, Some(CompoundFormat::TarBrotli)))
    } else if lower.ends_with(".tar.lz") || lower.ends_with(".tlz") {
        Ok((DetectedFormat::Lzip, Some(CompoundFormat::TarLzip)))
    } else if lower.ends_with(".tar.lrz") || lower.ends_with(".tlrz") {
        Ok((DetectedFormat::Lrzip, Some(CompoundFormat::TarLrzip)))
    } else if lower.ends_with(".tar") || lower.ends_with(".cbt") {
        Ok((DetectedFormat::Tar, None))
    } else if lower.ends_with(".rar") || lower.ends_with(".cbr") {
        Ok((DetectedFormat::Rar, None))
    } else if lower.ends_with(".cab") {
        Ok((DetectedFormat::Cab, None))
    } else if lower.ends_with(".iso") || lower.ends_with(".img") {
        Ok((DetectedFormat::Iso, None))
    } else if lower.ends_with(".wim") || lower.ends_with(".swm") || lower.ends_with(".esd") {
        Ok((DetectedFormat::Wim, None))
    } else if lower.ends_with(".dmg") {
        Ok((DetectedFormat::Dmg, None))
    } else if lower.ends_with(".xar") || lower.ends_with(".pkg") {
        Ok((DetectedFormat::Xar, None))
    } else if lower.ends_with(".cpio") {
        Ok((DetectedFormat::Cpio, None))
    } else if lower.ends_with(".ar") || lower.ends_with(".a") {
        Ok((DetectedFormat::Ar, None))
    } else if lower.ends_with(".deb") {
        Ok((DetectedFormat::Deb, None))
    } else if lower.ends_with(".rpm") {
        Ok((DetectedFormat::Rpm, None))
    } else if lower.ends_with(".squashfs") || lower.ends_with(".sqsh") {
        Ok((DetectedFormat::Squashfs, None))
    } else if lower.ends_with(".aar") || lower.ends_with(".aea") {
        Ok((DetectedFormat::Aar, None))
    } else if lower.ends_with(".lzh") || lower.ends_with(".lha") {
        Ok((DetectedFormat::Lzh, None))
    } else if lower.ends_with(".lzfse") {
        Ok((DetectedFormat::Lzfse, None))
    } else if lower.ends_with(".sz") || lower.ends_with(".snappy") {
        Ok((DetectedFormat::Snappy, None))
    } else if lower.ends_with(".lz") || lower.ends_with(".lzip") {
        Ok((DetectedFormat::Lzip, None))
    } else if lower.ends_with(".lrz") || lower.ends_with(".lrzip") {
        Ok((DetectedFormat::Lrzip, None))
    } else if lower.ends_with(".lz4") {
        Ok((DetectedFormat::Lz4, None))
    } else if lower.ends_with(".br") {
        Ok((DetectedFormat::Brotli, None))
    } else if lower.ends_with(".gz") {
        Ok((DetectedFormat::Gzip, None))
    } else if lower.ends_with(".bz2") {
        Ok((DetectedFormat::Bzip2, None))
    } else if lower.ends_with(".xz") {
        Ok((DetectedFormat::Xz, None))
    } else if lower.ends_with(".zst") {
        Ok((DetectedFormat::Zstd, None))
    } else {
        Ok((DetectedFormat::Unknown, None))
    }
}

/// Resolves target archive creation format from explicit format and destination path.
pub fn resolve_create_format(format: TTZipArchiveFormat, dest_path: &Path) -> TTZipArchiveFormat {
    if format != TTZipArchiveFormat::Auto {
        return format;
    }

    let name = dest_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    // 1. Compound Tar formats
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        TTZipArchiveFormat::TarGz
    } else if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") || name.ends_with(".tbz") {
        TTZipArchiveFormat::TarBz2
    } else if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        TTZipArchiveFormat::TarXz
    } else if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
        TTZipArchiveFormat::TarZstd
    } else if name.ends_with(".tar.lz4") || name.ends_with(".tlz4") {
        TTZipArchiveFormat::TarLz4
    } else if name.ends_with(".tar.br") || name.ends_with(".tbr") {
        TTZipArchiveFormat::TarBrotli
    } else if name.ends_with(".tar.lz") || name.ends_with(".tlz") {
        TTZipArchiveFormat::TarLzip
    } else if name.ends_with(".tar.lrz") || name.ends_with(".tlrz") {
        TTZipArchiveFormat::TarLrzip
    // 2. Primary container formats
    } else if name.ends_with(".7z") || name.ends_with(".cb7") {
        TTZipArchiveFormat::SevenZip
    } else if name.ends_with(".tar") || name.ends_with(".cbt") {
        TTZipArchiveFormat::Tar
    } else if name.ends_with(".iso") || name.ends_with(".img") {
        TTZipArchiveFormat::Iso
    } else if name.ends_with(".cab") {
        TTZipArchiveFormat::Cab
    } else if name.ends_with(".wim") || name.ends_with(".swm") || name.ends_with(".esd") {
        TTZipArchiveFormat::Wim
    } else if name.ends_with(".cpio") {
        TTZipArchiveFormat::Cpio
    } else if name.ends_with(".deb") {
        TTZipArchiveFormat::Deb
    } else if name.ends_with(".ar") || name.ends_with(".a") {
        TTZipArchiveFormat::Ar
    } else if name.ends_with(".rpm") {
        TTZipArchiveFormat::Rpm
    } else if name.ends_with(".squashfs") || name.ends_with(".sqsh") {
        TTZipArchiveFormat::Squashfs
    } else if name.ends_with(".aar") || name.ends_with(".aea") {
        TTZipArchiveFormat::Aar
    } else if name.ends_with(".xar") || name.ends_with(".pkg") {
        TTZipArchiveFormat::Xar
    } else if name.ends_with(".dmg") {
        TTZipArchiveFormat::Dmg
    } else if name.ends_with(".rar") || name.ends_with(".cbr") {
        TTZipArchiveFormat::Rar
    } else if name.ends_with(".lzh") || name.ends_with(".lha") {
        TTZipArchiveFormat::Lzh
    // 3. Single-stream compression formats
    } else if name.ends_with(".gz") {
        TTZipArchiveFormat::Gzip
    } else if name.ends_with(".bz2") {
        TTZipArchiveFormat::Bzip2
    } else if name.ends_with(".xz") {
        TTZipArchiveFormat::Xz
    } else if name.ends_with(".zst") {
        TTZipArchiveFormat::Zstd
    } else if name.ends_with(".lz4") {
        TTZipArchiveFormat::Lz4
    } else if name.ends_with(".br") {
        TTZipArchiveFormat::Brotli
    } else if name.ends_with(".lzfse") {
        TTZipArchiveFormat::Lzfse
    } else if name.ends_with(".sz") || name.ends_with(".snappy") {
        TTZipArchiveFormat::Snappy
    } else if name.ends_with(".lz") || name.ends_with(".lzip") {
        TTZipArchiveFormat::Lzip
    } else if name.ends_with(".lrz") || name.ends_with(".lrzip") {
        TTZipArchiveFormat::Lrzip
    // 4. Default format
    } else {
        TTZipArchiveFormat::Zip
    }
}
