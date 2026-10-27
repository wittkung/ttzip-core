// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Comprehensive Integration & Quality Tests for:
//! TTZipMT Multi-Core Ordered Streaming Pipeline (`ttzip_mt_drainer.rs`).

use std::io::Cursor;
use std::sync::Arc;
use ttzip_engine::archive::ttzip_mt_drainer::*;

// ============================================================================
// TTZipMT Multi-Core Streaming Pipeline Tests
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
