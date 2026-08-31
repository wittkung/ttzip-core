// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive LZ4 frametest cascade, multi-frame streaming, and memory guard test suite.
//!
//! Directly ported and enhanced from upstream `frametest.c` and `fuzzer.c`:
//! 1. Truncated Frame & Reset (HeaderIncomplete, mid-block truncation, dirty state purge, bug1227).
//! 2. Skippable Frames Concatenation (16 magic numbers `0x184D2A50..0x184D2A5F`, 0-byte, 10-byte,
//!    random payload, interleaved multi-frame stream concatenation).
//! 3. Canary Byte Memory Guard (zero-byte out-of-bounds write defense across one-shot & streaming).
//! 4. Non-contiguous Buffer Slicing (pointer offset addressing across disjoint heap memory chunks).

use std::ffi::CStr;
use ttzip_engine::codecs::lz4::{
    is_lz4_frame_magic, is_lz4_skippable_magic, BlockIndependence, BlockMaxSize, FrameDescriptor,
    LZ4F_MAGICNUMBER, LZ4F_MAGIC_SKIPPABLE_END, LZ4F_MAGIC_SKIPPABLE_START,
};

// MARK: - Native C LZ4 Frame FFI Bindings

#[allow(dead_code)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LZ4FErrorCode {
    NoError = 0,
    Generic = 1,
    MaxBlockSizeInvalid = 2,
    BlockModeInvalid = 3,
    ParameterInvalid = 4,
    CompressionLevelInvalid = 5,
    HeaderVersionWrong = 6,
    BlockChecksumInvalid = 7,
    ReservedFlagSet = 8,
    AllocationFailed = 9,
    SrcSizeTooLarge = 10,
    DstMaxSizeTooSmall = 11,
    FrameHeaderIncomplete = 12,
    FrameTypeUnknown = 13,
    FrameSizeWrong = 14,
    SrcPtrWrong = 15,
    DecompressionFailed = 16,
    HeaderChecksumInvalid = 17,
    ContentChecksumInvalid = 18,
    FrameDecodingAlreadyStarted = 19,
    CompressionStateUninitialized = 20,
    ParameterNull = 21,
    IoWrite = 22,
    IoRead = 23,
    MaxCode = 24,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct LZ4FFrameInfo {
    pub block_size_id: u32,
    pub block_mode: u32,
    pub content_checksum_flag: u32,
    pub frame_type: u32,
    pub content_size: u64,
    pub dict_id: u32,
    pub block_checksum_flag: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct LZ4FPreferences {
    pub frame_info: LZ4FFrameInfo,
    pub compression_level: libc::c_int,
    pub auto_flush: libc::c_uint,
    pub favor_dec_speed: libc::c_uint,
    pub reserved: [libc::c_uint; 3],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct LZ4FCompressOptions {
    pub stable_src: libc::c_uint,
    pub reserved: [libc::c_uint; 3],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct LZ4FDecompressOptions {
    pub stable_dst: libc::c_uint,
    pub skip_checksums: libc::c_uint,
    pub reserved1: libc::c_uint,
    pub reserved0: libc::c_uint,
}

enum LZ4FCompressCtxOpaque {}
enum LZ4FDecompressCtxOpaque {}

extern "C" {
    fn LZ4F_isError(code: usize) -> libc::c_uint;
    fn LZ4F_getErrorName(code: usize) -> *const libc::c_char;
    fn LZ4F_getErrorCode(code: usize) -> LZ4FErrorCode;

    fn LZ4F_createCompressionContext(
        cctx_ptr: *mut *mut LZ4FCompressCtxOpaque,
        version: libc::c_uint,
    ) -> usize;
    fn LZ4F_freeCompressionContext(cctx: *mut LZ4FCompressCtxOpaque) -> usize;

    fn LZ4F_createDecompressionContext(
        dctx_ptr: *mut *mut LZ4FDecompressCtxOpaque,
        version: libc::c_uint,
    ) -> usize;
    fn LZ4F_freeDecompressionContext(dctx: *mut LZ4FDecompressCtxOpaque) -> usize;
    fn LZ4F_resetDecompressionContext(dctx: *mut LZ4FDecompressCtxOpaque);

    fn LZ4F_compressFrame(
        dst_buffer: *mut libc::c_void,
        dst_capacity: usize,
        src_buffer: *const libc::c_void,
        src_size: usize,
        preferences_ptr: *const LZ4FPreferences,
    ) -> usize;

    fn LZ4F_compressFrameBound(
        src_size: usize,
        preferences_ptr: *const LZ4FPreferences,
    ) -> usize;

    fn LZ4F_compressBound(
        src_size: usize,
        preferences_ptr: *const LZ4FPreferences,
    ) -> usize;

    fn LZ4F_compressBegin(
        cctx: *mut LZ4FCompressCtxOpaque,
        dst_buffer: *mut libc::c_void,
        dst_capacity: usize,
        prefs_ptr: *const LZ4FPreferences,
    ) -> usize;

    fn LZ4F_compressUpdate(
        cctx: *mut LZ4FCompressCtxOpaque,
        dst_buffer: *mut libc::c_void,
        dst_capacity: usize,
        src_buffer: *const libc::c_void,
        src_size: usize,
        c_opt_ptr: *const LZ4FCompressOptions,
    ) -> usize;

    fn LZ4F_compressEnd(
        cctx: *mut LZ4FCompressCtxOpaque,
        dst_buffer: *mut libc::c_void,
        dst_capacity: usize,
        c_opt_ptr: *const LZ4FCompressOptions,
    ) -> usize;

    fn LZ4F_decompress(
        dctx: *mut LZ4FDecompressCtxOpaque,
        dst_buffer: *mut libc::c_void,
        dst_size_ptr: *mut usize,
        src_buffer: *const libc::c_void,
        src_size_ptr: *mut usize,
        d_opt_ptr: *const LZ4FDecompressOptions,
    ) -> usize;
}

const LZ4F_VERSION: libc::c_uint = 100;

// MARK: - Safe RAII Decompression Wrapper

struct SafeDecompressCtx {
    raw: *mut LZ4FDecompressCtxOpaque,
}

impl SafeDecompressCtx {
    fn new() -> Self {
        let mut raw = std::ptr::null_mut();
        let ret = unsafe { LZ4F_createDecompressionContext(&mut raw, LZ4F_VERSION) };
        assert_eq!(
            unsafe { LZ4F_isError(ret) },
            0,
            "LZ4F_createDecompressionContext failed"
        );
        Self { raw }
    }

    fn reset(&mut self) {
        unsafe { LZ4F_resetDecompressionContext(self.raw) };
    }

    fn decompress(
        &mut self,
        dst: &mut [u8],
        src: &[u8],
        options: Option<&LZ4FDecompressOptions>,
    ) -> Result<(usize, usize, usize), (LZ4FErrorCode, String)> {
        let mut dst_size = dst.len();
        let mut src_size = src.len();
        let opt_ptr = options.map_or(std::ptr::null(), |o| o as *const _);
        let ret = unsafe {
            LZ4F_decompress(
                self.raw,
                dst.as_mut_ptr() as *mut libc::c_void,
                &mut dst_size,
                src.as_ptr() as *const libc::c_void,
                &mut src_size,
                opt_ptr,
            )
        };
        if unsafe { LZ4F_isError(ret) } != 0 {
            let err_code = unsafe { LZ4F_getErrorCode(ret) };
            let err_name = unsafe {
                CStr::from_ptr(LZ4F_getErrorName(ret))
                    .to_string_lossy()
                    .into_owned()
            };
            Err((err_code, err_name))
        } else {
            Ok((dst_size, src_size, ret))
        }
    }
}

impl Drop for SafeDecompressCtx {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { LZ4F_freeDecompressionContext(self.raw) };
        }
    }
}

// MARK: - Safe RAII Compression Wrapper

struct SafeCompressCtx {
    raw: *mut LZ4FCompressCtxOpaque,
}

impl SafeCompressCtx {
    fn new() -> Self {
        let mut raw = std::ptr::null_mut();
        let ret = unsafe { LZ4F_createCompressionContext(&mut raw, LZ4F_VERSION) };
        assert_eq!(
            unsafe { LZ4F_isError(ret) },
            0,
            "LZ4F_createCompressionContext failed"
        );
        Self { raw }
    }
}

impl Drop for SafeCompressCtx {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { LZ4F_freeCompressionContext(self.raw) };
        }
    }
}

