// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Single-file compressed stream direct in-place update engine (GZ, BZ2, XZ, ZST, Snappy, Brotli, LZFSE).

use super::InPlaceAction;
use crate::codecs::brotli::brotli_compress_to_vec;
use crate::codecs::bzip2::bzip2_compress_to_vec;
use crate::codecs::deflate::{gzip_compress, gzip_compress_bound};
use crate::codecs::fast_blocks::lzfse_compress;
use crate::codecs::lz4::lz4_frame_compress_to_vec;
use crate::codecs::snappy::snappy_frame_encode_to_vec;
use crate::codecs::zstd::{zstd_compress, zstd_compress_bound};
use crate::standards::signatures::DetectedFormat;
use crate::types::TTZipStatus;
use crate::xz::{xz_compress, XzEncoderOptions};
use std::fs;
use std::path::Path;

/// Modifies a single-file compressed stream directly in-place.
pub fn in_place_edit_single_stream(
    _archive_path: &Path,
    shadow_path: &Path,
    format: DetectedFormat,
    actions: &[InPlaceAction],
) -> Result<(), TTZipStatus> {
    let src_path = find_last_source_path(actions).ok_or(TTZipStatus::ErrInvalidParam)?;

    if format == DetectedFormat::Zstd && try_stream_large_zstd(src_path, shadow_path)? {
        return Ok(());
    }

    let raw_data = fs::read(src_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let comp_bytes = compress_single_stream_payload(format, raw_data)?;

    fs::write(shadow_path, comp_bytes).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    Ok(())
}

fn find_last_source_path(actions: &[InPlaceAction]) -> Option<&Path> {
    actions.iter().rev().find_map(|a| match a {
        InPlaceAction::Replace { source_path, .. } | InPlaceAction::Append { source_path, .. } => {
            Some(source_path.as_path())
        }
        _ => None,
    })
}

fn try_stream_large_zstd(src_path: &Path, shadow_path: &Path) -> Result<bool, TTZipStatus> {
    let meta = fs::metadata(src_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    if meta.len() <= 64 * 1024 * 1024 {
        return Ok(false);
    }
    let in_f = fs::File::open(src_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let out_f = fs::File::create(shadow_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut reader = std::io::BufReader::with_capacity(64 * 1024, in_f);
    let mut writer = crate::codecs::zstd::ZstdStreamWriter::with_level(out_f, 3)?;
    std::io::copy(&mut reader, &mut writer).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    writer.finish()?;
    Ok(true)
}

fn compress_single_stream_payload(
    format: DetectedFormat,
    raw_data: Vec<u8>,
) -> Result<Vec<u8>, TTZipStatus> {
    match format {
        DetectedFormat::Gzip => {
            let max_bound = gzip_compress_bound(raw_data.len(), 6);
            let mut out = vec![0u8; max_bound];
            let len = gzip_compress(&raw_data, &mut out, 6)?;
            out.truncate(len);
            Ok(out)
        }
        DetectedFormat::Zstd => {
            let mut out = vec![0u8; zstd_compress_bound(raw_data.len())];
            let len = zstd_compress(&raw_data, &mut out, 3)?;
            out.truncate(len);
            Ok(out)
        }
        DetectedFormat::Xz => xz_compress(&raw_data, &XzEncoderOptions::default().with_preset_level(3))
            .map_err(|_| TTZipStatus::ErrCompressionFailed),
        DetectedFormat::Bzip2 => bzip2_compress_to_vec(&raw_data, 6),
        DetectedFormat::Lz4 => lz4_frame_compress_to_vec(&raw_data, None, 1),
        DetectedFormat::Snappy => snappy_frame_encode_to_vec(&raw_data),
        DetectedFormat::Brotli => brotli_compress_to_vec(&raw_data, 6, 22),
        DetectedFormat::Lzfse => {
            let mut out = vec![0u8; raw_data.len() + 1024];
            let len = lzfse_compress(&raw_data, &mut out)?;
            out.truncate(len);
            Ok(out)
        }
        _ => Ok(raw_data),
    }
}
