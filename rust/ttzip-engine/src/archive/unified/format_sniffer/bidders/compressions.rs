// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Single-stream compression and raw fallback format bidders.

use super::{ArchiveFormatBidder, BidScore};
use crate::archive::unified::format_sniffer::formats::ArchiveFormat;

/// Gzip (`.gz`, `.tgz`) bidder (`\x1F\x8B`).
pub struct GzipBidder;
impl ArchiveFormatBidder for GzipBidder {
    fn format_name(&self) -> &'static str { "gzip" }
    fn format(&self) -> ArchiveFormat { ArchiveFormat::Gzip }
    fn max_score(&self) -> BidScore { BidScore::GZIP }
    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::GZIP || buffer.len() < 2 {
            return BidScore::NONE;
        }
        if buffer.starts_with(b"\x1F\x8B") {
            BidScore::GZIP
        } else {
            BidScore::NONE
        }
    }
}

/// Bzip2 (`.bz2`, `.tbz2`) bidder (`BZh`).
pub struct Bzip2Bidder;
impl ArchiveFormatBidder for Bzip2Bidder {
    fn format_name(&self) -> &'static str { "bzip2" }
    fn format(&self) -> ArchiveFormat { ArchiveFormat::Bzip2 }
    fn max_score(&self) -> BidScore { BidScore::BZIP2 }
    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::BZIP2 || buffer.len() < 3 {
            return BidScore::NONE;
        }
        if buffer.starts_with(b"BZh") {
            BidScore::BZIP2
        } else {
            BidScore::NONE
        }
    }
}

/// XZ (`.xz`, `.txz`) bidder (`\xFD7zXZ\x00`).
pub struct XzBidder;
impl ArchiveFormatBidder for XzBidder {
    fn format_name(&self) -> &'static str { "xz" }
    fn format(&self) -> ArchiveFormat { ArchiveFormat::Xz }
    fn max_score(&self) -> BidScore { BidScore::XZ }
    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::XZ || buffer.len() < 6 {
            return BidScore::NONE;
        }
        if buffer.starts_with(b"\xFD7zXZ\x00") {
            BidScore::XZ
        } else {
            BidScore::NONE
        }
    }
}

/// Zstandard (`.zst`, `.tzst`) bidder (`\x28\xB5\x2F\xFD`).
pub struct ZstdBidder;
impl ArchiveFormatBidder for ZstdBidder {
    fn format_name(&self) -> &'static str { "zstd" }
    fn format(&self) -> ArchiveFormat { ArchiveFormat::Zstd }
    fn max_score(&self) -> BidScore { BidScore::ZSTD }
    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::ZSTD || buffer.len() < 4 {
            return BidScore::NONE;
        }
        if buffer.starts_with(b"\x28\xB5\x2F\xFD") {
            BidScore::ZSTD
        } else {
            BidScore::NONE
        }
    }
}

/// LZ4 (`.lz4`, `.tlz4`) bidder (`\x04\x22\x4D\x18` or skippable frame).
pub struct Lz4Bidder;
impl ArchiveFormatBidder for Lz4Bidder {
    fn format_name(&self) -> &'static str { "lz4" }
    fn format(&self) -> ArchiveFormat { ArchiveFormat::Lz4 }
    fn max_score(&self) -> BidScore { BidScore::new(48) }
    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::new(48) || buffer.len() < 4 {
            return BidScore::NONE;
        }
        if buffer.starts_with(b"\x04\x22\x4D\x18") || buffer.starts_with(b"\x02\x21\x4C\x18") {
            BidScore::new(48)
        } else {
            BidScore::NONE
        }
    }
}

/// Snappy (`.sz`, `.snappy`) bidder (`\xFF\x06\x00\x00sNaPpY`).
pub struct SnappyBidder;
impl ArchiveFormatBidder for SnappyBidder {
    fn format_name(&self) -> &'static str { "snappy" }
    fn format(&self) -> ArchiveFormat { ArchiveFormat::Snappy }
    fn max_score(&self) -> BidScore { BidScore::new(48) }
    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::new(48) || buffer.len() < 10 {
            return BidScore::NONE;
        }
        if buffer.starts_with(b"\xFF\x06\x00\x00sNaPpY") {
            BidScore::new(48)
        } else {
            BidScore::NONE
        }
    }
}

/// LZFSE (`.lzfse`) bidder (`bvx-` or `bvx1` or `bvx2` or `bvxn`).
pub struct LzfseBidder;
impl ArchiveFormatBidder for LzfseBidder {
    fn format_name(&self) -> &'static str { "lzfse" }
    fn format(&self) -> ArchiveFormat { ArchiveFormat::Lzfse }
    fn max_score(&self) -> BidScore { BidScore::new(48) }
    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::new(48) || buffer.len() < 4 {
            return BidScore::NONE;
        }
        if buffer.starts_with(b"bvx-")
            || buffer.starts_with(b"bvx1")
            || buffer.starts_with(b"bvx2")
            || buffer.starts_with(b"bvxn")
        {
            BidScore::new(48)
        } else {
            BidScore::NONE
        }
    }
}

