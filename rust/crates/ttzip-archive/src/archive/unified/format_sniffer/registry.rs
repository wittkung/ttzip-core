// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

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

/// Canonical static slice of default format bidders in Cheap-First Order.
/// Zero heap allocations, strictly aligned with `src/standards/signatures.rs`.
pub static DEFAULT_BIDDERS: &[&dyn ArchiveFormatBidder] = &[
    // 1. Cheap O(1) Magic Number Bidders (Header Lookahead)
    &SevenZipBidder,
    &XarBidder,
    &CabBidder,
    &Rar5Bidder,
    &Rar4Bidder,
    &WarcBidder,
    &WimBidder,
    &ArBidder,
    &XzBidder,
    &GzipBidder,
    &Bzip2Bidder,
    &ZstdBidder,
    &SquashfsBidder,
    &ArjBidder,
    &AarBidder,
    &LzipBidder,
    &Lz4Bidder,
    &SnappyBidder,
    &LzfseBidder,
    &BrotliBidder,
    &CpioBidder,
    &LhaBidder,
    &MtreeBidder,
    &ZipBidder,
    // 2. Medium Complexity Arithmetic / Octal Checksum Bidder (TAR)
    &TarBidder,
    // 3. Sector & Trailer Offset Bidders (ISO 9660 & DMG)
    &Iso9660Bidder,
    &DmgBidder,
    // 4. Full Buffer Fallbacks (Empty & Raw)
    &EmptyBidder,
    &RawBidder,
];

/// Static registry and dispatcher for all archive format bidders.
pub struct FormatBidderRegistry {
    custom_bidders: Vec<Box<dyn ArchiveFormatBidder>>,
}

impl Default for FormatBidderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatBidderRegistry {
    /// Creates a registry initialized with 0 heap allocations, backed by the static rules table.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            custom_bidders: Vec::new(),
        }
    }

    /// Registers a custom format bidder.
    pub fn register(&mut self, bidder: Box<dyn ArchiveFormatBidder>) {
        self.custom_bidders.push(bidder);
    }

    /// Returns an iterator over all active format bidders in Cheap-First Order.
    #[inline]
    pub fn bidders(&self) -> impl Iterator<Item = &dyn ArchiveFormatBidder> {
        DEFAULT_BIDDERS
            .iter()
            .copied()
            .chain(self.custom_bidders.iter().map(|b| b.as_ref()))
    }

    /// Arbitrates best format candidate from an in-memory buffer using Cheap-First ordering and short-circuiting.
    #[must_use]
    pub fn bid(&self, buffer: &[u8]) -> BidResult {
        let mut best_score = BidScore::NONE;
        let mut best_format = ArchiveFormat::Unknown;
        let mut best_name = "unknown";

        for bidder in self.bidders() {
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
        for bidder in self.bidders() {
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
