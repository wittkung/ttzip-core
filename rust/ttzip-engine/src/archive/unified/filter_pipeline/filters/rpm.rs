// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Streaming RPM lead and header stripper filter.

use std::io::{self, Read};

use crate::archive::unified::filter_pipeline::kinds::FilterKind;
use crate::archive::unified::filter_pipeline::lookahead::SlidingLookaheadReader;
use crate::archive::unified::filter_pipeline::traits::StreamFilter;

/// Streaming RPM lead and header stripper filter.
///
/// Strips 96-byte RPM Lead structure and subsequent RPM header records (`\x8E\xAD\xE8\x01`)
/// to expose the payload archive stream.
pub struct RpmLeadFilter<R: Read + Send> {
    reader: SlidingLookaheadReader<R>,
    lead_stripped: bool,
    bytes_consumed: u64,
    bytes_produced: u64,
}

impl<R: Read + Send> RpmLeadFilter<R> {
    /// Creates a new RPM stripper filter.
    pub fn new(reader: R) -> Self {
        Self {
            reader: SlidingLookaheadReader::new(reader),
            lead_stripped: false,
            bytes_consumed: 0,
            bytes_produced: 0,
        }
    }

    fn strip_rpm_envelope(&mut self) -> io::Result<()> {
        if self.lead_stripped {
            return Ok(());
        }

        // 1. Consume 96-byte RPM Lead if magic matches
        let peeked = self.reader.peek(96)?;
        if peeked.len() >= 96 && peeked[..4] == [0xED, 0xAB, 0xEE, 0xDB] {
            self.reader.consume_bytes(96);
            self.bytes_consumed += 96;
        }

        // 2. Consume any RPM Header structures: \x8E\xAD\xE8\x01
        loop {
            let hdr_peek = self.reader.peek(16)?;
            if hdr_peek.len() >= 16 && hdr_peek[..4] == [0x8E, 0xAD, 0xE8, 0x01] {
                let nindex = u32::from_be_bytes([hdr_peek[8], hdr_peek[9], hdr_peek[10], hdr_peek[11]]) as usize;
                let data_size = u32::from_be_bytes([hdr_peek[12], hdr_peek[13], hdr_peek[14], hdr_peek[15]]) as usize;
                let total_hdr = 16 + (nindex * 16) + data_size;
                // RPM headers are aligned to 8-byte boundaries
                let aligned_hdr = (total_hdr + 7) & !7;

                // Peek and consume full header
                let _ = self.reader.peek(aligned_hdr)?;
                self.reader.consume_bytes(aligned_hdr);
                self.bytes_consumed += aligned_hdr as u64;
            } else {
                break;
            }
        }

        self.lead_stripped = true;
        Ok(())
    }
}

impl<R: Read + Send> Read for RpmLeadFilter<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if !self.lead_stripped {
            self.strip_rpm_envelope()?;
        }
        let n = self.reader.read(buf)?;
        self.bytes_produced += n as u64;
        self.bytes_consumed += n as u64;
        Ok(n)
    }
}

impl<R: Read + Send> StreamFilter for RpmLeadFilter<R> {
    fn filter_kind(&self) -> FilterKind {
        FilterKind::Rpm
    }
    fn bytes_consumed(&self) -> u64 {
        self.bytes_consumed
    }
    fn bytes_produced(&self) -> u64 {
        self.bytes_produced
    }
}