/// Brotli (`.br`, `.tbr`) bidder.
pub struct BrotliBidder;
impl ArchiveFormatBidder for BrotliBidder {
    fn format_name(&self) -> &'static str { "brotli" }
    fn format(&self) -> ArchiveFormat { ArchiveFormat::Brotli }
    fn max_score(&self) -> BidScore { BidScore::new(30) }
    fn probe_head(&self, _buffer: &[u8], _best_so_far: BidScore) -> BidScore {
        BidScore::NONE
    }
}

/// Lzip (`.lz`, `.tlz`) bidder (`LZIP`).
pub struct LzipBidder;
impl ArchiveFormatBidder for LzipBidder {
    fn format_name(&self) -> &'static str { "lzip" }
    fn format(&self) -> ArchiveFormat { ArchiveFormat::Lzip }
    fn max_score(&self) -> BidScore { BidScore::new(48) }
    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::new(48) || buffer.len() < 4 {
            return BidScore::NONE;
        }
        if buffer.starts_with(b"LZIP") {
            BidScore::new(48)
        } else {
            BidScore::NONE
        }
    }
}

/// Squashfs (`.sqsh`, `.squashfs`) bidder (`hsqs`, `sqsh`).
pub struct SquashfsBidder;
impl ArchiveFormatBidder for SquashfsBidder {
    fn format_name(&self) -> &'static str { "squashfs" }
    fn format(&self) -> ArchiveFormat { ArchiveFormat::Squashfs }
    fn max_score(&self) -> BidScore { BidScore::SQUASHFS }
    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::SQUASHFS || buffer.len() < 4 {
            return BidScore::NONE;
        }
        if buffer.starts_with(b"hsqs") || buffer.starts_with(b"sqsh") || buffer.starts_with(b"shsq") {
            BidScore::SQUASHFS
        } else {
            BidScore::NONE
        }
    }
}

/// ARJ (`.arj`) bidder (`\x60\xEA`).
pub struct ArjBidder;
impl ArchiveFormatBidder for ArjBidder {
    fn format_name(&self) -> &'static str { "arj" }
    fn format(&self) -> ArchiveFormat { ArchiveFormat::Arj }
    fn max_score(&self) -> BidScore { BidScore::ARJ }
    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::ARJ || buffer.len() < 2 {
            return BidScore::NONE;
        }
        if buffer.starts_with(b"\x60\xEA") {
            BidScore::ARJ
        } else {
            BidScore::NONE
        }
    }
}

/// Apple Archive (`.aar`, `.aea`) bidder (`AA01` or `AEA1`).
pub struct AarBidder;
impl ArchiveFormatBidder for AarBidder {
    fn format_name(&self) -> &'static str { "aar" }
    fn format(&self) -> ArchiveFormat { ArchiveFormat::Aar }
    fn max_score(&self) -> BidScore { BidScore::AAR }
    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::AAR || buffer.len() < 4 {
            return BidScore::NONE;
        }
        if buffer.starts_with(b"AA01") || buffer.starts_with(b"AEA1") || buffer.starts_with(b"YAA1") {
            BidScore::AAR
        } else {
            BidScore::NONE
        }
    }
}

/// Empty buffer bidder (all-zero buffer).
pub struct EmptyBidder;
impl ArchiveFormatBidder for EmptyBidder {
    fn format_name(&self) -> &'static str { "empty" }
    fn format(&self) -> ArchiveFormat { ArchiveFormat::Empty }
    fn max_score(&self) -> BidScore { BidScore::EMPTY }
    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::EMPTY {
            return BidScore::NONE;
        }
        if buffer.is_empty() || buffer.iter().all(|&b| b == 0) {
            BidScore::EMPTY
        } else {
            BidScore::NONE
        }
    }
}

/// Generic raw pass-through fallback bidder.
pub struct RawBidder;
impl ArchiveFormatBidder for RawBidder {
    fn format_name(&self) -> &'static str { "raw" }
    fn format(&self) -> ArchiveFormat { ArchiveFormat::Raw }
    fn max_score(&self) -> BidScore { BidScore::FALLBACK }
    fn probe_head(&self, buffer: &[u8], _best_so_far: BidScore) -> BidScore {
        if !buffer.is_empty() {
            BidScore::FALLBACK
        } else {
            BidScore::NONE
        }
    }
}
