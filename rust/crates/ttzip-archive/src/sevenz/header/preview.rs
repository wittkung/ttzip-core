// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Metadata admission for the local solid LZMA2 preview experiment.

use super::models::SevenZHeaderInfo;
use super::stream::parse_7z_header_stream_with_admission;
use crate::crypto::crc32::crc32_fast;
use crate::sevenz::format::{K_DUMMY, K_END, K_HEADER, METHOD_LZMA2, SevenZSignatureHeader};
use crate::types::TTZipStatus;

const MAX_PREVIEW_HEADER_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy)]
pub(super) enum HeaderAdmission {
    Standard,
    SolidPreview,
}

impl HeaderAdmission {
    pub(super) const fn is_standard(self) -> bool {
        matches!(self, Self::Standard)
    }

    pub(super) fn count(self, count: u64, max: u64) -> Result<(), TTZipStatus> {
        if !self.is_standard() && count > max {
            return Err(TTZipStatus::ErrSolidBudgetExceeded);
        }
        Ok(())
    }

    pub(super) fn unsupported(self) -> Result<(), TTZipStatus> {
        if self.is_standard() {
            Ok(())
        } else {
            Err(TTZipStatus::ErrUnsupportedFeature)
        }
    }

    pub(super) fn require_complete_substream_crcs(
        self,
        crcs: &[Option<u32>],
    ) -> Result<(), TTZipStatus> {
        if !self.is_standard() && crcs.iter().any(Option::is_none) {
            return Err(TTZipStatus::ErrUnsupportedFeature);
        }
        Ok(())
    }

    pub(super) fn coder(self, bytes: Option<&[u8]>) -> Result<(), TTZipStatus> {
        const LZMA2: u8 = METHOD_LZMA2 as u8;
        // The simple coder form has one method byte and one dictionary property byte.
        if !matches!(bytes, Some([0x21, LZMA2, 1, _])) {
            self.unsupported()?;
        }
        Ok(())
    }

    pub(super) fn next_tag(
        self,
        bytes: &[u8],
        position: &mut usize,
        seen: &mut u64,
    ) -> Result<u8, TTZipStatus> {
        let tag = *bytes.get(*position).ok_or(TTZipStatus::ErrCorruptHeader)?;
        *position += 1;
        if !self.is_standard() && !matches!(tag, K_END | K_DUMMY) {
            let bit = 1u64.checked_shl(u32::from(tag)).ok_or(TTZipStatus::ErrUnsupportedFeature)?;
            if *seen & bit != 0 {
                return Err(TTZipStatus::ErrUnsupportedFeature);
            }
            *seen |= bit;
        }
        Ok(tag)
    }
}

/// Parses only small plain headers for the local solid preview candidate.
pub fn parse_solid_preview_metadata(
    mapped: &[u8],
    is_cancelled: &dyn Fn() -> bool,
) -> Result<SevenZHeaderInfo, TTZipStatus> {
    let signature = SevenZSignatureHeader::parse(mapped)?;
    if signature.next_header_size > MAX_PREVIEW_HEADER_BYTES {
        return Err(TTZipStatus::ErrSolidBudgetExceeded);
    }
    let offset = usize::try_from(signature.next_header_offset).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    let start = offset.checked_add(32).ok_or(TTZipStatus::ErrCorruptHeader)?;
    let end = start
        .checked_add(signature.next_header_size as usize)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let header = mapped.get(start..end).ok_or(TTZipStatus::ErrCorruptHeader)?;
    match header.first() {
        Some(&K_HEADER) => {}
        Some(_) => return Err(TTZipStatus::ErrUnsupportedFeature),
        None => return Err(TTZipStatus::ErrCorruptHeader),
    }
    if is_cancelled() {
        return Err(TTZipStatus::Cancelled);
    }
    if crc32_fast(0, header) != signature.next_header_crc {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    if is_cancelled() {
        return Err(TTZipStatus::Cancelled);
    }
    let mut info = SevenZHeaderInfo {
        payload_offset: 32,
        payload_len: offset,
        ..Default::default()
    };
    parse_7z_header_stream_with_admission(header, &mut info, HeaderAdmission::SolidPreview)?;
    Ok(info)
}
