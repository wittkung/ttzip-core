// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-Zip Archive Metadata Parser from mapped memory.

use super::models::SevenZHeaderInfo;
use crate::sevenz::encrypted_header::EncodedHeaderDecoder;
use crate::types::TTZipStatus;

/// Parses 7z Header metadata from memory mapped archive, optionally decrypting encoded headers.
pub fn parse_7z_metadata(mapped: &[u8], password: Option<&str>) -> Result<SevenZHeaderInfo, TTZipStatus> {
    EncodedHeaderDecoder::default()
        .decode(mapped, password)
        .map_err(Into::into)
}
