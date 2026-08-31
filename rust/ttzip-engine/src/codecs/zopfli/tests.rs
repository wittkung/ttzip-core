// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use super::*;
use crate::codecs::deflate::{deflate_decompress, gzip_decompress, zlib_decompress};

#[test]
fn test_shortest_path_matcher_repetitive() {
    let mut matcher = ZopfliShortestPathMatcher::new();
    let cost_model = ZopfliCostModel::uniform();
    let data = b"abcde_abcde_abcde_abcde";

    let tokens = matcher.find_shortest_path(data, 0, data.len(), &cost_model, 1024);
    assert!(!tokens.is_empty());

    // Verify reconstructed tokens match original data exactly
    let mut reconstructed = Vec::new();
    for token in tokens {
        match token {
            ZopfliToken::Literal(lit) => reconstructed.push(lit),
            ZopfliToken::Match { length, distance } => {
                let start = reconstructed.len() - (distance as usize);
                for i in 0..(length as usize) {
                    let b = reconstructed[start + i];
                    reconstructed.push(b);
                }
            }
        }
    }

    assert_eq!(reconstructed.as_slice(), data);
}

#[test]
fn test_shannon_cost_model_self_information() {
    let mut lit_freqs = [0u32; NUM_LITLEN_SYMS];
    let mut dist_freqs = [0u32; NUM_DIST_SYMS];

    lit_freqs[b'A' as usize] = 100;
    lit_freqs[b'B' as usize] = 10;
    dist_freqs[0] = 50;

    let model = ZopfliCostModel::from_shannon_frequencies(&lit_freqs, &dist_freqs);

    // High frequency symbol 'A' must have lower bit cost than low frequency symbol 'B'
    assert!(model.literal_cost(b'A') < model.literal_cost(b'B'));
    assert!(model.literal_cost(b'A') > 0.0);
    assert!(model.match_cost(1, 3) > 0.0);
}

#[test]
fn test_squeeze_convergence_and_optimization() {
    let mut squeeze = ZopfliSqueeze::new();
    let options = ZopfliOptions {
        num_iterations: 10,
        max_block_splits: 0,
        max_chain: 512,
    };

    let data = b"The quick brown fox jumps over the lazy dog. The quick brown fox jumps again!";
    let stats = squeeze.squeeze(data, 0, data.len(), &options);

    assert!(!stats.tokens.is_empty());
    assert!(stats.total_bits > 0.0);
    assert!(stats.num_litlen_syms >= 257);
    assert!(stats.num_dist_syms >= 1);
}

#[test]
fn test_block_splitter_homogeneous_vs_heterogeneous() {
    // 1. Homogeneous data (single uniform text pattern)
    let uniform_data = vec![b'x'; 4096];
    let splits_uniform = ZopfliBlockSplitter::split_optimal(&uniform_data, 0, uniform_data.len(), 5);
    // Uniform data has no regime shifts and should not be split
    assert!(splits_uniform.is_empty());

    // 2. Heterogeneous data (2048 bytes of 'A' followed by 2048 bytes of sequential counting bytes)
    let mut heterogeneous = vec![b'A'; 2048];
    for i in 0..2048 {
        heterogeneous.push((i & 0xFF) as u8);
    }

    let splits_hetero = ZopfliBlockSplitter::split_optimal(&heterogeneous, 0, heterogeneous.len(), 5);
    assert!(!splits_hetero.is_empty());

    // Split should be placed near the transition boundary (offset 2048)
    let split_pos = splits_hetero[0];
    let diff = (split_pos as isize - 2048).abs();
    assert!(diff <= 128, "Split position {} too far from boundary 2048", split_pos);
}

#[test]
fn test_encoder_empty_payload() {
    let options = ZopfliOptions::fast();
    let encoder = ZopfliEncoder::new(options);

    // 1. Empty Deflate
    let def_bytes = encoder.compress_deflate(&[]).expect("Empty deflate compress");
    let mut def_decomp = vec![0u8; 16];
    let def_len = deflate_decompress(&def_bytes, &mut def_decomp).expect("Empty deflate decompress");
    assert_eq!(def_len, 0);

    // 2. Empty Zlib
    let zlib_bytes = encoder.compress_zlib(&[]).expect("Empty zlib compress");
    let mut zlib_decomp = vec![0u8; 16];
    let zlib_len = zlib_decompress(&zlib_bytes, &mut zlib_decomp).expect("Empty zlib decompress");
    assert_eq!(zlib_len, 0);

    // 3. Empty Gzip
    let gzip_bytes = encoder.compress_gzip(&[]).expect("Empty gzip compress");
    let mut gzip_decomp = vec![0u8; 16];
    let gzip_len = gzip_decompress(&gzip_bytes, &mut gzip_decomp).expect("Empty gzip decompress");
    assert_eq!(gzip_len, 0);
}

#[test]
fn test_encoder_deflate_roundtrip() {
    let options = ZopfliOptions {
        num_iterations: 5,
        max_block_splits: 2,
        max_chain: 512,
    };

    let corpora: [&[u8]; 5] = [
        b"A",
        b"Hello, World! Zopfli optimal compression test.",
        b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        b"Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.",
        &[0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0],
    ];

    for (idx, &src) in corpora.iter().enumerate() {
        let compressed = zopfli_compress_deflate(src, &options)
            .unwrap_or_else(|e| panic!("Failed to compress corpus #{}: {:?}", idx, e));

        let mut decompressed = vec![0u8; src.len() + 64];
        let n = deflate_decompress(&compressed, &mut decompressed)
            .unwrap_or_else(|e| panic!("Failed to decompress corpus #{}: {:?}", idx, e));

        assert_eq!(&decompressed[..n], src, "Corpus #{} mismatch", idx);
    }
}

#[test]
fn test_encoder_zlib_roundtrip() {
    let options = ZopfliOptions::fast();
    let mut src = Vec::with_capacity(8192);
    for i in 0..8192 {
        src.push(((i * 41 + 7) & 0xFF) as u8);
    }

    let compressed = zopfli_compress_zlib(&src, &options).expect("Zlib compress");
    let mut decompressed = vec![0u8; src.len() + 64];
    let n = zlib_decompress(&compressed, &mut decompressed).expect("Zlib decompress");

    assert_eq!(&decompressed[..n], &src);
}

#[test]
fn test_encoder_gzip_roundtrip() {
    let options = ZopfliOptions::fast();
    let text = "The quick brown fox jumps over the lazy dog. ".repeat(64);
    let src = text.as_bytes();

    let compressed = zopfli_compress_gzip(src, &options).expect("Gzip compress");
    let mut decompressed = vec![0u8; src.len() + 64];
    let n = gzip_decompress(&compressed, &mut decompressed).expect("Gzip decompress");

    assert_eq!(&decompressed[..n], src);
}
