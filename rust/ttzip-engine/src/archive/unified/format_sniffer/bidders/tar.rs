// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TAR archive format bidder with dual unsigned/signed checksum calculation
//! and POSIX USTAR / GNU / Unix V7 disambiguation.

use super::{ArchiveFormatBidder, BidScore};
use crate::archive::unified::format_sniffer::formats::ArchiveFormat;

/// Bidder for TAR archives (POSIX ustar, GNU tar, old V7 tar).
pub struct TarBidder;

impl ArchiveFormatBidder for TarBidder {
    fn format_name(&self) -> &'static str {
        "tar"
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Tar
    }

    fn max_score(&self) -> BidScore {
        BidScore::TAR_USTAR
    }

    fn probe_head(&self, buffer: &[u8], best_so_far: BidScore) -> BidScore {
        if best_so_far >= BidScore::TAR_USTAR || buffer.len() < 512 {
            return BidScore::NONE;
        }

        // Verify checksum at offset 148..156
        let chksum_field = &buffer[148..156];
        let chksum_str = match std::str::from_utf8(chksum_field) {
            Ok(s) => s.trim_matches(|c: char| c.is_whitespace() || c == '\0'),
            Err(_) => return BidScore::NONE,
        };

        if chksum_str.is_empty() {
            return BidScore::NONE;
        }

        let expected_chksum = match u32::from_str_radix(chksum_str, 8) {
            Ok(v) => v,
            Err(_) => return BidScore::NONE,
        };

        let mut unsigned_sum: u32 = 0;
        let mut signed_sum: i32 = 0;

        for (i, &b) in buffer[..512].iter().enumerate() {
            let val = if (148..156).contains(&i) { b' ' } else { b };
            unsigned_sum += val as u32;
            signed_sum += (val as i8) as i32;
        }

        if expected_chksum == unsigned_sum || expected_chksum == (signed_sum as u32) {
            let magic = &buffer[257..265];
            if magic.starts_with(b"ustar\0") {
                BidScore::TAR_USTAR
            } else if magic.starts_with(b"ustar  \0") {
                BidScore::TAR_GNU
            } else {
                BidScore::TAR_V7
            }
        } else {
            BidScore::NONE
        }
    }
}
