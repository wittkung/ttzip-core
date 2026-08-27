// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Single-file compressed stream direct in-place update engine (GZ, BZ2, XZ, ZST, Snappy, Brotli, LZFSE).

use super::InPlaceAction;
use crate::codecs::brotli::brotli_compress_to_vec;
use crate::codecs::deflate::{gzip_compress, gzip_compress_bound};
use crate::codecs::fast_blocks::lzfse_compress;
use crate::codecs::lzma2::{fl2_compress, fl2_compress_bound};
use crate::codecs::snappy::snappy_frame_encode_to_vec;
use crate::codecs::zstd::{zstd_compress, zstd_compress_bound};
use crate::standards::signatures::DetectedFormat;
use crate::types::TTZipStatus;
use std::fs;
use std::path::Path;

/// Modifies a single-file compressed stream directly in-place.
pub fn in_place_edit_single_stream(
    _archive_path: &Path,
    shadow_path: &Path,
    format: DetectedFormat,
    actions: &[InPlaceAction],
) -> Result<(), TTZipStatus> {
    let target_src = actions.iter().rev().find_map(|a| match a {
        InPlaceAction::Replace { source_path, .. } | InPlaceAction::Append { source_path, .. } => Some(source_path),
        _ => None,
    });

    let src_path = match target_src {
        Some(p) => p,
        None => return Err(TTZipStatus::ErrInvalidParam),
    };

    let raw_data = fs::read(src_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;

    let comp_bytes = match format {
        DetectedFormat::Gzip => {
            let max_bound = gzip_compress_bound(raw_data.len(), 6);
            let mut out = vec![0u8; max_bound];
            let len = gzip_compress(&raw_data, &mut out, 6)?;
            out.truncate(len);
            out
        }
        DetectedFormat::Zstd => {
            let mut out = vec![0u8; zstd_compress_bound(raw_data.len())];
            let len = zstd_compress(&raw_data, &mut out, 3)?;
            out.truncate(len);
            out
        }
        DetectedFormat::Xz => {
            let mut out = vec![0u8; fl2_compress_bound(raw_data.len()) + 1024];
            let len = fl2_compress(&raw_data, &mut out, 3, 2)?;
            out.truncate(len);
            out
        }
        DetectedFormat::Snappy => snappy_frame_encode_to_vec(&raw_data)?,
        DetectedFormat::Brotli => brotli_compress_to_vec(&raw_data, 6, 22)?,
        DetectedFormat::Lzfse => {
            let mut out = vec![0u8; raw_data.len() + 1024];
            let len = lzfse_compress(&raw_data, &mut out)?;
            out.truncate(len);
            out
        }
        _ => raw_data,
    };

    fs::write(shadow_path, comp_bytes).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    Ok(())
}
