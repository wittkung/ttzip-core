// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! C-ABI Native Codec Endpoints aligned with `sdk/include/ttzip.h`.
//!
//! Provides zero-copy buffer compression and decompression routines for DEFLATE,
//! Zstandard, LZ4, Snappy, LZFSE, LZVN, Brotli, Fast-LZMA2, and Bzip2, as well
//! as Zstandard dictionary training and compression.

use super::helpers::{run_codec_op, safe_slice, safe_slice_mut};
use crate::codecs::brotli::{brotli_compress, brotli_decompress};
use crate::codecs::bzip2::{bzip2_compress, bzip2_compress_bound, bzip2_decompress};
use crate::codecs::deflate::{
    deflate_compress, deflate_compress_bound, deflate_decompress, gzip_compress, gzip_decompress,
    zlib_compress, zlib_decompress,
};
use crate::codecs::fast_blocks::{lz4_compress, lz4_compress_bound, lz4_decompress};
use crate::codecs::lzfse::{
    lzfse_compress, lzfse_compress_bound, lzfse_compress_stream, lzfse_decompress,
    lzfse_decompress_raw, lzfse_decompress_stream, lzfse_validate, lzvn_compress,
    lzvn_compress_bound, lzvn_decompress, lzvn_decompress_raw, lzvn_validate,
};
use crate::codecs::lzma2::{
    fl2_compress, fl2_compress_bound, fl2_decompress, fl2_find_decompressed_size,
};
use crate::codecs::snappy::{
    snappy_compress, snappy_compress_bound, snappy_decompress, snappy_uncompressed_length,
};
use crate::codecs::zstd::{
    zstd_compress, zstd_compress_advanced, zstd_compress_bound, zstd_decompress,
    zstd_get_decompressed_size, ZstdConfig,
};
use crate::types::TTZipStatus;
use libc::size_t;
use std::panic::catch_unwind;

// MARK: - 1. DEFLATE / zlib / gzip

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_deflate_compress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    level: i32,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, |i, o| {
        deflate_compress(i, o, level)
    })
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_deflate_decompress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, deflate_decompress)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_zlib_compress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    level: i32,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, |i, o| {
        zlib_compress(i, o, level)
    })
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_zlib_decompress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, zlib_decompress)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_gzip_compress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    level: i32,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, |i, o| {
        gzip_compress(i, o, level)
    })
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_gzip_decompress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, gzip_decompress)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_deflate_compress_bound(src_len: size_t, level: i32) -> size_t {
    deflate_compress_bound(src_len, level)
}

// MARK: - 2. Zstandard

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_zstd_compress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    level: i32,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, |i, o| {
        zstd_compress(i, o, level)
    })
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_zstd_compress_advanced(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    level: i32,
    nb_workers: u32,
    job_size_mb: u32,
    overlap_log: u32,
    window_log: u32,
    enable_ldm: bool,
    out_len: *mut size_t,
) -> TTZipStatus {
    let config = ZstdConfig {
        level,
        nb_workers,
        job_size_mb,
        overlap_log,
        window_log,
        enable_ldm,
        enable_checksum: true,
        ldm_hash_log: 0,
        ldm_min_match: 0,
        ldm_bucket_size_log: 0,
        ldm_hash_rate_log: 0,
    };
    run_codec_op(src, src_len, dst, dst_capacity, out_len, |i, o| {
        zstd_compress_advanced(i, o, &config)
    })
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_zstd_decompress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, zstd_decompress)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_zstd_compress_bound(src_len: size_t) -> size_t {
    zstd_compress_bound(src_len)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_zstd_get_decompressed_size(src: *const u8, src_len: size_t) -> u64 {
    match safe_slice(src, src_len) {
        Ok(slice) => zstd_get_decompressed_size(slice).unwrap_or(0),
        Err(_) => 0,
    }
}

// MARK: - 3. LZ4

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_lz4_compress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, lz4_compress)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_lz4_decompress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, lz4_decompress)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_lz4_compress_bound(src_len: size_t) -> size_t {
    lz4_compress_bound(src_len)
}

