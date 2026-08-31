// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-throughput LZ4 Preloaded Dictionary (`Lz4PreloadedDict`), Zero-Copy Attached Dictionary
//! Compressor (`Lz4DictCompressor`), and External Segment Decompressor (`lz4_decompress_safe_ext_dict`).
//!
//! # Architecture & Capabilities
//!
//! 1. **`Lz4PreloadedDict` (Thread-Safe Immutable Dictionary Context)**:
//!    - Pre-digests standard $64\,\text{KB}$ sliding history windows into a $32768$-slot hash table.
//!    - Dual loading strategies:
//!      - `load_dict_fast`: Accelerated strided stepping for high-throughput initialization.
//!      - `load_dict_slow`: 1-byte step secondary scan providing 100% dictionary sequence indexing.
//!    - Strictly `Send + Sync`, enabling zero-copy sharing via `Arc<Lz4PreloadedDict>` across concurrent threads.
//!
//! 2. **`Lz4DictCompressor` (Zero-Copy AttachDictionary Engine)**:
//!    - `attach_dictionary`: Zero-allocation attachment without copying or mutating the preloaded dictionary table.
//!    - Dual-tiered compression engine:
//!      - Small Blocks ($\le 4\,\text{KB}$): Two-level lookup (`local_table` + `dict_table`), bypassing full table clears.
//!      - Large Blocks ($> 4\,\text{KB}$): Unified virtual address space single-table match finding.
//!
//! 3. **`lz4_decompress_safe_ext_dict` (External Memory Segment Decompression)**:
//!    - Seamlessly resolves match references across independent disjoint memory buffers (`src`, `dst`, `dict`).
//!    - Multi-mode match reconstruction:
//!      - Intra-block match (`offset <= op`): Decoded from current output buffer.
//!      - Intra-dictionary match (`offset > op && match_len <= bytes_in_dict`): Decoded directly from external dictionary.
//!      - Cross-boundary match (`offset > op && match_len > bytes_in_dict`): Dual-segment reconstruction seamlessly
//!        stitching dictionary tail and output buffer prefix.
//!    - Strictly bounded: zero-offset rejection, cascade sum overflow defense, and out-of-history validation.

mod compressor;
mod decompress;
mod preloaded;

pub use compressor::*;
pub use decompress::*;
pub use preloaded::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preloaded_dict_properties() {
        let sample = b"Common JSON keys: id, name, timestamp, path, size, flags, metadata.";
        let dict = Lz4PreloadedDict::with_dict_id(sample, 0xABCDEF01);
        assert_eq!(dict.dict_id(), Some(0xABCDEF01));
        assert_eq!(dict.len(), sample.len());
        assert!(!dict.is_empty());
        assert_eq!(dict.as_slice(), sample);
        assert_eq!(dict.effective_slice(), sample);
    }

    #[test]
    fn test_ext_dict_small_and_large_block_roundtrips() {
        let dict_data = b"Common structured dictionary headers, schema prefixes, and repeated keywords 2026.";
        let dict = Lz4PreloadedDict::new(dict_data);

        // 1. Small block (<= 4KB)
        let small_payload = b"schema prefixes and Common structured dictionary headers for record #12345.";
        let small_comp = dict.compress_to_vec(small_payload, 1).expect("small compress");
        let small_decomp = dict
            .decompress_to_vec(&small_comp, small_payload.len())
            .expect("small decompress");
        assert_eq!(small_decomp.as_slice(), small_payload);

        // 2. Large block (> 4KB)
        let mut large_payload = Vec::new();
        for i in 0..100 {
            large_payload.extend_from_slice(
                format!("Record #{i:04}: Common structured dictionary headers schema prefixes repeated keywords\n").as_bytes(),
            );
        }
        let large_comp = dict.compress_to_vec(&large_payload, 1).expect("large compress");
        let large_decomp = dict
            .decompress_to_vec(&large_comp, large_payload.len())
            .expect("large decompress");
        assert_eq!(large_decomp.as_slice(), large_payload.as_slice());
    }

    #[test]
    fn test_ext_dict_cross_boundary_dual_segment_decompression() {
        let dict_data = b"ABCDEFGHIJ_TAIL_DICTIONARY_12345";
        let dict = Lz4PreloadedDict::new(dict_data);

        let mut payload = Vec::new();
        payload.extend_from_slice(b"_TAIL_DICTIONARY_12345");
        payload.extend_from_slice(b"_AND_BLOCK_PREFIX_AND_BLOCK_PREFIX_AND_BLOCK_PREFIX");

        let comp = dict.compress_to_vec(&payload, 1).expect("compress cross boundary");
        let decomp = dict.decompress_to_vec(&comp, payload.len()).expect("decompress cross boundary");
        assert_eq!(decomp.as_slice(), payload.as_slice());
    }
}
