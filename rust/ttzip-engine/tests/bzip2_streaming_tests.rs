// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for Bzip2 streaming Reader and Writer.

use std::io::{Read, Write};
use ttzip_engine::codecs::bzip2::{Bzip2Reader, Bzip2Writer};

#[test]
fn test_streaming_reader_writer_roundtrip() {
    let original = b"Streaming reader and writer test for Bzip2 chunked processing.";
    let mut compressed_buf = Vec::new();

    {
        let mut writer = Bzip2Writer::new(&mut compressed_buf, 9);
        writer.write_all(original).unwrap();
        writer.finish().unwrap();
    }

    let mut reader = Bzip2Reader::new(&compressed_buf[..]).unwrap();
    let mut decompressed = Vec::new();
    reader.read_to_end(&mut decompressed).unwrap();

    assert_eq!(&decompressed, original);
}
