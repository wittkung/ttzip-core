// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Multi-file archive, container, disk image, and package format bidders.

use super::{ArchiveFormatBidder, BidScore, SeekableStream};
use crate::archive::unified::format_sniffer::formats::ArchiveFormat;
use std::io::SeekFrom;

// ---------------------------------------------------------------------------
// 1. CPIO Bidder
// ---------------------------------------------------------------------------

/// Bidder for CPIO archives (SVR4, newc, crc, odc, binary LE/BE).
pub struct CpioBidder;

impl ArchiveFormatBidder for CpioBidder {
    fn format_name(&self) -> &'static str {
        "cpio"
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Cpio
    }

    fn max_score(&self) -> BidScore {
        BidScore::CPIO
    }

    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::CPIO || buffer.len() < 2 {
            return BidScore::NONE;
        }

        if buffer.starts_with(b"070701")
            || buffer.starts_with(b"070702")
            || buffer.starts_with(b"070707")
            || buffer.starts_with(b"070700")
            || buffer.starts_with(b"\xC7\x71")
            || buffer.starts_with(b"\x71\xC7")
        {
            BidScore::CPIO
        } else {
            BidScore::NONE
        }
    }
}

// ---------------------------------------------------------------------------
// 2. ISO 9660 & UDF Bidder
// ---------------------------------------------------------------------------

/// Bidder for ISO 9660 / UDF optical disc images.
pub struct Iso9660Bidder;

impl ArchiveFormatBidder for Iso9660Bidder {
    fn format_name(&self) -> &'static str {
        "iso9660"
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Iso
    }

    fn max_score(&self) -> BidScore {
        BidScore::ISO9660
    }

    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::ISO9660 {
            return BidScore::NONE;
        }

        const ISO_SECTOR_16: usize = 32768;
        if buffer.len() >= ISO_SECTOR_16 + 6 {
            let magic = &buffer[ISO_SECTOR_16 + 1..ISO_SECTOR_16 + 6];
            if magic == b"CD001" || magic == b"BEA01" || magic == b"CDROM" {
                return BidScore::ISO9660;
            }
        }

        BidScore::NONE
    }

    fn probe_seekable(&self, reader: &mut dyn SeekableStream, best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::ISO9660 {
            return BidScore::NONE;
        }

        const ISO_SECTOR_16: usize = 32768;
        if reader.seek(SeekFrom::Start(ISO_SECTOR_16 as u64)).is_ok() {
            let mut buf = [0u8; 6];
            if reader.read_exact(&mut buf).is_ok() {
                let magic = &buf[1..6];
                if magic == b"CD001" || magic == b"BEA01" || magic == b"CDROM" {
                    return BidScore::ISO9660;
                }
            }
        }

        BidScore::NONE
    }
}

// ---------------------------------------------------------------------------
// 3. ZIP Bidder (Streamable vs Seekable)
// ---------------------------------------------------------------------------

/// Bidder for ZIP and ZIP64 archives.
pub struct ZipBidder;

impl ArchiveFormatBidder for ZipBidder {
    fn format_name(&self) -> &'static str {
        "zip"
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Zip
    }

    fn max_score(&self) -> BidScore {
        BidScore::ZIP_SEEKABLE
    }

    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::ZIP_SEEKABLE {
            return BidScore::NONE;
        }

        if buffer.starts_with(b"PK\x05\x06") {
            BidScore::ZIP_SEEKABLE
        } else if buffer.starts_with(b"PK\x03\x04") {
            BidScore::ZIP_STREAMABLE
        } else if buffer.starts_with(b"PK\x07\x08") {
            BidScore::new(28)
        } else {
            BidScore::NONE
        }
    }

    fn probe_seekable(&self, reader: &mut dyn SeekableStream, best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::ZIP_SEEKABLE {
            return BidScore::NONE;
        }

        let file_len = match reader.seek(SeekFrom::End(0)) {
            Ok(len) => len,
            Err(_) => return BidScore::NONE,
        };

        if file_len < 22 {
            return BidScore::NONE;
        }

        let read_len = (file_len.min(65557)) as usize;
        if reader.seek(SeekFrom::End(-(read_len as i64))).is_err() {
            return BidScore::NONE;
        }

        let mut buf = vec![0u8; read_len];
        if reader.read_exact(&mut buf).is_err() {
            return BidScore::NONE;
        }

        for i in (0..=read_len.saturating_sub(22)).rev() {
            if &buf[i..i + 4] == b"PK\x05\x06" {
                return BidScore::ZIP_SEEKABLE;
            }
        }

        BidScore::NONE
    }
}

// ---------------------------------------------------------------------------
// 4. 7-Zip Bidder
// ---------------------------------------------------------------------------

/// Bidder for 7-Zip archives.
pub struct SevenZipBidder;

impl ArchiveFormatBidder for SevenZipBidder {
    fn format_name(&self) -> &'static str {
        "7z"
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::SevenZip
    }

    fn max_score(&self) -> BidScore {
        BidScore::SEVEN_ZIP
    }

    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::SEVEN_ZIP {
            return BidScore::NONE;
        }

        if buffer.starts_with(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
            BidScore::SEVEN_ZIP
        } else {
            BidScore::NONE
        }
    }
}

// ---------------------------------------------------------------------------
// 5. Microsoft Cabinet Bidder
// ---------------------------------------------------------------------------

/// Bidder for Microsoft Cabinet archives.
pub struct CabBidder;

impl ArchiveFormatBidder for CabBidder {
    fn format_name(&self) -> &'static str {
        "cab"
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Cab
    }

    fn max_score(&self) -> BidScore {
        BidScore::CAB
    }

    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::CAB {
            return BidScore::NONE;
        }

        if buffer.starts_with(b"MSCF") {
            BidScore::CAB
        } else {
            BidScore::NONE
        }
    }
}

