// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 7-Zip Solid Stream selective entry decoding with Bounded Memory and Early Termination.

pub(crate) use super::codec_pump::StreamingFilterState;
use super::payload::{
    decode_7z_folder_streaming, decode_7z_packed_folder_slice_bounded, folder_final_unpack_size,
    MAX_IN_MEMORY_DECOMPRESS_BYTES,
};
pub(crate) use super::stream_folder::{
    stream_7z_folder_from_reader, stream_7z_folder_from_reader_with_budget,
    FolderReaderStreamBounds,
};
use crate::crypto::crc32::crc32_fast;
use crate::sevenz::header::{SevenZEntryLocation, SevenZHeaderInfo, SevenZSeekIndex};
use crate::types::TTZipStatus;

fn resolve_seek_entry_bounds(info: &SevenZHeaderInfo, loc: &SevenZEntryLocation) -> (u64, u64) {
    let folder_idx = loc.folder_index.unwrap_or(0);
    if let Some(folder) = info.folders.get(folder_idx) {
        let final_unpack = folder_final_unpack_size(folder);
        let raw_last = folder.unpack_sizes.last().copied().unwrap_or(0);
        if final_unpack != raw_last && final_unpack > loc.offset_in_folder {
            if let Some(range) =
                super::solid_stream::SolidFolderIndex::build(info).lookup(loc.file_index)
            {
                return (range.offset_start, range.uncompressed_size);
            }
        }
    }
    (loc.offset_in_folder, loc.uncompressed_size)
}

fn verify_extracted_entry_bytes(
    result_vec: &[u8],
    target_len: u64,
    expected_crc: Option<u32>,
    is_encrypted: bool,
) -> Result<(), TTZipStatus> {
    let corrupt_err = if is_encrypted {
        TTZipStatus::ErrInvalidPassword
    } else {
        TTZipStatus::ErrCorruptHeader
    };
    if result_vec.len() as u64 != target_len {
        return Err(corrupt_err);
    }
    if let Some(crc) = expected_crc {
        if crc32_fast(0, result_vec) != crc {
            return Err(corrupt_err);
        }
    }
    Ok(())
}

struct EntryWindowTarget {
    target_end: u64,
    target_len: u64,
}

fn decode_in_memory_folder_fallback<F>(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    folder: &crate::sevenz::header::SevenZFolder,
    folder_idx: usize,
    password: Option<&str>,
    target_len: u64,
    sink: F,
) -> Result<u64, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let remaining_budget = (MAX_IN_MEMORY_DECOMPRESS_BYTES as u64).saturating_sub(target_len);
    if folder
        .coders
        .iter()
        .any(|c| c.method_id == crate::sevenz::format::METHOD_BCJ2)
    {
        return decode_7z_folder_streaming(mapped, info, folder_idx, password, 1, sink);
    }
    let payload_end = folder
        .packed_offset
        .checked_add(folder.packed_len)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    if payload_end > mapped.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    decode_7z_packed_folder_slice_bounded(
        &mapped[folder.packed_offset..payload_end],
        info,
        folder,
        password,
        1,
        Some(remaining_budget),
        sink,
    )
}

