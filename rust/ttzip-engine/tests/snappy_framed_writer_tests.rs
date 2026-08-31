// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and conformance test suite for pure Safe-Rust `SnappyFramedWriter`.

use std::io::{Cursor, Read, Write};
use std::sync::{Arc, Mutex};
use ttzip_engine::codecs::snappy::*;

/// Simple deterministic pseudo-random number generator (SplitMix64) for high-entropy corpus generation.
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut chunks = dest.chunks_exact_mut(8);
        for chunk in &mut chunks {
            let val = self.next_u64();
            chunk.copy_from_slice(&val.to_le_bytes());
        }
        let rem = chunks.into_remainder();
        if !rem.is_empty() {
            let val = self.next_u64();
            let bytes = val.to_le_bytes();
            rem.copy_from_slice(&bytes[..rem.len()]);
        }
    }
}

/// Helper sink that allows inspecting written bytes even if writer is dropped.
#[derive(Clone)]
struct SharedVecWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl SharedVecWriter {
    fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn to_vec(&self) -> Vec<u8> {
        self.buffer.lock().unwrap().clone()
    }
}

impl Write for SharedVecWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_framed_writer_empty_stream() {
    let mut dest = Vec::new();
    {
        let encoder = SnappyFramedWriter::new(&mut dest);
        let finished_writer = encoder.finish().expect("finish empty stream");
        let _ = finished_writer;
    }

    assert_eq!(dest.len(), 10);
    assert_eq!(&dest[..], &SNAPPY_STREAM_IDENTIFIER[..]);
    assert!(is_framed_snappy(&dest));
    assert!(snappy_frame_validate(&dest));

    // Decode with internal SnappyFramedReader
    let mut reader = SnappyFramedReader::new(Cursor::new(&dest));
    let mut decoded = Vec::new();
    reader.read_to_end(&mut decoded).expect("read empty stream");
    assert!(decoded.is_empty());

    // Interoperability with standard snap::read::FrameDecoder
    let mut snap_reader = snap::read::FrameDecoder::new(Cursor::new(&dest));
    let mut snap_decoded = Vec::new();
    snap_reader
        .read_to_end(&mut snap_decoded)
        .expect("snap read empty stream");
    assert!(snap_decoded.is_empty());
}

#[test]
fn test_framed_writer_small_data_matrix() {
    for len in 1..=100 {
        // 1. Monotonic sequential bytes
        let mut data = Vec::with_capacity(len);
        for i in 0..len {
            data.push(b'A' + (i % 26) as u8);
        }

        let mut dest = Vec::new();
        {
            let mut writer = SnappyFramedWriter::new(&mut dest);
            writer.write_all(&data).expect("write small monotonic");
            writer.finish().expect("finish small monotonic");
        }

        assert!(is_framed_snappy(&dest));
        assert!(snappy_frame_validate(&dest));

        let mut reader = SnappyFramedReader::new(Cursor::new(&dest));
        let mut decoded = Vec::new();
        reader.read_to_end(&mut decoded).expect("decode monotonic");
        assert_eq!(decoded, data);

        // 2. Uniform repeated byte sequence
        let rep_data = vec![0x3Cu8; len];
        let mut rep_dest = Vec::new();
        {
            let mut rep_writer = SnappyFramedWriter::new(&mut rep_dest);
            rep_writer.write_all(&rep_data).expect("write repeated");
            rep_writer.finish().expect("finish repeated");
        }

        assert!(is_framed_snappy(&rep_dest));
        assert!(snappy_frame_validate(&rep_dest));

        let mut rep_reader = SnappyFramedReader::new(Cursor::new(&rep_dest));
        let mut rep_decoded = Vec::new();
        rep_reader
            .read_to_end(&mut rep_decoded)
            .expect("decode repeated");
        assert_eq!(rep_decoded, rep_data);
    }
}

#[test]
fn test_framed_writer_multi_64kb_chunks_roundtrip() {
    let mut rng = SplitMix64::new(0x123456789ABCDEF0);
    let size = 500 * 1024; // 500 KB
    let mut payload = Vec::with_capacity(size);

    let pattern = b"TTZip High-Performance Pure Rust Snappy Framing Engine 2026! ";
    while payload.len() < size {
        let to_copy = pattern.len().min(size - payload.len());
        payload.extend_from_slice(&pattern[..to_copy]);
        if payload.len() % 8192 == 0 && payload.len() < size {
            // Inject small entropy disturbance
            let rand_val = (rng.next_u64() & 0xFF) as u8;
            payload.push(rand_val);
        }
    }
    payload.truncate(size);

    // Test writing in arbitrary chunk slices (e.g. 17 bytes, 4096 bytes, 65536 bytes)
    for slice_size in [17, 4096, 65536, 100000] {
        let mut compressed = Vec::new();
        {
            let mut writer = SnappyFramedWriter::new(&mut compressed);
            for chunk in payload.chunks(slice_size) {
                writer.write_all(chunk).expect("write multi-chunk");
            }
            writer.finish().expect("finish multi-chunk");
        }

        assert!(is_framed_snappy(&compressed));
        assert!(snappy_frame_validate(&compressed));
        assert!(compressed.len() < payload.len() / 2);

        // Decompress with SnappyFramedReader
        let mut reader = SnappyFramedReader::new(Cursor::new(&compressed));
        let mut decompressed = Vec::new();
        reader
            .read_to_end(&mut decompressed)
            .expect("decompress multi-chunk");
        assert_eq!(decompressed.len(), payload.len());
        assert_eq!(decompressed, payload);

        // Interop verify with snap FrameDecoder
        let mut snap_decoder = snap::read::FrameDecoder::new(Cursor::new(&compressed));
        let mut snap_decompressed = Vec::new();
        snap_decoder
            .read_to_end(&mut snap_decompressed)
            .expect("snap decompress multi-chunk");
        assert_eq!(snap_decompressed, payload);
    }
}

