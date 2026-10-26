// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive container benchmark suites: ZIP (Deflate, Zstd, LZMA2), 7-Zip, and POSIX TAR.

use std::hint::black_box;
use std::io::Read;

use ttzip_engine::archive::tar::{TarArchive, TarWriter};
use ttzip_engine::bench_telemetry::{
    BenchmarkMetricSamples, DecompressionBenchmarkMetrics, MultiFormatBenchmarkRecord,
    TocBenchmarkMetrics,
};
use ttzip_engine::codecs::deflate::{
    with_thread_local_compressor, with_thread_local_decompressor,
};
use ttzip_engine::codecs::lzma2::{fl2_compress, fl2_compress_bound, fl2_decompress};
use ttzip_engine::codecs::zstd::{
    stream::ZstdStreamReader, zstd_compress, zstd_compress_bound, zstd_decompress,
};
use ttzip_engine::sevenz::{create_7z_solid_archive_bytes, SevenZArchive};
use ttzip_engine::zip::writer::assemble_zip_archive;
use ttzip_engine::zip::{ZipArchive, ZipCompressedItem};

use crate::corpus::{benchmark_repeats, SyntheticCorpus, CHUNK_SIZE_64KB, MEASURE_RUNS};

/// Benchmarks ZIP container compression, TOC latency, and decompression for a specific codec.
pub fn bench_zip_variant(
    corpus: &SyntheticCorpus,
    codec_name: &'static str,
    actual_method: u16,
    distributions: &mut Vec<BenchmarkMetricSamples>,
) -> MultiFormatBenchmarkRecord {
    let input_bytes = corpus.total_uncompressed_bytes;

    // 1. In-memory compression & assembly
    let compressed_items: Vec<ZipCompressedItem> = corpus
        .files
        .iter()
        .map(|(path, data)| {
            let mut payload = Vec::new();
            match actual_method {
                8 => {
                    with_thread_local_compressor(6, |comp| {
                        let mut buf = vec![0u8; comp.compress_bound(data.len())];
                        let sz = comp.compress(data, &mut buf).unwrap();
                        buf.truncate(sz);
                        payload = buf;
                        Ok(())
                    })
                    .unwrap();
                }
                93 => {
                    let mut buf = vec![0u8; zstd_compress_bound(data.len())];
                    let sz = zstd_compress(data, &mut buf, 3).unwrap();
                    buf.truncate(sz);
                    payload = buf;
                }
                14 => {
                    let mut buf = vec![0u8; fl2_compress_bound(data.len())];
                    let sz = fl2_compress(data, &mut buf, 6, 1).unwrap();
                    buf.truncate(sz);
                    payload = buf;
                }
                _ => payload = data.clone(),
            }
            ZipCompressedItem {
                rel_path: path.clone(),
                uncompressed_size: data.len() as u64,
                compressed_size: payload.len() as u64,
                crc32: 0x12345678,
                compression_method: actual_method,
                actual_method,
                aes_strength: 0,
                payload,
                mtime_epoch_secs: 1770000000,
                mode: 0o644,
                is_directory: false,
                is_encrypted: false,
            }
        })
        .collect();

    let zip_bytes = assemble_zip_archive(&compressed_items).expect("assemble zip");
    let compressed_size = zip_bytes.len() as u64;
    let compression_ratio = compressed_size as f64 / input_bytes as f64;

    // 2. TOC Extraction Benchmark
    let (toc_dur, toc_mem, toc_samples) = benchmark_repeats(MEASURE_RUNS, || {
        let zip = ZipArchive::open_slice(&zip_bytes).unwrap();
        black_box(zip.entries().len());
    });
    distributions.push(BenchmarkMetricSamples::from_samples(
        format!("ZIP-{}-TOC-Latency", codec_name),
        "latency_micros",
        "µs",
        toc_samples,
    ));

    let toc_metrics = TocBenchmarkMetrics::new(
        "ZIP",
        corpus.files.len(),
        compressed_size,
        input_bytes as u64,
        toc_dur,
        toc_mem,
    );

    // 3. Sequential Decompression Benchmark
    let (seq_dur, seq_mem, seq_latencies) = benchmark_repeats(MEASURE_RUNS, || {
        let zip = ZipArchive::open_slice(&zip_bytes).unwrap();
        black_box(zip.entries().len());
        for item in &compressed_items {
            let mut out = vec![0u8; item.uncompressed_size as usize];
            match item.actual_method {
                8 => {
                    with_thread_local_decompressor(|dec| {
                        dec.decompress(&item.payload, &mut out).unwrap();
                        Ok(())
                    })
                    .unwrap();
                }
                93 => {
                    zstd_decompress(&item.payload, &mut out).unwrap();
                }
                14 => {
                    fl2_decompress(&item.payload, &mut out, 1).unwrap();
                }
                _ => out.copy_from_slice(&item.payload),
            }
            black_box(out);
        }
    });

    let seq_mb_s = (input_bytes as f64 / 1_000_000.0) / seq_dur.as_secs_f64().max(1e-9);
    let seq_samples_mb_s = seq_latencies
        .iter()
        .map(|lat| (input_bytes as f64 / 1_000_000.0) / (lat / 1_000_000.0))
        .collect();
    distributions.push(BenchmarkMetricSamples::from_samples(
        format!("ZIP-{}-Seq-Decomp", codec_name),
        "throughput_mb_s",
        "MB/s",
        seq_samples_mb_s,
    ));

    let seq_decompression = DecompressionBenchmarkMetrics {
        container_format: "ZIP".to_string(),
        codec: codec_name.to_string(),
        mode: "sequential".to_string(),
        input_size_bytes: compressed_size,
        uncompressed_size_bytes: input_bytes as u64,
        latency_nanos: seq_dur.as_nanos() as u64,
        throughput_mb_sec: seq_mb_s,
        memory_baseline_rss: seq_mem.baseline_rss_bytes,
        memory_peak_rss: seq_mem.peak_rss_bytes,
        memory_delta_bytes: seq_mem.resident_delta_bytes,
    };

    // 4. Chunked (64KB bounded) Decompression Benchmark
    let (chk_dur, chk_mem, chk_latencies) = benchmark_repeats(MEASURE_RUNS, || {
        let mut chunk_buf = [0u8; CHUNK_SIZE_64KB];
        for item in &compressed_items {
            match item.actual_method {
                8 => {
                    let mut dec = flate2::read::DeflateDecoder::new(&item.payload[..]);
                    while let Ok(n) = dec.read(&mut chunk_buf) {
                        if n == 0 {
                            break;
                        }
                        black_box(&chunk_buf[..n]);
                    }
                }
                93 => {
                    let mut reader = ZstdStreamReader::new(&item.payload[..]).unwrap();
                    while let Ok(n) = reader.read(&mut chunk_buf) {
                        if n == 0 {
                            break;
                        }
                        black_box(&chunk_buf[..n]);
                    }
                }
                14 => {
                    let mut out = vec![0u8; item.uncompressed_size as usize];
                    fl2_decompress(&item.payload, &mut out, 1).unwrap();
                    for chunk in out.chunks(CHUNK_SIZE_64KB) {
                        chunk_buf[..chunk.len()].copy_from_slice(chunk);
                        black_box(&chunk_buf[..chunk.len()]);
                    }
                }
                _ => {}
            }
        }
    });

    let chk_mb_s = (input_bytes as f64 / 1_000_000.0) / chk_dur.as_secs_f64().max(1e-9);
    let chk_samples_mb_s = chk_latencies
        .iter()
        .map(|lat| (input_bytes as f64 / 1_000_000.0) / (lat / 1_000_000.0))
        .collect();
    distributions.push(BenchmarkMetricSamples::from_samples(
        format!("ZIP-{}-Chunked-Decomp", codec_name),
        "throughput_mb_s",
        "MB/s",
        chk_samples_mb_s,
    ));

    let chunked_decompression = DecompressionBenchmarkMetrics {
        container_format: "ZIP".to_string(),
        codec: codec_name.to_string(),
        mode: "chunked".to_string(),
        input_size_bytes: compressed_size,
        uncompressed_size_bytes: input_bytes as u64,
        latency_nanos: chk_dur.as_nanos() as u64,
        throughput_mb_sec: chk_mb_s,
        memory_baseline_rss: chk_mem.baseline_rss_bytes,
        memory_peak_rss: chk_mem.peak_rss_bytes,
        memory_delta_bytes: chk_mem.resident_delta_bytes,
    };

    MultiFormatBenchmarkRecord {
        container: "ZIP".to_string(),
        codec: codec_name.to_string(),
        input_size_bytes: input_bytes as u64,
        compressed_size_bytes: compressed_size,
        compression_ratio,
        toc_metrics: Some(toc_metrics),
        seq_decompression,
        chunked_decompression,
    }
}

