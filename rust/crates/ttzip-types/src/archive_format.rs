// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Supported archive, container, disk image, and compressed stream formats.

use serde::{Deserialize, Serialize};
use crate::status::TTZipStatus;

/// Supported archive, container, disk image, and compressed stream formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
#[repr(i32)]
pub enum ArchiveFormat {
    Unknown = 0,
    // Primary Containers
    SevenZip = 1,
    Zip = 2,
    Zip64 = 3,
    Tar = 4,
    Rar4 = 5,
    Rar5 = 6,
    Cab = 7,
    Wim = 8,
    Iso = 9,
    Udf = 10,
    Dmg = 11,
    Arj = 12,
    Lzh = 13,
    Cpio = 14,
    Chm = 15,
    Nsis = 16,
    Msi = 17,
    // Single Stream Compression
    Gzip = 18,
    Bzip2 = 19,
    Xz = 20,
    Zstd = 21,
    Lz4 = 22,
    Snappy = 23,
    Lzfse = 24,
    Brotli = 25,
    Lzip = 26,
    Lrzip = 27,
    MsLz = 28,
    // Virtual Disk & Filesystem Images
    Apfs = 29,
    HfsPlus = 30,
    Vhd = 31,
    Vhdx = 32,
    Vmdk = 33,
    Qcow2 = 34,
    Ext2 = 35,
    Ext3 = 36,
    Ext4 = 37,
    Fat = 38,
    Ntfs = 39,
    Squashfs = 40,
    Cramfs = 41,
    // Packages & Apple Standards
    Aar = 42,
    Deb = 43,
    Rpm = 44,
    Xar = 45,
    Ar = 46,
    // Executables, Media & Miscellaneous
    PeExe = 47,
    Elf = 48,
    MachO = 49,
    Swf = 50,
    Flac = 51,
    Dcx = 52,
    Arc = 53,
    Zoo = 54,
    Pak = 55,
    Mub = 56,
    Warc = 57,
    Mtree = 58,
    Raw = 59,
    Empty = 60,
    // Consolidated Formats
    Auto = 61,
    Rar = 62,
    TarGz = 63,
    TarBz2 = 64,
    TarXz = 65,
    TarZstd = 66,
    TarLz4 = 67,
    TarBrotli = 68,
    TarLzip = 69,
    TarLrzip = 70,
}

pub type TTZipArchiveFormat = ArchiveFormat;
pub type DetectedFormat = ArchiveFormat;
pub type CompoundFormat = ArchiveFormat;