#[test]
fn test_framed_writer_large_512kb_file() {
    let size = 512 * 1024; // 512 KB
    let mut payload = Vec::with_capacity(size);
    let sample_json = br#"{"event":"compression_benchmark","engine":"ttzip","version":"1.0.0","speed_mbps":1250.5,"safe_rust":true,"features":["simd","uniffi","streaming"]}"#;

    while payload.len() < size {
        let rem = size - payload.len();
        let chunk_len = sample_json.len().min(rem);
        payload.extend_from_slice(&sample_json[..chunk_len]);
    }

    let mut compressed = Vec::new();
    {
        let mut writer = SnappyFramedWriter::new(&mut compressed);
        writer.write_all(&payload).expect("write 512kb payload");
        writer.finish().expect("finish 512kb payload");
    }

    assert!(is_framed_snappy(&compressed));
    assert!(snappy_frame_validate(&compressed));

    let ratio = (compressed.len() as f64) / (payload.len() as f64);
    assert!(
        ratio < 0.10,
        "JSON payload compression ratio should be < 10%, got {:.4}",
        ratio
    );

    let mut reader = SnappyFramedReader::new(Cursor::new(&compressed));
    let mut decompressed = Vec::new();
    reader
        .read_to_end(&mut decompressed)
        .expect("decompress 512kb payload");
    assert_eq!(decompressed.len(), payload.len());
    assert_eq!(decompressed, payload);

    let helper_decompressed = snappy_frame_decode_to_vec(&compressed, 1024 * 1024)
        .expect("snappy_frame_decode_to_vec 512kb");
    assert_eq!(helper_decompressed, payload);
}

#[test]
fn test_framed_writer_uncompressible_high_entropy_fallback() {
    let mut rng = SplitMix64::new(0xDEADBEEFCAFE0001);
    let size = 65536; // 64 KB of pure pseudo-random high entropy bytes
    let mut uncompressible = vec![0u8; size];
    rng.fill_bytes(&mut uncompressible);

    let mut compressed = Vec::new();
    {
        let mut writer = SnappyFramedWriter::new(&mut compressed);
        writer
            .write_all(&uncompressible)
            .expect("write uncompressible");
        writer.finish().expect("finish uncompressible");
    }

    assert!(is_framed_snappy(&compressed));
    assert!(snappy_frame_validate(&compressed));

    // Verify chunk header at offset 10:
    // Offset 0..10: Stream Identifier (0xFF, 0x06, 0x00, 0x00, 's', 'N', 'a', 'P', 'p', 'Y')
    // Offset 10: Chunk Type ID MUST be 0x01 (Uncompressed Chunk) because high-entropy bytes cannot be compressed by >= 12.5%
    let chunk_type = compressed[10];
    assert_eq!(
        chunk_type, 0x01,
        "High-entropy block must emit chunk type 0x01 (Uncompressed Chunk), got {chunk_type:#04x}"
    );

    let chunk_len = (compressed[11] as usize)
        | ((compressed[12] as usize) << 8)
        | ((compressed[13] as usize) << 16);
    assert_eq!(
        chunk_len,
        4 + size,
        "Uncompressed chunk length must be 4 (CRC) + size"
    );

    // Decompress and verify byte-for-byte fidelity
    let mut reader = SnappyFramedReader::new(Cursor::new(&compressed));
    let mut decompressed = Vec::new();
    reader
        .read_to_end(&mut decompressed)
        .expect("decompress uncompressible");
    assert_eq!(decompressed, uncompressible);

    // Verify standard snap FrameDecoder decompress
    let mut snap_decoder = snap::read::FrameDecoder::new(Cursor::new(&compressed));
    let mut snap_decomp = Vec::new();
    snap_decoder
        .read_to_end(&mut snap_decomp)
        .expect("snap decompress uncompressible");
    assert_eq!(snap_decomp, uncompressible);
}

