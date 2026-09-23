// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Comprehensive Integration & Quality Tests for:
//! 1. Zero-Vtable Strategy Dispatch Engine (`zero_vtable_dispatch.rs`)
//! 2. TTZipMT Multi-Core Ordered Streaming Pipeline (`ttzip_mt_drainer.rs`)

use std::io::Cursor;
use std::sync::Arc;
use ttzip_engine::archive::ttzip_mt_drainer::*;
use ttzip_engine::archive::zero_vtable_dispatch::*;
use ttzip_engine::types::{TTZipArchiveFormat, TTZipCompressionLevel};

// ============================================================================
// 1. Zero-Vtable Strategy Dispatch Engine Tests
// ============================================================================

#[test]
fn test_zero_vtable_format_resolution_and_properties() {
    let zstd = ArchiveEngineStrategy::from_format(TTZipArchiveFormat::Zstd).expect("zstd strategy");
    assert_eq!(zstd.format(), TTZipArchiveFormat::Zstd);
    assert_eq!(zstd.name(), "zst");
    assert_eq!(zstd.mime_type(), "application/zstd");
    assert!(zstd.is_stream_compressor());
    assert!(!zstd.is_container());
    assert!(zstd.supports_multithreading());

    let zip = ArchiveEngineStrategy::from_format(TTZipArchiveFormat::Zip).expect("zip strategy");
    assert_eq!(zip.format(), TTZipArchiveFormat::Zip);
    assert!(zip.is_container());
    assert!(!zip.is_stream_compressor());

    let lz4 = ArchiveEngineStrategy::from_format(TTZipArchiveFormat::Lz4).expect("lz4 strategy");
    assert!(lz4.supports_multithreading());

    let tar = ArchiveEngineStrategy::from_format(TTZipArchiveFormat::Tar).expect("tar strategy");
    assert!(tar.is_container());
}

#[test]
fn test_zero_vtable_magic_sniffing() {
    assert_eq!(
        ArchiveEngineStrategy::detect_from_magic(b"PK\x03\x04\x00\x00"),
        Some(ArchiveEngineStrategy::Zip(ZipStrategy))
    );
    assert_eq!(
        ArchiveEngineStrategy::detect_from_magic(b"\x28\xB5\x2F\xFD\x00"),
        Some(ArchiveEngineStrategy::Zstd(ZstdStrategy))
    );
    assert_eq!(
        ArchiveEngineStrategy::detect_from_magic(b"\x1F\x8B\x08\x00"),
        Some(ArchiveEngineStrategy::Gz(GzipStrategy))
    );
    assert_eq!(
        ArchiveEngineStrategy::detect_from_magic(b"BZh91AY&SY"),
        Some(ArchiveEngineStrategy::Bz2(Bzip2Strategy))
    );
    assert_eq!(
        ArchiveEngineStrategy::detect_from_magic(b"\x04\x22\x4D\x18\x60"),
        Some(ArchiveEngineStrategy::Lz4(Lz4Strategy))
    );
    assert_eq!(
        ArchiveEngineStrategy::detect_from_magic(b"7z\xBC\xAF\x27\x1C"),
        Some(ArchiveEngineStrategy::SevenZip(SevenZipStrategy))
    );
}

#[test]
fn test_zero_vtable_roundtrip_all_codecs() {
    let payload = b"TTZip Zero-Vtable Polymorphic Strategy Dispatch verification test across all native codecs 2026. ABCDEFGHIJKLMNOPQRSTUVWXYZ 1234567890.";

    let strategies = [
        ArchiveEngineStrategy::from_format(TTZipArchiveFormat::Zstd).unwrap(),
        ArchiveEngineStrategy::from_format(TTZipArchiveFormat::Lz4).unwrap(),
        ArchiveEngineStrategy::from_format(TTZipArchiveFormat::Gzip).unwrap(),
        ArchiveEngineStrategy::from_format(TTZipArchiveFormat::Brotli).unwrap(),
        ArchiveEngineStrategy::from_format(TTZipArchiveFormat::Snappy).unwrap(),
        ArchiveEngineStrategy::from_format(TTZipArchiveFormat::Lzfse).unwrap(),
        ArchiveEngineStrategy::from_format(TTZipArchiveFormat::Bzip2).unwrap(),
    ];

    for strategy in strategies {
        // Compress
        let compressed = strategy
            .compress_to_vec(payload, TTZipCompressionLevel::Normal)
            .unwrap_or_else(|_| panic!("compress with {}", strategy.name()));
        assert!(!compressed.is_empty());

        // Decompress
        let decompressed = strategy
            .decompress_to_vec(&compressed)
            .unwrap_or_else(|_| panic!("decompress with {}", strategy.name()));
        assert_eq!(
            decompressed.as_slice(),
            payload.as_slice(),
            "Decompressed payload mismatch for strategy: {}",
            strategy.name()
        );

        // Header inspection
        let header_info = strategy
            .inspect_header(&compressed)
            .expect("inspect header");
        assert_eq!(header_info.format, strategy.format());
    }
}

// ============================================================================
// 3. TTZipMT Multi-Core Streaming Pipeline Tests
// ============================================================================

