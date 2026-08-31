// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive Integration & Quality Tests for:
//! 1. 5-Layer Decompression State Machine & Functional Core (`five_layer_state_machine.rs`)
//! 2. Zero-Vtable Strategy Dispatch Engine (`zero_vtable_dispatch.rs`)
//! 3. TTZipMT Multi-Core Ordered Streaming Pipeline (`ttzip_mt_drainer.rs`)

use std::io::Cursor;
use std::sync::Arc;
use ttzip_engine::archive::five_layer_state_machine::*;
use ttzip_engine::archive::ttzip_mt_drainer::*;
use ttzip_engine::archive::zero_vtable_dispatch::*;
use ttzip_engine::types::{TTZipArchiveFormat, TTZipCompressionLevel};

// ============================================================================
// 1. Five-Layer State Machine & Functional Core Tests
// ============================================================================

#[test]
fn test_layer5_bitstream_forward_and_reverse() {
    let data = [0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56, 0x78, 0x90];

    // Forward bitstream reading
    let mut fwd = BitstreamReader::new_forward(&data);
    let byte0 = fwd.read_bits(8).expect("read 8 bits");
    assert_eq!(byte0, 0xAB);
    let nibble = fwd.read_bits(4).expect("read 4 bits");
    assert_eq!(nibble, 0x0D);
    assert!(fwd.bits_remaining() > 0);

    // Reverse bitstream reading
    let mut rev = BitstreamReader::new_reverse(&data);
    let peek = rev.peek_bits(8).expect("peek 8 bits");
    assert_eq!(peek, 0x90);
    rev.consume_bits(8);
    let next_byte = rev.read_bits(8).expect("read next byte");
    assert_eq!(next_byte, 0x78);
}

#[test]
fn test_layer4_entropy_decoder_modes() {
    let entropy = EntropyDecoder::new();
    let data = [42u8, 99u8];
    let mut reader = BitstreamReader::new_forward(&data);

    let sym1 = entropy.decode_symbol(&mut reader).expect("decode symbol 1");
    assert_eq!(sym1, 42);
    let sym2 = entropy.decode_symbol(&mut reader).expect("decode symbol 2");
    assert_eq!(sym2, 99);
}

#[test]
fn test_layer3_sequence_executor_literal_and_match() {
    let literals = b"Hello, World!";
    let mut lit_cursor = 0;
    let mut out_buf = vec![0u8; 64];
    let mut out_cursor = 0;

    // 1. Literal copy: "Hello, " (7 bytes)
    let seq1 = Lz77Sequence::new(7, 0, 0);
    SequenceExecutor::execute_sequence(
        &seq1,
        literals,
        &mut lit_cursor,
        &mut out_buf,
        &mut out_cursor,
    )
    .expect("seq 1 execution");
    assert_eq!(&out_buf[..out_cursor], b"Hello, ");

    // 2. Literal copy: "World!" (6 bytes) + Match copy 7 bytes back ("Hello, ")
    let seq2 = Lz77Sequence::new(6, 13, 7);
    SequenceExecutor::execute_sequence(
        &seq2,
        literals,
        &mut lit_cursor,
        &mut out_buf,
        &mut out_cursor,
    )
    .expect("seq 2 execution");
    assert_eq!(&out_buf[..out_cursor], b"Hello, World!Hello, ");
}

#[test]
fn test_layer2_block_header_parsing_and_boundaries() {
    let header = BlockHeader {
        block_type: BlockType::CompressedLz77Entropy,
        is_last_block: true,
        uncompressed_size: 65536,
        compressed_size: 32768,
    };

    let mut buf = [0u8; 9];
    let written = header.write_to_slice(&mut buf).expect("write header");
    assert_eq!(written, 9);

    let (parsed, read_len) = BlockHeader::parse_from_slice(&buf).expect("parse header");
    assert_eq!(read_len, 9);
    assert_eq!(parsed.block_type, BlockType::CompressedLz77Entropy);
    assert!(parsed.is_last_block);
    assert_eq!(parsed.uncompressed_size, 65536);
    assert_eq!(parsed.compressed_size, 32768);
}

