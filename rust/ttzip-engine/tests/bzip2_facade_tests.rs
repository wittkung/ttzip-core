// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for Bzip2 facade APIs and validation.

use ttzip_engine::codecs::bzip2::{
    bzip2_compress, bzip2_compress_bound, bzip2_compress_vec, bzip2_decompress,
    bzip2_decompress_vec, bzip2_inspect_header, bzip2_validate,
};

#[test]
fn test_facade_buffer_and_vec_apis() {
    let payload = b"Testing facade buffer APIs for Bzip2 pure safe Rust engine.";
    let bound = bzip2_compress_bound(payload.len());
    let mut dst_comp = vec![0u8; bound];

    let comp_len = bzip2_compress(payload, &mut dst_comp, 6).unwrap();
    assert!(comp_len > 0);
    assert!(bzip2_validate(&dst_comp[..comp_len]));

    let header_info = bzip2_inspect_header(&dst_comp[..comp_len]).unwrap();
    assert_eq!(header_info.block_size_100k, 6);
    assert_eq!(header_info.block_size_bytes, 600_000);

    let mut dst_decomp = vec![0u8; payload.len() + 100];
    let decomp_len = bzip2_decompress(&dst_comp[..comp_len], &mut dst_decomp).unwrap();
    assert_eq!(&dst_decomp[..decomp_len], payload);

    // Vector APIs
    let comp_vec = bzip2_compress_vec(payload, 9).unwrap();
    let decomp_vec = bzip2_decompress_vec(&comp_vec).unwrap();
    assert_eq!(&decomp_vec, payload);
}