// MARK: - Compression Helper

fn compress_frame(src: &[u8], prefs: Option<&LZ4FPreferences>) -> Vec<u8> {
    let prefs_ptr = prefs.map_or(std::ptr::null(), |p| p as *const _);
    let bound = unsafe { LZ4F_compressFrameBound(src.len(), prefs_ptr) };
    let mut dst = vec![0u8; bound];
    let ret = unsafe {
        LZ4F_compressFrame(
            dst.as_mut_ptr() as *mut libc::c_void,
            dst.len(),
            src.as_ptr() as *const libc::c_void,
            src.len(),
            prefs_ptr,
        )
    };
    assert_eq!(unsafe { LZ4F_isError(ret) }, 0, "LZ4F_compressFrame failed");
    dst.truncate(ret);
    dst
}

fn create_skippable_frame(magic: u32, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(8 + payload.len());
    frame.extend_from_slice(&magic.to_le_bytes());
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(payload);
    frame
}

// MARK: - Suite 1: Truncated Frame & Reset Tests

#[test]
fn test_truncated_frame_header_reports_incomplete() {
    let payload = b"TTZip LZ4 Frame Header Truncation Test Payload 2026.";
    let compressed = compress_frame(payload, None);
    assert!(compressed.len() >= 15);

    let mut dctx = SafeDecompressCtx::new();

    // Test truncated header sizes from 1 up to 6 bytes (less than 7-byte minimal header)
    for trunc_len in 1..=6 {
        let mut dst = [0u8; 128];
        let res = dctx.decompress(&mut dst, &compressed[..trunc_len], None);
        match res {
            Ok((written, consumed, hint)) => {
                assert_eq!(written, 0, "No data should be decoded from partial header");
                assert_eq!(consumed, trunc_len, "All truncated bytes buffered");
                assert!(hint > 0, "Decoder must request more bytes for header completion");
            }
            Err((err_code, _)) => {
                assert!(
                    matches!(
                        err_code,
                        LZ4FErrorCode::FrameHeaderIncomplete | LZ4FErrorCode::DecompressionFailed
                    ),
                    "Expected header incomplete or decompression failed, got: {err_code:?}"
                );
            }
        }
        dctx.reset();
    }
}