fn decode_folder_into_entry_window<F>(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    folder: &crate::sevenz::header::SevenZFolder,
    folder_idx: usize,
    password: Option<&str>,
    window: EntryWindowTarget,
    mut sink: F,
) -> Result<(), TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let mut cursor = std::io::Cursor::new(mapped);
    let decode_res = match stream_7z_folder_from_reader_with_budget(
        &mut cursor,
        info,
        folder,
        password,
        1,
        FolderReaderStreamBounds {
            max_unpack_hint: Some(window.target_end),
            existing_allocated_bytes: window.target_len,
            allow_reader_fallback: false,
        },
        &mut sink,
    ) {
        Ok(_) => Ok(()),
        Err(TTZipStatus::ErrUnsupportedFeature) => decode_in_memory_folder_fallback(
            mapped,
            info,
            folder,
            folder_idx,
            password,
            window.target_len,
            sink,
        )
        .map(|_| ()),
        Err(e) => Err(e),
    };

    match decode_res {
        Ok(_) | Err(TTZipStatus::Eof) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Decompresses and extracts a single entry from 7z solid stream with 4MB micro-buffer discarding,
/// precise target slice extraction, and early termination upon completion.
pub fn extract_single_entry_bounded(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    seek_index: &SevenZSeekIndex,
    entry_idx: usize,
    password: Option<&str>,
    max_preceding_budget_bytes: u64,
) -> Result<Vec<u8>, TTZipStatus> {
    let loc = seek_index
        .get_by_index(entry_idx)
        .ok_or(TTZipStatus::ErrInvalidOffset)?;

    let (target_start, target_len) = resolve_seek_entry_bounds(info, loc);
    if loc.is_directory || loc.is_empty_stream || target_len == 0 {
        return Ok(Vec::new());
    }

    let target_end = target_start
        .checked_add(target_len)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;

    if max_preceding_budget_bytes > 0 && target_start > max_preceding_budget_bytes {
        return Err(TTZipStatus::ErrSolidBudgetExceeded);
    }
    if target_len > MAX_IN_MEMORY_DECOMPRESS_BYTES as u64 {
        return Err(TTZipStatus::ErrOutOfMemory);
    }

    let mut current_offset: u64 = 0;
    let mut result_vec = Vec::with_capacity(target_len as usize);
    let folder_idx = loc.folder_index.unwrap_or(0);
    let folder = info
        .folders
        .get(folder_idx)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;

    decode_folder_into_entry_window(
        mapped,
        info,
        folder,
        folder_idx,
        password,
        EntryWindowTarget {
            target_end,
            target_len,
        },
        |chunk| {
            append_entry_window_chunk(
                chunk,
                &mut current_offset,
                target_start,
                target_end,
                &mut result_vec,
            )
        },
    )?;

    verify_extracted_entry_bytes(&result_vec, target_len, loc.crc, info.is_encrypted)?;
    Ok(result_vec)
}

/// Explicit work policy for a single plaintext LZMA2 preview experiment.
pub struct SolidLzma2Preview<'a> {
    pub max_packed_bytes: usize,
    pub max_decoded_bytes: u64,
    pub cancellation: &'a dyn Fn() -> bool,
}

fn bounded_lzma2_payload<'a>(
    mapped: &'a [u8],
    info: &SevenZHeaderInfo,
    loc: &SevenZEntryLocation,
    end: u64,
) -> Result<(&'a [u8], u8), TTZipStatus> {
    let folder = loc.folder_index.and_then(|index| info.folders.get(index))
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let [coder] = folder.coders.as_slice() else {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    };
    if info.is_encrypted || !folder.bind_pairs.is_empty() || !folder.packed_streams.is_empty()
        || coder.method_id != crate::sevenz::format::METHOD_LZMA2
        || coder.num_in_streams != 1 || coder.num_out_streams != 1
    {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    let [prop] = coder.properties.as_slice() else {
        return Err(TTZipStatus::ErrCorruptHeader);
    };
    let dictionary = super::codec_pump::lzma2_dict_size_from_prop(*prop)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    if dictionary > 1024 * 1024 {
        return Err(TTZipStatus::ErrOutOfMemory);
    }
    let [folder_size] = folder.unpack_sizes.as_slice() else {
        return Err(TTZipStatus::ErrCorruptHeader);
    };
    if end > *folder_size {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    Ok((lzma2_packed_payload(mapped, folder)?, *prop))
}

fn lzma2_packed_payload<'a>(
    mapped: &'a [u8],
    folder: &crate::sevenz::header::SevenZFolder,
) -> Result<&'a [u8], TTZipStatus> {
    let sig = crate::sevenz::format::SevenZSignatureHeader::parse(mapped)?;
    let metadata_start = usize::try_from(sig.next_header_offset).ok()
        .and_then(|offset| offset.checked_add(32))
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let packed_end = folder.packed_offset.checked_add(folder.packed_len)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    if folder.packed_offset < 32 || packed_end > metadata_start {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    mapped.get(folder.packed_offset..packed_end)
        .ok_or(TTZipStatus::ErrCorruptHeader)
}

fn pump_preview_entry<F>(
    packed: &[u8],
    prop: u8,
    cap: usize,
    work: &mut super::codec_pump::PreviewWork<'_>,
    mut sink: F,
) -> Result<(), TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    const STEP_BYTES: usize = 64 * 1024;
    let mut decoder = crate::codecs::lzma2::Fl2DStream::new(1)?;
    decoder.init(Some(prop))?;
    let mut scratch = vec![0; STEP_BYTES];
    let mut decoded = 0;
    let input = &packed[..packed.len().min(cap)];
    let chunks = input.chunks(STEP_BYTES);
    let count = chunks.len();
    for (index, chunk) in chunks.enumerate() {
        let final_input = index + 1 == count && input.len() == packed.len();
        let result = super::codec_pump::pump_lzma2_stream(
            &mut decoder,
            chunk,
            &mut scratch,
            final_input,
            &mut decoded,
            super::codec_pump::Lzma2Work::Preview(work),
            &mut sink,
        );
        match result {
            Err(TTZipStatus::Eof) => return Ok(()),
            Ok(true) => return Err(TTZipStatus::ErrCorruptHeader),
            Ok(false) => {}
            Err(status) => return Err(status),
        }
    }
    if input.len() < packed.len() {
        return Err(TTZipStatus::ErrSolidBudgetExceeded);
    }
    Err(TTZipStatus::ErrCorruptHeader)
}

