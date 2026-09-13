// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! VFS remote directory listing and zero-download remote archive inspection.

use ttzip_device_android::error::DeviceError;

use super::registry::{block_on_driver, get_registered_driver};
use super::types::{UniFFIAndroidVfsNode, UniFFIVfsEntryType};
use crate::archive::source::device_source::DeviceArchiveSource;
use crate::archive::source::ArchiveSource;
use crate::uniffi_api::types::TTZipError;

/// Traverses and lists directory entries on the remote Android storage volume.
#[uniffi::export]
pub fn uniffi_list_device_directory(
    device_id: String,
    path: String,
) -> Result<Vec<UniFFIAndroidVfsNode>, TTZipError> {
    let driver = get_registered_driver(&device_id)?;

    let nodes = block_on_driver(driver.list_directory(&path))
        .map_err(|e: DeviceError| TTZipError::IoError {
            message: e.to_string(),
        })?;

    Ok(nodes.into_iter().map(Into::into).collect())
}

/// Inspects a remote archive on the Android device via zero-download partial reads.
#[uniffi::export]
pub fn uniffi_inspect_remote_archive(
    device_id: String,
    archive_path: String,
) -> Result<Vec<UniFFIAndroidVfsNode>, TTZipError> {
    let driver = get_registered_driver(&device_id)?;

    let parent_path = if let Some(idx) = archive_path.rfind('/') {
        if idx == 0 { "/" } else { &archive_path[..idx] }
    } else {
        "/"
    };

    let nodes = block_on_driver(driver.list_directory(parent_path))
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;

    let file_node = nodes
        .into_iter()
        .find(|n| n.path == archive_path)
        .ok_or_else(|| TTZipError::FileNotFound {
            path: archive_path.clone(),
        })?;

    if file_node.size_bytes < 22 {
        return Err(TTZipError::CorruptHeader {
            details: "Archive size is smaller than minimum ZIP EOCD (22 bytes)".to_string(),
            offset: 0,
        });
    }

    let source = DeviceArchiveSource::new(
        driver,
        &archive_path,
        file_node.size_bytes,
    );

    let search_len = 65557u64.min(file_node.size_bytes);
    let search_offset = file_node.size_bytes - search_len;
    let mut tail_buf = vec![0u8; search_len as usize];
    source
        .read_at(&mut tail_buf, search_offset)
        .map_err(|s| TTZipError::EngineError { code: s as i32 })?;

    let mut eocd_pos = None;
    for i in (0..=tail_buf.len().saturating_sub(22)).rev() {
        if &tail_buf[i..i + 4] == b"PK\x05\x06" {
            eocd_pos = Some(search_offset + i as u64);
            break;
        }
    }

    let cd_entries = if let Some(eocd_off) = eocd_pos {
        let eocd_rel = (eocd_off - search_offset) as usize;
        let mut total_entries = u16::from_le_bytes([tail_buf[eocd_rel + 10], tail_buf[eocd_rel + 11]]) as u64;
        let mut cd_size = u32::from_le_bytes([
            tail_buf[eocd_rel + 12], tail_buf[eocd_rel + 13],
            tail_buf[eocd_rel + 14], tail_buf[eocd_rel + 15],
        ]) as u64;
        let mut cd_offset = u32::from_le_bytes([
            tail_buf[eocd_rel + 16], tail_buf[eocd_rel + 17],
            tail_buf[eocd_rel + 18], tail_buf[eocd_rel + 19],
        ]) as u64;

        if (cd_offset == 0xFFFFFFFF || total_entries == 0xFFFF) && eocd_rel >= 20 {
            let loc_pos = eocd_rel - 20;
            if &tail_buf[loc_pos..loc_pos + 4] == b"PK\x06\x07" {
                let zip64_eocd_off = u64::from_le_bytes([
                    tail_buf[loc_pos + 8], tail_buf[loc_pos + 9],
                    tail_buf[loc_pos + 10], tail_buf[loc_pos + 11],
                    tail_buf[loc_pos + 12], tail_buf[loc_pos + 13],
                    tail_buf[loc_pos + 14], tail_buf[loc_pos + 15],
                ]);
                let mut zip64_buf = [0u8; 56];
                if source.read_at(&mut zip64_buf, zip64_eocd_off).is_ok() && &zip64_buf[0..4] == b"PK\x06\x06" {
                    total_entries = u64::from_le_bytes([
                        zip64_buf[32], zip64_buf[33], zip64_buf[34], zip64_buf[35],
                        zip64_buf[36], zip64_buf[37], zip64_buf[38], zip64_buf[39],
                    ]);
                    cd_size = u64::from_le_bytes([
                        zip64_buf[40], zip64_buf[41], zip64_buf[42], zip64_buf[43],
                        zip64_buf[44], zip64_buf[45], zip64_buf[46], zip64_buf[47],
                    ]);
                    cd_offset = u64::from_le_bytes([
                        zip64_buf[48], zip64_buf[49], zip64_buf[50], zip64_buf[51],
                        zip64_buf[52], zip64_buf[53], zip64_buf[54], zip64_buf[55],
                    ]);
                }
            }
        }

        let cd_read_len = cd_size.min(16 * 1024 * 1024) as usize;
        let mut cd_buf = vec![0u8; cd_read_len];
        source.read_at(&mut cd_buf, cd_offset)
            .map_err(|s| TTZipError::EngineError { code: s as i32 })?;

        parse_cdfh_buffer(&cd_buf, total_entries)
    } else {
        vec![]
    };

    Ok(cd_entries)
}

fn parse_cdfh_buffer(buf: &[u8], expected_count: u64) -> Vec<UniFFIAndroidVfsNode> {
    let mut entries = Vec::new();
    let mut cursor = 0;
    let mut count = 0;

    while cursor + 46 <= buf.len() && (expected_count == 0 || count < expected_count) {
        if &buf[cursor..cursor + 4] != b"PK\x01\x02" {
            break;
        }

        let uncomp_size = u32::from_le_bytes([
            buf[cursor + 24], buf[cursor + 25], buf[cursor + 26], buf[cursor + 27],
        ]) as u64;
        let name_len = u16::from_le_bytes([buf[cursor + 28], buf[cursor + 29]]) as usize;
        let extra_len = u16::from_le_bytes([buf[cursor + 30], buf[cursor + 31]]) as usize;
        let comment_len = u16::from_le_bytes([buf[cursor + 32], buf[cursor + 33]]) as usize;

        let name_start = cursor + 46;
        let name_end = name_start + name_len;
        if name_end > buf.len() {
            break;
        }

        let name_raw = String::from_utf8_lossy(&buf[name_start..name_end]).to_string();
        let is_dir = name_raw.ends_with('/');
        let display_name = name_raw.trim_end_matches('/').rsplit('/').next().unwrap_or(&name_raw).to_string();

        entries.push(UniFFIAndroidVfsNode {
            path: format!("/{}", name_raw.trim_end_matches('/')),
            name: display_name,
            entry_type: if is_dir { UniFFIVfsEntryType::Directory } else { UniFFIVfsEntryType::File },
            size_bytes: if is_dir { 0 } else { uncomp_size },
            modified_timestamp: 0,
            object_handle: None,
            is_restricted: false,
        });

        cursor = name_end + extra_len + comment_len;
        count += 1;
    }

    entries
}