#[test]
fn test_truncated_mid_block_and_reset_recovery() {
    let payload = b"Repeated block content to enforce compression across multi-block LZ4 streams. "
        .repeat(200);
    let compressed = compress_frame(&payload, None);
    assert!(compressed.len() > 32);

    let mut dctx = SafeDecompressCtx::new();

    // Feed truncated stream (header + only 15 bytes of block)
    let trunc_len = 15.min(compressed.len());
    let mut dst = vec![0u8; payload.len()];
    let (written, consumed, hint) = dctx
        .decompress(&mut dst, &compressed[..trunc_len], None)
        .expect("partial feed should buffer without panic");
    assert!(consumed <= trunc_len);
    assert!(written < payload.len());
    assert!(hint > 0);

    // Reset decoder context - must completely purge unfinished session state
    dctx.reset();

    // Re-decode a valid empty 0-byte frame
    let empty_frame = compress_frame(b"", None);
    let mut empty_dst = [0u8; 16];
    let (empty_out, empty_in, empty_hint) = dctx
        .decompress(&mut empty_dst, &empty_frame, None)
        .expect("empty frame after reset");
    assert_eq!(empty_out, 0);
    assert_eq!(empty_in, empty_frame.len());
    assert_eq!(empty_hint, 0, "Frame finished indicator must be 0");

    // Re-decode a full multi-block frame to ensure zero residual corruption
    let mut full_dst = vec![0u8; payload.len()];
    let mut offset_in = 0;
    let mut offset_out = 0;
    while offset_in < compressed.len() {
        let (w, c, _) = dctx
            .decompress(
                &mut full_dst[offset_out..],
                &compressed[offset_in..],
                None,
            )
            .expect("full frame decompress after reset");
        offset_out += w;
        offset_in += c;
        if c == 0 && w == 0 {
            break;
        }
    }
    assert_eq!(offset_out, payload.len());
    assert_eq!(&full_dst[..offset_out], &payload[..]);
}

#[test]
fn test_bug1227_exact_reused_dctx_after_partial_error() {
    let mut dctx = SafeDecompressCtx::new();

    // 1. Session 1: Compress 9 zero bytes with explicit contentSize header
    let s9 = [0u8; 9];
    let mut pref = LZ4FPreferences::default();
    pref.frame_info.content_size = 9;
    let c9 = compress_frame(&s9, Some(&pref));
    assert!(c9.len() > 15);

    // Partially feed only 15 bytes (state left uncompleted)
    let mut d9 = [0u8; 9];
    let (_, src_consumed, _) = dctx
        .decompress(&mut d9, &c9[..15], None)
        .expect("decompress 15 bytes");
    assert!(src_consumed < c9.len());

    // 2. Unfinished session -> Reset makes it clean
    dctx.reset();

    // 3. Session 2: Decompress valid 0-size frame with no content size field
    let c0 = compress_frame(b"", None);
    let mut d0 = [0u8; 1];
    let (d0_out, d0_in, next_hint) = dctx
        .decompress(&mut d0, &c0, None)
        .expect("decompress 0-size frame");
    assert_eq!(d0_out, 0);
    assert_eq!(d0_in, c0.len());
    assert_eq!(next_hint, 0);
}

