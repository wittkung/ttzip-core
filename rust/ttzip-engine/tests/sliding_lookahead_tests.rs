// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and integration test suite for `SlidingLookaheadReader` and `LookaheadRead`.
//!
//! Validates:
//! 1. Mathematical slice consistency across 1B, 7B, 64KB, and 1MB chunks.
//! 2. Seamless 8 KB micro-buffer loop discard degradation for non-seekable pipes / streams.
//! 3. Monotonic buffer capacity expansion and zero-churn reuse.
//! 4. Strict EOF boundary error handling (`UnexpectedEof`).
//! 5. `Read`, `BufRead`, and `Seek` trait standard compliance.

use std::io::{BufRead, Cursor, ErrorKind, Read, Seek, SeekFrom};
use ttzip_engine::archive::unified::{
    LookaheadRead, SlidingLookaheadReader, DEFAULT_INITIAL_LOOKAHEAD_CAPACITY,
    MAX_LOOKAHEAD_CAPACITY,
};

/// Non-seekable wrapper simulating standard Unix pipes, TCP sockets, or streaming responses.
struct NonSeekablePipe<R> {
    inner: R,
}

impl<R> NonSeekablePipe<R> {
    fn new(inner: R) -> Self {
        Self { inner }
    }
}

impl<R: Read> Read for NonSeekablePipe<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

/// Deterministically generates a test payload of given size.
fn generate_deterministic_payload(size: usize) -> Vec<u8> {
    (0..size)
        .map(|i| ((i ^ (i >> 8) ^ (i >> 16)) % 251) as u8)
        .collect()
}

#[test]
fn test_peek_ahead_and_consume_1b_mathematical_consistency() {
    let payload = generate_deterministic_payload(16 * 1024);
    let mut reader = SlidingLookaheadReader::new(Cursor::new(payload.clone()));

    for (i, &expected_byte) in payload.iter().enumerate() {
        let peeked = reader.peek_ahead(1).expect("peek 1B failed");
        assert_eq!(
            peeked[0], expected_byte,
            "byte mismatch at index {i}: expected {expected_byte}, got {}",
            peeked[0]
        );
        reader.consume(1).expect("consume 1B failed");
        assert_eq!(reader.total_consumed(), (i + 1) as u64);
    }

    // Verify EOF detection
    assert_eq!(
        reader.peek_ahead(1).unwrap_err().kind(),
        ErrorKind::UnexpectedEof
    );
    assert!(reader.is_eof());
    assert_eq!(
        reader.consume(1).unwrap_err().kind(),
        ErrorKind::UnexpectedEof
    );
}

#[test]
fn test_peek_ahead_and_consume_7b_odd_prime_slices() {
    let total_size = 35 * 1024 + 13;
    let payload = generate_deterministic_payload(total_size);
    let mut reader = SlidingLookaheadReader::new(Cursor::new(payload.clone()));
    let slice_size = 7;
    let mut offset = 0;

    while offset + slice_size <= total_size {
        let peeked = reader.peek_ahead(slice_size).expect("peek 7B failed");
        assert!(peeked.len() >= slice_size);
        assert_eq!(
            &peeked[..slice_size],
            &payload[offset..offset + slice_size],
            "slice mismatch at offset {offset}"
        );
        reader.consume(slice_size).expect("consume 7B failed");
        offset += slice_size;
        assert_eq!(reader.total_consumed(), offset as u64);
    }

    let remainder = total_size - offset;
    if remainder > 0 {
        let peeked = reader.peek_ahead(remainder).expect("peek remainder failed");
        assert_eq!(&peeked[..remainder], &payload[offset..]);
        reader.consume(remainder).expect("consume remainder failed");
    }

    assert_eq!(reader.total_consumed(), total_size as u64);
    assert_eq!(
        reader.peek_ahead(1).unwrap_err().kind(),
        ErrorKind::UnexpectedEof
    );
    assert!(reader.is_eof());
}

#[test]
fn test_peek_ahead_and_consume_64kb_aligned_slices() {
    let chunk_size = 64 * 1024;
    let total_size = 512 * 1024;
    let payload = generate_deterministic_payload(total_size);
    let mut reader = SlidingLookaheadReader::new(Cursor::new(payload.clone()));

    let mut offset = 0;
    while offset < total_size {
        let peeked = reader.peek_ahead(chunk_size).expect("peek 64KB failed");
        assert!(peeked.len() >= chunk_size);
        assert_eq!(
            &peeked[..chunk_size],
            &payload[offset..offset + chunk_size],
            "64KB chunk mismatch at offset {offset}"
        );
        reader.consume(chunk_size).expect("consume 64KB failed");
        offset += chunk_size;
        assert_eq!(reader.total_consumed(), offset as u64);
    }

    assert_eq!(
        reader.peek_ahead(1).unwrap_err().kind(),
        ErrorKind::UnexpectedEof
    );
    assert!(reader.is_eof());
}

