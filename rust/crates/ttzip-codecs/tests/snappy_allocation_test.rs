// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use ttzip_codecs::snappy::{
    SnappyError, raw_decompress_to_vec, snappy_compress_raw, snappy_decompress,
    snappy_decompress_raw, snappy_decompress_to_vec,
};
use ttzip_codecs::types::TTZipStatus;

struct TrackingAllocator;

thread_local! {
    static LARGEST_REQUEST: Cell<Option<usize>> = const { Cell::new(None) };
}

fn record_request(size: usize) {
    let _ = LARGEST_REQUEST.try_with(|largest| {
        if let Some(previous) = largest.get() {
            largest.set(Some(previous.max(size)));
        }
    });
}

// SAFETY: Each operation forwards the caller's layout and pointer unchanged to System.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_request(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_request(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record_request(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

fn largest_request<T>(decode: impl FnOnce() -> T) -> (T, usize) {
    LARGEST_REQUEST.with(|largest| largest.set(Some(0)));
    let result = decode();
    let largest = LARGEST_REQUEST.with(|largest| largest.replace(None).expect("tracking enabled"));
    (result, largest)
}

const MALFORMED_64K: [u8; 6] = [0x80, 0x80, 0x04, 0x00, 0x01, 0x02];

#[test]
fn status_decoder_rejects_impossible_length_before_output_allocation() {
    let (result, largest) = largest_request(|| snappy_decompress_raw(&MALFORMED_64K));
    assert_eq!(result, Err(TTZipStatus::ErrExtractionFailed));
    assert_eq!(
        largest, 0,
        "malformed raw block requested {largest} output bytes"
    );
}

#[test]
fn status_alias_rejects_impossible_length_before_output_allocation() {
    let (result, largest) = largest_request(|| snappy_decompress_to_vec(&MALFORMED_64K));
    assert_eq!(result, Err(TTZipStatus::ErrExtractionFailed));
    assert_eq!(
        largest, 0,
        "malformed raw block requested {largest} output bytes"
    );
}

#[test]
fn typed_decoder_rejects_impossible_length_before_output_allocation() {
    let (result, largest) = largest_request(|| raw_decompress_to_vec(&MALFORMED_64K));
    assert_eq!(result, Err(SnappyError::DecompressionFailed));
    assert_eq!(
        largest, 0,
        "malformed raw block requested {largest} output bytes"
    );
}

#[test]
fn valid_large_blocks_keep_their_existing_decode_contract() {
    let input = vec![0x5a; 1024 * 1024];
    let compressed = snappy_compress_raw(&input).expect("compress valid block");
    assert_eq!(
        snappy_decompress_raw(&compressed).expect("status decode"),
        input
    );
    assert_eq!(
        snappy_decompress_to_vec(&compressed).expect("alias decode"),
        input
    );
    assert_eq!(
        raw_decompress_to_vec(&compressed).expect("typed decode"),
        input
    );
    let mut output = vec![0; input.len()];
    let (result, largest) = largest_request(|| snappy_decompress(&compressed, &mut output));
    assert_eq!(result, Ok(input.len()));
    assert_eq!(output, input);
    assert_eq!(largest, 0, "caller-owned decode must not allocate output");
}

#[test]
fn valid_overlapping_copies_above_one_megabyte_are_accepted() {
    let mut compressed = vec![0x81, 0x80, 0x40, 0x00, b'A'];
    for _ in 0..16384 {
        compressed.extend_from_slice(&[0xfe, 0x01, 0x00]);
    }
    let output = snappy_decompress_raw(&compressed).expect("decode legal overlapping copies");
    assert_eq!(output.len(), 1024 * 1024 + 1);
    assert!(output.iter().all(|&byte| byte == b'A'));
}