/// Benchmarks 7-Zip solid LZMA2 archive compression, TOC latency, and decompression.
pub fn bench_sevenz_suite(
    corpus: &SyntheticCorpus,
    distributions: &mut Vec<BenchmarkMetricSamples>,
) -> MultiFormatBenchmarkRecord {
    let input_bytes = corpus.total_uncompressed_bytes;
    let items = corpus.to_zip_input_items();
    let sevenz_bytes = create_7z_solid_archive_bytes(&items, 5, 2).expect("create 7z");
    let compressed_size = sevenz_bytes.len() as u64;
    let compression_ratio = compressed_size as f64 / input_bytes as f64;

    // TOC Extraction
    let (toc_dur, toc_mem, toc_samples) = benchmark_repeats(MEASURE_RUNS, || {
        let arch = SevenZArchive::open_slice(&sevenz_bytes).unwrap();
        black_box(arch.len());
    });
    distributions.push(BenchmarkMetricSamples::from_samples(
        "7Z-LZMA2-TOC-Latency",
        "latency_micros",
        "µs",
        toc_samples,
    ));

    let toc_metrics = TocBenchmarkMetrics::new(
        "7Z",
        corpus.files.len(),
        compressed_size,
        input_bytes as u64,
        toc_dur,
        toc_mem,
    );

    // Sequential Decompression
    let (seq_dur, seq_mem, seq_latencies) = benchmark_repeats(MEASURE_RUNS, || {
        let arch = SevenZArchive::open_slice(&sevenz_bytes).unwrap();
        for i in 0..arch.len() {
            let extracted = arch.extract_entry_bytes_stream(i, None).unwrap();
            black_box(extracted);
        }
    });

    let seq_mb_s = (input_bytes as f64 / 1_000_000.0) / seq_dur.as_secs_f64().max(1e-9);
    distributions.push(BenchmarkMetricSamples::from_samples(
        "7Z-LZMA2-Seq-Decomp",
        "throughput_mb_s",
        "MB/s",
        seq_latencies
            .iter()
            .map(|l| (input_bytes as f64 / 1_000_000.0) / (l / 1_000_000.0))
            .collect(),
    ));

    let seq_decompression = DecompressionBenchmarkMetrics {
        container_format: "7Z".to_string(),
        codec: "Solid-LZMA2".to_string(),
        mode: "sequential".to_string(),
        input_size_bytes: compressed_size,
        uncompressed_size_bytes: input_bytes as u64,
        latency_nanos: seq_dur.as_nanos() as u64,
        throughput_mb_sec: seq_mb_s,
        memory_baseline_rss: seq_mem.baseline_rss_bytes,
        memory_peak_rss: seq_mem.peak_rss_bytes,
        memory_delta_bytes: seq_mem.resident_delta_bytes,
    };

    // Chunked Decompression
    let (chk_dur, chk_mem, chk_latencies) = benchmark_repeats(MEASURE_RUNS, || {
        let arch = SevenZArchive::open_slice(&sevenz_bytes).unwrap();
        let mut chunk = [0u8; CHUNK_SIZE_64KB];
        for i in 0..arch.len() {
            let data = arch.extract_entry_bytes_stream(i, None).unwrap();
            for slice in data.chunks(CHUNK_SIZE_64KB) {
                chunk[..slice.len()].copy_from_slice(slice);
                black_box(&chunk[..slice.len()]);
            }
        }
    });

    let chk_mb_s = (input_bytes as f64 / 1_000_000.0) / chk_dur.as_secs_f64().max(1e-9);
    distributions.push(BenchmarkMetricSamples::from_samples(
        "7Z-LZMA2-Chunked-Decomp",
        "throughput_mb_s",
        "MB/s",
        chk_latencies
            .iter()
            .map(|l| (input_bytes as f64 / 1_000_000.0) / (l / 1_000_000.0))
            .collect(),
    ));

    let chunked_decompression = DecompressionBenchmarkMetrics {
        container_format: "7Z".to_string(),
        codec: "Solid-LZMA2".to_string(),
        mode: "chunked".to_string(),
        input_size_bytes: compressed_size,
        uncompressed_size_bytes: input_bytes as u64,
        latency_nanos: chk_dur.as_nanos() as u64,
        throughput_mb_sec: chk_mb_s,
        memory_baseline_rss: chk_mem.baseline_rss_bytes,
        memory_peak_rss: chk_mem.peak_rss_bytes,
        memory_delta_bytes: chk_mem.resident_delta_bytes,
    };

    MultiFormatBenchmarkRecord {
        container: "7Z".to_string(),
        codec: "Solid-LZMA2".to_string(),
        input_size_bytes: input_bytes as u64,
        compressed_size_bytes: compressed_size,
        compression_ratio,
        toc_metrics: Some(toc_metrics),
        seq_decompression,
        chunked_decompression,
    }
}

