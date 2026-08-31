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
pub mod count;
pub mod decoder;
pub mod decompress;
pub mod dict_buffer;
pub mod fastpos_table;
pub mod ffi;
pub mod inplace_buffer;
pub mod match_table;
pub mod radix_matcher;
pub mod range_enc;

pub use compress::{fl2_compress, fl2_compress_bound, Fl2CCtx, Fl2CStream};
pub use count::{
    count_common_bytes_32_debruijn, count_common_bytes_64, count_common_bytes_64_debruijn,
    count_match_length, count_match_length_raw, DE_BRUIJN_32, DE_BRUIJN_64,
    DE_BRUIJN_BYTE_POS_32, DE_BRUIJN_BYTE_POS_64,
};
pub use decoder::{
    encode_lzma2_literal_chunk, Lzma2ChunkHeader, Lzma2DecodeError, Lzma2Dict,
    Lzma2StreamDecoder, LZMA2_DEFAULT_DICT_SIZE, LZMA2_MAX_PACK_CHUNK_SIZE,
    LZMA2_MAX_UNPACK_CHUNK_SIZE,
};
pub use decompress::{
    fl2_decompress, fl2_find_decompressed_size, with_thread_local_fl2_dctx, Fl2DCtx, Fl2DStream,
};
pub use dict_buffer::{
    dict_shift, BufferId, BufferSlot, DictAlignedBuffer, DictBuffer, Lzma2AlignedBuffer,
    DEFAULT_DICT_SIZE as LZMA2_DICT_BUFFER_DEFAULT_SIZE, DEFAULT_OVERLAP_FRACTION,
    DEFAULT_TASK_RESIDENT_MEMORY_LIMIT, MAX_DICT_SIZE as LZMA2_DICT_BUFFER_MAX_SIZE,
    MAX_OVERLAP_FRACTION, MIN_DICT_SIZE as LZMA2_DICT_BUFFER_MIN_SIZE, OVERLAP_SCALE,
    SIMD_ALIGNMENT,
};
pub use fastpos_table::{
    get_pos_slot, get_pos_slot_fast, get_pos_slot_math_spec, FastPosTable, FAST_POS_TABLE,
    FAST_POS_TABLE_SIZE, K_FAST_DIST_BITS,
};
pub use ffi::{Fl2CParameter, Fl2InBuffer, Fl2OutBuffer};
pub use inplace_buffer::{
    InPlaceBufferGuard, InPlaceBufferPool, InPlaceError, InPlaceOutputWriter, PooledMatchTableGuard,
    DEFAULT_POOL_CAPACITY, DEFAULT_SAFETY_MARGIN_BYTES, DEFAULT_SLOT_CAPACITY_BYTES,
};
pub use match_table::{
    BitPackedEntry, MatchTable, MatchTableMode, StructuredMatchEntry, COMPACT_DICT_THRESHOLD,
    MAX_JUMP_CHAIN_HOPS,
};
pub use radix_matcher::{
    MatchEntry, RadixBuildMatch, RadixMatchFinder, BUFFER_LINK_MASK, MAX_BRUTE_FORCE_LIST_SIZE,
    MAX_REPEAT, RADIX16_TABLE_SIZE, RADIX8_TABLE_SIZE, RADIX_LINK_BITS, RADIX_LINK_MASK,
    RADIX_MAX_LENGTH, RADIX_NULL_LINK,
};
pub use range_enc::{
    get_bit_tree_price, get_direct_bits_price, get_price, get_price_0, get_price_1,
    get_reverse_bit_tree_price, Lzma2RangeEncoder, BIT_PRICE_UNIT, NUM_BIT_PRICE_SHIFT_BITS,
    PROB_PRICES, PROB_TABLE_SIZE,
};

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