// MARK: - Suite 2: Skippable Frames Concatenation Tests

#[test]
fn test_all_16_skippable_magic_numbers_isolated() {
    let mut dctx = SafeDecompressCtx::new();

    for magic in LZ4F_MAGIC_SKIPPABLE_START..=LZ4F_MAGIC_SKIPPABLE_END {
        assert!(is_lz4_skippable_magic(magic));

        // Case A: 0-byte payload skippable frame
        let skippable_0 = create_skippable_frame(magic, b"");
        assert_eq!(skippable_0.len(), 8);
        let mut dst = [0u8; 32];
        let (out_len, in_len, hint) = dctx
            .decompress(&mut dst, &skippable_0, None)
            .expect("decompress 0-byte skippable frame");
        assert_eq!(out_len, 0);
        assert_eq!(in_len, 8);
        assert_eq!(hint, 0, "Skippable frame must complete at byte 8");

        // Case B: 10-byte payload skippable frame
        let payload_10 = b"0123456789";
        let skippable_10 = create_skippable_frame(magic, payload_10);
        assert_eq!(skippable_10.len(), 18);
        let (out_10, in_10, hint_10) = dctx
            .decompress(&mut dst, &skippable_10, None)
            .expect("decompress 10-byte skippable frame");
        assert_eq!(out_10, 0);
        assert_eq!(in_10, 18);
        assert_eq!(hint_10, 0);

        // Case C: 256-byte pseudo-random payload skippable frame
        let random_payload: Vec<u8> = (0..256).map(|i| ((i * 37 + 13) & 0xFF) as u8).collect();
        let skippable_rand = create_skippable_frame(magic, &random_payload);
        assert_eq!(skippable_rand.len(), 8 + 256);
        let (out_rand, in_rand, hint_rand) = dctx
            .decompress(&mut dst, &skippable_rand, None)
            .expect("decompress 256-byte skippable frame");
        assert_eq!(out_rand, 0);
        assert_eq!(in_rand, 8 + 256);
        assert_eq!(hint_rand, 0);
    }
}

#[test]
fn test_skippable_and_standard_frames_interleaved_concatenation() {
    let mut dctx = SafeDecompressCtx::new();

    // Standard payloads
    let p1 = b"Chapter 1: Native macOS Microkernel and UniFFI Bridge Architecture.".repeat(10);
    let p2 = b"Chapter 2: Zero-Copy Memory Fences and High-Throughput Pipelines.".repeat(15);
    let p3 = b"Chapter 3: Stream-First Invariant-First Bounds-First TTZip Engine.".repeat(20);

    let f1 = compress_frame(&p1, None);
    let f2 = compress_frame(&p2, None);
    let f3 = compress_frame(&p3, None);

    // Skippable metadata frames
    let s1 = create_skippable_frame(0x184D_2A50, b"");
    let s2 = create_skippable_frame(0x184D_2A55, b"CUSTOM_METADATA_HEADER_V1");
    let s3_payload = vec![0xEEu8; 512];
    let s3 = create_skippable_frame(0x184D_2A5F, &s3_payload);

    // Interleave: s1 + f1 + s2 + f2 + s3 + f3
    let mut multi_stream = Vec::new();
    multi_stream.extend_from_slice(&s1);
    multi_stream.extend_from_slice(&f1);
    multi_stream.extend_from_slice(&s2);
    multi_stream.extend_from_slice(&f2);
    multi_stream.extend_from_slice(&s3);
    multi_stream.extend_from_slice(&f3);

    // Expected total uncompressed payload: p1 + p2 + p3
    let mut expected_payload = Vec::new();
    expected_payload.extend_from_slice(&p1);
    expected_payload.extend_from_slice(&p2);
    expected_payload.extend_from_slice(&p3);

    // Stream decompress in randomized chunk sizes (1..=47 bytes per step)
    let mut decoded_output = Vec::new();
    let mut stream_cursor = 0;
    let mut step = 1usize;

    while stream_cursor < multi_stream.len() {
        let chunk_in_size = (step % 47 + 1).min(multi_stream.len() - stream_cursor);
        let in_slice = &multi_stream[stream_cursor..stream_cursor + chunk_in_size];

        let mut out_chunk = vec![0u8; 128];
        let (out_written, in_consumed, _) = dctx
            .decompress(&mut out_chunk, in_slice, None)
            .expect("stream decompress multi cascade");

        if out_written > 0 {
            decoded_output.extend_from_slice(&out_chunk[..out_written]);
        }
        stream_cursor += in_consumed;
        step += 1;
    }

    assert_eq!(stream_cursor, multi_stream.len());
    assert_eq!(decoded_output.len(), expected_payload.len());
    assert_eq!(decoded_output, expected_payload);
}