/// Benchmarks POSIX TAR archive creation, TOC latency, and decompression.
pub fn bench_tar_suite(
    corpus: &SyntheticCorpus,
    distributions: &mut Vec<BenchmarkMetricSamples>,
) -> (Vec<u8>, MultiFormatBenchmarkRecord) {
    let input_bytes = corpus.total_uncompressed_bytes;
    let mut writer = TarWriter::new(Vec::with_capacity(input_bytes + 4096));
    for (path, data) in &corpus.files {
        writer
            .append_file(path, data, 0o644, 1770000000)
            .expect("append tar file");
    }
    writer.finish().expect("finish tar");
    let tar_bytes = writer.into_inner();
    let compressed_size = tar_bytes.len() as u64;

    // TOC Extraction
    let (toc_dur, toc_mem, toc_samples) = benchmark_repeats(MEASURE_RUNS, || {
        let tar = TarArchive::open_slice(&tar_bytes).unwrap();
        black_box(tar.len());
    });
    distributions.push(BenchmarkMetricSamples::from_samples(
        "TAR-PAX-TOC-Latency",
        "latency_micros",
        "µs",
        toc_samples,
    ));

    let toc_metrics = TocBenchmarkMetrics::new(
        "TAR",
        corpus.files.len(),
        compressed_size,
        input_bytes as u64,
        toc_dur,
        toc_mem,
    );

    // Sequential Decompression
    let (seq_dur, seq_mem, seq_latencies) = benchmark_repeats(MEASURE_RUNS, || {
        let tar = TarArchive::open_slice(&tar_bytes).unwrap();
        for i in 0..tar.len() {
            let extracted = tar.extract_entry_bytes(i).unwrap();
            black_box(extracted);
        }
    });

    let seq_mb_s = (input_bytes as f64 / 1_000_000.0) / seq_dur.as_secs_f64().max(1e-9);
    distributions.push(BenchmarkMetricSamples::from_samples(
        "TAR-PAX-Seq-Decomp",
        "throughput_mb_s",
        "MB/s",
        seq_latencies
            .iter()
            .map(|l| (input_bytes as f64 / 1_000_000.0) / (l / 1_000_000.0))
            .collect(),
    ));

    let seq_decompression = DecompressionBenchmarkMetrics {
        container_format: "TAR".to_string(),
        codec: "Store".to_string(),
        mode: "sequential".to_string(),
        input_size_bytes: compressed_size,
        uncompressed_size_bytes: input_bytes as u64,
        latency_nanos: seq_dur.as_nanos() as u64,
        throughput_mb_sec: seq_mb_s,
        memory_baseline_rss: seq_mem.baseline_rss_bytes,
        memory_peak_rss: seq_mem.peak_rss_bytes,
        memory_delta_bytes: seq_mem.resident_delta_bytes,
    };

    // Chunked Decompression
    let (chk_dur, chk_mem, chk_latencies) = benchmark_repeats(MEASURE_RUNS, || {
        let tar = TarArchive::open_slice(&tar_bytes).unwrap();
        let mut chunk = [0u8; CHUNK_SIZE_64KB];
        for i in 0..tar.len() {
            let extracted = tar.extract_entry_bytes(i).unwrap();
            for slice in extracted.chunks(CHUNK_SIZE_64KB) {
                chunk[..slice.len()].copy_from_slice(slice);
                black_box(&chunk[..slice.len()]);
            }
        }
    });

    let chk_mb_s = (input_bytes as f64 / 1_000_000.0) / chk_dur.as_secs_f64().max(1e-9);
    distributions.push(BenchmarkMetricSamples::from_samples(
        "TAR-PAX-Chunked-Decomp",
        "throughput_mb_s",
        "MB/s",
        chk_latencies
            .iter()
            .map(|l| (input_bytes as f64 / 1_000_000.0) / (l / 1_000_000.0))
            .collect(),
    ));

    let chunked_decompression = DecompressionBenchmarkMetrics {
        container_format: "TAR".to_string(),
        codec: "Store".to_string(),
        mode: "chunked".to_string(),
        input_size_bytes: compressed_size,
        uncompressed_size_bytes: input_bytes as u64,
        latency_nanos: chk_dur.as_nanos() as u64,
        throughput_mb_sec: chk_mb_s,
        memory_baseline_rss: chk_mem.baseline_rss_bytes,
        memory_peak_rss: chk_mem.peak_rss_bytes,
        memory_delta_bytes: chk_mem.resident_delta_bytes,
    };

    let record = MultiFormatBenchmarkRecord {
        container: "TAR".to_string(),
        codec: "Store".to_string(),
        input_size_bytes: input_bytes as u64,
        compressed_size_bytes: compressed_size,
        compression_ratio: compressed_size as f64 / input_bytes as f64,
        toc_metrics: Some(toc_metrics),
        seq_decompression,
        chunked_decompression,
    };

    (tar_bytes, record)
}
