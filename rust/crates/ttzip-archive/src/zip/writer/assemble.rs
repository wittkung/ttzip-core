// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! ZIP archive binary serialization and layout assembly (LFH, CDFH, Zip64, EOCD).

use super::types::{unix_to_dos_time, ZipCompressedItem};
use crate::types::TTZipStatus;
use crate::zip::alignment::{build_alignment_extra_field, AlignmentPaddingCalculator};
use crate::zip::extra::ZipExtraFields;
use crate::zip::parser::{
    MAGIC_CDFH, MAGIC_EOCD, MAGIC_LFH, MAGIC_ZIP64_EOCD, MAGIC_ZIP64_LOCATOR,
};

/// Assembles compressed items into final ZIP archive binary bytes.
pub fn assemble_zip_archive(items: &[ZipCompressedItem]) -> Result<Vec<u8>, TTZipStatus> {
    assemble_zip_archive_aligned(items, 0)
}

/// Assembles compressed items into final ZIP archive binary bytes with custom sector/page alignment.
///
/// If `target_alignment > 1`, injects `TAG_DATA_STREAM_ALIGNMENT` (`0xa11e`) padding into
/// each entry's Local File Header (LFH) so that every payload's file offset is aligned.
/// The alignment extra fields are stripped from the Central Directory to save catalog space.
pub fn assemble_zip_archive_aligned(
    items: &[ZipCompressedItem],
    target_alignment: u16,
) -> Result<Vec<u8>, TTZipStatus> {
    let mut out = Vec::new();
    let mut lfh_offsets = Vec::with_capacity(items.len());

    for item in items {
        let lfh_offset = out.len() as u64;
        lfh_offsets.push(lfh_offset);
        write_local_file_entry(&mut out, item, lfh_offset, target_alignment);
    }

    let cd_offset = out.len() as u64;
    for (i, item) in items.iter().enumerate() {
        write_central_directory_entry(&mut out, item, lfh_offsets[i]);
    }

    let cd_size = (out.len() as u64) - cd_offset;
    write_end_of_central_directory(&mut out, items.len() as u64, cd_size, cd_offset);

    Ok(out)
}

fn requires_zip64(item: &ZipCompressedItem, lfh_offset: u64) -> bool {
    item.uncompressed_size >= 0xFFFFFFFF
        || item.compressed_size >= 0xFFFFFFFF
        || lfh_offset >= 0xFFFFFFFF
}

fn resolve_version_needed(item: &ZipCompressedItem, use_zip64: bool) -> u16 {
    if use_zip64 {
        45u16
    } else if item.is_encrypted {
        51u16
    } else if item.compression_method == 8 {
        20u16
    } else {
        10u16
    }
}

fn clamp_u32_field(val: u64) -> u32 {
    if val >= 0xFFFFFFFF {
        0xFFFFFFFFu32
    } else {
        val as u32
    }
}

fn build_local_extra_bytes(
    item: &ZipCompressedItem,
    lfh_offset: u64,
    name_len: usize,
    use_zip64: bool,
    target_alignment: u16,
) -> Vec<u8> {
    let mut extra_bytes = Vec::new();
    if use_zip64 {
        let z64 = ZipExtraFields::build_zip64_extra(
            Some(item.uncompressed_size),
            Some(item.compressed_size),
            None,
        );
        extra_bytes.extend_from_slice(&z64);
    }
    if item.is_encrypted && item.compression_method == 99 {
        let aes_extra = ZipExtraFields::build_winzip_aes_extra(item.actual_method);
        extra_bytes.extend_from_slice(&aes_extra);
    }
    let ts_extra = ZipExtraFields::build_extended_timestamp(item.mtime_epoch_secs);
    extra_bytes.extend_from_slice(&ts_extra);

    if target_alignment > 1 {
        let pad_len = AlignmentPaddingCalculator::calculate(
            lfh_offset,
            name_len,
            extra_bytes.len(),
            target_alignment,
        );
        if pad_len > 0 {
            let align_extra = build_alignment_extra_field(pad_len, target_alignment);
            extra_bytes.extend_from_slice(&align_extra);
        }
    }
    extra_bytes
}

fn write_local_file_entry(
    out: &mut Vec<u8>,
    item: &ZipCompressedItem,
    lfh_offset: u64,
    target_alignment: u16,
) {
    let (dos_time, dos_date) = unix_to_dos_time(item.mtime_epoch_secs);
    let name_bytes = item.rel_path.as_bytes();
    let use_zip64 = requires_zip64(item, lfh_offset);
    let extra_bytes = build_local_extra_bytes(
        item,
        lfh_offset,
        name_bytes.len(),
        use_zip64,
        target_alignment,
    );

    let version_needed = resolve_version_needed(item, use_zip64);
    let flag = if item.is_encrypted { 0x0801u16 } else { 0x0800u16 };
    let uncomp_size_field = clamp_u32_field(item.uncompressed_size);
    let comp_size_field = clamp_u32_field(item.compressed_size);

    out.extend_from_slice(&MAGIC_LFH.to_le_bytes());
    out.extend_from_slice(&version_needed.to_le_bytes());
    out.extend_from_slice(&flag.to_le_bytes());
    out.extend_from_slice(&item.compression_method.to_le_bytes());
    out.extend_from_slice(&dos_time.to_le_bytes());
    out.extend_from_slice(&dos_date.to_le_bytes());
    out.extend_from_slice(&item.crc32.to_le_bytes());
    out.extend_from_slice(&comp_size_field.to_le_bytes());
    out.extend_from_slice(&uncomp_size_field.to_le_bytes());
    out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    out.extend_from_slice(&(extra_bytes.len() as u16).to_le_bytes());
    out.extend_from_slice(name_bytes);
    out.extend_from_slice(&extra_bytes);
    out.extend_from_slice(&item.payload);
}

