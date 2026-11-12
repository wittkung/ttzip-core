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

struct FormatDescriptor {
    name: &'static str,
    primary_ext: &'static str,
    mime_type: &'static str,
}

const FORMAT_DESCRIPTORS: [FormatDescriptor; 71] = [
    FormatDescriptor { name: "unknown", primary_ext: "bin", mime_type: "application/octet-stream" },
    FormatDescriptor { name: "7z", primary_ext: "7z", mime_type: "application/x-7z-compressed" },
    FormatDescriptor { name: "zip", primary_ext: "zip", mime_type: "application/zip" },
    FormatDescriptor { name: "zip64", primary_ext: "zip", mime_type: "application/zip" },
    FormatDescriptor { name: "tar", primary_ext: "tar", mime_type: "application/x-tar" },
    FormatDescriptor { name: "rar4", primary_ext: "rar", mime_type: "application/vnd.rar" },
    FormatDescriptor { name: "rar5", primary_ext: "rar", mime_type: "application/vnd.rar" },
    FormatDescriptor { name: "cab", primary_ext: "cab", mime_type: "application/vnd.ms-cab-compressed" },
    FormatDescriptor { name: "wim", primary_ext: "wim", mime_type: "application/x-ms-wim" },
    FormatDescriptor { name: "iso", primary_ext: "iso", mime_type: "application/x-iso9660-image" },
    FormatDescriptor { name: "udf", primary_ext: "udf", mime_type: "application/x-udf-image" },
    FormatDescriptor { name: "dmg", primary_ext: "dmg", mime_type: "application/x-apple-diskimage" },
    FormatDescriptor { name: "arj", primary_ext: "arj", mime_type: "application/x-arj" },
    FormatDescriptor { name: "lzh", primary_ext: "lzh", mime_type: "application/x-lzh-compressed" },
    FormatDescriptor { name: "cpio", primary_ext: "cpio", mime_type: "application/x-cpio" },
    FormatDescriptor { name: "chm", primary_ext: "chm", mime_type: "application/vnd.ms-htmlhelp" },
    FormatDescriptor { name: "nsis", primary_ext: "exe", mime_type: "application/x-msdownload" },
    FormatDescriptor { name: "msi", primary_ext: "msi", mime_type: "application/x-msi" },
    FormatDescriptor { name: "gz", primary_ext: "gz", mime_type: "application/gzip" },
    FormatDescriptor { name: "bz2", primary_ext: "bz2", mime_type: "application/x-bzip2" },
    FormatDescriptor { name: "xz", primary_ext: "xz", mime_type: "application/x-xz" },
    FormatDescriptor { name: "zst", primary_ext: "zst", mime_type: "application/zstd" },
    FormatDescriptor { name: "lz4", primary_ext: "lz4", mime_type: "application/x-lz4" },
    FormatDescriptor { name: "sz", primary_ext: "sz", mime_type: "application/x-snappy-framed" },
    FormatDescriptor { name: "lzfse", primary_ext: "lzfse", mime_type: "application/x-lzfse" },
    FormatDescriptor { name: "br", primary_ext: "br", mime_type: "application/x-brotli" },
    FormatDescriptor { name: "lz", primary_ext: "lz", mime_type: "application/x-lzip" },
    FormatDescriptor { name: "lrz", primary_ext: "lrz", mime_type: "application/x-lrzip" },
    FormatDescriptor { name: "mslz", primary_ext: "lz_", mime_type: "application/x-ms-compress-lz" },
    FormatDescriptor { name: "apfs", primary_ext: "apfs", mime_type: "application/x-apfs-image" },
    FormatDescriptor { name: "hfs+", primary_ext: "hfs", mime_type: "application/x-hfs-image" },
    FormatDescriptor { name: "vhd", primary_ext: "vhd", mime_type: "application/x-vhd" },
    FormatDescriptor { name: "vhdx", primary_ext: "vhdx", mime_type: "application/x-vhdx" },
    FormatDescriptor { name: "vmdk", primary_ext: "vmdk", mime_type: "application/x-vmdk" },
    FormatDescriptor { name: "qcow2", primary_ext: "qcow2", mime_type: "application/x-qemu-disk" },
    FormatDescriptor { name: "ext2", primary_ext: "ext2", mime_type: "application/x-ext-image" },
    FormatDescriptor { name: "ext3", primary_ext: "ext3", mime_type: "application/x-ext-image" },
    FormatDescriptor { name: "ext4", primary_ext: "ext4", mime_type: "application/x-ext-image" },
    FormatDescriptor { name: "fat", primary_ext: "fat", mime_type: "application/x-fat-image" },
    FormatDescriptor { name: "ntfs", primary_ext: "ntfs", mime_type: "application/x-ntfs-image" },
    FormatDescriptor { name: "squashfs", primary_ext: "squashfs", mime_type: "application/x-squashfs" },
    FormatDescriptor { name: "cramfs", primary_ext: "cramfs", mime_type: "application/x-cramfs" },
    FormatDescriptor { name: "aar", primary_ext: "aar", mime_type: "application/x-apple-archive" },
    FormatDescriptor { name: "deb", primary_ext: "deb", mime_type: "application/vnd.debian.binary-package" },
    FormatDescriptor { name: "rpm", primary_ext: "rpm", mime_type: "application/x-rpm" },
    FormatDescriptor { name: "xar", primary_ext: "xar", mime_type: "application/x-xar" },
    FormatDescriptor { name: "ar", primary_ext: "ar", mime_type: "application/x-archive" },
    FormatDescriptor { name: "pe", primary_ext: "exe", mime_type: "application/x-msdownload" },
    FormatDescriptor { name: "elf", primary_ext: "elf", mime_type: "application/x-executable" },
    FormatDescriptor { name: "macho", primary_ext: "dylib", mime_type: "application/x-mach-binary" },
    FormatDescriptor { name: "swf", primary_ext: "swf", mime_type: "application/x-shockwave-flash" },
    FormatDescriptor { name: "flac", primary_ext: "flac", mime_type: "audio/flac" },
    FormatDescriptor { name: "dcx", primary_ext: "dcx", mime_type: "image/x-dcx" },
    FormatDescriptor { name: "arc", primary_ext: "arc", mime_type: "application/x-arc" },
    FormatDescriptor { name: "zoo", primary_ext: "zoo", mime_type: "application/x-zoo" },
    FormatDescriptor { name: "pak", primary_ext: "pak", mime_type: "application/x-pak" },
    FormatDescriptor { name: "mub", primary_ext: "mub", mime_type: "application/octet-stream" },
    FormatDescriptor { name: "warc", primary_ext: "warc", mime_type: "application/warc" },
    FormatDescriptor { name: "mtree", primary_ext: "mtree", mime_type: "text/x-mtree" },
    FormatDescriptor { name: "raw", primary_ext: "raw", mime_type: "application/octet-stream" },
    FormatDescriptor { name: "empty", primary_ext: "empty", mime_type: "application/x-empty" },
    FormatDescriptor { name: "auto", primary_ext: "", mime_type: "application/octet-stream" },
    FormatDescriptor { name: "rar4", primary_ext: "rar", mime_type: "application/vnd.rar" },
    FormatDescriptor { name: "tar.gz", primary_ext: "tar.gz", mime_type: "application/gzip" },
    FormatDescriptor { name: "tar.bz2", primary_ext: "tar.bz2", mime_type: "application/x-bzip2" },
    FormatDescriptor { name: "tar.xz", primary_ext: "tar.xz", mime_type: "application/x-xz" },
    FormatDescriptor { name: "tar.zst", primary_ext: "tar.zst", mime_type: "application/zstd" },
    FormatDescriptor { name: "tar.lz4", primary_ext: "tar.lz4", mime_type: "application/x-lz4" },
    FormatDescriptor { name: "tar.br", primary_ext: "tar.br", mime_type: "application/x-brotli" },
    FormatDescriptor { name: "tar.lz", primary_ext: "tar.lz", mime_type: "application/x-lzip" },
    FormatDescriptor { name: "tar.lrz", primary_ext: "tar.lrz", mime_type: "application/x-lrzip" },
];

impl ArchiveFormat {
    #[inline]
    const fn descriptor(self) -> &'static FormatDescriptor {
        &FORMAT_DESCRIPTORS[self as usize]
    }

    /// Returns the canonical lower-case name of the format.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.descriptor().name
    }

    /// Primary standard file extension.
    #[must_use]
    pub const fn primary_extension(self) -> &'static str {
        self.descriptor().primary_ext
    }

    /// Primary MIME type identifier.
    #[must_use]
    pub const fn mime_type(self) -> &'static str {
        self.descriptor().mime_type
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
