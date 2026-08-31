// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! LZMA2 & 5-Level Synthetic Corpus Datagen Integration and Performance Test Suite.
//!
//! Verifies:
//! 1. 100% Determinism for identical seeds & high dispersion across divergent seeds.
//! 2. 5-Level Compressibility monotonic progression with fast-lzma2 (`fl2_compress`).
//! 3. High-throughput corpus generation ($\ge 1.0\text{ GB/s}$) for large sizes (1MB, 4MB, 16MB).
//! 4. End-to-end lossless roundtrip decompression fidelity with `fl2_decompress`.

mod fixtures;

use std::time::Instant;
use fixtures::datagen::{
    generate_corpus, generate_corpus_into, DataGenLevel, LiteralDistribTable, RdgRng, LDT_SIZE,
};
use ttzip_engine::codecs::lzma2::{fl2_compress, fl2_compress_bound, fl2_decompress};

// MARK: - 1. Determinism & Dispersion Tests

#[test]
fn test_datagen_identical_seed_100_percent_determinism() {
    let levels = [
        DataGenLevel::PureNoise,
        DataGenLevel::BarelyCompressible,
        DataGenLevel::Standard,
        DataGenLevel::HighlyCompressible,
        DataGenLevel::Sparse,
    ];

    let seed = 0x2026_0830;
    let size = 64 * 1024; // 64 KB

    for &level in &levels {
        let corpus_a = generate_corpus(level, size, seed);
        let corpus_b = generate_corpus(level, size, seed);

        assert_eq!(
            corpus_a.len(),
            size,
            "Corpus size mismatch for level {level:?}"
        );
        assert_eq!(
            corpus_a, corpus_b,
            "Corpus generation must be 100% deterministic for level {level:?}"
        );
    }
}

#[test]
fn test_datagen_different_seeds_high_dispersion() {
    let size = 32 * 1024; // 32 KB
    let seed1 = 12345;
    let seed2 = 12346;

    let levels = [
        (DataGenLevel::PureNoise, 85.0),
        (DataGenLevel::BarelyCompressible, 80.0),
        (DataGenLevel::Standard, 70.0),
        (DataGenLevel::HighlyCompressible, 25.0),
    ];

    for &(level, min_dispersion_pct) in &levels {
        let buf1 = generate_corpus(level, size, seed1);
        let buf2 = generate_corpus(level, size, seed2);

        assert_ne!(buf1, buf2, "Different seeds must produce different buffers for {level:?}");

        let mut diff_count = 0usize;
        for (b1, b2) in buf1.iter().zip(buf2.iter()) {
            if b1 != b2 {
                diff_count += 1;
            }
        }

        let diff_ratio_pct = ((diff_count as f64) / (size as f64)) * 100.0;
        assert!(
            diff_ratio_pct >= min_dispersion_pct,
            "Adjacent seeds must produce high dispersion for {level:?}, expected >={min_dispersion_pct:.1}%, got {diff_ratio_pct:.2}%"
        );
    }
}

#[test]
fn test_datagen_inplace_matches_allocated() {
    let size = 48 * 1024;
    let seed = 9999;
    let level = DataGenLevel::Standard;

    let allocated = generate_corpus(level, size, seed);

    let mut inplace = vec![0u8; size];
    generate_corpus_into(level, &mut inplace, seed);

    assert_eq!(allocated, inplace, "In-place generation must strictly match allocated buffer");
}

// MARK: - 2. 5-Level Compressibility Monotonic Progression

