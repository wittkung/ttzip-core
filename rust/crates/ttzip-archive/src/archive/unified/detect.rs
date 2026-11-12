// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Format detection and format resolution submodule for Unified Orchestrator.

use std::path::Path;
use crate::archive::unified::format_sniffer::{FormatSniffer, SniffResult};
use crate::standards::signatures::{CompoundFormat, DetectedFormat};
use crate::standards::sniffer::detect_format_file;
use crate::types::{TTZipArchiveFormat, TTZipStatus};

/// Sniffs detailed 50+ archive format with 3-state evaluation.
pub fn detect_archive_sniff(path: &Path) -> Result<SniffResult, TTZipStatus> {
    if let Some(special) = sniff_special_container_headers(path) {
        return Ok(SniffResult::Yes {
            confidence: 100,
            format: special,
        });
    }
    FormatSniffer::sniff_file(path).map_err(|_| TTZipStatus::ErrFileNotFound)
}

const DETECT_EXTENSION_TABLE: &[(&str, DetectedFormat, Option<CompoundFormat>)] = &[
    (".zip", DetectedFormat::Zip, None),
    (".cbz", DetectedFormat::Zip, None),
    (".jar", DetectedFormat::Zip, None),
    (".apk", DetectedFormat::Zip, None),
    (".epub", DetectedFormat::Zip, None),
    (".7z", DetectedFormat::SevenZip, None),
    (".cb7", DetectedFormat::SevenZip, None),
    (".tar.gz", DetectedFormat::Gzip, Some(CompoundFormat::TarGz)),
    (".tgz", DetectedFormat::Gzip, Some(CompoundFormat::TarGz)),
    (".tar.bz2", DetectedFormat::Bzip2, Some(CompoundFormat::TarBz2)),
    (".tbz2", DetectedFormat::Bzip2, Some(CompoundFormat::TarBz2)),
    (".tbz", DetectedFormat::Bzip2, Some(CompoundFormat::TarBz2)),
    (".tar.xz", DetectedFormat::Xz, Some(CompoundFormat::TarXz)),
    (".txz", DetectedFormat::Xz, Some(CompoundFormat::TarXz)),
    (".tar.zst", DetectedFormat::Zstd, Some(CompoundFormat::TarZstd)),
    (".tzst", DetectedFormat::Zstd, Some(CompoundFormat::TarZstd)),
    (".tar.lz4", DetectedFormat::Lz4, Some(CompoundFormat::TarLz4)),
    (".tlz4", DetectedFormat::Lz4, Some(CompoundFormat::TarLz4)),
    (".tar.br", DetectedFormat::Brotli, Some(CompoundFormat::TarBrotli)),
    (".tbr", DetectedFormat::Brotli, Some(CompoundFormat::TarBrotli)),
    (".tar.lz", DetectedFormat::Lzip, Some(CompoundFormat::TarLzip)),
    (".tlz", DetectedFormat::Lzip, Some(CompoundFormat::TarLzip)),
    (".tar.lrz", DetectedFormat::Lrzip, Some(CompoundFormat::TarLrzip)),
    (".tlrz", DetectedFormat::Lrzip, Some(CompoundFormat::TarLrzip)),
    (".tar", DetectedFormat::Tar, None),
    (".cbt", DetectedFormat::Tar, None),
    (".rar", DetectedFormat::Rar, None),
    (".cbr", DetectedFormat::Rar, None),
    (".cab", DetectedFormat::Cab, None),
    (".iso", DetectedFormat::Iso, None),
    (".img", DetectedFormat::Iso, None),
    (".wim", DetectedFormat::Wim, None),
    (".swm", DetectedFormat::Wim, None),
    (".esd", DetectedFormat::Wim, None),
    (".dmg", DetectedFormat::Dmg, None),
    (".xar", DetectedFormat::Xar, None),
    (".pkg", DetectedFormat::Xar, None),
    (".cpio", DetectedFormat::Cpio, None),
    (".ar", DetectedFormat::Ar, None),
    (".a", DetectedFormat::Ar, None),
    (".deb", DetectedFormat::Deb, None),
    (".rpm", DetectedFormat::Rpm, None),
    (".squashfs", DetectedFormat::Squashfs, None),
    (".sqsh", DetectedFormat::Squashfs, None),
    (".aar", DetectedFormat::Aar, None),
    (".aea", DetectedFormat::Aar, None),
    (".lzh", DetectedFormat::Lzh, None),
    (".lha", DetectedFormat::Lzh, None),
    (".lzfse", DetectedFormat::Lzfse, None),
    (".sz", DetectedFormat::Snappy, None),
    (".snappy", DetectedFormat::Snappy, None),
    (".lz", DetectedFormat::Lzip, None),
    (".lzip", DetectedFormat::Lzip, None),
    (".lrz", DetectedFormat::Lrzip, None),
    (".lrzip", DetectedFormat::Lrzip, None),
    (".lz4", DetectedFormat::Lz4, None),
    (".br", DetectedFormat::Brotli, None),
    (".gz", DetectedFormat::Gzip, None),
    (".bz2", DetectedFormat::Bzip2, None),
    (".xz", DetectedFormat::Xz, None),
    (".zst", DetectedFormat::Zstd, None),
];