fn build_central_extra_bytes(
    item: &ZipCompressedItem,
    lfh_offset: u64,
    use_zip64: bool,
) -> Vec<u8> {
    let mut extra_bytes = Vec::new();
    if use_zip64 {
        let u_sz = (item.uncompressed_size >= 0xFFFFFFFF).then_some(item.uncompressed_size);
        let c_sz = (item.compressed_size >= 0xFFFFFFFF).then_some(item.compressed_size);
        let off = (lfh_offset >= 0xFFFFFFFF).then_some(lfh_offset);
        let z64 = ZipExtraFields::build_zip64_extra(u_sz, c_sz, off);
        extra_bytes.extend_from_slice(&z64);
    }
    if item.is_encrypted && item.compression_method == 99 {
        let aes_extra = ZipExtraFields::build_winzip_aes_extra(item.actual_method);
        extra_bytes.extend_from_slice(&aes_extra);
    }
    let ts_extra = ZipExtraFields::build_extended_timestamp(item.mtime_epoch_secs);
    extra_bytes.extend_from_slice(&ts_extra);
    extra_bytes
}

fn write_central_directory_entry(
    out: &mut Vec<u8>,
    item: &ZipCompressedItem,
    lfh_offset: u64,
) {
    let (dos_time, dos_date) = unix_to_dos_time(item.mtime_epoch_secs);
    let name_bytes = item.rel_path.as_bytes();
    let use_zip64 = requires_zip64(item, lfh_offset);
    let extra_bytes = build_central_extra_bytes(item, lfh_offset, use_zip64);

    let version_made_by = 0x031Eu16;
    let version_needed = resolve_version_needed(item, use_zip64);
    let flag = if item.is_encrypted { 0x0801u16 } else { 0x0800u16 };
    let uncomp_size_field = clamp_u32_field(item.uncompressed_size);
    let comp_size_field = clamp_u32_field(item.compressed_size);
    let lfh_offset_field = clamp_u32_field(lfh_offset);
    let ext_attr = (item.mode << 16) | if item.is_directory { 0x10 } else { 0 };

    out.extend_from_slice(&MAGIC_CDFH.to_le_bytes());
    out.extend_from_slice(&version_made_by.to_le_bytes());
    out.extend_from_slice(&version_needed.to_le_bytes());
    out.extend_from_slice(&flag.to_le_bytes());
    out.extend_from_slice(&item.compression_method.to_le_bytes());
    out.extend_from_slice(&dos_time.to_le_bytes());
    out.extend_from_slice(&dos_date.to_le_bytes());
    out.extend_from_slice(&item.crc32.to_le_bytes());
    out.extend_from_slice(&comp_size_field.to_le_bytes());
    out.extend_from_slice(&uncomp_size_field.to_le_bytes());
    out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    out.extend_from_slice(&(extra_bytes.len() as u16).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&ext_attr.to_le_bytes());
    out.extend_from_slice(&lfh_offset_field.to_le_bytes());
    out.extend_from_slice(name_bytes);
    out.extend_from_slice(&extra_bytes);
}

fn write_end_of_central_directory(
    out: &mut Vec<u8>,
    num_entries: u64,
    cd_size: u64,
    cd_offset: u64,
) {
    let is_zip64_required =
        num_entries >= 0xFFFF || cd_size >= 0xFFFFFFFF || cd_offset >= 0xFFFFFFFF;

    if is_zip64_required {
        let z64_eocd_pos = out.len() as u64;
        let z64_record_size = 44u64;

        out.extend_from_slice(&MAGIC_ZIP64_EOCD.to_le_bytes());
        out.extend_from_slice(&z64_record_size.to_le_bytes());
        out.extend_from_slice(&45u16.to_le_bytes());
        out.extend_from_slice(&45u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&num_entries.to_le_bytes());
        out.extend_from_slice(&num_entries.to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());

        out.extend_from_slice(&MAGIC_ZIP64_LOCATOR.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&z64_eocd_pos.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
    }

    let entries_field = if num_entries >= 0xFFFF { 0xFFFFu16 } else { num_entries as u16 };
    let cd_size_field = clamp_u32_field(cd_size);
    let cd_offset_field = clamp_u32_field(cd_offset);

    out.extend_from_slice(&MAGIC_EOCD.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&entries_field.to_le_bytes());
    out.extend_from_slice(&entries_field.to_le_bytes());
    out.extend_from_slice(&cd_size_field.to_le_bytes());
    out.extend_from_slice(&cd_offset_field.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
}
