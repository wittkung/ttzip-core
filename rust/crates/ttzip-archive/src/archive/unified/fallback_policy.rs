// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Strict Refusal-to-Downgrade (No-Fallback) Security Policy for Unified Archive Orchestrator.
//!
//! Ensures that security violations (`ErrSecurityViolation`), user cancellations (`Cancelled`),
//! authentication failures (`ErrInvalidPassword`), corruption (`ErrCorruptHeader`), quota/memory
//! circuit breakers (`ErrSolidBudgetExceeded`, `ErrOutOfMemory`), and extraction I/O errors
//! (`ErrExtractionFailed`) are never swallowed to fall back to a secondary parser/extractor.

use crate::types::TTZipStatus;

/// Returns `true` if `status` represents a security, integrity, authentication, quota, or
/// cancellation failure that MUST abort immediately and NEVER fall back to another engine.
#[inline]
pub const fn should_abort_without_fallback(status: TTZipStatus) -> bool {
    matches!(
        status,
        TTZipStatus::Cancelled
            | TTZipStatus::ErrSecurityViolation
            | TTZipStatus::ErrInvalidPassword
            | TTZipStatus::ErrCorruptHeader
            | TTZipStatus::ErrExtractionFailed
            | TTZipStatus::ErrSolidBudgetExceeded
            | TTZipStatus::ErrOutOfMemory
            | TTZipStatus::ErrPathTooLong
            | TTZipStatus::ErrInvalidOffset
            | TTZipStatus::ErrUnsupportedFeature
            | TTZipStatus::ErrPanicCaught
    )
}

/// Returns `true` if `data` starts with the 6-byte 7z magic signature (`7z\xBC\xAF\x27\x1C`).
#[inline]
pub fn has_7z_signature(data: &[u8]) -> bool {
    data.len() >= 6 && &data[..6] == b"7z\xBC\xAF\x27\x1C"
}

/// Returns `true` if `data` starts with a local, empty-archive, or spanned ZIP signature.
#[inline]
pub fn has_zip_signature(data: &[u8]) -> bool {
    data.len() >= 4
        && (&data[..4] == b"PK\x03\x04"
            || &data[..4] == b"PK\x05\x06"
            || &data[..4] == b"PK\x07\x08")
}

/// Verifies the full GZIP stream CRC32 and ISIZE trailer when an archive starts with `[0x1F, 0x8B]`.
///
/// Necessary because `libarchive`'s inner TAR driver stops reading at the two 512-byte zero EOF
/// blocks before draining the trailing 10,240-byte record padding to the GZIP trailer.
pub(crate) fn verify_gzip_stream_if_applicable(
    archive_path: &std::path::Path,
) -> Result<(), TTZipStatus> {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(archive_path) else {
        return Ok(());
    };
    let mut magic = [0u8; 2];
    if f.read_exact(&mut magic).is_err() || magic != [0x1F, 0x8B] {
        return Ok(());
    }
    if f.seek(SeekFrom::Start(0)).is_err() {
        return Ok(());
    }
    let mut decoder =
        flate2::read::MultiGzDecoder::new(std::io::BufReader::with_capacity(65536, f));
    std::io::copy(&mut decoder, &mut std::io::sink()).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_abort_without_fallback_classifications() {
        assert!(should_abort_without_fallback(TTZipStatus::ErrSecurityViolation));
        assert!(should_abort_without_fallback(TTZipStatus::Cancelled));
        assert!(should_abort_without_fallback(TTZipStatus::ErrInvalidPassword));
        assert!(should_abort_without_fallback(TTZipStatus::ErrCorruptHeader));
        assert!(should_abort_without_fallback(TTZipStatus::ErrExtractionFailed));
        assert!(should_abort_without_fallback(TTZipStatus::ErrSolidBudgetExceeded));
        assert!(should_abort_without_fallback(TTZipStatus::ErrOutOfMemory));
        assert!(!should_abort_without_fallback(TTZipStatus::ErrArchiveInitFailed));
    }
}
