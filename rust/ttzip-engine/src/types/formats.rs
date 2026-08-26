// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Supported archive formats, compression levels, and encryption schemes.

use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TTZipArchiveFormat {
    Auto = 0,
    Zip = 1,
    SevenZip = 2,
    Tar = 3,
    TarGz = 4,
    TarBz2 = 5,
    TarXz = 6,
    TarZstd = 7,
    Dmg = 8,
    Lzfse = 9,
    Snappy = 10,
    Gzip = 11,
    Bzip2 = 12,
    Xz = 13,
    Zstd = 14,
    Lz4 = 15,
    Brotli = 16,
    Iso = 17,
    Cab = 18,
    Wim = 19,
    Rar = 20,
    Aar = 21,
    Lzip = 22,
    Lrzip = 23,
    Cpio = 24,
    Ar = 25,
    Deb = 26,
    Rpm = 27,
    Xar = 28,
    Squashfs = 29,
    Lzh = 30,
    TarLz4 = 31,
    TarBrotli = 32,
    TarLzip = 33,
    TarLrzip = 34,
    Unknown = 99,
}

impl TTZipArchiveFormat {
    /// Returns the canonical lower-case format identifier name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Zip => "zip",
            Self::SevenZip => "7z",
            Self::Tar => "tar",
            Self::TarGz => "tar.gz",
            Self::TarBz2 => "tar.bz2",
            Self::TarXz => "tar.xz",
            Self::TarZstd => "tar.zst",
            Self::TarLz4 => "tar.lz4",
            Self::TarBrotli => "tar.br",
            Self::TarLzip => "tar.lz",
            Self::TarLrzip => "tar.lrz",
            Self::Dmg => "dmg",
            Self::Lzfse => "lzfse",
            Self::Snappy => "sz",
            Self::Gzip => "gz",
            Self::Bzip2 => "bz2",
            Self::Xz => "xz",
            Self::Zstd => "zst",
            Self::Lz4 => "lz4",
            Self::Brotli => "br",
            Self::Iso => "iso",
            Self::Cab => "cab",
            Self::Wim => "wim",
            Self::Rar => "rar",
            Self::Aar => "aar",
            Self::Lzip => "lz",
            Self::Lrzip => "lrz",
            Self::Cpio => "cpio",
            Self::Ar => "ar",
            Self::Deb => "deb",
            Self::Rpm => "rpm",
            Self::Xar => "xar",
            Self::Squashfs => "squashfs",
            Self::Lzh => "lzh",
            Self::Unknown => "unknown",
        }
    }

    /// Primary standard file extension for this archive or compression format.
    #[must_use]
    pub const fn primary_extension(self) -> &'static str {
        match self {
            Self::Auto => "",
            Self::Zip => "zip",
            Self::SevenZip => "7z",
            Self::Tar => "tar",
            Self::TarGz => "tar.gz",
            Self::TarBz2 => "tar.bz2",
            Self::TarXz => "tar.xz",
            Self::TarZstd => "tar.zst",
            Self::TarLz4 => "tar.lz4",
            Self::TarBrotli => "tar.br",
            Self::TarLzip => "tar.lz",
            Self::TarLrzip => "tar.lrz",
            Self::Dmg => "dmg",
            Self::Lzfse => "lzfse",
            Self::Snappy => "sz",
            Self::Gzip => "gz",
            Self::Bzip2 => "bz2",
            Self::Xz => "xz",
            Self::Zstd => "zst",
            Self::Lz4 => "lz4",
            Self::Brotli => "br",
            Self::Iso => "iso",
            Self::Cab => "cab",
            Self::Wim => "wim",
            Self::Rar => "rar",
            Self::Aar => "aar",
            Self::Lzip => "lz",
            Self::Lrzip => "lrz",
            Self::Cpio => "cpio",
            Self::Ar => "ar",
            Self::Deb => "deb",
            Self::Rpm => "rpm",
            Self::Xar => "xar",
            Self::Squashfs => "squashfs",
            Self::Lzh => "lzh",
            Self::Unknown => "bin",
        }
    }

    /// Canonical MIME type string identifier.
    #[must_use]
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Auto => "application/octet-stream",
            Self::Zip => "application/zip",
            Self::SevenZip => "application/x-7z-compressed",
            Self::Tar => "application/x-tar",
            Self::TarGz | Self::Gzip => "application/gzip",
            Self::TarBz2 | Self::Bzip2 => "application/x-bzip2",
            Self::TarXz | Self::Xz => "application/x-xz",
            Self::TarZstd | Self::Zstd => "application/zstd",
            Self::TarLz4 | Self::Lz4 => "application/x-lz4",
            Self::TarBrotli | Self::Brotli => "application/x-brotli",
            Self::TarLzip | Self::Lzip => "application/x-lzip",
            Self::TarLrzip | Self::Lrzip => "application/x-lrzip",
            Self::Dmg => "application/x-apple-diskimage",
            Self::Lzfse => "application/x-lzfse",
            Self::Snappy => "application/x-snappy-framed",
            Self::Iso => "application/x-iso9660-image",
            Self::Cab => "application/vnd.ms-cab-compressed",
            Self::Wim => "application/x-ms-wim",
            Self::Rar => "application/vnd.rar",
            Self::Aar => "application/x-apple-archive",
            Self::Cpio => "application/x-cpio",
            Self::Ar => "application/x-archive",
            Self::Deb => "application/vnd.debian.binary-package",
            Self::Rpm => "application/x-rpm",
            Self::Xar => "application/x-xar",
            Self::Squashfs => "application/x-squashfs",
            Self::Lzh => "application/x-lzh-compressed",
            Self::Unknown => "application/octet-stream",
        }
    }

    /// Returns `true` if this format is a compound tar archive container.
    #[must_use]
    pub const fn is_compound_tar(self) -> bool {
        matches!(
            self,
            Self::TarGz
                | Self::TarBz2
                | Self::TarXz
                | Self::TarZstd
                | Self::TarLz4
                | Self::TarBrotli
                | Self::TarLzip
                | Self::TarLrzip
        )
    }

    /// Returns `true` if this format supports multi-file archive packing.
    #[must_use]
    pub const fn is_archive_container(self) -> bool {
        matches!(
            self,
            Self::Zip
                | Self::SevenZip
                | Self::Tar
                | Self::TarGz
                | Self::TarBz2
                | Self::TarXz
                | Self::TarZstd
                | Self::TarLz4
                | Self::TarBrotli
                | Self::TarLzip
                | Self::TarLrzip
                | Self::Iso
                | Self::Cab
                | Self::Wim
                | Self::Rar
                | Self::Aar
                | Self::Cpio
                | Self::Ar
                | Self::Deb
                | Self::Rpm
                | Self::Xar
                | Self::Squashfs
                | Self::Lzh
                | Self::Dmg
        )
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipCompressionLevel {
    Store = 0,
    Fastest = 1,
    Fast = 3,
    Normal = 6,
    Maximum = 9,
    Ultra = 12,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipEncryptionMethod {
    None = 0,
    ZipCrypto = 1,
    Aes128 = 2,
    Aes192 = 3,
    Aes256 = 4,
}
