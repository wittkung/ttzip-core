// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Master static rule matrix and anchor definitions for 50+ archive formats.

use super::formats::ArchiveFormat;

/// Anchor position strategy for format sniffing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SniffAnchor {
    /// Direct offset from start of file/buffer.
    Head(usize),
    /// Backward offset from end of file/buffer.
    Tail(usize),
    /// ISO 2048-byte sector offset.
    Sector(usize),
    /// TAR 512-byte header block relative offset.
    TarOffset(usize),
}

/// Static signature definition for format sniffing rule.
#[derive(Debug, Clone, Copy)]
pub struct SniffRule {
    pub format: ArchiveFormat,
    pub anchor: SniffAnchor,
    pub magic: &'static [u8],
    pub min_total_size: usize,
    pub confidence: u8,
}

/// Master table of static format signatures covering 50+ formats.
pub static SNIFF_RULES: &[SniffRule] = &[
    // --- 1. Archive Containers & High Confidence Formats ---
    SniffRule {
        format: ArchiveFormat::SevenZip,
        anchor: SniffAnchor::Head(0),
        magic: &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C],
        min_total_size: 32,
        confidence: 100,
    },
    SniffRule {
        format: ArchiveFormat::Xz,
        anchor: SniffAnchor::Head(0),
        magic: &[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00],
        min_total_size: 12,
        confidence: 100,
    },
    SniffRule {
        format: ArchiveFormat::Rar5,
        anchor: SniffAnchor::Head(0),
        magic: &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00],
        min_total_size: 8,
        confidence: 100,
    },
    SniffRule {
        format: ArchiveFormat::Rar4,
        anchor: SniffAnchor::Head(0),
        magic: &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00],
        min_total_size: 7,
        confidence: 99,
    },
    SniffRule {
        format: ArchiveFormat::Zstd,
        anchor: SniffAnchor::Head(0),
        magic: &[0x28, 0xB5, 0x2F, 0xFD],
        min_total_size: 4,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Xar,
        anchor: SniffAnchor::Head(0),
        magic: &[0x78, 0x61, 0x72, 0x21], // "xar!"
        min_total_size: 28,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Wim,
        anchor: SniffAnchor::Head(0),
        magic: &[0x4D, 0x53, 0x57, 0x49, 0x4D, 0x00, 0x00, 0x00], // "MSWIM\0\0\0"
        min_total_size: 208,
        confidence: 100,
    },
    SniffRule {
        format: ArchiveFormat::Cab,
        anchor: SniffAnchor::Head(0),
        magic: &[0x4D, 0x53, 0x43, 0x46], // "MSCF"
        min_total_size: 32,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Msi,
        anchor: SniffAnchor::Head(0),
        magic: &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1], // OLE CFB
        min_total_size: 512,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Chm,
        anchor: SniffAnchor::Head(0),
        magic: &[0x49, 0x54, 0x53, 0x46], // "ITSF"
        min_total_size: 32,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Ar,
        anchor: SniffAnchor::Head(0),
        magic: &[0x21, 0x3C, 0x61, 0x72, 0x63, 0x68, 0x3E, 0x0A], // "!<arch>\n"
        min_total_size: 8,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Rpm,
        anchor: SniffAnchor::Head(0),
        magic: &[0xED, 0xAB, 0xEE, 0xDB],
        min_total_size: 96,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Squashfs,
        anchor: SniffAnchor::Head(0),
        magic: &[0x68, 0x73, 0x71, 0x73], // "hsqs" (LE)
        min_total_size: 96,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Squashfs,
        anchor: SniffAnchor::Head(0),
        magic: &[0x73, 0x71, 0x73, 0x68], // "sqsh" (BE)
        min_total_size: 96,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Cpio,
        anchor: SniffAnchor::Head(0),
        magic: &[0x30, 0x37, 0x30, 0x37, 0x30, 0x31], // "070701" newc
        min_total_size: 110,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Cpio,
        anchor: SniffAnchor::Head(0),
        magic: &[0x30, 0x37, 0x30, 0x37, 0x30, 0x32], // "070702" crc
        min_total_size: 110,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Cpio,
        anchor: SniffAnchor::Head(0),
        magic: &[0x30, 0x37, 0x30, 0x37, 0x30, 0x37], // "070707" odc
        min_total_size: 76,
        confidence: 90,
    },
    SniffRule {
        format: ArchiveFormat::Cpio,
        anchor: SniffAnchor::Head(0),
        magic: &[0xC7, 0x71], // Binary LE
        min_total_size: 26,
        confidence: 85,
    },
    SniffRule {
        format: ArchiveFormat::Cpio,
        anchor: SniffAnchor::Head(0),
        magic: &[0x71, 0xC7], // Binary BE
        min_total_size: 26,
        confidence: 85,
    },
    SniffRule {
        format: ArchiveFormat::Snappy,
        anchor: SniffAnchor::Head(0),
        magic: &[0xFF, 0x06, 0x00, 0x00, 0x73, 0x4E, 0x61, 0x50, 0x70, 0x59], // "\xFF\x06\x00\x00sNaPpY"
        min_total_size: 10,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Lzip,
        anchor: SniffAnchor::Head(0),
        magic: &[0x4C, 0x5A, 0x49, 0x50], // "LZIP"
        min_total_size: 6,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Lrzip,
        anchor: SniffAnchor::Head(0),
        magic: &[0x4C, 0x52, 0x5A, 0x49], // "LRZI"
        min_total_size: 6,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Aar,
        anchor: SniffAnchor::Head(0),
        magic: &[0x41, 0x41, 0x30, 0x31], // "AA01"
        min_total_size: 16,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Aar,
        anchor: SniffAnchor::Head(0),
        magic: &[0x41, 0x45, 0x41, 0x31], // "AEA1"
        min_total_size: 16,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Lz4,
        anchor: SniffAnchor::Head(0),
        magic: &[0x04, 0x22, 0x4D, 0x18], // LZ4 Frame
        min_total_size: 7,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Lz4,
        anchor: SniffAnchor::Head(0),
        magic: &[0x02, 0x21, 0x4C, 0x18], // LZ4 Legacy
        min_total_size: 4,
        confidence: 90,
    },
    SniffRule {
        format: ArchiveFormat::Lzfse,
        anchor: SniffAnchor::Head(0),
        magic: &[0x62, 0x76, 0x78, 0x2D], // "bvx-"
        min_total_size: 4,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Lzfse,
        anchor: SniffAnchor::Head(0),
        magic: &[0x62, 0x76, 0x78, 0x31], // "bvx1"
        min_total_size: 4,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Lzfse,
        anchor: SniffAnchor::Head(0),
        magic: &[0x62, 0x76, 0x78, 0x32], // "bvx2"
        min_total_size: 4,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Lzfse,
        anchor: SniffAnchor::Head(0),
        magic: &[0x62, 0x76, 0x78, 0x6E], // "bvxn"
        min_total_size: 4,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Zip,
        anchor: SniffAnchor::Head(0),
        magic: &[0x50, 0x4B, 0x03, 0x04], // "PK\x03\x04"
        min_total_size: 30,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Zip,
        anchor: SniffAnchor::Head(0),
        magic: &[0x50, 0x4B, 0x05, 0x06], // "PK\x05\x06" (Empty ZIP)
        min_total_size: 22,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Zip,
        anchor: SniffAnchor::Head(0),
        magic: &[0x50, 0x4B, 0x07, 0x08], // "PK\x07\x08" (Spanned ZIP)
        min_total_size: 16,
        confidence: 90,
    },
    SniffRule {
        format: ArchiveFormat::Gzip,
        anchor: SniffAnchor::Head(0),
        magic: &[0x1F, 0x8B],
        min_total_size: 10,
        confidence: 90,
    },
    SniffRule {
        format: ArchiveFormat::Bzip2,
        anchor: SniffAnchor::Head(0),
        magic: &[0x42, 0x5A, 0x68], // "BZh"
        min_total_size: 14,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Tar,
        anchor: SniffAnchor::TarOffset(257),
        magic: &[0x75, 0x73, 0x74, 0x61, 0x72, 0x00], // "ustar\0"
        min_total_size: 512,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Tar,
        anchor: SniffAnchor::TarOffset(257),
        magic: &[0x75, 0x73, 0x74, 0x61, 0x72, 0x20, 0x20, 0x00], // "ustar  \0"
        min_total_size: 512,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Arj,
        anchor: SniffAnchor::Head(0),
        magic: &[0x60, 0xEA],
        min_total_size: 16,
        confidence: 85,
    },
    SniffRule {
        format: ArchiveFormat::Lzh,
        anchor: SniffAnchor::Head(2),
        magic: &[0x2D, 0x6C, 0x68], // "-lh"
        min_total_size: 24,
        confidence: 90,
    },
    SniffRule {
        format: ArchiveFormat::Lzh,
        anchor: SniffAnchor::Head(2),
        magic: &[0x2D, 0x6C, 0x7A], // "-lz"
        min_total_size: 24,
        confidence: 90,
    },
    SniffRule {
        format: ArchiveFormat::MsLz,
        anchor: SniffAnchor::Head(0),
        magic: &[0x53, 0x5A, 0x44, 0x44, 0x88, 0xF0, 0x27, 0x33], // "SZDD\x88\xF0\x27\x33"
        min_total_size: 14,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Cramfs,
        anchor: SniffAnchor::Head(0),
        magic: &[0x28, 0xCD, 0x3D, 0x45], // CramFS LE
        min_total_size: 64,
        confidence: 90,
    },
    SniffRule {
        format: ArchiveFormat::Cramfs,
        anchor: SniffAnchor::Head(0),
        magic: &[0x45, 0x3D, 0xCD, 0x28], // CramFS BE
        min_total_size: 64,
        confidence: 90,
    },
    SniffRule {
        format: ArchiveFormat::Flac,
        anchor: SniffAnchor::Head(0),
        magic: &[0x66, 0x4C, 0x61, 0x43], // "fLaC"
        min_total_size: 42,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Swf,
        anchor: SniffAnchor::Head(0),
        magic: &[0x46, 0x57, 0x53], // "FWS"
        min_total_size: 8,
        confidence: 85,
    },
    SniffRule {
        format: ArchiveFormat::Swf,
        anchor: SniffAnchor::Head(0),
        magic: &[0x43, 0x57, 0x53], // "CWS"
        min_total_size: 8,
        confidence: 85,
    },
    SniffRule {
        format: ArchiveFormat::Swf,
        anchor: SniffAnchor::Head(0),
        magic: &[0x5A, 0x57, 0x53], // "ZWS"
        min_total_size: 8,
        confidence: 85,
    },
    SniffRule {
        format: ArchiveFormat::Dcx,
        anchor: SniffAnchor::Head(0),
        magic: &[0xB1, 0x68, 0xDE, 0x3A],
        min_total_size: 4,
        confidence: 90,
    },
    SniffRule {
        format: ArchiveFormat::Arc,
        anchor: SniffAnchor::Head(0),
        magic: &[0x1A],
        min_total_size: 29,
        confidence: 60,
    },
    SniffRule {
        format: ArchiveFormat::Zoo,
        anchor: SniffAnchor::Head(20),
        magic: &[0x5A, 0x4F, 0x4F, 0x20], // "ZOO "
        min_total_size: 34,
        confidence: 90,
    },
    SniffRule {
        format: ArchiveFormat::Pak,
        anchor: SniffAnchor::Head(0),
        magic: &[0x50, 0x41, 0x43, 0x4B], // "PACK" (Quake PAK)
        min_total_size: 12,
        confidence: 85,
    },
    SniffRule {
        format: ArchiveFormat::Elf,
        anchor: SniffAnchor::Head(0),
        magic: &[0x7F, 0x45, 0x4C, 0x46], // "\x7FELF"
        min_total_size: 52,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::MachO,
        anchor: SniffAnchor::Head(0),
        magic: &[0xFE, 0xED, 0xFA, 0xCE], // Mach-O 32-bit
        min_total_size: 32,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::MachO,
        anchor: SniffAnchor::Head(0),
        magic: &[0xFE, 0xED, 0xFA, 0xCF], // Mach-O 64-bit
        min_total_size: 32,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::MachO,
        anchor: SniffAnchor::Head(0),
        magic: &[0xCE, 0xFA, 0xED, 0xFE], // Mach-O 32-bit LE
        min_total_size: 32,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::MachO,
        anchor: SniffAnchor::Head(0),
        magic: &[0xCF, 0xFA, 0xED, 0xFE], // Mach-O 64-bit LE
        min_total_size: 32,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::MachO,
        anchor: SniffAnchor::Head(0),
        magic: &[0xCA, 0xFE, 0xBA, 0xBE], // Universal Fat Binary
        min_total_size: 32,
        confidence: 90,
    },
    SniffRule {
        format: ArchiveFormat::PeExe,
        anchor: SniffAnchor::Head(0),
        magic: &[0x4D, 0x5A], // "MZ"
        min_total_size: 64,
        confidence: 75,
    },

    // --- 2. Virtual Disks & Filesystems ---
    SniffRule {
        format: ArchiveFormat::Apfs,
        anchor: SniffAnchor::Head(0),
        magic: &[0x4E, 0x58, 0x53, 0x42], // "NXSB" (APFS Superblock)
        min_total_size: 512,
        confidence: 100,
    },
    SniffRule {
        format: ArchiveFormat::Vhdx,
        anchor: SniffAnchor::Head(0),
        magic: &[0x76, 0x68, 0x64, 0x78, 0x66, 0x69, 0x6C, 0x65], // "vhdxfile"
        min_total_size: 65536,
        confidence: 100,
    },
    SniffRule {
        format: ArchiveFormat::Vmdk,
        anchor: SniffAnchor::Head(0),
        magic: &[0x4B, 0x44, 0x4D, 0x56], // "KDMV"
        min_total_size: 512,
        confidence: 100,
    },
    SniffRule {
        format: ArchiveFormat::Qcow2,
        anchor: SniffAnchor::Head(0),
        magic: &[0x51, 0x46, 0x49, 0xFB], // "QFI\xFB"
        min_total_size: 72,
        confidence: 100,
    },
    SniffRule {
        format: ArchiveFormat::Vhd,
        anchor: SniffAnchor::Head(0),
        magic: &[0x63, 0x6F, 0x6E, 0x65, 0x63, 0x74, 0x69, 0x78], // "conectix"
        min_total_size: 512,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::HfsPlus,
        anchor: SniffAnchor::Head(1024),
        magic: &[0x48, 0x2B], // "H+"
        min_total_size: 1536,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::HfsPlus,
        anchor: SniffAnchor::Head(1024),
        magic: &[0x48, 0x58], // "HX"
        min_total_size: 1536,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Ext4,
        anchor: SniffAnchor::Head(1080), // Superblock 1024 + 0x38 = 1080
        magic: &[0x53, 0xEF],
        min_total_size: 2048,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Ntfs,
        anchor: SniffAnchor::Head(3),
        magic: &[0x4E, 0x54, 0x46, 0x53, 0x20, 0x20, 0x20, 0x20], // "NTFS    "
        min_total_size: 512,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Fat,
        anchor: SniffAnchor::Head(3),
        magic: &[0x45, 0x58, 0x46, 0x41, 0x54, 0x20, 0x20, 0x20], // "EXFAT   "
        min_total_size: 512,
        confidence: 95,
    },

    // --- 3. Sector Anchored & Tail Formats ---
    SniffRule {
        format: ArchiveFormat::Iso,
        anchor: SniffAnchor::Head(0x8001), // Sector 16 + 1 (32769)
        magic: &[0x43, 0x44, 0x30, 0x30, 0x31], // "CD001"
        min_total_size: 32768 + 2048,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Udf,
        anchor: SniffAnchor::Head(0x8001), // Sector 16 + 1 (32769)
        magic: &[0x42, 0x45, 0x41, 0x30, 0x31], // "BEA01"
        min_total_size: 32768 + 2048,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Udf,
        anchor: SniffAnchor::Head(0x9001), // Sector 18 + 1 (36865)
        magic: &[0x4E, 0x53, 0x52, 0x30, 0x32], // "NSR02"
        min_total_size: 36870,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Udf,
        anchor: SniffAnchor::Head(0x9001), // Sector 18 + 1 (36865)
        magic: &[0x4E, 0x53, 0x52, 0x30, 0x33], // "NSR03"
        min_total_size: 36870,
        confidence: 95,
    },
    SniffRule {
        format: ArchiveFormat::Dmg,
        anchor: SniffAnchor::Tail(512),
        magic: &[0x6B, 0x6F, 0x6C, 0x79], // "koly"
        min_total_size: 512,
        confidence: 100,
    },
    SniffRule {
        format: ArchiveFormat::Vhd,
        anchor: SniffAnchor::Tail(512),
        magic: &[0x63, 0x6F, 0x6E, 0x65, 0x63, 0x74, 0x69, 0x78], // "conectix"
        min_total_size: 512,
        confidence: 95,
    },
];