#[test]
fn test_peek_ahead_and_consume_1mb_large_slices() {
    let total_size = 4 * 1024 * 1024; // 4 MB
    let slice_size = 1024 * 1024; // 1 MB
    let payload = generate_deterministic_payload(total_size);
    let mut reader = SlidingLookaheadReader::new(Cursor::new(payload.clone()));

    let mut offset = 0;
    while offset < total_size {
        let peeked = reader.peek_ahead(slice_size).expect("peek 1MB failed");
        assert!(peeked.len() >= slice_size);
        assert_eq!(
            &peeked[..slice_size],
            &payload[offset..offset + slice_size],
            "1MB slice mismatch at offset {offset}"
        );
        reader.consume(slice_size).expect("consume 1MB failed");
        offset += slice_size;
        assert_eq!(reader.total_consumed(), offset as u64);
    }

    assert_eq!(
        reader.peek_ahead(1).unwrap_err().kind(),
        ErrorKind::UnexpectedEof
    );
    assert!(reader.is_eof());
    assert!(reader.capacity() >= slice_size);
}

#[test]
fn test_non_seekable_pipe_stream_skip_degradation() {
    let total_size = 256 * 1024 + 777;
    let payload = generate_deterministic_payload(total_size);
    let pipe = NonSeekablePipe::new(Cursor::new(payload.clone()));
    let mut reader = SlidingLookaheadReader::new(pipe);

    // Initial peek to buffer some data
    let peeked = reader.peek_ahead(100).expect("initial peek failed");
    assert_eq!(&peeked[..100], &payload[..100]);

    // Skip 30 bytes from buffered area
    let skipped = reader.stream_skip(30).expect("skip 30 bytes failed");
    assert_eq!(skipped, 30);
    assert_eq!(reader.total_consumed(), 30);

    // Skip 70,000 bytes spanning beyond initial 64KB buffer into unbuffered stream
    let skip_large = 70_000u64;
    let skipped_large = reader
        .stream_skip(skip_large)
        .expect("large stream_skip failed");
    assert_eq!(skipped_large, skip_large);
    let current_offset = (30 + skip_large) as usize;
    assert_eq!(reader.total_consumed(), current_offset as u64);

    // Verify next peek matches exact bytes in payload after non-seekable skip
    let check_len = 512;
    let peeked_after_skip = reader
        .peek_ahead(check_len)
        .expect("peek after skip failed");
    assert_eq!(
        &peeked_after_skip[..check_len],
        &payload[current_offset..current_offset + check_len],
        "data mismatch after non-seekable adaptive skip"
    );

    // Skip remaining data to EOF
    let remaining = (total_size - current_offset) as u64;
    let skipped_end = reader
        .stream_skip(remaining)
        .expect("skip to EOF failed");
    assert_eq!(skipped_end, remaining);
    assert_eq!(reader.total_consumed(), total_size as u64);

    // Attempt skip past EOF
    let skipped_past_eof = reader.stream_skip(1024).expect("skip past EOF");
    assert_eq!(skipped_past_eof, 0);
}

#[test]
fn test_buffer_monotonic_expansion_and_reuse_zero_leak() {
    let payload = generate_deterministic_payload(2 * 1024 * 1024);
    let mut reader = SlidingLookaheadReader::new(Cursor::new(payload.clone()));

    assert_eq!(reader.capacity(), DEFAULT_INITIAL_LOOKAHEAD_CAPACITY);

    // Expand buffer to 128 KB
    let peek_128k = 128 * 1024;
    let peeked = reader.peek_ahead(peek_128k).expect("peek 128KB failed");
    assert_eq!(&peeked[..peek_128k], &payload[..peek_128k]);
    assert!(reader.capacity() >= peek_128k);
    let cap_after_128k = reader.capacity();

    // Consume entire 128 KB
    reader.consume(peek_128k).expect("consume 128KB failed");

    // Peeking smaller 32 KB chunk should reuse existing buffer capacity without shrinking or reallocation
    let peek_32k = 32 * 1024;
    let peeked_small = reader.peek_ahead(peek_32k).expect("peek 32KB failed");
    assert_eq!(
        &peeked_small[..peek_32k],
        &payload[peek_128k..peek_128k + peek_32k]
    );
    assert_eq!(reader.capacity(), cap_after_128k);

    // Partial consume and cross-boundary in-place sliding compaction
    reader.consume(10 * 1024).expect("consume 10KB failed");
    let current_offset = peek_128k + 10 * 1024;
    let peek_60k = 60 * 1024;
    let peeked_cross = reader.peek_ahead(peek_60k).expect("peek 60KB failed");
    assert_eq!(
        &peeked_cross[..peek_60k],
        &payload[current_offset..current_offset + peek_60k]
    );
    assert_eq!(reader.capacity(), cap_after_128k);
}