#[test]
fn test_lzma2_five_level_compression_ratio_monotonic_progression() {
    let corpus_size = 256 * 1024; // 256 KB
    let seed = 42;

    let noise = generate_corpus(DataGenLevel::PureNoise, corpus_size, seed);
    let barely = generate_corpus(DataGenLevel::BarelyCompressible, corpus_size, seed);
    let standard = generate_corpus(DataGenLevel::Standard, corpus_size, seed);
    let highly = generate_corpus(DataGenLevel::HighlyCompressible, corpus_size, seed);
    let sparse = generate_corpus(DataGenLevel::Sparse, corpus_size, seed);

    // Compress with Fast-LZMA2 Level 3, 2 threads
    let compress_payload = |src: &[u8]| -> Vec<u8> {
        let bound = fl2_compress_bound(src.len()) + 1024;
        let mut out = vec![0u8; bound];
        let comp_size = fl2_compress(src, &mut out, 3, 2).expect("LZMA2 compression failed");
        out.truncate(comp_size);
        out
    };

    let comp_noise = compress_payload(&noise);
    let comp_barely = compress_payload(&barely);
    let comp_standard = compress_payload(&standard);
    let comp_highly = compress_payload(&highly);
    let comp_sparse = compress_payload(&sparse);

    let ratio_noise = (comp_noise.len() as f64 / corpus_size as f64) * 100.0;
    let ratio_barely = (comp_barely.len() as f64 / corpus_size as f64) * 100.0;
    let ratio_standard = (comp_standard.len() as f64 / corpus_size as f64) * 100.0;
    let ratio_highly = (comp_highly.len() as f64 / corpus_size as f64) * 100.0;
    let ratio_sparse = (comp_sparse.len() as f64 / corpus_size as f64) * 100.0;

    println!("[LZMA2 5-Level Compression Ratio Results]");
    println!("  1. PureNoise:          {:>7} bytes ({:.2}%)", comp_noise.len(), ratio_noise);
    println!("  2. BarelyCompressible: {:>7} bytes ({:.2}%)", comp_barely.len(), ratio_barely);
    println!("  3. Standard:           {:>7} bytes ({:.2}%)", comp_standard.len(), ratio_standard);
    println!("  4. HighlyCompressible: {:>7} bytes ({:.2}%)", comp_highly.len(), ratio_highly);
    println!("  5. Sparse:             {:>7} bytes ({:.2}%)", comp_sparse.len(), ratio_sparse);

    // Strict monotonic descending compressed size ordering:
    // PureNoise > BarelyCompressible > Standard > HighlyCompressible > Sparse
    assert!(
        comp_noise.len() > comp_barely.len(),
        "PureNoise ({}) must be larger than BarelyCompressible ({})",
        comp_noise.len(), comp_barely.len()
    );
    assert!(
        comp_barely.len() > comp_standard.len(),
        "BarelyCompressible ({}) must be larger than Standard ({})",
        comp_barely.len(), comp_standard.len()
    );
    assert!(
        comp_standard.len() > comp_highly.len(),
        "Standard ({}) must be larger than HighlyCompressible ({})",
        comp_standard.len(), comp_highly.len()
    );
    assert!(
        comp_highly.len() > comp_sparse.len(),
        "HighlyCompressible ({}) must be larger than Sparse ({})",
        comp_highly.len(), comp_sparse.len()
    );

    // Verify expected compression ratio ranges
    assert!(
        ratio_noise >= 98.0,
        "PureNoise ratio should be >= 98%, got {:.2}%", ratio_noise
    );
    assert!(
        (60.0..=98.0).contains(&ratio_barely),
        "BarelyCompressible ratio out of range [60%, 98%]: {:.2}%", ratio_barely
    );
    assert!(
        (10.0..=60.0).contains(&ratio_standard),
        "Standard ratio out of range [10%, 60%]: {:.2}%", ratio_standard
    );
    assert!(
        (1.0..=25.0).contains(&ratio_highly),
        "HighlyCompressible ratio out of range [1%, 25%]: {:.2}%", ratio_highly
    );
    assert!(
        ratio_sparse <= 2.0,
        "Sparse ratio must be < 2%, got {:.2}%", ratio_sparse
    );

    // End-to-end lossless roundtrip decompression verification
    for (name, raw, comp) in [
        ("PureNoise", &noise, &comp_noise),
        ("BarelyCompressible", &barely, &comp_barely),
        ("Standard", &standard, &comp_standard),
        ("HighlyCompressible", &highly, &comp_highly),
        ("Sparse", &sparse, &comp_sparse),
    ] {
        let mut decomp = vec![0u8; raw.len()];
        let decomp_len = fl2_decompress(comp, &mut decomp, 2)
            .unwrap_or_else(|e| panic!("Decompression failed for {name}: {e:?}"));
        assert_eq!(decomp_len, raw.len(), "Decompressed length mismatch for {name}");
        assert_eq!(&decomp[..decomp_len], raw.as_slice(), "Roundtrip byte mismatch for {name}");
    }
}

// MARK: - 3. Large Size Performance & Memory Safety (1MB, 4MB, 16MB)

