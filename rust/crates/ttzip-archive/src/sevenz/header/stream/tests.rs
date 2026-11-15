// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use super::*;

#[test]
fn test_read_optional_crc_list_and_substreams_reject_oversized_counts_on_short_header() {
    let mut hpos = 0usize;
    assert_eq!(
        read_optional_crc_list(&[0x01u8, 0x00], &mut hpos, 10_000_000),
        Err(TTZipStatus::ErrCorruptHeader)
    );
    let mut hpos_mask = 0usize;
    assert_eq!(
        read_optional_crc_list(&[0x00u8, 0xFF], &mut hpos_mask, 500_000),
        Err(TTZipStatus::ErrCorruptHeader)
    );
    let header_bytes = [K_NUM_UNPACK_STREAM, 0xE0, 0x40, 0x4B, 0x4C, K_CRC, 0x01, K_END];
    let mut info = SevenZHeaderInfo {
        folders: vec![SevenZFolder::default()],
        ..Default::default()
    };
    let mut pos = 0usize;
    assert_eq!(
        parse_substreams_info_section(&header_bytes, &mut pos, &mut info, HeaderAdmission::Standard),
        Err(TTZipStatus::ErrCorruptHeader)
    );
}
