// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe RAII wrapper for `fast-lzma2` (FL2) multi-threaded LZMA2 engine.
//!
//! Provides parallel chunked LZMA2 compression, dictionary property extraction (for 7z/XZ),
//! streaming decompressor, and automatic resource reclamation.

pub mod compress;
pub mod decompress;
pub mod ffi;

pub use compress::{fl2_compress, fl2_compress_bound, Fl2CCtx, Fl2CStream};
pub use decompress::{
    fl2_decompress, fl2_find_decompressed_size, with_thread_local_fl2_dctx, Fl2DCtx, Fl2DStream,
};
pub use ffi::{Fl2CParameter, Fl2InBuffer, Fl2OutBuffer};

use crate::types::TTZipCompressionLevel;

/// Architectural maximum dictionary size for LZMA2 in 7-Zip (1536 MB = 1.5 GB).
pub const LZMA2_MAX_DICTIONARY_MB: usize = 1536;

/// Calculates LZMA2 compression dictionary size in MB based on compression level and physical RAM in GB.
pub fn calculate_lzma2_dictionary_mb(level: TTZipCompressionLevel, physical_ram_gb: f64) -> usize {
    match level {
        TTZipCompressionLevel::Store => 0,
        TTZipCompressionLevel::Fastest | TTZipCompressionLevel::Fast => 16,
        TTZipCompressionLevel::Normal => 64,
        TTZipCompressionLevel::Maximum | TTZipCompressionLevel::Ultra => {
            if physical_ram_gb >= 64.0 {
                1024 // 1 GB
            } else if physical_ram_gb >= 32.0 {
                512  // 512 MB
            } else if physical_ram_gb >= 16.0 {
                256  // 256 MB
            } else {
                128  // 128 MB
            }
        }
    }
}

/// Alias for calculate_lzma2_dictionary_mb.
pub fn calculate_dictionary_mb(level: TTZipCompressionLevel, physical_ram_gb: f64) -> usize {
    calculate_lzma2_dictionary_mb(level, physical_ram_gb)
}

/// Estimates physical memory budget in MB for LZMA2 compression per thread (BT4 match finder ~10.5x dictionary size).
pub fn lzma2_memory_budget_mb(dict_mb: usize, thread_count: usize) -> f64 {
    (dict_mb as f64) * 10.5 * (thread_count.max(1) as f64)
}

/// Estimates physical memory budget per single thread in MB.
pub fn lzma2_memory_budget_per_thread_mb(dict_mb: usize) -> f64 {
    lzma2_memory_budget_mb(dict_mb, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fl2_basic_roundtrip() {
        let input = b"TTZip Safe Rust Fast-LZMA2 Multi-threaded Engine testing payload.";
        let mut compressed = vec![0u8; fl2_compress_bound(input.len()) + 1024];
        let comp_len = fl2_compress(input, &mut compressed, 3, 2).expect("fl2 compression failed");
        assert!(comp_len > 0);

        let mut decompressed = vec![0u8; input.len()];
        let decomp_len = fl2_decompress(&compressed[..comp_len], &mut decompressed, 2)
            .expect("fl2 decompression failed");
        assert_eq!(decomp_len, input.len());
        assert_eq!(&decompressed[..decomp_len], input);
    }

    #[test]
    fn test_fl2_dstream_streaming() {
        let input = b"TTZip Safe Rust Fast-LZMA2 Multi-threaded Engine testing payload.";
        let mut compressed = vec![0u8; fl2_compress_bound(input.len()) + 1024];
        let comp_len = fl2_compress(input, &mut compressed, 3, 2).expect("fl2 compression failed");

        let mut dstream = Fl2DStream::new(1).expect("create dstream");
        dstream.init(None).expect("init dstream");

        let mut in_buf = Fl2InBuffer {
            src: compressed.as_ptr() as *const libc::c_void,
            size: comp_len,
            pos: 0,
        };
        let mut out_data = vec![0u8; input.len()];
        let mut out_buf = Fl2OutBuffer {
            dst: out_data.as_mut_ptr() as *mut libc::c_void,
            size: out_data.len(),
            pos: 0,
        };

        let _res = dstream.decompress_stream(&mut in_buf, &mut out_buf).expect("decompress stream failed");
        assert_eq!(out_buf.pos, input.len());
        assert_eq!(&out_data, input);
    }
}