/// Reads one CRC-protected entry using an explicit synchronous LZMA2 work budget.
pub fn extract_solid_lzma2_preview(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    seek_index: &SevenZSeekIndex,
    entry_idx: usize,
    policy: SolidLzma2Preview<'_>,
) -> Result<Vec<u8>, TTZipStatus> {
    let loc = seek_index.get_by_index(entry_idx).ok_or(TTZipStatus::ErrInvalidOffset)?;
    if loc.crc.is_none() {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    let (start, len) = resolve_seek_entry_bounds(info, loc);
    let end = start.checked_add(len).ok_or(TTZipStatus::ErrCorruptHeader)?;
    let mut work = super::codec_pump::PreviewWork {
        end,
        accepted: 0,
        cancellation: policy.cancellation,
    };
    work.check_cancelled()?;
    if len == 0 || loc.is_directory || loc.is_empty_stream {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    if len > 8 * 1024 * 1024 {
        return Err(TTZipStatus::ErrOutOfMemory);
    }
    if end > policy.max_decoded_bytes {
        return Err(TTZipStatus::ErrSolidBudgetExceeded);
    }
    let (packed, prop) = bounded_lzma2_payload(mapped, info, loc, end)?;
    let mut bytes = Vec::with_capacity(len as usize);
    let mut offset = 0;
    pump_preview_entry(packed, prop, policy.max_packed_bytes, &mut work, |chunk| {
        append_entry_window_chunk(chunk, &mut offset, start, end, &mut bytes)
    })?;
    verify_extracted_entry_bytes(&bytes, len, loc.crc, false)?;
    work.check_cancelled()?;
    Ok(bytes)
}

/// Backwards-compatible alias for bounded stream extraction.
pub use extract_single_entry_bounded as extract_entry_bytes_stream_bounded;

/// Backwards-compatible stream extraction with default unbounded budget (0 = disabled budget limit).
pub fn extract_entry_bytes_stream(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    seek_index: &SevenZSeekIndex,
    entry_idx: usize,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    extract_single_entry_bounded(mapped, info, seek_index, entry_idx, password, 0)
}

fn append_entry_window_chunk(
    chunk: &[u8],
    current_offset: &mut u64,
    target_start: u64,
    target_end: u64,
    result_vec: &mut Vec<u8>,
) -> Result<(), TTZipStatus> {
    let chunk_start = *current_offset;
    let chunk_end = chunk_start
        .checked_add(chunk.len() as u64)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    *current_offset = chunk_end;
    if chunk_end <= target_start {
        return Ok(());
    }
    if chunk_start < target_end && chunk_end > target_start {
        let s = (target_start.saturating_sub(chunk_start)) as usize;
        let e = (target_end.min(chunk_end) - chunk_start) as usize;
        result_vec.extend_from_slice(&chunk[s..e]);
    }
    if *current_offset >= target_end {
        return Err(TTZipStatus::Eof);
    }
    Ok(())
}

/// Extracts a single entry into memory from a seekable 7z reader (such as `VirtualMultiVolumeReader`).
pub fn extract_7z_single_entry_from_reader<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    info: &SevenZHeaderInfo,
    seek_index: &SevenZSeekIndex,
    entry_idx: usize,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    let loc = seek_index.get_by_index(entry_idx).ok_or(TTZipStatus::ErrInvalidOffset)?;
    let (target_start, target_len) = resolve_seek_entry_bounds(info, loc);
    if loc.is_directory || loc.is_empty_stream || target_len == 0 {
        return Ok(Vec::new());
    }
    if target_len > MAX_IN_MEMORY_DECOMPRESS_BYTES as u64 {
        return Err(TTZipStatus::ErrOutOfMemory);
    }
    let folder = info
        .folders
        .get(loc.folder_index.unwrap_or(0))
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let target_end = target_start
        .checked_add(target_len)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let mut current_offset = 0u64;
    let mut result_vec = Vec::with_capacity(target_len as usize);
    let mut sink = |chunk: &[u8]| -> Result<(), TTZipStatus> {
        append_entry_window_chunk(chunk, &mut current_offset, target_start, target_end, &mut result_vec)
    };
    match stream_7z_folder_from_reader_with_budget(
        reader,
        info,
        folder,
        password,
        1,
        FolderReaderStreamBounds {
            max_unpack_hint: Some(target_end),
            existing_allocated_bytes: target_len,
            allow_reader_fallback: true,
        },
        &mut sink,
    ) {
        Ok(_) | Err(TTZipStatus::Eof) => {}
        Err(e) => return Err(e),
    }
    verify_extracted_entry_bytes(&result_vec, target_len, loc.crc, info.is_encrypted)?;
    Ok(result_vec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sevenz::format::METHOD_COPY;
    use crate::sevenz::header::{
        SevenZCoder, SevenZEntryLocation, SevenZFileMeta, SevenZFolder,
    };
    use std::io::Cursor;

    fn make_truncated_no_crc_header_and_index(
        actual_payload: &[u8],
        declared_uncompressed_size: u64,
        offset_in_folder: u64,
    ) -> (SevenZHeaderInfo, SevenZSeekIndex) {
        let folder = SevenZFolder {
            coders: vec![SevenZCoder {
                method_id: METHOD_COPY,
                num_in_streams: 1,
                num_out_streams: 1,
                properties: Vec::new(),
            }],
            bind_pairs: Vec::new(),
            packed_streams: vec![0],
            unpack_sizes: vec![actual_payload.len() as u64],
            crc: None,
            num_unpack_streams: 1,
            packed_offset: 0,
            packed_len: actual_payload.len(),
        };
        let info = SevenZHeaderInfo {
            payload_offset: 0,
            payload_len: actual_payload.len(),
            pack_sizes: vec![actual_payload.len() as u64],
            folders: vec![folder],
            stream_sizes: vec![declared_uncompressed_size],
            stream_crcs: Vec::new(),
            files: vec![SevenZFileMeta {
                rel_path: "truncated_no_crc.bin".to_string(),
                is_directory: false,
                is_empty_stream: false,
                mtime_epoch_secs: None,
                mode: 0o644,
            }],
            primary_method_id: METHOD_COPY,
            coder_props: Vec::new(),
            is_encrypted: false,
            aes_salt: [0u8; 16],
            aes_salt_len: 0,
            aes_iv: [0u8; 16],
            aes_iv_len: 0,
            aes_num_cycles_power: 0,
        };
        let seek_index = SevenZSeekIndex {
            entries: vec![SevenZEntryLocation {
                file_index: 0,
                rel_path: "truncated_no_crc.bin".to_string(),
                is_directory: false,
                is_empty_stream: false,
                folder_index: Some(0),
                stream_index: Some(0),
                offset_in_folder,
                uncompressed_size: declared_uncompressed_size,
                crc: None,
            }],
        };
        (info, seek_index)
    }

    #[test]
    fn test_extract_single_entry_bounded_rejects_short_output_without_crc() {
        let short_payload = b"SHORT_STREAM_ONLY_24_BYT";
        let (info, seek_index) = make_truncated_no_crc_header_and_index(short_payload, 64, 0);
        let res = extract_single_entry_bounded(short_payload, &info, &seek_index, 0, None, 0);
        assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
    }

    #[test]
    fn test_extract_7z_single_entry_from_reader_rejects_short_output_without_crc() {
        let short_payload = b"SHORT_STREAM_ONLY_24_BYT";
        let (info, seek_index) = make_truncated_no_crc_header_and_index(short_payload, 64, 0);
        let mut reader = Cursor::new(short_payload.as_slice());
        let res = extract_7z_single_entry_from_reader(&mut reader, &info, &seek_index, 0, None);
        assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
    }

    #[test]
    fn test_extract_single_entry_rejects_offset_addition_overflow() {
        let payload = b"0123456789ABCDEF";
        let (info, seek_index) = make_truncated_no_crc_header_and_index(payload, 16, u64::MAX - 4);
        assert_eq!(
            extract_single_entry_bounded(payload, &info, &seek_index, 0, None, 0),
            Err(TTZipStatus::ErrCorruptHeader)
        );
        let mut reader = Cursor::new(payload.as_slice());
        assert_eq!(
            extract_7z_single_entry_from_reader(&mut reader, &info, &seek_index, 0, None),
            Err(TTZipStatus::ErrCorruptHeader)
        );
    }

    #[test]
    fn test_extract_single_entry_and_reader_fallback_enforce_aggregate_memory_ceiling() {
        // 30 MiB packed fallback folder + 40 MiB target output = 70 MiB > 64 MiB ceiling.
        // Using two COPY coders triggers the non-primary-stream fallback in FolderStreamDecoder.
        let packed_len = 30 * 1024 * 1024;
        let target_len = 40 * 1024 * 1024u64;
        let payload = vec![0u8; packed_len];
        let folder = SevenZFolder {
            coders: vec![
                SevenZCoder {
                    method_id: METHOD_COPY,
                    properties: Vec::new(),
                    num_in_streams: 1,
                    num_out_streams: 1,
                },
                SevenZCoder {
                    method_id: METHOD_COPY,
                    properties: Vec::new(),
                    num_in_streams: 1,
                    num_out_streams: 1,
                },
            ],
            bind_pairs: Vec::new(),
            packed_streams: vec![0],
            unpack_sizes: vec![target_len, target_len],
            crc: None,
            num_unpack_streams: 1,
            packed_offset: 0,
            packed_len,
        };
        let info = SevenZHeaderInfo {
            payload_offset: 0,
            payload_len: packed_len,
            pack_sizes: vec![packed_len as u64],
            folders: vec![folder],
            stream_sizes: vec![target_len],
            files: vec![SevenZFileMeta {
                rel_path: "large.bin".to_string(),
                is_directory: false,
                is_empty_stream: false,
                mtime_epoch_secs: None,
                mode: 0o644,
            }],
            primary_method_id: METHOD_COPY,
            ..Default::default()
        };
        let seek_index = SevenZSeekIndex {
            entries: vec![SevenZEntryLocation {
                file_index: 0,
                rel_path: "large.bin".to_string(),
                is_directory: false,
                is_empty_stream: false,
                folder_index: Some(0),
                stream_index: Some(0),
                offset_in_folder: 0,
                uncompressed_size: target_len,
                crc: None,
            }],
        };

        let mut reader = Cursor::new(payload.as_slice());
        assert_eq!(
            extract_7z_single_entry_from_reader(&mut reader, &info, &seek_index, 0, None),
            Err(TTZipStatus::ErrOutOfMemory),
            "Reader extraction must reject when target_len (40 MiB) + fallback packed_buf (30 MiB) exceeds 64 MiB"
        );
    }
}
