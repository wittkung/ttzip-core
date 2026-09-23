// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Supported archive formats, compression levels, and encryption schemes.

use serde::{Deserialize, Serialize};
use super::status::TTZipStatus;

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

    /// Returns `true` if this format supports password encryption / security protection.
    #[must_use]
    pub const fn supports_password(self) -> bool {
        matches!(
            self,
            Self::Zip | Self::SevenZip | Self::Wim | Self::Dmg | Self::Aar | Self::Rar
        )
    }

    /// Returns `true` if this format natively supports multi-part split volume output.
    #[must_use]
    pub const fn supports_split_volume(self) -> bool {
        matches!(
            self,
            Self::Zip | Self::SevenZip | Self::Wim | Self::Rar
        )
    }

    /// Returns `true` if this format supports solid mode archiving blocks across multiple files.
    #[must_use]
    pub const fn supports_solid_mode(self) -> bool {
        matches!(
            self,
            Self::SevenZip
                | Self::Tar
                | Self::TarGz
                | Self::TarBz2
                | Self::TarXz
                | Self::TarZstd
                | Self::TarLz4
                | Self::TarBrotli
                | Self::TarLzip
                | Self::TarLrzip
                | Self::Rar
                | Self::Squashfs
        )
    }

    /// Returns the minimum and maximum valid integer compression level `(min, max)`.
    #[must_use]
    pub const fn compression_level_range(self) -> (i32, i32) {
        match self {
            Self::Tar
            | Self::Dmg
            | Self::Iso
            | Self::Aar
            | Self::Cpio
            | Self::Ar
            | Self::Rar
            | Self::Squashfs
            | Self::Lzh => (0, 0),
            Self::Zip => (0, 12),
            Self::Zstd | Self::TarZstd => (0, 22),
            Self::Brotli | Self::TarBrotli => (0, 11),
            Self::SevenZip
            | Self::Gzip
            | Self::TarGz
            | Self::Bzip2
            | Self::TarBz2
            | Self::Xz
            | Self::TarXz
            | Self::Lzip
            | Self::TarLzip
            | Self::Lz4
            | Self::TarLz4
            | Self::Lrzip
            | Self::TarLrzip
            | Self::Snappy
            | Self::Wim
            | Self::Cab
            | Self::Deb
            | Self::Rpm
            | Self::Xar
            | Self::Lzfse
            | Self::Auto
            | Self::Unknown => (0, 9),
        }
    }

    /// Returns the default recommended compression level integer for standard operations.
    #[must_use]
    pub const fn default_compression_level(self) -> i32 {
        match self {
            Self::Tar
            | Self::Dmg
            | Self::Iso
            | Self::Aar
            | Self::Cpio
            | Self::Ar
            | Self::Rar
            | Self::Squashfs
            | Self::Lzh => 0,
            Self::Zstd | Self::TarZstd => 3,
            Self::Brotli | Self::TarBrotli => 4,
            Self::Lz4 | Self::TarLz4 => 1,
            _ => 6,
        }
    }

    /// Validates archive configuration parameters against the capability envelope.
    pub fn validate_configuration(
        self,
        level: i32,
        has_password: bool,
        is_split: bool,
    ) -> Result<(), TTZipStatus> {
        if has_password && !self.supports_password() {
            return Err(TTZipStatus::ErrUnsupportedFeature);
        }
        if is_split && !self.supports_split_volume() {
            return Err(TTZipStatus::ErrUnsupportedFeature);
        }
        let (min, max) = self.compression_level_range();
        if level < min || level > max {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        Ok(())
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