// MARK: - Suite 3: Canary Byte Memory Guard Tests

#[test]
fn test_canary_byte_guard_single_shot_decompression() {
    let payload = b"Canary memory sentinel protection payload for TTZip LZ4 decompression.";
    let compressed = compress_frame(payload, None);

    let guard_len = 64;
    let mut guarded_dst = vec![0u8; payload.len() + guard_len];

    // Seed guard zone with unique pseudo-random canary pattern
    for (i, byte) in guarded_dst[payload.len()..].iter_mut().enumerate() {
        *byte = ((i * 131 + 79) & 0xFF) as u8;
    }
    let expected_canary = guarded_dst[payload.len()..].to_vec();

    let mut dctx = SafeDecompressCtx::new();
    let (written, consumed, hint) = dctx
        .decompress(&mut guarded_dst[..payload.len()], &compressed, None)
        .expect("decompress into guarded slice");

    assert_eq!(written, payload.len());
    assert_eq!(consumed, compressed.len());
    assert_eq!(hint, 0);
    assert_eq!(&guarded_dst[..written], payload);

    // Verify 0 bytes out-of-bounds write occurred
    assert_eq!(
        &guarded_dst[payload.len()..],
        expected_canary.as_slice(),
        "Canary guard region was corrupted! Out-of-bounds write detected."
    );
}

#[test]
fn test_canary_byte_guard_streaming_chunked_decompression() {
    let payload = b"High-frequency multi-chunk streaming canary sentinel validation.".repeat(100);
    let compressed = compress_frame(&payload, None);

    let mut dctx = SafeDecompressCtx::new();
    let mut in_pos = 0;
    let mut reconstructed = Vec::new();

    while in_pos < compressed.len() {
        let chunk_capacity = 256;
        let guard_size = 16;
        let mut buffer = vec![0u8; chunk_capacity + guard_size];

        let canary_byte = 0xA5u8;
        buffer[chunk_capacity..].fill(canary_byte);

        let in_avail = (compressed.len() - in_pos).min(128);
        let in_slice = &compressed[in_pos..in_pos + in_avail];

        let (out_w, in_c, _) = dctx
            .decompress(&mut buffer[..chunk_capacity], in_slice, None)
            .expect("streaming canary chunk decompress");

        // Verify guard zone untouched
        for &b in &buffer[chunk_capacity..] {
            assert_eq!(b, canary_byte, "Sentinel byte corrupted during chunk write!");
        }

        reconstructed.extend_from_slice(&buffer[..out_w]);
        in_pos += in_c;
        if in_c == 0 && out_w == 0 {
            break;
        }
    }

    assert_eq!(reconstructed.len(), payload.len());
    assert_eq!(reconstructed, payload);
}

#[test]
fn test_canary_byte_guard_compress_end() {
    let cctx = SafeCompressCtx::new();
    let pref = LZ4FPreferences {
        compression_level: 1,
        ..Default::default()
    };

    let mut header_buf = [0u8; 64];
    let header_size = unsafe {
        LZ4F_compressBegin(
            cctx.raw,
            header_buf.as_mut_ptr() as *mut libc::c_void,
            header_buf.len(),
            &pref,
        )
    };
    assert_eq!(unsafe { LZ4F_isError(header_size) }, 0);

    let payload = b"CompressEnd canary verification payload.".repeat(10);
    let mut body_buf = vec![0u8; payload.len() * 2];
    let body_size = unsafe {
        LZ4F_compressUpdate(
            cctx.raw,
            body_buf.as_mut_ptr() as *mut libc::c_void,
            body_buf.len(),
            payload.as_ptr() as *const libc::c_void,
            payload.len(),
            std::ptr::null(),
        )
    };
    assert_eq!(unsafe { LZ4F_isError(body_size) }, 0);

    // Guard compressEnd
    let end_safe_size = unsafe { LZ4F_compressBound(0, &pref) };
    let guard_zone_size = 32;
    let mut end_buf = vec![0u8; end_safe_size + guard_zone_size];
    let canary_mark = 0x7Eu8;
    end_buf[end_safe_size..].fill(canary_mark);

    let end_size = unsafe {
        LZ4F_compressEnd(
            cctx.raw,
            end_buf.as_mut_ptr() as *mut libc::c_void,
            end_safe_size,
            std::ptr::null(),
        )
    };
    assert_eq!(unsafe { LZ4F_isError(end_size) }, 0);

    // Verify canary bytes after dstCapacity
    for (idx, &b) in end_buf[end_safe_size..].iter().enumerate() {
        assert_eq!(
            b, canary_mark,
            "LZ4F_compressEnd wrote beyond dstCapacity at offset {idx}!"
        );
    }
}

