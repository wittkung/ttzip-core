// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for Bzip2 48-bit Pi magic block container encoding and decoding.

use ttzip_engine::codecs::bzip2::block::{decode_bzip2_block, encode_bzip2_block, BitWriter};
use ttzip_engine::codecs::bzip2::crc::Bzip2CombinedCrc;
use ttzip_engine::codecs::bzip2::huffman::BitReader;

#[test]
fn test_block_encode_decode_roundtrip() {
    let payload = b"Hello world! This is a test of the 48-bit Pi Bzip2 block container format.";
    let mut writer = BitWriter::new();
    let mut enc_combined_crc = Bzip2CombinedCrc::new();

    encode_bzip2_block(payload, &mut writer, &mut enc_combined_crc).unwrap();
    writer.flush_to_byte_boundary();

    let mut reader = BitReader::new(&writer.buf);
    let mut decoded = Vec::new();
    let mut dec_combined_crc = Bzip2CombinedCrc::new();

    let has_more = decode_bzip2_block(&mut reader, &mut decoded, &mut dec_combined_crc).unwrap();
    assert!(has_more);
    assert_eq!(&decoded, payload);
    assert_eq!(enc_combined_crc.finalize(), dec_combined_crc.finalize());
}