#[test]
fn test_framed_writer_flush_and_finish_state_transitions() {
    let mut compressed = Vec::new();
    {
        let mut writer = SnappyFramedWriter::new(&mut compressed);
        let part1 = b"Part 1: Initial telemetry stream message.";
        writer.write_all(part1).expect("write part 1");
        writer.flush().expect("flush part 1");

        let size_after_flush1 = writer.get_ref().map(|w| w.len()).unwrap_or(0);
        assert!(
            size_after_flush1 > 10,
            "Flushing part 1 must emit the first chunk immediately"
        );

        let part2 = b"Part 2: Mid-stream event dispatched synchronously.";
        writer.write_all(part2).expect("write part 2");
        writer.flush().expect("flush part 2");

        let size_after_flush2 = writer.get_ref().map(|w| w.len()).unwrap_or(0);
        assert!(
            size_after_flush2 > size_after_flush1,
            "Flushing part 2 must emit second chunk"
        );

        let part3 = b"Part 3: Final shutdown notification.";
        writer.write_all(part3).expect("write part 3");
        let finished_writer = writer.finish().expect("finish");
        let _ = finished_writer;
    }

    assert!(snappy_frame_validate(&compressed));

    let mut expected = Vec::new();
    expected.extend_from_slice(b"Part 1: Initial telemetry stream message.");
    expected.extend_from_slice(b"Part 2: Mid-stream event dispatched synchronously.");
    expected.extend_from_slice(b"Part 3: Final shutdown notification.");

    let mut reader = SnappyFramedReader::new(Cursor::new(&compressed));
    let mut decoded = Vec::new();
    reader
        .read_to_end(&mut decoded)
        .expect("read multi-flushed stream");
    assert_eq!(decoded, expected);
}

#[test]
fn test_framed_writer_concatenated_streams_interop() {
    let stream1_data = b"Stream One Payload: System Diagnostics Initialized.";
    let stream2_data = b"Stream Two Payload: Secondary Task Pipeline Ready.";

    let mut stream1_comp = Vec::new();
    {
        let mut w1 = SnappyFramedWriter::new(&mut stream1_comp);
        w1.write_all(stream1_data).expect("write stream 1");
        w1.finish().expect("finish stream 1");
    }

    let mut stream2_comp = Vec::new();
    {
        let mut w2 = SnappyFramedWriter::new(&mut stream2_comp);
        w2.write_all(stream2_data).expect("write stream 2");
        w2.finish().expect("finish stream 2");
    }

    let mut concatenated = Vec::new();
    concatenated.extend_from_slice(&stream1_comp);
    concatenated.extend_from_slice(&stream2_comp);

    // Decompress concatenated stream with SnappyFramedReader
    let mut reader = SnappyFramedReader::new(Cursor::new(&concatenated));
    let mut decoded = Vec::new();
    reader
        .read_to_end(&mut decoded)
        .expect("decompress concatenated");

    let mut expected = Vec::new();
    expected.extend_from_slice(stream1_data);
    expected.extend_from_slice(stream2_data);
    assert_eq!(decoded, expected);

    // Interop with snap FrameDecoder
    let mut snap_decoder = snap::read::FrameDecoder::new(Cursor::new(&concatenated));
    let mut snap_decoded = Vec::new();
    snap_decoder
        .read_to_end(&mut snap_decoded)
        .expect("snap decompress concatenated");
    assert_eq!(snap_decoded, expected);
}

#[test]
fn test_framed_writer_drop_safety() {
    let shared_sink = SharedVecWriter::new();
    let sample = b"Testing automatic flush and emission on SnappyFramedWriter drop.";

    {
        let mut writer = SnappyFramedWriter::new(shared_sink.clone());
        writer.write_all(sample).expect("write to dropped writer");
        // Writer dropped here without explicit finish()
    }

    let output = shared_sink.to_vec();
    assert!(
        !output.is_empty(),
        "Dropping writer must flush pending data"
    );
    assert!(is_framed_snappy(&output));
    assert!(snappy_frame_validate(&output));

    let mut reader = SnappyFramedReader::new(Cursor::new(&output));
    let mut decoded = Vec::new();
    reader
        .read_to_end(&mut decoded)
        .expect("decode dropped writer output");
    assert_eq!(decoded, sample);
}

#[test]
fn test_framed_writer_byte_by_byte_rollover() {
    let total_bytes = 70_000; // Crosses 65,536 boundary by 4,464 bytes
    let mut data = Vec::with_capacity(total_bytes);
    for i in 0..total_bytes {
        data.push((i % 251) as u8);
    }

    let mut compressed = Vec::new();
    {
        let mut writer = SnappyFramedWriter::new(&mut compressed);
        for &byte in &data {
            writer.write_all(&[byte]).expect("write single byte");
        }
        writer.finish().expect("finish byte-by-byte");
    }

    assert!(snappy_frame_validate(&compressed));

    let mut reader = SnappyFramedReader::new(Cursor::new(&compressed));
    let mut decoded = Vec::new();
    reader
        .read_to_end(&mut decoded)
        .expect("decode byte-by-byte");
    assert_eq!(decoded.len(), total_bytes);
    assert_eq!(decoded, data);
}
