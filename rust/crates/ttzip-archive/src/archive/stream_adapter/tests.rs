// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Stream adapter unit tests.

use super::*;
use std::io::{Cursor, SeekFrom};

#[test]
fn test_stream_reader_state_reading() {
    let sample_data = b"Hello, TTZip streaming micro-buffer pipeline!";
    let cursor = Cursor::new(sample_data.to_vec());
    let mut state = StreamReaderState::new(cursor, 16);

    let (ptr, len) = state.read_chunk().expect("read_chunk should succeed");
    assert_eq!(len, sample_data.len());
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    assert_eq!(&slice[..sample_data.len()], sample_data);
    assert_eq!(state.bytes_consumed, sample_data.len() as u64);

    let (_, len2) = state.read_chunk().expect("read_chunk at EOF");
    assert_eq!(len2, 0);
    assert!(state.is_eof);
}

#[test]
fn test_stream_reader_state_seek_and_skip() {
    let sample_data = b"0123456789ABCDEF";
    let cursor = Cursor::new(sample_data.to_vec());
    let mut state = StreamReaderState::new(cursor, 64 * 1024);

    let skipped = state.skip(4).expect("skip forward");
    assert_eq!(skipped, 4);

    let new_pos = state.seek(SeekFrom::Start(10)).expect("seek to 10");
    assert_eq!(new_pos, 10);

    let new_pos2 = state.seek(SeekFrom::End(-2)).expect("seek from end");
    assert_eq!(new_pos2, 14);
}

#[test]
fn test_stream_writer_state_writing() {
    let mut output = Vec::new();
    let mut state = StreamWriterState::new(&mut output, 64 * 1024);

    let chunk1 = b"Chunk 1: Hello World; ";
    let chunk2 = b"Chunk 2: TTZip Rust Glue!";
    let n1 = state.write_chunk(chunk1).expect("write chunk 1");
    let n2 = state.write_chunk(chunk2).expect("write chunk 2");

    assert_eq!(n1, chunk1.len());
    assert_eq!(n2, chunk2.len());
    assert_eq!(state.bytes_written, (chunk1.len() + chunk2.len()) as u64);
    assert_eq!(output, [chunk1.as_slice(), chunk2.as_slice()].concat());
}

#[test]
fn test_read_callback_trampoline_panic_catch() {
    unsafe {
        let res = archive_read_callback_trampoline::<Cursor<Vec<u8>>>(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert_eq!(res, ARCHIVE_FATAL as libc::ssize_t);
    }
}

#[test]
fn test_seek_callback_trampoline_panic_catch() {
    unsafe {
        let res = archive_seek_callback_trampoline::<Cursor<Vec<u8>>>(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            libc::SEEK_SET,
        );
        assert_eq!(res, ARCHIVE_FATAL as i64);
    }
}

struct SparseCopy7zReader {
    sig_header: [u8; 32],
    tail_offset: u64,
    tail_bytes: Vec<u8>,
    total_len: u64,
    pos: u64,
    prefix_bytes_read: u64,
}

impl std::io::Read for SparseCopy7zReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.total_len || buf.is_empty() {
            return Ok(0);
        }
        if self.pos < 32 {
            let avail = (32 - self.pos as usize).min(buf.len());
            let start = self.pos as usize;
            buf[..avail].copy_from_slice(&self.sig_header[start..start + avail]);
            self.pos += avail as u64;
            return Ok(avail);
        }
        if self.pos < self.tail_offset {
            let avail = ((self.tail_offset - self.pos) as usize).min(buf.len());
            buf[..avail].fill(0xAA);
            self.pos += avail as u64;
            self.prefix_bytes_read += avail as u64;
            return Ok(avail);
        }
        let rel = (self.pos - self.tail_offset) as usize;
        let avail = (self.tail_bytes.len().saturating_sub(rel)).min(buf.len());
        buf[..avail].copy_from_slice(&self.tail_bytes[rel..rel + avail]);
        self.pos += avail as u64;
        Ok(avail)
    }
}

impl std::io::Seek for SparseCopy7zReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let next = match pos {
            SeekFrom::Start(o) => i128::from(o),
            SeekFrom::Current(d) => i128::from(self.pos) + i128::from(d),
            SeekFrom::End(d) => i128::from(self.total_len) + i128::from(d),
        };
        if next < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "negative seek",
            ));
        }
        self.pos = next as u64;
        Ok(self.pos)
    }
}

