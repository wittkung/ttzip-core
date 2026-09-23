// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Detected container and compression formats identified via magic signatures.

use serde::{Deserialize, Serialize};
use crate::formats::TTZipArchiveFormat;

/// Supported archive and compression formats identified via magic signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum DetectedFormat {
    Unknown = 0,
    Zip = 1,
    SevenZip = 2,
    Tar = 3,
    Gzip = 4,
    Bzip2 = 5,
    Xz = 6,
    Zstd = 7,
    Rar = 8,
    Cab = 9,
    Iso = 10,
    Dmg = 11,
    Xar = 12,
    Lzh = 13,
    Ar = 14,
    Lzfse = 15,
    Snappy = 16,
    Lz4 = 17,
    Lzip = 18,
    Lrzip = 19,
    Brotli = 20,
    Aar = 21,
    Wim = 22,
    Cpio = 23,
    Deb = 24,
    Rpm = 25,
    Squashfs = 26,
}

impl DetectedFormat {
    /// Maps detected format to TTZip core archive format enum.
    #[must_use]
    pub const fn to_ttzip_format(self) -> TTZipArchiveFormat {
        match self {
            Self::Zip => TTZipArchiveFormat::Zip,
            Self::SevenZip => TTZipArchiveFormat::SevenZip,
            Self::Tar => TTZipArchiveFormat::Tar,
            Self::Gzip => TTZipArchiveFormat::Gzip,
            Self::Bzip2 => TTZipArchiveFormat::Bzip2,
            Self::Xz => TTZipArchiveFormat::Xz,
            Self::Zstd => TTZipArchiveFormat::Zstd,
            Self::Rar => TTZipArchiveFormat::Rar,
            Self::Cab => TTZipArchiveFormat::Cab,
            Self::Iso => TTZipArchiveFormat::Iso,
            Self::Dmg => TTZipArchiveFormat::Dmg,
            Self::Xar => TTZipArchiveFormat::Xar,
            Self::Lzh => TTZipArchiveFormat::Lzh,
            Self::Ar => TTZipArchiveFormat::Ar,
            Self::Lzfse => TTZipArchiveFormat::Lzfse,
            Self::Snappy => TTZipArchiveFormat::Snappy,
            Self::Lz4 => TTZipArchiveFormat::Lz4,
            Self::Lzip => TTZipArchiveFormat::Lzip,
            Self::Lrzip => TTZipArchiveFormat::Lrzip,
            Self::Brotli => TTZipArchiveFormat::Brotli,
            Self::Aar => TTZipArchiveFormat::Aar,
            Self::Wim => TTZipArchiveFormat::Wim,
            Self::Cpio => TTZipArchiveFormat::Cpio,
            Self::Deb => TTZipArchiveFormat::Deb,
            Self::Rpm => TTZipArchiveFormat::Rpm,
            Self::Squashfs => TTZipArchiveFormat::Squashfs,
            Self::Unknown => TTZipArchiveFormat::Unknown,
        }
    }

    /// Primary standard file extension for this format.
    #[must_use]
    pub const fn primary_extension(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::SevenZip => "7z",
            Self::Tar => "tar",
            Self::Gzip => "gz",
            Self::Bzip2 => "bz2",
            Self::Xz => "xz",
            Self::Zstd => "zst",
            Self::Rar => "rar",
            Self::Cab => "cab",
            Self::Iso => "iso",
            Self::Dmg => "dmg",
            Self::Xar => "xar",
            Self::Lzh => "lzh",
            Self::Ar => "ar",
            Self::Lzfse => "lzfse",
            Self::Snappy => "sz",
            Self::Lz4 => "lz4",
            Self::Lzip => "lz",
            Self::Lrzip => "lrz",
            Self::Brotli => "br",
            Self::Aar => "aar",
            Self::Wim => "wim",
            Self::Cpio => "cpio",
            Self::Deb => "deb",
            Self::Rpm => "rpm",
            Self::Squashfs => "squashfs",
            Self::Unknown => "bin",
        }
    }

    /// Primary MIME type identifier.
    #[must_use]
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Zip => "application/zip",
            Self::SevenZip => "application/x-7z-compressed",
            Self::Tar => "application/x-tar",
            Self::Gzip => "application/gzip",
            Self::Bzip2 => "application/x-bzip2",
            Self::Xz => "application/x-xz",
            Self::Zstd => "application/zstd",
            Self::Rar => "application/vnd.rar",
            Self::Cab => "application/vnd.ms-cab-compressed",
            Self::Iso => "application/x-iso9660-image",
            Self::Dmg => "application/x-apple-diskimage",
            Self::Xar => "application/x-xar",
            Self::Lzh => "application/x-lzh-compressed",
            Self::Ar => "application/x-archive",
            Self::Lzfse => "application/x-lzfse",
            Self::Snappy => "application/x-snappy-framed",
            Self::Lz4 => "application/x-lz4",
            Self::Lzip => "application/x-lzip",
            Self::Lrzip => "application/x-lrzip",
            Self::Brotli => "application/x-brotli",
            Self::Aar => "application/x-apple-archive",
            Self::Wim => "application/x-ms-wim",
            Self::Cpio => "application/x-cpio",
            Self::Deb => "application/vnd.debian.binary-package",
            Self::Rpm => "application/x-rpm",
            Self::Squashfs => "application/x-squashfs",
            Self::Unknown => "application/octet-stream",
        }
    }
}

impl From<DetectedFormat> for TTZipArchiveFormat {
    fn from(fmt: DetectedFormat) -> Self {
        fmt.to_ttzip_format()
    }
}

impl std::fmt::Display for DetectedFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.primary_extension())
    }
}

/// Compound tar formats derived from outer compression and file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum CompoundFormat {
    TarGz,
    TarBz2,
    TarXz,
    TarZstd,
    TarLz4,
    TarBrotli,
    TarLzip,
    TarLrzip,
}

impl CompoundFormat {
    /// Maps compound format to TTZip core archive format enum.
    #[must_use]
    pub const fn to_ttzip_format(self) -> TTZipArchiveFormat {
        match self {
            Self::TarGz => TTZipArchiveFormat::TarGz,
            Self::TarBz2 => TTZipArchiveFormat::TarBz2,
            Self::TarXz => TTZipArchiveFormat::TarXz,
            Self::TarZstd => TTZipArchiveFormat::TarZstd,
            Self::TarLz4 => TTZipArchiveFormat::TarLz4,
            Self::TarBrotli => TTZipArchiveFormat::TarBrotli,
            Self::TarLzip => TTZipArchiveFormat::TarLzip,
            Self::TarLrzip => TTZipArchiveFormat::TarLrzip,
        }
    }

    /// Primary file extension for compound format.
    #[must_use]
    pub const fn primary_extension(self) -> &'static str {
        match self {
            Self::TarGz => "tar.gz",
            Self::TarBz2 => "tar.bz2",
            Self::TarXz => "tar.xz",
            Self::TarZstd => "tar.zst",
            Self::TarLz4 => "tar.lz4",
            Self::TarBrotli => "tar.br",
            Self::TarLzip => "tar.lz",
            Self::TarLrzip => "tar.lrz",
        }
    }
}

impl From<CompoundFormat> for TTZipArchiveFormat {
    fn from(fmt: CompoundFormat) -> Self {
        fmt.to_ttzip_format()
    }
}

impl std::fmt::Display for CompoundFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.primary_extension())
    }
}