// MARK: - Suite 4: Non-contiguous Buffer Slicing Tests

#[test]
fn test_noncontiguous_output_buffer_slicing_with_gaps() {
    let payload = b"Non-contiguous buffer slicing test for dynamic memory pointer offsets."
        .repeat(50);
    let compressed = compress_frame(&payload, None);

    let mut dctx = SafeDecompressCtx::new();
    let gap_size = 32;
    let poison_byte = 0xEEu8;

    // Allocate memory with deliberate poison gaps between output regions
    let total_alloc = payload.len() + (payload.len() / 64 + 2) * gap_size;
    let mut memory_arena = vec![poison_byte; total_alloc];

    let mut in_cursor = 0;
    let mut arena_cursor = 0;
    let mut extracted_chunks = Vec::new();

    while in_cursor < compressed.len() {
        let chunk_cap = 64.min(memory_arena.len() - arena_cursor - gap_size);
        if chunk_cap == 0 {
            break;
        }

        let in_slice = &compressed[in_cursor..];
        let dst_slice = &mut memory_arena[arena_cursor..arena_cursor + chunk_cap];

        let (w, c, _) = dctx
            .decompress(dst_slice, in_slice, None)
            .expect("decompress noncontiguous slice");

        if w > 0 {
            extracted_chunks.extend_from_slice(&memory_arena[arena_cursor..arena_cursor + w]);
            arena_cursor += w;
            // Advance past the poison gap
            arena_cursor += gap_size;
        }
        in_cursor += c;
        if c == 0 && w == 0 {
            break;
        }
    }

    assert_eq!(extracted_chunks.len(), payload.len());
    assert_eq!(extracted_chunks, payload);
}

#[test]
fn test_independent_disjoint_heap_slices_decompression() {
    let payload = b"Disjoint heap vectors testing that decoder does not assume memory continuity."
        .repeat(80);
    let compressed = compress_frame(&payload, None);

    let mut dctx = SafeDecompressCtx::new();
    let mut in_pos = 0;
    let mut disjoint_chunks: Vec<Vec<u8>> = Vec::new();

    while in_pos < compressed.len() {
        // Allocate a brand new heap vector for each iteration
        let mut disjoint_vec = vec![0u8; 128];
        let in_slice = &compressed[in_pos..];

        let (written, consumed, _) = dctx
            .decompress(&mut disjoint_vec, in_slice, None)
            .expect("decompress into independent heap vector");

        if written > 0 {
            disjoint_vec.truncate(written);
            disjoint_chunks.push(disjoint_vec);
        }
        in_pos += consumed;
        if consumed == 0 && written == 0 {
            break;
        }
    }

    let assembled: Vec<u8> = disjoint_chunks.into_iter().flatten().collect();
    assert_eq!(assembled.len(), payload.len());
    assert_eq!(assembled, payload);
}

#[test]
fn test_frame_descriptor_roundtrip_fidelity() {
    let desc = FrameDescriptor {
        version: 1,
        block_independence: BlockIndependence::Independent,
        block_checksum: true,
        content_checksum: true,
        content_size: Some(1048576),
        dict_id: Some(0x12345678),
        block_max_size: BlockMaxSize::Max256KB,
    };

    let mut buf = [0u8; 32];
    let written = desc.emit_with_magic(&mut buf).expect("emit descriptor");
    assert!(written >= 15);
    assert!(is_lz4_frame_magic(LZ4F_MAGICNUMBER));

    let (parsed, consumed) =
        FrameDescriptor::parse_with_magic(&buf[..written]).expect("parse descriptor");
    assert_eq!(consumed, written);
    assert_eq!(parsed, desc);
}