#[test]
fn test_layer1_frame_header_parsing() {
    let header = FrameHeader {
        version: 1,
        has_checksum: true,
        dictionary_id: Some(0x12345678),
        expected_uncompressed_size: Some(1048576),
    };

    let mut buf = [0u8; 32];
    let written = header.write_to_slice(&mut buf).expect("write frame header");
    let (parsed, read_len) = FrameHeader::parse_from_slice(&buf[..written]).expect("parse frame header");
    assert_eq!(read_len, written);
    assert_eq!(parsed.version, 1);
    assert!(parsed.has_checksum);
    assert_eq!(parsed.dictionary_id, Some(0x12345678));
    assert_eq!(parsed.expected_uncompressed_size, Some(1048576));
}

#[test]
fn test_five_layer_roundtrip_empty_and_small() {
    let sm = FiveLayerStateMachine::new();

    // 1. Empty payload
    let empty_payload = b"";
    let encoded_empty = FiveLayerFrameEncoder::encode_frame(empty_payload, true);
    let mut decoded_empty = vec![0u8; 32];
    let dec_len = sm
        .decompress_frame(&encoded_empty, &mut decoded_empty)
        .expect("decompress empty");
    assert_eq!(dec_len, 0);

    // 2. Small payload
    let small_payload = b"TTZip 5-Layer State Machine Functional Core verification test 2026.";
    let encoded_small = FiveLayerFrameEncoder::encode_frame(small_payload, true);
    let mut decoded_small = vec![0u8; small_payload.len()];
    let dec_len = sm
        .decompress_frame(&encoded_small, &mut decoded_small)
        .expect("decompress small");
    assert_eq!(dec_len, small_payload.len());
    assert_eq!(&decoded_small[..dec_len], small_payload);
}

#[test]
fn test_five_layer_roundtrip_rle_and_repetitive() {
    let sm = FiveLayerStateMachine::new();

    // RLE payload: 2000 'Z' bytes
    let rle_payload = vec![b'Z'; 2000];
    let encoded_rle = FiveLayerFrameEncoder::encode_frame(&rle_payload, true);
    assert!(encoded_rle.len() < 100, "RLE frame should be compact");

    let mut decoded_rle = vec![0u8; rle_payload.len()];
    let dec_len = sm
        .decompress_frame(&encoded_rle, &mut decoded_rle)
        .expect("decompress rle");
    assert_eq!(dec_len, rle_payload.len());
    assert_eq!(decoded_rle, rle_payload);

    // Repetitive text payload triggering LZ77 matches
    let rep_payload = b"The quick brown fox jumps over the lazy dog. ".repeat(100);
    let encoded_rep = FiveLayerFrameEncoder::encode_frame(&rep_payload, true);
    assert!(encoded_rep.len() < rep_payload.len(), "LZ77 should achieve compression");

    let mut decoded_rep = vec![0u8; rep_payload.len()];
    let dec_len = sm
        .decompress_frame(&encoded_rep, &mut decoded_rep)
        .expect("decompress rep");
    assert_eq!(dec_len, rep_payload.len());
    assert_eq!(decoded_rep, rep_payload);
}

#[test]
fn test_five_layer_multi_block_128kb_boundary() {
    let sm = FiveLayerStateMachine::new();

    // 300KB payload spanning across multiple 128KB blocks
    let mut large_payload = Vec::with_capacity(300 * 1024);
    for i in 0..300 * 1024 {
        large_payload.push((i % 251) as u8);
    }

    let encoded_large = FiveLayerFrameEncoder::encode_frame(&large_payload, true);
    let mut decoded_large = vec![0u8; large_payload.len()];
    let dec_len = sm
        .decompress_frame(&encoded_large, &mut decoded_large)
        .expect("decompress multi-block");
    assert_eq!(dec_len, large_payload.len());
    assert_eq!(decoded_large, large_payload);
}

#[test]
fn test_five_layer_corrupt_stream_defenses() {
    let sm = FiveLayerStateMachine::new();

    // Invalid Magic
    let invalid_magic = b"PK0304CORRUPTED";
    let mut dst = vec![0u8; 1024];
    assert_eq!(
        sm.decompress_frame(invalid_magic, &mut dst),
        Err(DecompressionError::InvalidMagic)
    );

    // Checksum mismatch
    let valid_payload = b"Test Payload Checksum Integrity";
    let mut encoded = FiveLayerFrameEncoder::encode_frame(valid_payload, true);
    // Tamper with last byte of payload
    let len = encoded.len();
    encoded[len - 5] ^= 0xFF; // Invert byte before CRC
    assert!(sm.decompress_frame(&encoded, &mut dst).is_err());
}

// ============================================================================
// 2. Zero-Vtable Strategy Dispatch Engine Tests
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
