// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unified FormatBidderRegistry dispatcher with cheap-first arbitration ordering and short-circuit optimization.

use super::bidders::{
    AarBidder, ArchiveFormatBidder, ArBidder, ArjBidder, BidScore, BrotliBidder, Bzip2Bidder,
    CabBidder, CpioBidder, DmgBidder, EmptyBidder, GzipBidder, Iso9660Bidder, LhaBidder,
    Lz4Bidder, LzfseBidder, LzipBidder, MtreeBidder, Rar4Bidder, Rar5Bidder, RawBidder,
    SeekableStream, SevenZipBidder, SnappyBidder, SquashfsBidder, TarBidder, WarcBidder,
    WimBidder, XarBidder, XzBidder, ZipBidder, ZstdBidder,
};
use super::formats::ArchiveFormat;
use std::io::{self, SeekFrom};

/// Outcome of format bidding arbitration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BidResult {
    /// Winning identified format.
    pub format: ArchiveFormat,
    /// Winning bidding score.
    pub score: BidScore,
    /// Canonical format name string.
    pub format_name: &'static str,
}

impl BidResult {
    /// Empty non-matching result.
    pub const NONE: BidResult = BidResult {
        format: ArchiveFormat::Unknown,
        score: BidScore::NONE,
        format_name: "unknown",
    };

    /// Returns `true` if a format was positively identified.
    #[inline]
    #[must_use]
    pub const fn is_matched(&self) -> bool {
        self.score.is_positive() && !matches!(self.format, ArchiveFormat::Unknown)
    }
}

/// Static registry and dispatcher for all archive format bidders.
pub struct FormatBidderRegistry {
    bidders: Vec<Box<dyn ArchiveFormatBidder>>,
}

impl Default for FormatBidderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatBidderRegistry {
    /// Creates a registry initialized with all 20+ format bidders in Cheap-First Order.
    #[must_use]
    pub fn new() -> Self {
        let mut bidders: Vec<Box<dyn ArchiveFormatBidder>> = Vec::with_capacity(32);

        // 1. Cheap O(1) Magic Number Bidders (Header Lookahead)
        bidders.push(Box::new(SevenZipBidder));
        bidders.push(Box::new(XarBidder));
        bidders.push(Box::new(CabBidder));
        bidders.push(Box::new(Rar5Bidder));
        bidders.push(Box::new(Rar4Bidder));
        bidders.push(Box::new(WarcBidder));
        bidders.push(Box::new(WimBidder));
        bidders.push(Box::new(ArBidder));
        bidders.push(Box::new(XzBidder));
        bidders.push(Box::new(GzipBidder));
        bidders.push(Box::new(Bzip2Bidder));
        bidders.push(Box::new(ZstdBidder));
        bidders.push(Box::new(SquashfsBidder));
        bidders.push(Box::new(ArjBidder));
        bidders.push(Box::new(AarBidder));
        bidders.push(Box::new(LzipBidder));
        bidders.push(Box::new(Lz4Bidder));
        bidders.push(Box::new(SnappyBidder));
        bidders.push(Box::new(LzfseBidder));
        bidders.push(Box::new(BrotliBidder));
        bidders.push(Box::new(CpioBidder));
        bidders.push(Box::new(LhaBidder));
        bidders.push(Box::new(MtreeBidder));
        bidders.push(Box::new(ZipBidder));

        // 2. Medium Complexity Arithmetic / Octal Checksum Bidder (TAR)
        bidders.push(Box::new(TarBidder));

        // 3. Sector & Trailer Offset Bidders (ISO 9660 & DMG)
        bidders.push(Box::new(Iso9660Bidder));
        bidders.push(Box::new(DmgBidder));

        // 4. Full Buffer Fallbacks (Empty & Raw)
        bidders.push(Box::new(EmptyBidder));
        bidders.push(Box::new(RawBidder));

        Self { bidders }
    }

    /// Registers a custom format bidder.
    pub fn register(&mut self, bidder: Box<dyn ArchiveFormatBidder>) {
        self.bidders.push(bidder);
    }

    /// Arbitrates best format candidate from an in-memory buffer using Cheap-First ordering and short-circuiting.
    #[must_use]
    pub fn bid(&self, buffer: &[u8]) -> BidResult {
        let mut best_score = BidScore::NONE;
        let mut best_format = ArchiveFormat::Unknown;
        let mut best_name = "unknown";

        for bidder in &self.bidders {
            // Short-circuit: skip bidder if it cannot beat current highest score
            if bidder.max_score() <= best_score {
                continue;
            }

            let score = bidder.probe_head(buffer, best_score);
            if score > best_score {
                best_score = score;
                best_format = bidder.format();
                best_name = bidder.format_name();

                // Definitive high score short-circuit (e.g. USTAR=106)
                if best_score >= BidScore::TAR_USTAR {
                    break;
                }
            }
        }

        if best_score.is_positive() {
            BidResult {
                format: best_format,
                score: best_score,
                format_name: best_name,
            }
        } else {
            BidResult::NONE
        }
    }

    /// Arbitrates best format candidate from a seekable stream.
    ///
    /// First examines the initial lookahead buffer, then executes seekable probes
    /// (e.g. ZIP central directory trailer, ISO 9660 sector jumps, DMG trailer) for disambiguation.
    pub fn bid_seekable<R: SeekableStream>(&self, reader: &mut R) -> io::Result<BidResult> {
        let original_pos = reader.stream_position().unwrap_or(0);
        let file_len = reader.seek(SeekFrom::End(0)).unwrap_or(0);
        reader.seek(SeekFrom::Start(original_pos))?;

        if file_len == 0 {
            return Ok(BidResult {
                format: ArchiveFormat::Empty,
                score: BidScore::EMPTY,
                format_name: "empty",
            });
        }

        // Read up to 64 KB head buffer
        let head_len = (file_len.min(65536)) as usize;
        let mut head_buf = vec![0u8; head_len];
        reader.read_exact(&mut head_buf)?;

        // 1. Initial head probe
        let mut result = self.bid(&head_buf);

        // 2. Seekable trailer & sector probes
        for bidder in &self.bidders {
            let score = bidder.probe_seekable(reader, result.score);
            if score > result.score {
                result.score = score;
                result.format = bidder.format();
                result.format_name = bidder.format_name();
            }
        }

        // Restore initial reader position
        let _ = reader.seek(SeekFrom::Start(original_pos));

        Ok(result)
    }
}
