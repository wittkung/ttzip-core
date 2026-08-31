// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for `bzip2recover` 48-bit Pi magic scanner and disaster recovery.

use ttzip_engine::codecs::bzip2::{bzip2_compress_vec, bzip2_decompress_vec};
use ttzip_engine::codecs::bzip2::recover::{bzip2_recover_block, bzip2_scan_blocks};

#[test]
fn test_bzip2_scan_and_recover_single_block() {
    let payload = b"Recoverable block test in Bzip2 archive.";
    let compressed = bzip2_compress_vec(payload, 9).unwrap();

    let slices = bzip2_scan_blocks(&compressed);
    assert_eq!(slices.len(), 1);

    let recovered_bz2 = bzip2_recover_block(&compressed, &slices[0]).unwrap();
    let decompressed = bzip2_decompress_vec(&recovered_bz2).unwrap();
    assert_eq!(&decompressed, payload);
}
