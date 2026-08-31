// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-performance Libdeflate-compatible Binary Tree (BT) matching and Near-Optimal DP parsing engine.
//!
//! Provides:
//! - [`BtMatchfinder`]: Binary tree matchfinder with top-down tree re-rooting and monotonic match streams.
//! - [`OptParser`]: Fixed-point cost model (`BIT_COST = 16`), backward DP min-cost path search, and EM refinement.

pub mod bt_matchfinder;
pub mod checksum;
pub mod compress;
pub mod container;
pub mod decompress;
pub mod decompress_tables;
pub mod huffman;
pub mod matchfinder;
pub mod opt_parser;
pub mod reader;
pub mod writer;

pub use checksum::*;
pub use compress::*;
pub use container::*;
pub use decompress::*;
pub use decompress_tables::*;
pub use matchfinder::*;
pub use reader::*;
pub use writer::*;

pub use bt_matchfinder::{
    load_u24_le, load_u32_le, lz_hash, BtMatchfinder, LzMatch, BT_HASH3_ORDER, BT_HASH3_SIZE,
    BT_HASH3_WAYS, BT_HASH4_ORDER, BT_HASH4_SIZE, BT_REQUIRED_NBYTES, DEFAULT_MAX_SEARCH_DEPTH,
    DEFAULT_NICE_MATCH_LEN, DEFLATE_MAX_MATCH_LEN, DEFLATE_MIN_MATCH_LEN, MATCHFINDER_INITVAL,
    MATCHFINDER_WINDOW_ORDER, MATCHFINDER_WINDOW_SIZE,
};
pub use opt_parser::{
    build_matches_cache, compute_huffman_lengths, find_min_cost_path, optimize_parse_em, CostModel,
    OptimumNode, SequenceItem, BIT_COST, DEFLATE_END_OF_BLOCK, DEFLATE_FIRST_LEN_SYM,
    DEFLATE_NUM_LITERALS, DEFLATE_NUM_LITLEN_SYMS, DEFLATE_NUM_OFFSET_SYMS, EXTRA_LENGTH_BITS,
    EXTRA_OFFSET_BITS, LENGTH_SLOT_MAP, MAX_HUFFMAN_CODE_LEN,
};
pub use huffman::{
    compute_num_explicit_precode_lens, compute_precode_items, deflate_make_huffman_code,
    reverse_codeword, FastBitWriter, FastBitWriterError, FastBitWriterVec, PrecodeEncodedHeader,
    PrecodeEncoder, DEFLATE_EXTRA_LENGTH_BITS, DEFLATE_EXTRA_OFFSET_BITS,
    DEFLATE_EXTRA_PRECODE_BITS, DEFLATE_MAX_CODEWORD_LEN, DEFLATE_MAX_LITLEN_CODEWORD_LEN,
    DEFLATE_MAX_NUM_SYMS, DEFLATE_NUM_PRECODE_SYMS, DEFLATE_PRECODE_LENS_PERMUTATION, FREQ_MASK,
    MAX_LITLEN_CODEWORD_LEN, MAX_OFFSET_CODEWORD_LEN, MAX_PRE_CODEWORD_LEN, NUM_SYMBOL_BITS,
    SYMBOL_MASK,
};


