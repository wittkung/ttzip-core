// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use super::*;

fn run_differential(window: &[u8], max_pos: usize) {
    let mut scalar = Vec::new();
    find_eocd_candidates_scalar(window, max_pos, &mut scalar);

    #[cfg(target_arch = "aarch64")]
    {
        let mut neon = Vec::new();
        unsafe {
            find_eocd_candidates_neon(window, max_pos, &mut neon);
        }
        assert_eq!(
            neon, scalar,
            "NEON scanner output deviated from scalar reference for window len {} at max_pos {}",
            window.len(),
            max_pos
        );
    }

    let unified = find_eocd_candidate_offsets(window);
    if window.len() >= EOCD_MIN_SIZE && max_pos == window.len() - EOCD_MIN_SIZE {
        assert_eq!(unified, scalar);
    }
}

#[test]
fn test_eocd_empty_and_sub_minimum_window() {
    assert!(find_eocd_candidate_offsets(&[]).is_empty());
    assert!(find_eocd_candidate_offsets(&[b'P', b'K', 0x05, 0x06]).is_empty());
    assert!(find_eocd_candidate_offsets(&[0u8; 21]).is_empty());
}

#[test]
fn test_differential_all_single_offsets() {
    for buf_len in [25, 36, 37, 48, 64, 128, 256] {
        let max_pos = buf_len - EOCD_MIN_SIZE;
        for target_pos in 0..=max_pos {
            let mut buf = vec![0xAA; buf_len];
            buf[target_pos..target_pos + 4].copy_from_slice(&MAGIC_EOCD.to_le_bytes());
            run_differential(&buf, max_pos);
        }
    }
}

#[test]
fn test_differential_multiple_and_consecutive_signatures() {
    let mut buf = vec![0x55; 160];
    let max_pos = buf.len() - EOCD_MIN_SIZE;

    for &pos in &[0, 4, 8, 12, 15, 16, 20, 31, 32, 45, 60, 90, 100] {
        if pos <= max_pos {
            buf[pos..pos + 4].copy_from_slice(&MAGIC_EOCD.to_le_bytes());
        }
    }
    run_differential(&buf, max_pos);
}

#[test]
fn test_differential_high_entropy_and_p_noise() {
    let mut buf = Vec::with_capacity(1024);
    for i in 0..1024 {
        match i % 4 {
            0 => buf.push(b'P'),
            1 => buf.push(b'K'),
            2 => buf.push(0x05),
            _ => buf.push(0x07),
        }
    }
    let max_pos = buf.len() - EOCD_MIN_SIZE;
    run_differential(&buf, max_pos);

    buf[33..37].copy_from_slice(&MAGIC_EOCD.to_le_bytes());
    run_differential(&buf, max_pos);
}

#[test]
fn test_differential_large_65kb_comment_window() {
    let mut lcg = 0x12345678u64;
    let mut buf = vec![0u8; MAX_EOCD_SEARCH_WINDOW];
    for b in buf.iter_mut() {
        lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1);
        *b = (lcg >> 32) as u8;
    }

    let max_pos = buf.len() - EOCD_MIN_SIZE;
    buf[0..4].copy_from_slice(&MAGIC_EOCD.to_le_bytes());
    buf[15..19].copy_from_slice(&MAGIC_EOCD.to_le_bytes());
    buf[512..516].copy_from_slice(&MAGIC_EOCD.to_le_bytes());
    buf[max_pos - 1..max_pos + 3].copy_from_slice(&MAGIC_EOCD.to_le_bytes());
    buf[max_pos..max_pos + 4].copy_from_slice(&MAGIC_EOCD.to_le_bytes());

    run_differential(&buf, max_pos);
}

#[test]
fn test_scan_slice_and_reader_roundtrip() {
    let cdfh_offset = 0usize;
    let mut zip_data = Vec::new();

    // CDFH (Central Directory File Header, 46 bytes min)
    zip_data.extend_from_slice(&MAGIC_CDFH.to_le_bytes());
    zip_data.extend_from_slice(&[0u8; 42]);

    let cd_size = (zip_data.len() - cdfh_offset) as u32;
    let comment = b"hello";

    // EOCD (22 bytes + comment)
    zip_data.extend_from_slice(&MAGIC_EOCD.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&1u16.to_le_bytes());
    zip_data.extend_from_slice(&1u16.to_le_bytes());
    zip_data.extend_from_slice(&cd_size.to_le_bytes());
    zip_data.extend_from_slice(&(cdfh_offset as u32).to_le_bytes());
    zip_data.extend_from_slice(&(comment.len() as u16).to_le_bytes());
    zip_data.extend_from_slice(comment);

    let slice_info = EocdScanner::scan_slice(&zip_data).expect("slice scan failed");
    assert_eq!(slice_info.eocd_offset, 46);
    assert_eq!(slice_info.total_entries, 1);
    assert_eq!(slice_info.cd_size, cd_size as u64);
    assert_eq!(slice_info.cd_offset, cdfh_offset as u64);
    assert_eq!(slice_info.comment, comment);

    let mut cursor = std::io::Cursor::new(&zip_data);
    let stream_info = EocdScanner::scan(&mut cursor, zip_data.len() as u64)
        .expect("stream scan failed");
    assert_eq!(stream_info, slice_info);
}