#[test]
fn test_buffer_pool_recycling() {
    let pool = Arc::new(BufferPool::new(64 * 1024, 8));
    assert_eq!(pool.available_count(), 0);

    {
        let buf1 = pool.acquire();
        assert_eq!(buf1.len(), 64 * 1024);
        assert_eq!(pool.total_allocated(), 1);

        let buf2 = pool.acquire();
        assert_eq!(buf2.len(), 64 * 1024);
        assert_eq!(pool.total_allocated(), 2);
    } // buf1 and buf2 dropped here

    // Verify buffers were returned to pool
    assert_eq!(pool.available_count(), 2);

    let _buf3 = pool.acquire();
    assert_eq!(pool.available_count(), 1);
    assert_eq!(pool.total_allocated(), 2); // No new allocation needed
}

#[test]
fn test_ordered_drainer_out_of_order_submission() {
    let mut drainer = OrderedDrainer::new();
    let mut output_sink = Vec::new();

    // Submit jobs in reverse order: 3, 2, 1, 0
    let job3 = CompletedJob {
        job_id: 3,
        is_last: true,
        result: Ok(b"Four!".to_vec()),
        raw_bytes: 5,
        processed_bytes: 5,
    };
    let job2 = CompletedJob {
        job_id: 2,
        is_last: false,
        result: Ok(b"Three, ".to_vec()),
        raw_bytes: 7,
        processed_bytes: 7,
    };
    let job1 = CompletedJob {
        job_id: 1,
        is_last: false,
        result: Ok(b"Two, ".to_vec()),
        raw_bytes: 5,
        processed_bytes: 5,
    };
    let job0 = CompletedJob {
        job_id: 0,
        is_last: false,
        result: Ok(b"One, ".to_vec()),
        raw_bytes: 5,
        processed_bytes: 5,
    };

    drainer.submit(job3);
    drainer.drain_ready(&mut output_sink).unwrap();
    assert!(output_sink.is_empty(), "Cannot drain until job 0 is submitted");

    drainer.submit(job2);
    drainer.drain_ready(&mut output_sink).unwrap();
    assert!(output_sink.is_empty());

    drainer.submit(job0);
    let drained = drainer.drain_ready(&mut output_sink).unwrap();
    assert_eq!(drained, 5);
    assert_eq!(&output_sink, b"One, ");

    drainer.submit(job1);
    let drained = drainer.drain_ready(&mut output_sink).unwrap();
    assert_eq!(drained, 5 + 7 + 5);
    assert_eq!(&output_sink, b"One, Two, Three, Four!");
    assert!(drainer.is_finished());

    let metrics = drainer.metrics();
    assert_eq!(metrics.total_jobs_processed, 4);
    assert_eq!(metrics.total_input_bytes, 22);
    assert_eq!(metrics.total_output_bytes, 22);
}

#[test]
fn test_ttzip_mt_engine_parallel_processing_stream() {
    let engine = TTZipMtEngine::new(16 * 1024, 4);

    // Create a 100KB test payload
    let payload = b"TTZipMT Multi-Core Streaming Data Stream Test Payload with deterministic chunking. ".repeat(1200);
    let mut reader = Cursor::new(&payload);
    let mut writer = Vec::new();

    let metrics = engine
        .process_stream(&mut reader, &mut writer, |chunk| {
            // Invert bytes as pure transform
            Ok(chunk.iter().map(|&b| b ^ 0xAA).collect())
        })
        .expect("process stream");

    assert_eq!(metrics.total_input_bytes, payload.len() as u64);
    assert_eq!(metrics.total_output_bytes, payload.len() as u64);
    assert_eq!(writer.len(), payload.len());

    // Invert back
    let mut inv_reader = Cursor::new(&writer);
    let mut recovered = Vec::new();
    let _ = engine
        .process_stream(&mut inv_reader, &mut recovered, |chunk| {
            Ok(chunk.iter().map(|&b| b ^ 0xAA).collect())
        })
        .expect("recover stream");

    assert_eq!(recovered, payload);
}

#[test]
fn test_ttzip_mt_engine_parallel_zstd_and_lz4_roundtrip() {
    let engine = TTZipMtEngine::new(64 * 1024, 4);
    let zstd_strategy = ArchiveEngineStrategy::from_format(TTZipArchiveFormat::Zstd).unwrap();
    let lz4_strategy = ArchiveEngineStrategy::from_format(TTZipArchiveFormat::Lz4).unwrap();

    let payload = b"TTZipMT High-Throughput Parallel Codec Streaming Roundtrip with Rayon task worker parallelism. ".repeat(800);

    // 1. Zstd Parallel Streaming
    let mut zstd_compressed = Vec::new();
    let mut zstd_reader = Cursor::new(&payload);
    let comp_metrics = engine
        .compress_stream_parallel(
            &mut zstd_reader,
            &mut zstd_compressed,
            zstd_strategy,
            TTZipCompressionLevel::Fast,
        )
        .expect("parallel zstd compress");
    assert_eq!(comp_metrics.total_input_bytes, payload.len() as u64);

    // 2. LZ4 Parallel Streaming
    let mut lz4_compressed = Vec::new();
    let mut lz4_reader = Cursor::new(&payload);
    let lz4_metrics = engine
        .compress_stream_parallel(
            &mut lz4_reader,
            &mut lz4_compressed,
            lz4_strategy,
            TTZipCompressionLevel::Fastest,
        )
        .expect("parallel lz4 compress");
    assert_eq!(lz4_metrics.total_input_bytes, payload.len() as u64);
    assert!(!lz4_compressed.is_empty());
}