const CREATE_FORMAT_EXTENSION_TABLE: &[(&str, TTZipArchiveFormat)] = &[
    (".tar.gz", TTZipArchiveFormat::TarGz),
    (".tgz", TTZipArchiveFormat::TarGz),
    (".tar.bz2", TTZipArchiveFormat::TarBz2),
    (".tbz2", TTZipArchiveFormat::TarBz2),
    (".tbz", TTZipArchiveFormat::TarBz2),
    (".tar.xz", TTZipArchiveFormat::TarXz),
    (".txz", TTZipArchiveFormat::TarXz),
    (".tar.zst", TTZipArchiveFormat::TarZstd),
    (".tzst", TTZipArchiveFormat::TarZstd),
    (".tar.lz4", TTZipArchiveFormat::TarLz4),
    (".tlz4", TTZipArchiveFormat::TarLz4),
    (".tar.br", TTZipArchiveFormat::TarBrotli),
    (".tbr", TTZipArchiveFormat::TarBrotli),
    (".tar.lz", TTZipArchiveFormat::TarLzip),
    (".tlz", TTZipArchiveFormat::TarLzip),
    (".tar.lrz", TTZipArchiveFormat::TarLrzip),
    (".tlrz", TTZipArchiveFormat::TarLrzip),
    (".7z", TTZipArchiveFormat::SevenZip),
    (".cb7", TTZipArchiveFormat::SevenZip),
    (".tar", TTZipArchiveFormat::Tar),
    (".cbt", TTZipArchiveFormat::Tar),
    (".iso", TTZipArchiveFormat::Iso),
    (".img", TTZipArchiveFormat::Iso),
    (".cab", TTZipArchiveFormat::Cab),
    (".wim", TTZipArchiveFormat::Wim),
    (".swm", TTZipArchiveFormat::Wim),
    (".esd", TTZipArchiveFormat::Wim),
    (".cpio", TTZipArchiveFormat::Cpio),
    (".deb", TTZipArchiveFormat::Deb),
    (".ar", TTZipArchiveFormat::Ar),
    (".a", TTZipArchiveFormat::Ar),
    (".rpm", TTZipArchiveFormat::Rpm),
    (".squashfs", TTZipArchiveFormat::Squashfs),
    (".sqsh", TTZipArchiveFormat::Squashfs),
    (".aar", TTZipArchiveFormat::Aar),
    (".aea", TTZipArchiveFormat::Aar),
    (".xar", TTZipArchiveFormat::Xar),
    (".pkg", TTZipArchiveFormat::Xar),
    (".dmg", TTZipArchiveFormat::Dmg),
    (".rar", TTZipArchiveFormat::Rar),
    (".cbr", TTZipArchiveFormat::Rar),
    (".lzh", TTZipArchiveFormat::Lzh),
    (".lha", TTZipArchiveFormat::Lzh),
    (".gz", TTZipArchiveFormat::Gzip),
    (".bz2", TTZipArchiveFormat::Bzip2),
    (".xz", TTZipArchiveFormat::Xz),
    (".zst", TTZipArchiveFormat::Zstd),
    (".lz4", TTZipArchiveFormat::Lz4),
    (".br", TTZipArchiveFormat::Brotli),
    (".lzfse", TTZipArchiveFormat::Lzfse),
    (".sz", TTZipArchiveFormat::Snappy),
    (".snappy", TTZipArchiveFormat::Snappy),
    (".lz", TTZipArchiveFormat::Lzip),
    (".lzip", TTZipArchiveFormat::Lzip),
    (".lrz", TTZipArchiveFormat::Lrzip),
    (".lrzip", TTZipArchiveFormat::Lrzip),
];