impl ArchiveFormat {
    /// Returns the canonical lower-case name of the format.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SevenZip => "7z",
            Self::Zip => "zip",
            Self::Zip64 => "zip64",
            Self::Tar => "tar",
            Self::Rar | Self::Rar4 => "rar4",
            Self::Rar5 => "rar5",
            Self::Cab => "cab",
            Self::Wim => "wim",
            Self::Iso => "iso",
            Self::Udf => "udf",
            Self::Dmg => "dmg",
            Self::Arj => "arj",
            Self::Lzh => "lzh",
            Self::Cpio => "cpio",
            Self::Chm => "chm",
            Self::Nsis => "nsis",
            Self::Msi => "msi",
            Self::Gzip => "gz",
            Self::Bzip2 => "bz2",
            Self::Xz => "xz",
            Self::Zstd => "zst",
            Self::Lz4 => "lz4",
            Self::Snappy => "sz",
            Self::Lzfse => "lzfse",
            Self::Brotli => "br",
            Self::Lzip => "lz",
            Self::Lrzip => "lrz",
            Self::MsLz => "mslz",
            Self::Apfs => "apfs",
            Self::HfsPlus => "hfs+",
            Self::Vhd => "vhd",
            Self::Vhdx => "vhdx",
            Self::Vmdk => "vmdk",
            Self::Qcow2 => "qcow2",
            Self::Ext2 => "ext2",
            Self::Ext3 => "ext3",
            Self::Ext4 => "ext4",
            Self::Fat => "fat",
            Self::Ntfs => "ntfs",
            Self::Squashfs => "squashfs",
            Self::Cramfs => "cramfs",
            Self::Aar => "aar",
            Self::Deb => "deb",
            Self::Rpm => "rpm",
            Self::Xar => "xar",
            Self::Ar => "ar",
            Self::PeExe => "pe",
            Self::Elf => "elf",
            Self::MachO => "macho",
            Self::Swf => "swf",
            Self::Flac => "flac",
            Self::Dcx => "dcx",
            Self::Arc => "arc",
            Self::Zoo => "zoo",
            Self::Pak => "pak",
            Self::Mub => "mub",
            Self::Warc => "warc",
            Self::Mtree => "mtree",
            Self::Raw => "raw",
            Self::Empty => "empty",
            Self::Auto => "auto",
            Self::TarGz => "tar.gz",
            Self::TarBz2 => "tar.bz2",
            Self::TarXz => "tar.xz",
            Self::TarZstd => "tar.zst",
            Self::TarLz4 => "tar.lz4",
            Self::TarBrotli => "tar.br",
            Self::TarLzip => "tar.lz",
            Self::TarLrzip => "tar.lrz",
            Self::Unknown => "unknown",
        }
    }

    /// Primary standard file extension.
    #[must_use]
    pub const fn primary_extension(self) -> &'static str {
        match self {
            Self::SevenZip => "7z",
            Self::Zip | Self::Zip64 => "zip",
            Self::Tar => "tar",
            Self::Rar | Self::Rar4 | Self::Rar5 => "rar",
            Self::Cab => "cab",
            Self::Wim => "wim",
            Self::Iso => "iso",
            Self::Udf => "udf",
            Self::Dmg => "dmg",
            Self::Arj => "arj",
            Self::Lzh => "lzh",
            Self::Cpio => "cpio",
            Self::Chm => "chm",
            Self::Nsis => "exe",
            Self::Msi => "msi",
            Self::Gzip => "gz",
            Self::Bzip2 => "bz2",
            Self::Xz => "xz",
            Self::Zstd => "zst",
            Self::Lz4 => "lz4",
            Self::Snappy => "sz",
            Self::Lzfse => "lzfse",
            Self::Brotli => "br",
            Self::Lzip => "lz",
            Self::Lrzip => "lrz",
            Self::MsLz => "lz_",
            Self::Apfs => "apfs",
            Self::HfsPlus => "hfs",
            Self::Vhd => "vhd",
            Self::Vhdx => "vhdx",
            Self::Vmdk => "vmdk",
            Self::Qcow2 => "qcow2",
            Self::Ext2 => "ext2",
            Self::Ext3 => "ext3",
            Self::Ext4 => "ext4",
            Self::Fat => "fat",
            Self::Ntfs => "ntfs",
            Self::Squashfs => "squashfs",
            Self::Cramfs => "cramfs",
            Self::Aar => "aar",
            Self::Deb => "deb",
            Self::Rpm => "rpm",
            Self::Xar => "xar",
            Self::Ar => "ar",
            Self::PeExe => "exe",
            Self::Elf => "elf",
            Self::MachO => "dylib",
            Self::Swf => "swf",
            Self::Flac => "flac",
            Self::Dcx => "dcx",
            Self::Arc => "arc",
            Self::Zoo => "zoo",
            Self::Pak => "pak",
            Self::Mub => "mub",
            Self::Warc => "warc",
            Self::Mtree => "mtree",
            Self::Raw => "raw",
            Self::Empty => "empty",
            Self::Auto => "",
            Self::TarGz => "tar.gz",
            Self::TarBz2 => "tar.bz2",
            Self::TarXz => "tar.xz",
            Self::TarZstd => "tar.zst",
            Self::TarLz4 => "tar.lz4",
            Self::TarBrotli => "tar.br",
            Self::TarLzip => "tar.lz",
            Self::TarLrzip => "tar.lrz",
            Self::Unknown => "bin",
        }
    }

    /// Primary MIME type identifier.
    #[must_use]
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Auto => "application/octet-stream",
            Self::SevenZip => "application/x-7z-compressed",
            Self::Zip | Self::Zip64 => "application/zip",
            Self::Tar => "application/x-tar",
            Self::Rar | Self::Rar4 | Self::Rar5 => "application/vnd.rar",
            Self::Cab => "application/vnd.ms-cab-compressed",
            Self::Wim => "application/x-ms-wim",
            Self::Iso => "application/x-iso9660-image",
            Self::Udf => "application/x-udf-image",
            Self::Dmg => "application/x-apple-diskimage",
            Self::Arj => "application/x-arj",
            Self::Lzh => "application/x-lzh-compressed",
            Self::Cpio => "application/x-cpio",
            Self::Chm => "application/vnd.ms-htmlhelp",
            Self::Nsis | Self::PeExe => "application/x-msdownload",
            Self::Msi => "application/x-msi",
            Self::Gzip | Self::TarGz => "application/gzip",
            Self::Bzip2 | Self::TarBz2 => "application/x-bzip2",
            Self::Xz | Self::TarXz => "application/x-xz",
            Self::Zstd | Self::TarZstd => "application/zstd",
            Self::Lz4 | Self::TarLz4 => "application/x-lz4",
            Self::Snappy => "application/x-snappy-framed",
            Self::Lzfse => "application/x-lzfse",
            Self::Brotli | Self::TarBrotli => "application/x-brotli",
            Self::Lzip | Self::TarLzip => "application/x-lzip",
            Self::Lrzip | Self::TarLrzip => "application/x-lrzip",
            Self::MsLz => "application/x-ms-compress-lz",
            Self::Apfs => "application/x-apfs-image",
            Self::HfsPlus => "application/x-hfs-image",
            Self::Vhd => "application/x-vhd",
            Self::Vhdx => "application/x-vhdx",
            Self::Vmdk => "application/x-vmdk",
            Self::Qcow2 => "application/x-qemu-disk",
            Self::Ext2 | Self::Ext3 | Self::Ext4 => "application/x-ext-image",
            Self::Fat => "application/x-fat-image",
            Self::Ntfs => "application/x-ntfs-image",
            Self::Squashfs => "application/x-squashfs",
            Self::Cramfs => "application/x-cramfs",
            Self::Aar => "application/x-apple-archive",
            Self::Deb => "application/vnd.debian.binary-package",
            Self::Rpm => "application/x-rpm",
            Self::Xar => "application/x-xar",
            Self::Ar => "application/x-archive",
            Self::Elf => "application/x-executable",
            Self::MachO => "application/x-mach-binary",
            Self::Swf => "application/x-shockwave-flash",
            Self::Flac => "audio/flac",
            Self::Dcx => "image/x-dcx",
            Self::Arc => "application/x-arc",
            Self::Zoo => "application/x-zoo",
            Self::Pak => "application/x-pak",
            Self::Mub => "application/octet-stream",
            Self::Warc => "application/warc",
            Self::Mtree => "text/x-mtree",
            Self::Raw => "application/octet-stream",
            Self::Empty => "application/x-empty",
            Self::Unknown => "application/octet-stream",
        }
    }

    /// Returns `true` if this format is a compound tar archive container.
    #[inline]
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
                | Self::Zip64
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
                | Self::Udf
                | Self::Cab
                | Self::Wim
                | Self::Rar
                | Self::Rar4
                | Self::Rar5
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
    #[inline]
    #[must_use]
    pub const fn supports_password(self) -> bool {
        matches!(
            self,
            Self::Zip | Self::Zip64 | Self::SevenZip | Self::Wim | Self::Dmg | Self::Aar | Self::Rar | Self::Rar4 | Self::Rar5
        )
    }

    /// Returns `true` if this format natively supports multi-part split volume output.
    #[inline]
    #[must_use]
    pub const fn supports_split_volume(self) -> bool {
        matches!(
            self,
            Self::Zip | Self::Zip64 | Self::SevenZip | Self::Wim | Self::Rar | Self::Rar4 | Self::Rar5
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
                | Self::Rar4
                | Self::Rar5
                | Self::Squashfs
        )
    }

    /// Returns `true` if this archive format is solid or inherently solid-stream compressed.
    #[inline]
    #[must_use]
    pub const fn is_solid(self) -> bool {
        self.supports_solid_mode()
    }

    /// Returns `true` if this format supports password encryption or security protection.
    #[inline]
    #[must_use]
    pub const fn is_encrypted(self) -> bool {
        self.supports_password()
    }

    /// Returns `true` if this format supports single-pass linear streaming decompression without random seek.
    #[must_use]
    pub const fn supports_streaming(self) -> bool {
        matches!(
            self,
            Self::Tar
                | Self::Gzip
                | Self::Bzip2
                | Self::Xz
                | Self::Zstd
                | Self::Lz4
                | Self::Snappy
                | Self::Brotli
                | Self::Lzip
                | Self::Lrzip
                | Self::Lzfse
                | Self::TarGz
                | Self::TarBz2
                | Self::TarXz
                | Self::TarZstd
                | Self::TarLz4
                | Self::TarBrotli
                | Self::TarLzip
                | Self::TarLrzip
                | Self::Cpio
                | Self::Ar
        )
    }

    /// Returns the minimum and maximum valid integer compression level `(min, max)`.
    #[must_use]
    pub const fn compression_level_range(self) -> (i32, i32) {
        match self {
            Self::Tar
            | Self::Dmg
            | Self::Iso
            | Self::Udf
            | Self::Aar
            | Self::Cpio
            | Self::Ar
            | Self::Rar
            | Self::Rar4
            | Self::Rar5
            | Self::Squashfs
            | Self::Lzh => (0, 0),
            Self::Zip | Self::Zip64 => (0, 12),
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
            _ => (0, 9),
        }
    }

    /// Returns the default recommended compression level integer for standard operations.
    #[must_use]
    pub const fn default_compression_level(self) -> i32 {
        match self {
            Self::Tar
            | Self::Dmg
            | Self::Iso
            | Self::Udf
            | Self::Aar
            | Self::Cpio
            | Self::Ar
            | Self::Rar
            | Self::Rar4
            | Self::Rar5
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

    /// Converts into existing DetectedFormat enum (self-identity).
    #[inline]
    #[must_use]
    pub const fn to_detected_format(self) -> Self {
        self
    }

    /// Converts to TTZipArchiveFormat enum (self-identity).
    #[inline]
    #[must_use]
    pub const fn to_ttzip_format(self) -> Self {
        self
    }
}

impl std::fmt::Display for ArchiveFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
