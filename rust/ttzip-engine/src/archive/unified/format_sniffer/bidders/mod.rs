// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Strongly typed format bidding scores, `ArchiveFormatBidder` Trait,
//! and domain-separated format bidder modules inspired by libarchive.

pub mod compressions;
pub mod containers;
pub mod tar;

pub use compressions::{
    AarBidder, ArjBidder, BrotliBidder, Bzip2Bidder, EmptyBidder, GzipBidder, Lz4Bidder,
    LzfseBidder, LzipBidder, RawBidder, SnappyBidder, SquashfsBidder, XzBidder, ZstdBidder,
};
pub use containers::{
    ArBidder, CabBidder, CpioBidder, DmgBidder, Iso9660Bidder, LhaBidder, MtreeBidder, Rar4Bidder,
    Rar5Bidder, SevenZipBidder, WarcBidder, WimBidder, XarBidder, ZipBidder,
};
pub use tar::TarBidder;

use super::formats::ArchiveFormat;
use std::io::{Read, Seek};

/// Seekable stream trait alias for random-access format inspection.
pub trait SeekableStream: Read + Seek {}
impl<T: Read + Seek + ?Sized> SeekableStream for T {}

/// Strongly typed bidding score representing confidence in a format match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct BidScore(pub u16);

impl BidScore {
    /// No match (0 score).
    pub const NONE: BidScore = BidScore(0);
    /// Generic raw stream / pass-through fallback (1 score).
    pub const FALLBACK: BidScore = BidScore(1);
    /// All-zero / empty buffer fallback (10 score).
    pub const EMPTY: BidScore = BidScore(10);
    /// Streamable ZIP local file header (`PK\x03\x04`) match (29 score).
    pub const ZIP_STREAMABLE: BidScore = BidScore(29);
    /// LHA / LZH compression header match (30 score).
    pub const LHA: BidScore = BidScore(30);
    /// Mtree specification / manifest header match (32 score).
    pub const MTREE: BidScore = BidScore(32);
    /// Confirmed seekable ZIP central directory / EOCD match (32 score).
    pub const ZIP_SEEKABLE: BidScore = BidScore(32);
    /// CPIO archive header match (48 score).
    pub const CPIO: BidScore = BidScore(48);
    /// ISO 9660 / UDF optical disc volume descriptor match (48 score).
    pub const ISO9660: BidScore = BidScore(48);
    /// 7-Zip container header match (48 score).
    pub const SEVEN_ZIP: BidScore = BidScore(48);
    /// Gzip compressed stream header match (48 score).
    pub const GZIP: BidScore = BidScore(48);
    /// Bzip2 compressed stream header match (48 score).
    pub const BZIP2: BidScore = BidScore(48);
    /// XZ compressed stream header match (48 score).
    pub const XZ: BidScore = BidScore(48);
    /// Zstandard compressed stream header match (48 score).
    pub const ZSTD: BidScore = BidScore(48);
    /// ARJ archive header match (48 score).
    pub const ARJ: BidScore = BidScore(48);
    /// Legacy Unix V7 TAR format with verified octal checksum (60 score).
    pub const TAR_V7: BidScore = BidScore(60);
    /// Unix AR / Debian binary package match (64 score).
    pub const AR: BidScore = BidScore(64);
    /// Microsoft Cabinet archive header match (64 score).
    pub const CAB: BidScore = BidScore(64);
    /// RAR v4 archive header match (64 score).
    pub const RAR4: BidScore = BidScore(64);
    /// RAR v5 archive header match (64 score).
    pub const RAR5: BidScore = BidScore(64);
    /// Web ARChive (WARC) header match (64 score).
    pub const WARC: BidScore = BidScore(64);
    /// Windows Imaging (WIM) header match (64 score).
    pub const WIM: BidScore = BidScore(64);
    /// SquashFS compressed filesystem image match (64 score).
    pub const SQUASHFS: BidScore = BidScore(64);
    /// Apple Archive (AAR / AEA) match (64 score).
    pub const AAR: BidScore = BidScore(64);
    /// Apple Disk Image (DMG) confirmed koly trailer match (64 score).
    pub const DMG: BidScore = BidScore(64);
    /// eXtensible ARchive (XAR) header match (96 score).
    pub const XAR: BidScore = BidScore(96);
    /// Definitive match threshold (100 score).
    pub const DEFINITIVE: BidScore = BidScore(100);
    /// POSIX USTAR standard TAR header match (106 score).
    pub const TAR_USTAR: BidScore = BidScore(106);
    /// GNU standard TAR header match (106 score).
    pub const TAR_GNU: BidScore = BidScore(106);
    /// POSIX PAX extended TAR header match (106 score).
    pub const TAR_PAX: BidScore = BidScore(106);

    /// Constructs a new BidScore from numerical score value.
    #[inline]
    #[must_use]
    pub const fn new(score: u16) -> Self {
        Self(score)
    }

    /// Returns the inner numerical score value.
    #[inline]
    #[must_use]
    pub const fn score(self) -> u16 {
        self.0
    }

    /// Returns `true` if the score is greater than zero.
    #[inline]
    #[must_use]
    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }
}

/// Interface for archive format candidate evaluation.
pub trait ArchiveFormatBidder: Send + Sync {
    /// Format identifier name (e.g. "tar", "zip", "7z").
    fn format_name(&self) -> &'static str;

    /// Corresponding canonical `ArchiveFormat` variant.
    fn format(&self) -> ArchiveFormat;

    /// Theoretical maximum score this bidder can award (used for short-circuiting).
    fn max_score(&self) -> BidScore;

    /// Evaluates input buffer lookahead window and returns a bidding score.
    ///
    /// If `self.max_score() <= best_so_far`, the implementation may short-circuit immediately.
    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore;

    /// Evaluates seekable stream (e.g. backward trailer scans or sector lookahead).
    /// Defaults to `BidScore::NONE`.
    fn probe_seekable(&self, _reader: &mut dyn SeekableStream, _best_so_far: BidScore) -> BidScore {
        BidScore::NONE
    }
}
