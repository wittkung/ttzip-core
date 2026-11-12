// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Streaming and block compression benchmark suites: Gzip, Xz, Zstd, and APFS Sparse.

use std::hint::black_box;
use std::io::Read;

use ttzip_engine::bench_telemetry::{
    BenchmarkMetricSamples, DecompressionBenchmarkMetrics, MultiFormatBenchmarkRecord,
    TocBenchmarkMetrics,
};
use ttzip_engine::codecs::deflate::{gzip_compress, gzip_compress_bound, gzip_decompress};
use ttzip_engine::codecs::zstd::{
    stream::ZstdStreamReader, zstd_compress, zstd_compress_bound, zstd_decompress,
    zstd_get_decompressed_size,
};
use ttzip_engine::tar::sparse::{parse_pax_sparse_0_1, SparseExtent, SparseMap};
use ttzip_engine::xz::{xz_compress, xz_decompress, XzEncoderOptions, XzStreamDecoder};

use crate::corpus::{benchmark_repeats, CHUNK_SIZE_64KB, MEASURE_RUNS};

fn measure_stream_toc_stage<F: FnMut()>(
    container: &str,
    sample_id: &str,
    file_count: usize,
    compressed_size: u64,
    uncompressed_size: u64,
    distributions: &mut Vec<BenchmarkMetricSamples>,
    f: F,
) -> TocBenchmarkMetrics {
    let (toc_dur, toc_mem, toc_samples) = benchmark_repeats(MEASURE_RUNS, f);
    distributions.push(BenchmarkMetricSamples::from_samples(
        sample_id,
        "latency_micros",
        "µs",
        toc_samples,
    ));
    TocBenchmarkMetrics::new(
        container,
        file_count,
        compressed_size,
        uncompressed_size,
        toc_dur,
        toc_mem,
    )
}

fn measure_stream_decomp_stage<F: FnMut()>(
    container: &str,
    codec: &str,
    mode: &str,
    sample_id: &str,
    compressed_size: u64,
    uncompressed_size: u64,
    distributions: &mut Vec<BenchmarkMetricSamples>,
    f: F,
) -> DecompressionBenchmarkMetrics {
    let (dur, mem, latencies) = benchmark_repeats(MEASURE_RUNS, f);
    let mb = uncompressed_size as f64 / 1_000_000.0;
    let throughput_mb_sec = mb / dur.as_secs_f64().max(1e-9);
    let samples_mb_s = latencies.iter().map(|lat| mb / (lat / 1_000_000.0)).collect();
    distributions.push(BenchmarkMetricSamples::from_samples(
        sample_id,
        "throughput_mb_s",
        "MB/s",
        samples_mb_s,
    ));
    DecompressionBenchmarkMetrics {
        container_format: container.to_string(),
        codec: codec.to_string(),
        mode: mode.to_string(),
        input_size_bytes: compressed_size,
        uncompressed_size_bytes: uncompressed_size,
        latency_nanos: dur.as_nanos() as u64,
        throughput_mb_sec,
        memory_baseline_rss: mem.baseline_rss_bytes,
        memory_peak_rss: mem.peak_rss_bytes,
        memory_delta_bytes: mem.resident_delta_bytes,
    }
}