#[test]
fn test_exceeding_max_resident_lookahead_limit_fails() {
    let payload = generate_deterministic_payload(1024);
    let mut reader = SlidingLookaheadReader::new(Cursor::new(payload));

    let excessive_size = MAX_LOOKAHEAD_CAPACITY + 1;
    let err = reader.peek_ahead(excessive_size).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidInput);
}

#[test]
fn test_std_io_read_and_bufread_compatibility() {
    let payload = generate_deterministic_payload(128 * 1024);
    let mut reader = SlidingLookaheadReader::new(Cursor::new(payload.clone()));

    // Peek 500 bytes first
    let peeked = reader.peek_ahead(500).expect("peek 500B failed");
    assert_eq!(&peeked[..500], &payload[..500]);

    // Read 200 bytes using std::io::Read
    let mut read_buf = [0u8; 200];
    reader.read_exact(&mut read_buf).expect("read_exact 200B failed");
    assert_eq!(&read_buf[..], &payload[..200]);
    assert_eq!(reader.total_consumed(), 200);

    // Fill buf via BufRead
    let buf_slice = reader.fill_buf().expect("fill_buf failed");
    assert_eq!(&buf_slice[..300], &payload[200..500]);
    reader.consume(300).expect("consume 300B failed");
    assert_eq!(reader.total_consumed(), 500);

    // Read all remaining bytes using std::io::copy into memory buffer
    let mut output = Vec::new();
    std::io::copy(&mut reader, &mut output).expect("copy failed");
    assert_eq!(output.len(), payload.len() - 500);
    assert_eq!(&output[..], &payload[500..]);
    assert_eq!(reader.total_consumed(), payload.len() as u64);
}

#[test]
fn test_seekable_seek_and_position_tracking() {
    let payload = generate_deterministic_payload(100 * 1024);
    let mut reader = SlidingLookaheadReader::new_seekable(Cursor::new(payload.clone()));

    // Initial position
    assert_eq!(reader.stream_position().unwrap(), 0);

    // Peek ahead 1000 bytes (logical pos should remain 0)
    let peeked = reader.peek_ahead(1000).expect("peek 1000B failed");
    assert_eq!(&peeked[..1000], &payload[..1000]);
    assert_eq!(reader.stream_position().unwrap(), 0);

    // Consume 100 bytes (logical pos becomes 100)
    reader.consume(100).expect("consume 100B failed");
    assert_eq!(reader.stream_position().unwrap(), 100);

    // Seek relative within buffered window
    reader.seek(SeekFrom::Current(50)).expect("seek relative");
    assert_eq!(reader.stream_position().unwrap(), 150);

    // Seek absolute outside buffered window
    reader.seek(SeekFrom::Start(50_000)).expect("seek start 50k");
    assert_eq!(reader.stream_position().unwrap(), 50_000);

    let peeked_50k = reader.peek_ahead(200).expect("peek at 50k failed");
    assert_eq!(&peeked_50k[..200], &payload[50_000..50_200]);

    // Fast seek_skip
    let skipped = reader.seek_skip(10_000).expect("seek_skip failed");
    assert_eq!(skipped, 10_000);
    assert_eq!(reader.stream_position().unwrap(), 60_000);
}

#[test]
fn test_slice_lookahead_read_implementation() {
    let payload = generate_deterministic_payload(1024);
    let mut slice: &[u8] = &payload;

    let peeked = slice.peek_ahead(64).expect("slice peek 64B");
    assert_eq!(&peeked[..64], &payload[..64]);

    LookaheadRead::consume(&mut slice, 64).expect("slice consume 64B");
    assert_eq!(slice.len(), 1024 - 64);

    let skipped = slice.stream_skip(100).expect("slice stream_skip 100B");
    assert_eq!(skipped, 100);
    assert_eq!(slice.len(), 1024 - 164);
}

#[test]
fn test_cursor_lookahead_read_implementation() {
    let payload = generate_deterministic_payload(1024);
    let mut cursor = Cursor::new(payload.clone());

    let peeked = cursor.peek_ahead(128).expect("cursor peek 128B");
    assert_eq!(&peeked[..128], &payload[..128]);

    LookaheadRead::consume(&mut cursor, 128).expect("cursor consume 128B");
    assert_eq!(cursor.position(), 128);

    let skipped = cursor.stream_skip(200).expect("cursor skip 200B");
    assert_eq!(skipped, 200);
    assert_eq!(cursor.position(), 328);
}