// ---------------------------------------------------------------------------
// 6. RAR v4 & v5 Bidders
// ---------------------------------------------------------------------------

/// Bidder for RAR v4 archives.
pub struct Rar4Bidder;

impl ArchiveFormatBidder for Rar4Bidder {
    fn format_name(&self) -> &'static str {
        "rar4"
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Rar4
    }

    fn max_score(&self) -> BidScore {
        BidScore::RAR4
    }

    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::RAR4 {
            return BidScore::NONE;
        }

        if buffer.starts_with(&[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00]) {
            BidScore::RAR4
        } else {
            BidScore::NONE
        }
    }
}

/// Bidder for RAR v5 archives.
pub struct Rar5Bidder;

impl ArchiveFormatBidder for Rar5Bidder {
    fn format_name(&self) -> &'static str {
        "rar5"
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Rar5
    }

    fn max_score(&self) -> BidScore {
        BidScore::RAR5
    }

    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::RAR5 {
            return BidScore::NONE;
        }

        if buffer.starts_with(&[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00]) {
            BidScore::RAR5
        } else {
            BidScore::NONE
        }
    }
}

// ---------------------------------------------------------------------------
// 7. WARC Bidder
// ---------------------------------------------------------------------------

/// Bidder for Web ARChive (WARC) files.
pub struct WarcBidder;

impl ArchiveFormatBidder for WarcBidder {
    fn format_name(&self) -> &'static str {
        "warc"
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Warc
    }

    fn max_score(&self) -> BidScore {
        BidScore::WARC
    }

    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::WARC {
            return BidScore::NONE;
        }

        if buffer.starts_with(b"WARC/") {
            BidScore::WARC
        } else {
            BidScore::NONE
        }
    }
}

// ---------------------------------------------------------------------------
// 8. AR & XAR Bidders
// ---------------------------------------------------------------------------

/// Bidder for Unix AR and Debian (.deb) packages.
pub struct ArBidder;

impl ArchiveFormatBidder for ArBidder {
    fn format_name(&self) -> &'static str {
        "ar"
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Ar
    }

    fn max_score(&self) -> BidScore {
        BidScore::AR
    }

    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::AR {
            return BidScore::NONE;
        }

        if buffer.starts_with(b"!<arch>\n") {
            BidScore::AR
        } else {
            BidScore::NONE
        }
    }
}

/// Bidder for eXtensible ARchive (XAR) format.
pub struct XarBidder;

impl ArchiveFormatBidder for XarBidder {
    fn format_name(&self) -> &'static str {
        "xar"
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Xar
    }

    fn max_score(&self) -> BidScore {
        BidScore::XAR
    }

    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::XAR {
            return BidScore::NONE;
        }

        if buffer.starts_with(b"xar!") {
            BidScore::XAR
        } else {
            BidScore::NONE
        }
    }
}

// ---------------------------------------------------------------------------
// 9. LHA & Mtree Bidders
// ---------------------------------------------------------------------------

/// Bidder for LHA / LZH archives.
pub struct LhaBidder;

impl ArchiveFormatBidder for LhaBidder {
    fn format_name(&self) -> &'static str {
        "lha"
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Lzh
    }

    fn max_score(&self) -> BidScore {
        BidScore::LHA
    }

    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::LHA || buffer.len() < 7 {
            return BidScore::NONE;
        }

        if (buffer[2..5] == *b"-lh" || buffer[2..5] == *b"-lz") && buffer[6] == b'-' {
            BidScore::LHA
        } else {
            BidScore::NONE
        }
    }
}

/// Bidder for BSD mtree manifest files.
pub struct MtreeBidder;

impl ArchiveFormatBidder for MtreeBidder {
    fn format_name(&self) -> &'static str {
        "mtree"
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Mtree
    }

    fn max_score(&self) -> BidScore {
        BidScore::MTREE
    }

    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::MTREE {
            return BidScore::NONE;
        }

        if buffer.starts_with(b"#mtree")
            || buffer.starts_with(b"# Manifest")
            || buffer.starts_with(b"/set ")
        {
            BidScore::MTREE
        } else {
            BidScore::NONE
        }
    }
}

// ---------------------------------------------------------------------------
// 10. WIM & DMG Bidders
// ---------------------------------------------------------------------------

/// Bidder for Windows Imaging (WIM) format.
pub struct WimBidder;

impl ArchiveFormatBidder for WimBidder {
    fn format_name(&self) -> &'static str {
        "wim"
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Wim
    }

    fn max_score(&self) -> BidScore {
        BidScore::WIM
    }

    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far < BidScore::WIM && buffer.starts_with(b"MSWIM\0\0\0") {
            BidScore::WIM
        } else {
            BidScore::NONE
        }
    }
}

/// Bidder for Apple Disk Image (DMG) via seekable trailer inspection.
pub struct DmgBidder;

impl ArchiveFormatBidder for DmgBidder {
    fn format_name(&self) -> &'static str {
        "dmg"
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Dmg
    }

    fn max_score(&self) -> BidScore {
        BidScore::DMG
    }

    fn probe_head(&self, _buffer: &[u8], _best_so_far: BidScore) -> BidScore {
        BidScore::NONE
    }

    fn probe_seekable(&self, reader: &mut dyn SeekableStream, best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::DMG {
            return BidScore::NONE;
        }
        if reader.seek(SeekFrom::End(-512)).is_ok() {
            let mut buf = [0u8; 4];
            if reader.read_exact(&mut buf).is_ok() && &buf == b"koly" {
                return BidScore::DMG;
            }
        }
        BidScore::NONE
    }
}