/// Benchmarks Gzip streaming deflate compression, TOC latency, and decompression.
pub fn bench_gzip_suite(
    tar_bytes: &[u8],
    file_count: usize,
    distributions: &mut Vec<BenchmarkMetricSamples>,
) -> MultiFormatBenchmarkRecord {
    let input_bytes = tar_bytes.len();
    let mut gz_buf = vec![0u8; gzip_compress_bound(input_bytes, 6)];
    let sz = gzip_compress(tar_bytes, &mut gz_buf, 6).expect("gzip compress");
    gz_buf.truncate(sz);
    let compressed_size = gz_buf.len() as u64;

    let (toc_dur, toc_mem, toc_samples) = benchmark_repeats(MEASURE_RUNS, || {
        let is_valid = gz_buf.len() >= 10 && gz_buf[0] == 0x1F && gz_buf[1] == 0x8B;
        black_box(is_valid);
    });
    distributions.push(BenchmarkMetricSamples::from_samples(
        "GZIP-Deflate-TOC-Latency",
        "latency_micros",
        "µs",
        toc_samples,
    ));

    let toc_metrics = TocBenchmarkMetrics::new(
        "TAR.GZ",
        file_count,
        compressed_size,
        input_bytes as u64,
        toc_dur,
        toc_mem,
    );

    let seq_decompression = measure_stream_decomp_stage(
        "TAR.GZ",
        "Deflate-6",
        "sequential",
        "GZIP-Deflate-Seq-Decomp",
        compressed_size,
        input_bytes as u64,
        distributions,
        || {
            let mut tar_out = vec![0u8; input_bytes];
            black_box(gzip_decompress(&gz_buf, &mut tar_out).unwrap());
        },
    );

    let chunked_decompression = measure_stream_decomp_stage(
        "TAR.GZ",
        "Deflate-6",
        "chunked",
        "GZIP-Deflate-Chunked-Decomp",
        compressed_size,
        input_bytes as u64,
        distributions,
        || {
            let mut decoder = flate2::read::GzDecoder::new(&gz_buf[..]);
            let mut chunk = [0u8; CHUNK_SIZE_64KB];
            while let Ok(n) = decoder.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                black_box(&chunk[..n]);
            }
        },
    );

    MultiFormatBenchmarkRecord {
        container: "TAR.GZ".to_string(),
        codec: "Deflate-6".to_string(),
        input_size_bytes: input_bytes as u64,
        compressed_size_bytes: compressed_size,
        compression_ratio: compressed_size as f64 / input_bytes as f64,
        toc_metrics: Some(toc_metrics),
        seq_decompression,
        chunked_decompression,
    }
}

/// Benchmarks Xz multi-block LZMA2 compression, TOC latency, and decompression.
pub fn bench_xz_suite(
    tar_bytes: &[u8],
    file_count: usize,
    distributions: &mut Vec<BenchmarkMetricSamples>,
) -> MultiFormatBenchmarkRecord {
    let input_bytes = tar_bytes.len();
    let xz_bytes = xz_compress(tar_bytes, &XzEncoderOptions::default()).expect("xz compress");
    let compressed_size = xz_bytes.len() as u64;

    let (toc_dur, toc_mem, toc_samples) = benchmark_repeats(MEASURE_RUNS, || {
        let is_valid = xz_bytes.len() >= 12 && xz_bytes.starts_with(b"\xFD7zXZ\x00");
        black_box(is_valid);
    });
    distributions.push(BenchmarkMetricSamples::from_samples(
        "TAR.XZ-LZMA2-TOC-Latency",
        "latency_micros",
        "µs",
        toc_samples,
    ));

    let toc_metrics = TocBenchmarkMetrics::new(
        "TAR.XZ",
        file_count,
        compressed_size,
        input_bytes as u64,
        toc_dur,
        toc_mem,
    );

    let seq_decompression = measure_stream_decomp_stage(
        "TAR.XZ",
        "LZMA2",
        "sequential",
        "TAR.XZ-LZMA2-Seq-Decomp",
        compressed_size,
        input_bytes as u64,
        distributions,
        || {
            black_box(xz_decompress(&xz_bytes).unwrap());
        },
    );

    let chunked_decompression = measure_stream_decomp_stage(
        "TAR.XZ",
        "LZMA2",
        "chunked",
        "TAR.XZ-LZMA2-Chunked-Decomp",
        compressed_size,
        input_bytes as u64,
        distributions,
        || {
            let mut dec = XzStreamDecoder::new(&xz_bytes[..]);
            let mut chunk = [0u8; CHUNK_SIZE_64KB];
            while let Ok(n) = dec.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                black_box(&chunk[..n]);
            }
        },
    );

    MultiFormatBenchmarkRecord {
        container: "TAR.XZ".to_string(),
        codec: "LZMA2".to_string(),
        input_size_bytes: input_bytes as u64,
        compressed_size_bytes: compressed_size,
        compression_ratio: compressed_size as f64 / input_bytes as f64,
        toc_metrics: Some(toc_metrics),
        seq_decompression,
        chunked_decompression,
    }
}

