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

fn measure_toc_stage<F: FnMut()>(
    container: &str,
    sample_id: String,
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

fn measure_decomp_stage<F: FnMut()>(
    container: &str,
    codec: &str,
    mode: &str,
    sample_id: String,
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

fn compress_zip_items(corpus: &SyntheticCorpus, actual_method: u16) -> Vec<ZipCompressedItem> {
    corpus
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
        .collect()
}

fn decompress_zip_item_seq(item: &ZipCompressedItem) {
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

fn decompress_zip_item_chunked(item: &ZipCompressedItem, chunk_buf: &mut [u8; CHUNK_SIZE_64KB]) {
    match item.actual_method {
        8 => {
            let mut dec = flate2::read::DeflateDecoder::new(&item.payload[..]);
            while let Ok(n) = dec.read(chunk_buf) {
                if n == 0 {
                    break;
                }
                black_box(&chunk_buf[..n]);
            }
        }
        93 => {
            let mut reader = ZstdStreamReader::new(&item.payload[..]).unwrap();
            while let Ok(n) = reader.read(chunk_buf) {
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

/// Benchmarks ZIP container compression, TOC latency, and decompression for a specific codec.
pub fn bench_zip_variant(
    corpus: &SyntheticCorpus,
    codec_name: &'static str,
    actual_method: u16,
    distributions: &mut Vec<BenchmarkMetricSamples>,
) -> MultiFormatBenchmarkRecord {
    let input_bytes = corpus.total_uncompressed_bytes;
    let compressed_items = compress_zip_items(corpus, actual_method);
    let zip_bytes = assemble_zip_archive(&compressed_items).expect("assemble zip");
    let compressed_size = zip_bytes.len() as u64;

    let (toc_dur, toc_mem, toc_samples) = benchmark_repeats(MEASURE_RUNS, || {
        let zip = ZipArchive::open_slice(&zip_bytes).unwrap();
        black_box(zip.entries().len());
    });
    distributions.push(BenchmarkMetricSamples::from_samples(
        format!("ZIP-{codec_name}-TOC-Latency"),
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

    let seq_decompression = measure_decomp_stage(
        "ZIP",
        codec_name,
        "sequential",
        format!("ZIP-{codec_name}-Seq-Decomp"),
        compressed_size,
        input_bytes as u64,
        distributions,
        || {
            let zip = ZipArchive::open_slice(&zip_bytes).unwrap();
            black_box(zip.entries().len());
            for item in &compressed_items {
                decompress_zip_item_seq(item);
            }
        },
    );

    let chunked_decompression = measure_decomp_stage(
        "ZIP",
        codec_name,
        "chunked",
        format!("ZIP-{codec_name}-Chunked-Decomp"),
        compressed_size,
        input_bytes as u64,
        distributions,
        || {
            let mut chunk_buf = [0u8; CHUNK_SIZE_64KB];
            for item in &compressed_items {
                decompress_zip_item_chunked(item, &mut chunk_buf);
            }
        },
    );

    MultiFormatBenchmarkRecord {
        container: "ZIP".to_string(),
        codec: codec_name.to_string(),
        input_size_bytes: input_bytes as u64,
        compressed_size_bytes: compressed_size,
        compression_ratio: compressed_size as f64 / input_bytes as f64,
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

    let toc_metrics = measure_toc_stage(
        "7Z",
        "7Z-LZMA2-TOC-Latency".to_string(),
        corpus.files.len(),
        compressed_size,
        input_bytes as u64,
        distributions,
        || {
            let arch = SevenZArchive::open_slice(&sevenz_bytes).unwrap();
            black_box(arch.len());
        },
    );

    let seq_decompression = measure_decomp_stage(
        "7Z",
        "Solid-LZMA2",
        "sequential",
        "7Z-LZMA2-Seq-Decomp".to_string(),
        compressed_size,
        input_bytes as u64,
        distributions,
        || {
            let arch = SevenZArchive::open_slice(&sevenz_bytes).unwrap();
            for i in 0..arch.len() {
                black_box(arch.extract_entry_bytes_stream(i, None).unwrap());
            }
        },
    );

    let chunked_decompression = measure_decomp_stage(
        "7Z",
        "Solid-LZMA2",
        "chunked",
        "7Z-LZMA2-Chunked-Decomp".to_string(),
        compressed_size,
        input_bytes as u64,
        distributions,
        || {
            let arch = SevenZArchive::open_slice(&sevenz_bytes).unwrap();
            let mut chunk = [0u8; CHUNK_SIZE_64KB];
            for i in 0..arch.len() {
                let data = arch.extract_entry_bytes_stream(i, None).unwrap();
                for slice in data.chunks(CHUNK_SIZE_64KB) {
                    chunk[..slice.len()].copy_from_slice(slice);
                    black_box(&chunk[..slice.len()]);
                }
            }
        },
    );

    MultiFormatBenchmarkRecord {
        container: "7Z".to_string(),
        codec: "Solid-LZMA2".to_string(),
        input_size_bytes: input_bytes as u64,
        compressed_size_bytes: compressed_size,
        compression_ratio: compressed_size as f64 / input_bytes as f64,
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

    let toc_metrics = measure_toc_stage(
        "TAR",
        "TAR-PAX-TOC-Latency".to_string(),
        corpus.files.len(),
        compressed_size,
        input_bytes as u64,
        distributions,
        || {
            let tar = TarArchive::open_slice(&tar_bytes).unwrap();
            black_box(tar.len());
        },
    );

    let seq_decompression = measure_decomp_stage(
        "TAR",
        "Store",
        "sequential",
        "TAR-PAX-Seq-Decomp".to_string(),
        compressed_size,
        input_bytes as u64,
        distributions,
        || {
            let tar = TarArchive::open_slice(&tar_bytes).unwrap();
            for i in 0..tar.len() {
                black_box(tar.extract_entry_bytes(i).unwrap());
            }
        },
    );

    let chunked_decompression = measure_decomp_stage(
        "TAR",
        "Store",
        "chunked",
        "TAR-PAX-Chunked-Decomp".to_string(),
        compressed_size,
        input_bytes as u64,
        distributions,
        || {
            let tar = TarArchive::open_slice(&tar_bytes).unwrap();
            let mut chunk = [0u8; CHUNK_SIZE_64KB];
            for i in 0..tar.len() {
                let extracted = tar.extract_entry_bytes(i).unwrap();
                for slice in extracted.chunks(CHUNK_SIZE_64KB) {
                    chunk[..slice.len()].copy_from_slice(slice);
                    black_box(&chunk[..slice.len()]);
                }
            }
        },
    );

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
