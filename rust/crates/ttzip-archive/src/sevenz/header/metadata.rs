// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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