fn build_sparse_large_copy_7z(prefix_len: u64, jpeg_payload: &[u8], gallery_payload: &[u8]) -> SparseCopy7zReader {
    use crate::crypto::crc32::crc32_fast;
    use crate::sevenz::format::{SevenZSignatureHeader, METHOD_COPY};
    use crate::sevenz::writer::build_7z_metadata_header;
    use crate::zip::writer::ZipInputItem;

    let items = vec![
        ZipInputItem {
            rel_path: "2026.7.31 哲学在证明开始以前/通用4k.mp4".to_string(),
            data: vec![1],
            mtime_epoch_secs: 1_700_000_000,
            mode: 0o100644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "2026.7.31 哲学在证明开始以前/布劳威尔.jpeg".to_string(),
            data: vec![1],
            mtime_epoch_secs: 1_700_000_000,
            mode: 0o100644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "2026.7.31 哲学在证明开始以前/图片/希尔伯特.jpg".to_string(),
            data: vec![1],
            mtime_epoch_secs: 1_700_000_000,
            mode: 0o100644,
            is_directory: false,
        },
    ];
    let stream_sizes = [
        prefix_len,
        jpeg_payload.len() as u64,
        gallery_payload.len() as u64,
    ];
    let stream_crcs = [
        0x1234_5678,
        crc32_fast(0, jpeg_payload),
        crc32_fast(0, gallery_payload),
    ];
    let total_uncompressed: u64 = stream_sizes.iter().sum();
    let end_header = build_7z_metadata_header(
        &items,
        &stream_sizes,
        &stream_crcs,
        total_uncompressed,
        total_uncompressed,
        METHOD_COPY,
        &[],
    );
    let sig_header = SevenZSignatureHeader {
        major_version: 0,
        minor_version: 4,
        start_header_crc: 0,
        next_header_offset: total_uncompressed,
        next_header_size: end_header.len() as u64,
        next_header_crc: crc32_fast(0, &end_header),
    }
    .serialize();

    let mut tail_bytes = Vec::with_capacity(jpeg_payload.len() + gallery_payload.len() + end_header.len());
    tail_bytes.extend_from_slice(jpeg_payload);
    tail_bytes.extend_from_slice(gallery_payload);
    tail_bytes.extend_from_slice(&end_header);
    let tail_offset = 32 + prefix_len;
    let total_len = tail_offset + tail_bytes.len() as u64;

    SparseCopy7zReader {
        sig_header,
        tail_offset,
        tail_bytes,
        total_len,
        pos: 0,
        prefix_bytes_read: 0,
    }
}

#[test]
fn test_issue_304_selected_and_single_entry_beyond_10gib_prefix() {
    use crate::archive::unified::selection::EntrySelection;
    use crate::fs::collision::staging::StagingRoot;
    use crate::types::TTZipExtractOptions;

    let prefix_12_gib = 12 * 1024 * 1024 * 1024u64;
    let jpeg_bytes: Vec<u8> = (0..9441).map(|i| ((i * 37 + 13) & 0xFF) as u8).collect();
    let gallery_bytes: Vec<u8> = (0..4096).map(|i| ((i * 53 + 7) & 0xFF) as u8).collect();
    let mut reader = build_sparse_large_copy_7z(prefix_12_gib, &jpeg_bytes, &gallery_bytes);

    let tmp = tempfile::tempdir().expect("tempdir");
    let staging = StagingRoot(tmp.path().join("staging"));
    let selection = EntrySelection::new(
        [
            "2026.7.31 哲学在证明开始以前/布劳威尔.jpeg",
            "2026.7.31 哲学在证明开始以前/图片/希尔伯特.jpg",
        ],
        false,
    );
    let count = extract_sevenz_selected_from_reader(
        &mut reader,
        &selection,
        &staging,
        &TTZipExtractOptions::default(),
        None,
    )
    .expect("selective extraction past 12 GiB prefix must succeed");
    assert_eq!(count, 2);
    assert_eq!(
        reader.prefix_bytes_read, 0,
        "unencrypted METHOD_COPY must seek over the 12 GiB prefix without reading it"
    );
    assert_eq!(
        std::fs::read(staging.path().join("2026.7.31 哲学在证明开始以前/布劳威尔.jpeg")).expect("read jpeg"),
        jpeg_bytes
    );
    assert_eq!(
        std::fs::read(staging.path().join("2026.7.31 哲学在证明开始以前/图片/希尔伯特.jpg")).expect("read gallery"),
        gallery_bytes
    );

    let previewed = extract_7z_single_entry_from_seekable(
        &mut reader,
        Some("2026.7.31 哲学在证明开始以前/布劳威尔.jpeg"),
        -1,
        None,
    )
    .expect("single-entry seekable preview past 12 GiB prefix must succeed");
    assert_eq!(previewed, jpeg_bytes);
    assert_eq!(reader.prefix_bytes_read, 0);
}

#[test]
fn test_issue_304_default_selected_quota_guard_preserves_bomb_defenses() {
    use super::sevenz_selected::{default_selected_quota_guard, SelectedTarget};
    use crate::sevenz::header::{SevenZFolder, SevenZHeaderInfo};
    use crate::types::TTZipStatus;
    use std::collections::BTreeMap;

    let mut info = SevenZHeaderInfo::default();
    info.folders.push(SevenZFolder {
        packed_len: 32,
        ..Default::default()
    });
    let mut folder_map = BTreeMap::new();
    folder_map.insert(
        0,
        vec![SelectedTarget {
            rel_path: "bomb.bin".to_string(),
            out_rel: "bomb.bin".to_string(),
            offset_start: 0,
            offset_end: 2 * 1024 * 1024,
            uncompressed_size: 2 * 1024 * 1024,
            expected_crc: None,
            is_symlink: false,
        }],
    );

    let mut guard = default_selected_quota_guard(&info, &folder_map);
    guard.cumulative_compressed_bytes = 32;
    assert_eq!(
        guard.observe_and_validate(0, 2 * 1024 * 1024),
        Err(TTZipStatus::ErrSecurityViolation),
        "ratio > 50,000:1 must still trip ErrSecurityViolation"
    );
    assert_eq!(
        guard.observe_and_validate(1024 * 1024, 11 * 1024 * 1024 * 1024),
        Err(TTZipStatus::ErrSecurityViolation),
        "uncompressed output exceeding 10 GiB from a tiny folder must trip ErrSecurityViolation"
    );
}