// MARK: - 4. Snappy

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_snappy_compress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, snappy_compress)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_snappy_decompress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, snappy_decompress)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_snappy_max_compressed_length(src_len: size_t) -> size_t {
    snappy_compress_bound(src_len)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_snappy_uncompressed_length(
    src: *const u8,
    src_len: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match safe_slice(src, src_len) {
            Ok(s) => s,
            Err(st) => return st,
        };
        match snappy_uncompressed_length(in_slice) {
            Ok(len) => {
                *out_len = len;
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_snappy_validate(src: *const u8, src_len: size_t) -> bool {
    match safe_slice(src, src_len) {
        Ok(slice) => crate::codecs::snappy::snappy_validate(slice),
        Err(_) => false,
    }
}

// MARK: - 5. Apple LZFSE & LZVN

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_lzfse_compress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, lzfse_compress)
}

pub use ttzip_rust_lzfse_compress as ttzip_rust_lzfse_compress_raw;

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_lzfse_decompress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, lzfse_decompress)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_lzfse_decompress_raw(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    uncompressed_len: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match safe_slice(src, src_len) {
            Ok(s) => s,
            Err(st) => return st,
        };
        let out_slice = match safe_slice_mut(dst, dst_capacity) {
            Ok(s) => s,
            Err(st) => return st,
        };
        if out_slice.len() < uncompressed_len {
            return TTZipStatus::ErrInvalidParam;
        }
        match lzfse_decompress_raw(in_slice, uncompressed_len) {
            Ok(bytes) => {
                out_slice[..bytes.len()].copy_from_slice(&bytes);
                *out_len = bytes.len();
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_lzfse_compress_stream(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match safe_slice(src, src_len) {
            Ok(s) => s,
            Err(st) => return st,
        };
        let out_slice = match safe_slice_mut(dst, dst_capacity) {
            Ok(s) => s,
            Err(st) => return st,
        };
        match lzfse_compress_stream(in_slice) {
            Ok(bytes) => {
                if out_slice.len() < bytes.len() {
                    return TTZipStatus::ErrInvalidParam;
                }
                out_slice[..bytes.len()].copy_from_slice(&bytes);
                *out_len = bytes.len();
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_lzfse_decompress_stream(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match safe_slice(src, src_len) {
            Ok(s) => s,
            Err(st) => return st,
        };
        let out_slice = match safe_slice_mut(dst, dst_capacity) {
            Ok(s) => s,
            Err(st) => return st,
        };
        match lzfse_decompress_stream(in_slice) {
            Ok(bytes) => {
                if out_slice.len() < bytes.len() {
                    return TTZipStatus::ErrInvalidParam;
                }
                out_slice[..bytes.len()].copy_from_slice(&bytes);
                *out_len = bytes.len();
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_lzfse_compress_bound(src_len: size_t) -> size_t {
    lzfse_compress_bound(src_len)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_lzfse_validate(src: *const u8, src_len: size_t) -> bool {
    match safe_slice(src, src_len) {
        Ok(slice) => lzfse_validate(slice),
        Err(_) => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_lzvn_compress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, lzvn_compress)
}

pub use ttzip_rust_lzvn_compress as ttzip_rust_lzvn_compress_raw;

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_lzvn_decompress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, lzvn_decompress)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_lzvn_decompress_raw(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    uncompressed_len: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match safe_slice(src, src_len) {
            Ok(s) => s,
            Err(st) => return st,
        };
        let out_slice = match safe_slice_mut(dst, dst_capacity) {
            Ok(s) => s,
            Err(st) => return st,
        };
        if out_slice.len() < uncompressed_len {
            return TTZipStatus::ErrInvalidParam;
        }
        match lzvn_decompress_raw(in_slice, uncompressed_len) {
            Ok(bytes) => {
                out_slice[..bytes.len()].copy_from_slice(&bytes);
                *out_len = bytes.len();
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_lzvn_compress_bound(src_len: size_t) -> size_t {
    lzvn_compress_bound(src_len)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_lzvn_validate(src: *const u8, src_len: size_t) -> bool {
    match safe_slice(src, src_len) {
        Ok(slice) => lzvn_validate(slice),
        Err(_) => false,
    }
}

// MARK: - 6. Brotli

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_brotli_compress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    quality: u32,
    lgwin: u32,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, |i, o| {
        brotli_compress(i, o, quality, lgwin)
    })
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_brotli_decompress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, brotli_decompress)
}

// MARK: - 7. Fast-LZMA2 (FL2)

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_fl2_compress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    level: i32,
    nb_threads: u32,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, |i, o| {
        fl2_compress(i, o, level, nb_threads)
    })
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_fl2_decompress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    nb_threads: u32,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, |i, o| {
        fl2_decompress(i, o, nb_threads)
    })
}

#[no_mangle]
pub extern "C" fn ttzip_rust_fl2_compress_bound(src_len: size_t) -> size_t {
    fl2_compress_bound(src_len)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_fl2_find_decompressed_size(src: *const u8, src_len: size_t) -> u64 {
    match safe_slice(src, src_len) {
        Ok(slice) => fl2_find_decompressed_size(slice).unwrap_or(0),
        Err(_) => 0,
    }
}

// MARK: - 8. Bzip2

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_bzip2_compress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    level: i32,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, |i, o| {
        bzip2_compress(i, o, level)
    })
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_bzip2_decompress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    run_codec_op(src, src_len, dst, dst_capacity, out_len, bzip2_decompress)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_bzip2_compress_bound(src_len: size_t) -> size_t {
    bzip2_compress_bound(src_len)
}

// MARK: - 9. Zstandard Dictionary Training & Compression

/// Trains a Zstandard dictionary from sample byte buffers.
///
/// # Safety
/// - `sample_ptrs` must point to `sample_count` readable pointers.
/// - `sample_lens` must point to `sample_count` lengths.
/// - `out_dict` must point to `dict_capacity` writable bytes.
/// - `out_dict_len` must point to a writable `size_t`.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_zstd_train_dict(
    sample_ptrs: *const *const u8,
    sample_lens: *const size_t,
    sample_count: size_t,
    target_dict_size: size_t,
    level: i32,
    out_dict: *mut u8,
    dict_capacity: size_t,
    out_dict_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if sample_ptrs.is_null()
            || sample_lens.is_null()
            || out_dict.is_null()
            || out_dict_len.is_null()
            || sample_count == 0
        {
            return TTZipStatus::ErrInvalidParam;
        }

        let mut samples = Vec::with_capacity(sample_count);
        for i in 0..sample_count {
            let ptr = *sample_ptrs.add(i);
            let len = *sample_lens.add(i);
            let slice = match safe_slice(ptr, len) {
                Ok(s) => s,
                Err(st) => return st,
            };
            samples.push(slice);
        }

        let dict_bytes = match crate::codecs::zstd::dict::zstd_train_dictionary(
            &samples,
            target_dict_size,
            level,
        ) {
            Ok(b) => b,
            Err(st) => return st,
        };

        if dict_bytes.len() > dict_capacity {
            return TTZipStatus::ErrInvalidParam;
        }

        std::ptr::copy_nonoverlapping(dict_bytes.as_ptr(), out_dict, dict_bytes.len());
        *out_dict_len = dict_bytes.len();
        TTZipStatus::Ok
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// Compresses data using a pre-digested Zstandard dictionary.
///
/// # Safety
/// - `src` must point to `src_len` readable bytes.
/// - `dst` must point to `dst_capacity` writable bytes.
/// - `dict` must point to `dict_len` readable bytes.
/// - `out_len` must point to a writable `size_t`.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_zstd_dict_compress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    dict: *const u8,
    dict_len: size_t,
    level: i32,
    out_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match safe_slice(src, src_len) {
            Ok(s) => s,
            Err(st) => return st,
        };
        let out_slice = match safe_slice_mut(dst, dst_capacity) {
            Ok(s) => s,
            Err(st) => return st,
        };
        let dict_slice = match safe_slice(dict, dict_len) {
            Ok(s) => s,
            Err(st) => return st,
        };

        let dictionary = match crate::codecs::zstd::dict::ZstdDictionary::from_bytes(
            "ephemeral",
            dict_slice.to_vec(),
            level,
        ) {
            Ok(d) => d,
            Err(st) => return st,
        };

        match dictionary.compress_small(in_slice, out_slice) {
            Ok(written) => {
                *out_len = written;
                TTZipStatus::Ok
            }
            Err(st) => st,
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// Decompresses data using a pre-digested Zstandard dictionary.
///
/// # Safety
/// - `src` must point to `src_len` readable bytes.
/// - `dst` must point to `dst_capacity` writable bytes.
/// - `dict` must point to `dict_len` readable bytes.
/// - `out_len` must point to a writable `size_t`.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_zstd_dict_decompress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    dict: *const u8,
    dict_len: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match safe_slice(src, src_len) {
            Ok(s) => s,
            Err(st) => return st,
        };
        let out_slice = match safe_slice_mut(dst, dst_capacity) {
            Ok(s) => s,
            Err(st) => return st,
        };
        let dict_slice = match safe_slice(dict, dict_len) {
            Ok(s) => s,
            Err(st) => return st,
        };

        let dictionary = match crate::codecs::zstd::dict::ZstdDictionary::from_bytes(
            "ephemeral",
            dict_slice.to_vec(),
            3,
        ) {
            Ok(d) => d,
            Err(st) => return st,
        };

        match dictionary.decompress_small(in_slice, out_slice) {
            Ok(written) => {
                *out_len = written;
                TTZipStatus::Ok
            }
            Err(st) => st,
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}
