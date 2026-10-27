// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Format signature registry for 16 primary container and compression formats.

use super::anchors::Anchor;
pub use ttzip_types::{CompoundFormat, DetectedFormat};

/// Magic signature entry in static lookup table.
#[derive(Debug, Clone, Copy)]
pub struct SignatureEntry {
    pub format: DetectedFormat,
    pub anchor: Anchor,
    pub magic: &'static [u8],
    pub min_total_size: usize,
    pub description: &'static str,
    pub priority: u32,
}

/// Static prioritized signature table covering 16 primary formats.
pub static PRIORITIZED_SIGNATURES: &[SignatureEntry] = &[
    // 1. 7-Zip Container (High priority, exact 6-byte match)
    SignatureEntry {
        format: DetectedFormat::SevenZip,
        anchor: Anchor::Head(0),
        magic: &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C],
        min_total_size: 6,
        description: "7-Zip Archive",
        priority: 100,
    },
    // 2. XZ Compression Container (6 bytes)
    SignatureEntry {
        format: DetectedFormat::Xz,
        anchor: Anchor::Head(0),
        magic: &[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00],
        min_total_size: 6,
        description: "XZ Compressed Stream",
        priority: 95,
    },
    // 3. RAR5 Archive
    SignatureEntry {
        format: DetectedFormat::Rar,
        anchor: Anchor::Head(0),
        magic: &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00],
        min_total_size: 8,
        description: "RAR5 Archive",
        priority: 90,
    },
    // 4. RAR4 Archive
    SignatureEntry {
        format: DetectedFormat::Rar,
        anchor: Anchor::Head(0),
        magic: &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00],
        min_total_size: 7,
        description: "RAR4 Archive",
        priority: 89,
    },
    // 5. Zstandard Compression Frame
    SignatureEntry {
        format: DetectedFormat::Zstd,
        anchor: Anchor::Head(0),
        magic: &[0x28, 0xB5, 0x2F, 0xFD],
        min_total_size: 4,
        description: "Zstandard Compressed Stream",
        priority: 85,
    },
    // 6. XAR Archive (e.g. macOS pkg)
    SignatureEntry {
        format: DetectedFormat::Xar,
        anchor: Anchor::Head(0),
        magic: &[0x78, 0x61, 0x72, 0x21], // "xar!"
        min_total_size: 4,
        description: "eXtensible ARchive (XAR)",
        priority: 80,
    },
    // 6b. Microsoft WIM Container
    SignatureEntry {
        format: DetectedFormat::Wim,
        anchor: Anchor::Head(0),
        magic: &[0x4D, 0x53, 0x57, 0x49, 0x4D, 0x00, 0x00, 0x00], // "MSWIM\0\0\0"
        min_total_size: 208,
        description: "Microsoft Windows Imaging Format",
        priority: 76,
    },
    // 7. Microsoft CAB Container
    SignatureEntry {
        format: DetectedFormat::Cab,
        anchor: Anchor::Head(0),
        magic: &[0x4D, 0x53, 0x43, 0x46], // "MSCF"
        min_total_size: 4,
        description: "Microsoft Cabinet Archive",
        priority: 75,
    },
    // 8. Unix AR / Debian package
    SignatureEntry {
        format: DetectedFormat::Ar,
        anchor: Anchor::Head(0),
        magic: &[0x21, 0x3C, 0x61, 0x72, 0x63, 0x68, 0x3E, 0x0A], // "!<arch>\n"
        min_total_size: 8,
        description: "UNIX Archive / Debian Package",
        priority: 70,
    },
    // 8b. Red Hat Package Manager (RPM)
    SignatureEntry {
        format: DetectedFormat::Rpm,
        anchor: Anchor::Head(0),
        magic: &[0xED, 0xAB, 0xEE, 0xDB],
        min_total_size: 4,
        description: "RPM Package Archive",
        priority: 69,
    },
    // 8c. Squashfs Archive (Little Endian)
    SignatureEntry {
        format: DetectedFormat::Squashfs,
        anchor: Anchor::Head(0),
        magic: &[0x68, 0x73, 0x71, 0x73], // "hsqs"
        min_total_size: 4,
        description: "Squashfs Compressed Filesystem (LE)",
        priority: 68,
    },
    // 8d. Squashfs Archive (Big Endian)
    SignatureEntry {
        format: DetectedFormat::Squashfs,
        anchor: Anchor::Head(0),
        magic: &[0x73, 0x71, 0x73, 0x68], // "sqsh"
        min_total_size: 4,
        description: "Squashfs Compressed Filesystem (BE)",
        priority: 68,
    },
    // 8e. CPIO Archive (SVR4 newc)
    SignatureEntry {
        format: DetectedFormat::Cpio,
        anchor: Anchor::Head(0),
        magic: &[0x30, 0x37, 0x30, 0x37, 0x30, 0x31], // "070701"
        min_total_size: 6,
        description: "CPIO Archive (SVR4 newc)",
        priority: 67,
    },
    // 8f. CPIO Archive (SVR4 with CRC)
    SignatureEntry {
        format: DetectedFormat::Cpio,
        anchor: Anchor::Head(0),
        magic: &[0x30, 0x37, 0x30, 0x37, 0x30, 0x32], // "070702"
        min_total_size: 6,
        description: "CPIO Archive (SVR4 crc)",
        priority: 67,
    },
    // 8g. CPIO Archive (POSIX odc)
    SignatureEntry {
        format: DetectedFormat::Cpio,
        anchor: Anchor::Head(0),
        magic: &[0x30, 0x37, 0x30, 0x37, 0x30, 0x37], // "070707"
        min_total_size: 6,
        description: "CPIO Archive (POSIX odc)",
        priority: 66,
    },
    // 8h. CPIO Archive (Binary LE)
    SignatureEntry {
        format: DetectedFormat::Cpio,
        anchor: Anchor::Head(0),
        magic: &[0xC7, 0x71],
        min_total_size: 2,
        description: "CPIO Archive (Binary LE)",
        priority: 65,
    },
    // 8i. CPIO Archive (Binary BE)
    SignatureEntry {
        format: DetectedFormat::Cpio,
        anchor: Anchor::Head(0),
        magic: &[0x71, 0xC7],
        min_total_size: 2,
        description: "CPIO Archive (Binary BE)",
        priority: 65,
    },
    // 9. Snappy Framed Stream
    SignatureEntry {
        format: DetectedFormat::Snappy,
        anchor: Anchor::Head(0),
        magic: &[0xFF, 0x06, 0x00, 0x00, 0x73, 0x4E, 0x61, 0x50, 0x70, 0x59], // "\xFF\x06\x00\x00sNaPpY"
        min_total_size: 10,
        description: "Snappy Framed Stream",
        priority: 65,
    },
    // 9b. Lzip Compressed Stream
    SignatureEntry {
        format: DetectedFormat::Lzip,
        anchor: Anchor::Head(0),
        magic: &[0x4C, 0x5A, 0x49, 0x50], // "LZIP"
        min_total_size: 6,
        description: "Lzip Compressed Stream",
        priority: 64,
    },
    // 9c. Long Range ZIP (LRZIP) Stream
    SignatureEntry {
        format: DetectedFormat::Lrzip,
        anchor: Anchor::Head(0),
        magic: &[0x4C, 0x52, 0x5A, 0x49], // "LRZI"
        min_total_size: 6,
        description: "Long Range ZIP (LRZIP) Stream",
        priority: 63,
    },
    // 9d. Apple Archive (AAR)
    SignatureEntry {
        format: DetectedFormat::Aar,
        anchor: Anchor::Head(0),
        magic: &[0x41, 0x41, 0x30, 0x31], // "AA01"
        min_total_size: 4,
        description: "Apple Archive (AAR)",
        priority: 62,
    },
    // 9e. Apple Encrypted Archive (AEA)
    SignatureEntry {
        format: DetectedFormat::Aar,
        anchor: Anchor::Head(0),
        magic: &[0x41, 0x45, 0x41, 0x31], // "AEA1"
        min_total_size: 4,
        description: "Apple Encrypted Archive (AEA)",
        priority: 61,
    },
    // 10. LZ4 Framed Stream
    SignatureEntry {
        format: DetectedFormat::Lz4,
        anchor: Anchor::Head(0),
        magic: &[0x04, 0x22, 0x4D, 0x18],
        min_total_size: 4,
        description: "LZ4 Framed Stream",
        priority: 60,
    },
    // 11. Apple LZFSE Stream
    SignatureEntry {
        format: DetectedFormat::Lzfse,
        anchor: Anchor::Head(0),
        magic: &[0x62, 0x76, 0x78, 0x2D], // "bvx-"
        min_total_size: 4,
        description: "Apple LZFSE Compressed Stream",
        priority: 55,
    },
    // 12. ZIP Container Local File Header
    SignatureEntry {
        format: DetectedFormat::Zip,
        anchor: Anchor::Head(0),
        magic: &[0x50, 0x4B, 0x03, 0x04], // "PK\x03\x04"
        min_total_size: 4,
        description: "ZIP Archive (Local File Header)",
        priority: 50,
    },
    // 13. ZIP Empty Archive (EOCD only)
    SignatureEntry {
        format: DetectedFormat::Zip,
        anchor: Anchor::Head(0),
        magic: &[0x50, 0x4B, 0x05, 0x06], // "PK\x05\x06"
        min_total_size: 4,
        description: "ZIP Empty Archive (EOCD)",
        priority: 49,
    },
    // 14. GZIP Container
    SignatureEntry {
        format: DetectedFormat::Gzip,
        anchor: Anchor::Head(0),
        magic: &[0x1F, 0x8B],
        min_total_size: 2,
        description: "GZIP Compressed Stream",
        priority: 45,
    },
    // 15. BZIP2 Container
    SignatureEntry {
        format: DetectedFormat::Bzip2,
        anchor: Anchor::Head(0),
        magic: &[0x42, 0x5A, 0x68], // "BZh"
        min_total_size: 3,
        description: "BZIP2 Compressed Stream",
        priority: 40,
    },
    // 16. POSIX.1 ustar TAR (ustar\0)
    SignatureEntry {
        format: DetectedFormat::Tar,
        anchor: Anchor::TarOffset(257),
        magic: &[0x75, 0x73, 0x74, 0x61, 0x72, 0x00], // "ustar\0"
        min_total_size: 512,
        description: "POSIX.1 ustar Archive",
        priority: 35,
    },
    // 17. GNU TAR (ustar  \0)
    SignatureEntry {
        format: DetectedFormat::Tar,
        anchor: Anchor::TarOffset(257),
        magic: &[0x75, 0x73, 0x74, 0x61, 0x72, 0x20, 0x20, 0x00], // "ustar  \0"
        min_total_size: 512,
        description: "GNU tar Archive",
        priority: 34,
    },
    // 18. ISO 9660 Volume Descriptor at Sector 16 (offset 32769 = 0x8001)
    SignatureEntry {
        format: DetectedFormat::Iso,
        anchor: Anchor::Head(0x8001),
        magic: &[0x43, 0x44, 0x30, 0x30, 0x31], // "CD001"
        min_total_size: 32768 + 2048,
        description: "ISO 9660 Disk Image",
        priority: 30,
    },
    // 19. Apple DMG UDIF koly trailer (Tail 512)
    SignatureEntry {
        format: DetectedFormat::Dmg,
        anchor: Anchor::Tail(512),
        magic: &[0x6B, 0x6F, 0x6C, 0x79], // "koly"
        min_total_size: 512,
        description: "Apple Disk Image (UDIF DMG)",
        priority: 25,
    },
    // 20. LHA / LZH Archive (-lh5-)
    SignatureEntry {
        format: DetectedFormat::Lzh,
        anchor: Anchor::Head(2),
        magic: &[0x2D, 0x6C, 0x68], // "-lh"
        min_total_size: 24,
        description: "LHA/LZH Archive",
        priority: 20,
    },
];