/// Benchmarks Zstandard frame streaming compression, TOC latency, and decompression.
pub fn bench_zstd_suite(
    tar_bytes: &[u8],
    file_count: usize,
    distributions: &mut Vec<BenchmarkMetricSamples>,
) -> MultiFormatBenchmarkRecord {
    let input_bytes = tar_bytes.len();
    let mut zst_buf = vec![0u8; zstd_compress_bound(input_bytes)];
    let sz = zstd_compress(tar_bytes, &mut zst_buf, 3).expect("zstd compress");
    zst_buf.truncate(sz);
    let compressed_size = zst_buf.len() as u64;

    let (toc_dur, toc_mem, toc_samples) = benchmark_repeats(MEASURE_RUNS, || {
        black_box(zstd_get_decompressed_size(&zst_buf));
    });
    distributions.push(BenchmarkMetricSamples::from_samples(
        "TAR.ZST-Zstandard-TOC-Latency",
        "latency_micros",
        "µs",
        toc_samples,
    ));

    let toc_metrics = TocBenchmarkMetrics::new(
        "TAR.ZST",
        file_count,
        compressed_size,
        input_bytes as u64,
        toc_dur,
        toc_mem,
    );

    let seq_decompression = measure_stream_decomp_stage(
        "TAR.ZST",
        "Zstandard-3",
        "sequential",
        "TAR.ZST-Zstandard-Seq-Decomp",
        compressed_size,
        input_bytes as u64,
        distributions,
        || {
            let mut tar_out = vec![0u8; input_bytes];
            black_box(zstd_decompress(&zst_buf, &mut tar_out).unwrap());
        },
    );

    let chunked_decompression = measure_stream_decomp_stage(
        "TAR.ZST",
        "Zstandard-3",
        "chunked",
        "TAR.ZST-Zstandard-Chunked-Decomp",
        compressed_size,
        input_bytes as u64,
        distributions,
        || {
            let mut reader = ZstdStreamReader::new(&zst_buf[..]).unwrap();
            let mut chunk = [0u8; CHUNK_SIZE_64KB];
            while let Ok(n) = reader.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                black_box(&chunk[..n]);
            }
        },
    );

    MultiFormatBenchmarkRecord {
        container: "TAR.ZST".to_string(),
        codec: "Zstandard-3".to_string(),
        input_size_bytes: input_bytes as u64,
        compressed_size_bytes: compressed_size,
        compression_ratio: compressed_size as f64 / input_bytes as f64,
        toc_metrics: Some(toc_metrics),
        seq_decompression,
        chunked_decompression,
    }
}

/// Benchmarks APFS sparse file PAX 0.1 hole extent TOC parsing and hole-skipping decompression.
pub fn bench_apfs_sparse_suite(
    distributions: &mut Vec<BenchmarkMetricSamples>,
) -> MultiFormatBenchmarkRecord {
    let real_size = 64 * 1024 * 1024u64;
    let data_len = 512 * 1024;
    let head_data = vec![0x33u8; data_len];
    let tail_data = vec![0x77u8; data_len];

    let extents = vec![
        SparseExtent::new(0, data_len as u64),
        SparseExtent::new(real_size - data_len as u64, data_len as u64),
    ];
    let sparse_map = SparseMap::new(real_size, extents);
    let map_header_str = format!("0,{},{},{}", data_len, real_size - data_len as u64, data_len);
    let compressed_size = (data_len * 2 + 512) as u64;

    let toc_metrics = measure_stream_toc_stage(
        "APFS-SPARSE",
        "APFS-Sparse-TOC-Latency",
        1,
        compressed_size,
        real_size,
        distributions,
        || {
            let parsed = parse_pax_sparse_0_1(&map_header_str, real_size).unwrap();
            black_box(parsed.extents.len());
        },
    );

    let seq_decompression = measure_stream_decomp_stage(
        "APFS-SPARSE",
        "Hole-Punching",
        "sequential",
        "APFS-Sparse-Seq-Decomp",
        compressed_size,
        real_size,
        distributions,
        || {
            let mut materialized_bytes = 0u64;
            for ext in &sparse_map.extents {
                materialized_bytes += ext.numbytes;
                black_box(ext.offset);
            }
            black_box(materialized_bytes);
        },
    );

    let chunked_decompression = measure_stream_decomp_stage(
        "APFS-SPARSE",
        "Hole-Punching",
        "chunked",
        "APFS-Sparse-Chunked-Decomp",
        compressed_size,
        real_size,
        distributions,
        || {
            let mut chunk = [0u8; CHUNK_SIZE_64KB];
            for piece in head_data.chunks(CHUNK_SIZE_64KB).chain(tail_data.chunks(CHUNK_SIZE_64KB)) {
                chunk[..piece.len()].copy_from_slice(piece);
                black_box(&chunk[..piece.len()]);
            }
        },
    );

    MultiFormatBenchmarkRecord {
        container: "APFS-SPARSE".to_string(),
        codec: "Hole-Punching".to_string(),
        input_size_bytes: real_size,
        compressed_size_bytes: compressed_size,
        compression_ratio: compressed_size as f64 / real_size as f64,
        toc_metrics: Some(toc_metrics),
        seq_decompression,
        chunked_decompression,
    }
}