/// Detects format of an archive from file headers with extension fallback.
pub fn detect_format(path: &Path) -> Result<(DetectedFormat, Option<CompoundFormat>), TTZipStatus> {
    if let Some(special) = sniff_special_container_headers(path) {
        return Ok((special, None));
    }

    let sniff = detect_format_file(path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    if sniff.format != DetectedFormat::Unknown {
        return Ok((sniff.format, sniff.compound_format));
    }

    if let Ok(SniffResult::Yes { format, .. }) = FormatSniffer::sniff_file(path) {
        let detected = format.to_detected_format();
        if detected != DetectedFormat::Unknown {
            return Ok((detected, None));
        }
    }

    let ext_str = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let lower_raw = ext_str.to_lowercase();
    let lower = strip_split_suffix_for_detect(&lower_raw);

    let matched = DETECT_EXTENSION_TABLE
        .iter()
        .find_map(|&(ext, det, comp)| lower.ends_with(ext).then_some((det, comp)))
        .unwrap_or((DetectedFormat::Unknown, None));
    Ok(matched)
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

    CREATE_FORMAT_EXTENSION_TABLE
        .iter()
        .find_map(|&(ext, fmt)| name.ends_with(ext).then_some(fmt))
        .unwrap_or(TTZipArchiveFormat::Zip)
}

fn strip_split_suffix_for_detect(name_lower: &str) -> &str {
    if let Some((stem, ext)) = name_lower.rsplit_once('.') {
        let is_num = ext.len() >= 3 && ext.bytes().all(|b| b.is_ascii_digit());
        let is_z = ext.starts_with('z')
            && ext.len() >= 3
            && ext.as_bytes()[1..].iter().all(|b| b.is_ascii_digit());
        if is_num || is_z {
            return stem;
        }
    }
    name_lower
}

fn sniff_special_container_headers(path: &Path) -> Option<DetectedFormat> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 8];
    let n = f.read(&mut head).ok()?;
    if n >= 4
        && (&head[..4] == b"pbze"
            || &head[..4] == b"AA01"
            || &head[..4] == b"YAA1"
            || &head[..4] == b"AEA1")
    {
        return Some(DetectedFormat::Aar);
    }
    let file_len = f.metadata().ok()?.len();
    if file_len >= 512 && f.seek(SeekFrom::End(-512)).is_ok() {
        let mut trailer = [0u8; 16];
        if f.read_exact(&mut trailer).is_ok() && &trailer[..4] == b"koly" {
            let version = u32::from_be_bytes([trailer[4], trailer[5], trailer[6], trailer[7]]);
            let header_size =
                u32::from_be_bytes([trailer[8], trailer[9], trailer[10], trailer[11]]);
            if version == 4 && header_size == 512 {
                return Some(DetectedFormat::Dmg);
            }
        }
    }
    None
}