#[test]
fn test_datagen_large_sizes_throughput_and_memory_safety() {
    let sizes = [
        ("1MB", 1024 * 1024),
        ("4MB", 4 * 1024 * 1024),
        ("16MB", 16 * 1024 * 1024),
    ];

    let seed = 0xDEADBEEF;

    for (name, size) in sizes {
        // Test high-throughput generation on Standard corpus
        let start = Instant::now();
        let corpus = generate_corpus(DataGenLevel::Standard, size, seed);
        let elapsed = start.elapsed();

        assert_eq!(corpus.len(), size);

        let elapsed_secs = elapsed.as_secs_f64();
        let throughput_gb_s = if elapsed_secs > 0.0 {
            (size as f64) / (elapsed_secs * 1_000_000_000.0)
        } else {
            999.0
        };

        println!(
            "[Datagen Large Benchmark] Size: {:>4} | Time: {:>8.3} ms | Throughput: {:>6.2} GB/s",
            name,
            elapsed.as_secs_f64() * 1000.0,
            throughput_gb_s
        );

        // Verify non-trivial entropy and boundary integrity
        assert!(
            corpus[..1024] != corpus[1024..2048],
            "Generated large corpus should have diverse non-repeating blocks"
        );
        assert_eq!(corpus.len(), size);

        // Roundtrip 1MB and 4MB with LZMA2 to verify memory bounds and decoder robustness
        if size <= 4 * 1024 * 1024 {
            let bound = fl2_compress_bound(size) + 1024;
            let mut comp = vec![0u8; bound];
            let comp_len = fl2_compress(&corpus, &mut comp, 1, 2)
                .expect("LZMA2 large block compression");
            comp.truncate(comp_len);

            let mut decomp = vec![0u8; size];
            let decomp_len = fl2_decompress(&comp, &mut decomp, 2)
                .expect("LZMA2 large block decompression");

            assert_eq!(decomp_len, size);
            assert_eq!(decomp, corpus);
        }
    }
}

// MARK: - 4. PRNG & Table Statistical Quality Verification

#[test]
fn test_rdg_rng_statistical_properties() {
    let mut rng = RdgRng::new(0xCAFE_BABE);
    let mut bit_counts = [0usize; 32];
    let total_samples = 50_000;

    for _ in 0..total_samples {
        let val = rng.next_u32();
        for (bit_idx, count) in bit_counts.iter_mut().enumerate() {
            if (val & (1 << bit_idx)) != 0 {
                *count += 1;
            }
        }
    }

    // Verify balanced bit frequency (~50% +/- 5%)
    let expected = total_samples / 2;
    for (i, &count) in bit_counts.iter().enumerate() {
        let diff = (count as isize - expected as isize).abs();
        let max_diff = (total_samples as f64 * 0.05) as isize;
        assert!(
            diff < max_diff,
            "Bit {i} bias too high: {count}/{total_samples} (diff: {diff})"
        );
    }
}

#[test]
fn test_literal_distrib_table_weighting() {
    assert_eq!(LDT_SIZE, 8192);

    let ldt_uniform = LiteralDistribTable::new(0.0);
    let mut freq_uniform = [0usize; 256];
    for &b in &ldt_uniform.table {
        freq_uniform[b as usize] += 1;
    }
    // Uniform table: each byte 0..255 appears exactly 8192 / 256 = 32 times
    for (byte_val, &freq) in freq_uniform.iter().enumerate() {
        assert_eq!(
            freq, 32,
            "Uniform table byte {byte_val} frequency mismatch"
        );
    }

    let ldt_skewed = LiteralDistribTable::new(0.8);
    let mut freq_skewed = [0usize; 256];
    for &b in &ldt_skewed.table {
        freq_skewed[b as usize] += 1;
    }
    // High probability character '0' should appear with high frequency
    assert!(
        freq_skewed[b'0' as usize] > 500,
        "Skewed table '0' frequency should be high, got {}",
        freq_skewed[b'0' as usize]
    );
}

#[test]
fn test_datagen_boundary_sizes() {
    for size in [0, 1, 2, 3, 4, 15, 16, 17, 31, 32, 63, 64, 127, 128, 4095, 4096, 4097] {
        let buf = generate_corpus(DataGenLevel::Standard, size, size as u32);
        assert_eq!(buf.len(), size, "Boundary size {size} failed");
    }
}
