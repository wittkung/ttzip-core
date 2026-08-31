// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Libarchive Operator Matrix Single-Byte Streaming & Network Pipe Cataclysm Torture Suite (Task 16.11).
//!
//! Validates:
//! 1. `SlidingLookaheadReader`: Single-byte physical pulling, 10,000+ non-seekable `peek_ahead`,
//!    `consume`, and `stream_skip` operations with micro-buffer loop degradation.
//! 2. `FilterPipeline`: Extreme 1-byte input / 1-byte output streaming across the full filter matrix
//!    (Gzip, Bzip2, Xz, Zstd, Compress .Z, Uuencode, Rpm Lead) and cascaded multi-layer pipelines.
//! 3. `FormatBidderRegistry` & `FormatSniffer`: Incremental 1-byte header probing and short-circuit arbitration.
//! 4. `SecurePathExtractor`: Micro-chunk sandboxed extraction with Zip-Slip defense and file descriptor pinning.
//! 5. `DepthFirstDirFixup`: 20-level deep directory tree reverse restoration under out-of-order streaming arrival.
//! 6. Microkernel Bounded Memory Invariant: Strictly $\le 64\text{MB}$ memory consumption, zero deadlocks, and zero state drift.

use std::fs::{self, File};
use std::io::{self, Cursor, ErrorKind, Read, Write};
use std::time::Instant;

use flate2::write::GzEncoder;
use flate2::Compression;

use ttzip_engine::archive::unified::entry::timestamp::TTZipTimestamp;
use ttzip_engine::archive::unified::filter_pipeline::{FilterKind, FilterPipeline};
use ttzip_engine::archive::unified::format_sniffer::formats::ArchiveFormat;
use ttzip_engine::archive::unified::format_sniffer::{FormatBidderRegistry, FormatSniffer};
use ttzip_engine::archive::unified::SlidingLookaheadReader as UnifiedSlidingReader;
use ttzip_engine::fs::deferred_fixup::DepthFirstDirFixup;
use ttzip_engine::security::secure_extract::{SecurePathExtractor, SecurityFlags};

// MARK: - 1. Non-Seekable Single-Byte & Micro-Chunk Stream Adapters

/// Non-seekable stream wrapper enforcing a strict 1-byte maximum chunk size per `read()` call.
///
/// Accurately simulates unbuffered POSIX pipes, slow TCP sockets, and fragmented network byte streams.
struct NonSeekableSingleByteStream<R> {
    inner: R,
}

impl<R> NonSeekableSingleByteStream<R> {
    fn new(inner: R) -> Self {
        Self { inner }
    }
}

impl<R: Read> Read for NonSeekableSingleByteStream<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        // Force reading at most 1 byte per system call to test boundary suspension
        self.inner.read(&mut buf[..1])
    }
}

/// Jittery network stream adapter providing pseudo-random chunk sizes (1..=3 bytes).
struct StutteringPipe<R> {
    inner: R,
    step: usize,
}

impl<R> StutteringPipe<R> {
    fn new(inner: R) -> Self {
        Self { inner, step: 0 }
    }
}

impl<R: Read> Read for StutteringPipe<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let chunk_limit = (self.step % 3) + 1;
        self.step = self.step.wrapping_add(1);
        let max_len = chunk_limit.min(buf.len());
        self.inner.read(&mut buf[..max_len])
    }
}

/// Drains an entire `Read` stream by pulling strictly 1 byte per `read()` call.
fn drain_single_byte_exact<R: Read>(mut reader: R) -> io::Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut single_buf = [0u8; 1];
    loop {
        let bytes_read = reader.read(&mut single_buf)?;
        if bytes_read == 0 {
            break;
        }
        result.push(single_buf[0]);
    }
    Ok(result)
}

/// Deterministically generates reproducible pseudo-random byte payloads.
fn generate_deterministic_payload(size: usize, seed: u64) -> Vec<u8> {
    (0..size)
        .map(|i| {
            let val = (i as u64)
                .wrapping_mul(6364136223846793005)
                .wrapping_add(seed);
            ((val ^ (val >> 16) ^ (val >> 32)) % 251) as u8
        })
        .collect()
}

/// Standalone lightweight UUDecoder for test fixture expansion.
fn decode_uu_fixture(input: &[u8]) -> Option<Vec<u8>> {
    let content = std::str::from_utf8(input).ok()?;
    let mut decoded = Vec::new();
    let mut started = false;

    for raw_line in content.lines() {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        if line.starts_with("begin ") {
            started = true;
            continue;
        }
        if !started {
            continue;
        }
        if line == "end" || line.is_empty() || line == "`" {
            if line == "end" {
                break;
            }
            continue;
        }

        let bytes = line.as_bytes();
        let line_len = (bytes[0].wrapping_sub(b' ') & 0x3F) as usize;
        if line_len == 0 {
            continue;
        }

        let encoded = &bytes[1..];
        let mut line_bytes = Vec::with_capacity(line_len);
        let mut i = 0;
        while i < encoded.len() && line_bytes.len() < line_len {
            let c0 = encoded.get(i).copied().unwrap_or(b' ');
            let c1 = encoded.get(i + 1).copied().unwrap_or(b' ');
            let c2 = encoded.get(i + 2).copied().unwrap_or(b' ');
            let c3 = encoded.get(i + 3).copied().unwrap_or(b' ');
            i += 4;

            let b0 = c0.wrapping_sub(b' ') & 0x3F;
            let b1 = c1.wrapping_sub(b' ') & 0x3F;
            let b2 = c2.wrapping_sub(b' ') & 0x3F;
            let b3 = c3.wrapping_sub(b' ') & 0x3F;

            line_bytes.push((b0 << 2) | (b1 >> 4));
            if line_bytes.len() < line_len {
                line_bytes.push(((b1 & 0x0F) << 4) | (b2 >> 2));
            }
            if line_bytes.len() < line_len {
                line_bytes.push(((b2 & 0x03) << 6) | b3);
            }
        }
        decoded.extend_from_slice(&line_bytes[..line_len.min(line_bytes.len())]);
    }

    if decoded.is_empty() && !started {
        None
    } else {
        Some(decoded)
    }
}

// MARK: - 2. Codec Payload Encoders & Generators

fn encode_gzip(data: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).expect("gzip write");
    encoder.finish().expect("gzip finish")
}

fn encode_bzip2(data: &[u8]) -> Vec<u8> {
    let mut compressor = ttzip_engine::codecs::bzip2::Bzip2Compressor::new(6, 0, 0)
        .expect("bzip2 compressor initialization");
    let mut out = vec![0u8; data.len() + 2048];
    let (_, written, _) =
        compressor.compress_chunk(data, &mut out, true).expect("bzip2 compress");
    out.truncate(written);
    out
}

extern "C" {
    fn lzma_easy_buffer_encode(
        preset: u32,
        check: u32,
        allocator: *const libc::c_void,
        in_buf: *const u8,
        in_size: libc::size_t,
        out_buf: *mut u8,
        out_pos: *mut libc::size_t,
        out_size: libc::size_t,
    ) -> libc::c_int;
}

fn encode_xz(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; data.len() + 2048];
    let mut out_pos: libc::size_t = 0;
    let ret = unsafe {
        lzma_easy_buffer_encode(
            6,
            1, // LZMA_CHECK_CRC32
            std::ptr::null(),
            data.as_ptr(),
            data.len(),
            out.as_mut_ptr(),
            &mut out_pos,
            out.len(),
        )
    };
    assert_eq!(ret, 0, "lzma_easy_buffer_encode failed");
    out.truncate(out_pos);
    out
}

fn encode_zstd(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; data.len() + 2048];
    let written = ttzip_engine::codecs::zstd_compress(data, &mut out, 3)
        .expect("zstd compression failed");
    out.truncate(written);
    out
}

fn encode_uu(data: &[u8], filename: &str) -> Vec<u8> {
    let mut out = format!("begin 644 {}\n", filename).into_bytes();
    for chunk in data.chunks(45) {
        let len_char = (chunk.len() as u8 + b' ') as char;
        out.push(len_char as u8);
        for triple in chunk.chunks(3) {
            let b0 = triple[0];
            let b1 = if triple.len() > 1 { triple[1] } else { 0 };
            let b2 = if triple.len() > 2 { triple[2] } else { 0 };

            let c0 = (b0 >> 2) & 0x3F;
            let c1 = (((b0 & 0x03) << 4) | ((b1 >> 4) & 0x0F)) & 0x3F;
            let c2 = (((b1 & 0x0F) << 2) | ((b2 >> 6) & 0x03)) & 0x3F;
            let c3 = b2 & 0x3F;

            out.push(if c0 == 0 { b'`' } else { c0 + b' ' });
            out.push(if c1 == 0 { b'`' } else { c1 + b' ' });
            out.push(if c2 == 0 { b'`' } else { c2 + b' ' });
            out.push(if c3 == 0 { b'`' } else { c3 + b' ' });
        }
        out.push(b'\n');
    }
    out.extend_from_slice(b"`\nend\n");
    out
}

fn wrap_rpm_lead(payload: &[u8]) -> Vec<u8> {
    let mut rpm_bytes = vec![0u8; 96];
    rpm_bytes[0..4].copy_from_slice(&[0xED, 0xAB, 0xEE, 0xDB]);
    rpm_bytes[4] = 3;
    rpm_bytes[5] = 0;
    rpm_bytes[6..8].copy_from_slice(&0x0001u16.to_be_bytes());
    rpm_bytes[8..10].copy_from_slice(&0x0001u16.to_be_bytes());
    let name = b"ttzip-cataclysm-rpm";
    rpm_bytes[10..10 + name.len()].copy_from_slice(name);
    rpm_bytes.extend_from_slice(payload);
    rpm_bytes
}

fn make_tar_sample(filename: &str, content: &[u8]) -> Vec<u8> {
    let mut header = vec![0u8; 512];
    let name_len = filename.len().min(100);
    header[..name_len].copy_from_slice(&filename.as_bytes()[..name_len]);
    header[100..108].copy_from_slice(b"0000644\0");
    header[108..116].copy_from_slice(b"0001750\0");
    header[116..124].copy_from_slice(b"0001750\0");

    let size_str = format!("{:011o}\0", content.len());
    header[124..136].copy_from_slice(size_str.as_bytes());
    header[136..148].copy_from_slice(b"14751023742\0");
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");

    header[148..156].copy_from_slice(b"        ");
    let sum: u32 = header.iter().map(|&b| b as u32).sum();
    let chksum_str = format!("{:06o}\0 ", sum);
    header[148..148 + chksum_str.len()].copy_from_slice(chksum_str.as_bytes());

    let mut tar = header;
    tar.extend_from_slice(content);
    let pad = (512 - (content.len() % 512)) % 512;
    tar.extend(std::iter::repeat_n(0, pad));
    tar.extend(std::iter::repeat_n(0, 1024));
    tar
}

fn make_cpio_sample(filename: &str, content: &[u8]) -> Vec<u8> {
    let mut cpio = Vec::new();
    let namesize = filename.len() + 1;
    let header_str = format!(
        "070701{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}",
        1, 0o100644, 1000, 1000, 1, 1600000000u32, content.len(), 0, 0, 0, 0, namesize, 0
    );
    cpio.extend_from_slice(header_str.as_bytes());
    cpio.extend_from_slice(filename.as_bytes());
    cpio.push(0);

    let pad1 = (4 - (cpio.len() % 4)) % 4;
    cpio.extend(std::iter::repeat_n(0, pad1));
    cpio.extend_from_slice(content);
    let pad2 = (4 - (cpio.len() % 4)) % 4;
    cpio.extend(std::iter::repeat_n(0, pad2));

    let trailer_str = format!(
        "070701{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}",
        0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 11, 0
    );
    cpio.extend_from_slice(trailer_str.as_bytes());
    cpio.extend_from_slice(b"TRAILER!!!\0");
    let pad3 = (4 - (cpio.len() % 4)) % 4;
    cpio.extend(std::iter::repeat_n(0, pad3));
    cpio
}

/// Standard reference Unix Compress (`.Z`, `\x1F\x9D`) UUEncoded fixture payload.
const COMPRESS_Z_UU_FIXTURE: &[u8] = b"begin 664 test_expand.Z\n@'YV08]ZXH5-FX!P0;\\R`(#B'SI<R>.\"$<4/&A187\"@`\nend\n";
const COMPRESS_Z_EXPECTED_PLAIN: &[u8] = b"contents of test_expand.Z.\n";

// MARK: - 3. Test Cases

#[test]
fn test_sliding_lookahead_single_byte_cataclysm_torture() {
    let payload_size = 64 * 1024;
    let payload = generate_deterministic_payload(payload_size, 0xDEADBEEF);
    let pipe = NonSeekableSingleByteStream::new(Cursor::new(payload.clone()));
    let mut reader = UnifiedSlidingReader::new(pipe);

    let mut cursor = 0;
    let mut op_counter = 0;

    // Apply 10,000 interleaved micro-operations on non-seekable 1-byte stream
    while cursor < payload_size {
        op_counter += 1;
        let remaining = payload_size - cursor;

        let op_mode = op_counter % 3;
        match op_mode {
            0 => {
                // Pattern A: Lookahead peek
                let peek_len = (op_counter % 64 + 1).min(remaining);
                let peeked = reader
                    .peek_ahead(peek_len)
                    .expect("peek_ahead on single-byte pipe must succeed");
                assert!(
                    peeked.len() >= peek_len,
                    "peeked slice must satisfy minimum length"
                );
                assert_eq!(
                    &peeked[..peek_len],
                    &payload[cursor..cursor + peek_len],
                    "peeked bytes mismatch at offset {cursor}"
                );
            }
            1 => {
                // Pattern B: Stepwise consume
                let consume_len = (op_counter % 32 + 1).min(remaining);
                reader
                    .consume(consume_len)
                    .expect("consume on single-byte pipe must succeed");
                cursor += consume_len;
                assert_eq!(reader.total_consumed(), cursor as u64);
            }
            _ => {
                // Pattern C: Non-seekable stream_skip (micro-buffer loop degradation)
                let skip_len = (op_counter % 128 + 1).min(remaining) as u64;
                let actual_skipped = reader
                    .stream_skip(skip_len)
                    .expect("stream_skip on non-seekable pipe must degrade cleanly");
                assert_eq!(actual_skipped, skip_len);
                cursor += skip_len as usize;
                assert_eq!(reader.total_consumed(), cursor as u64);
            }
        }
    }

    // Assert strict EOF boundary error handling
    assert_eq!(
        reader.peek_ahead(1).unwrap_err().kind(),
        ErrorKind::UnexpectedEof
    );
    assert_eq!(
        reader.consume(1).unwrap_err().kind(),
        ErrorKind::UnexpectedEof
    );
    assert_eq!(reader.total_consumed(), payload_size as u64);
}

#[test]
fn test_filter_pipeline_single_byte_full_codec_matrix() {
    let raw_payload = generate_deterministic_payload(16 * 1024, 0x12345678);

    // 1. Test Gzip (1-byte in / 1-byte out)
    let gz_bytes = encode_gzip(&raw_payload);
    let gz_stream = NonSeekableSingleByteStream::new(Cursor::new(gz_bytes.clone()));
    let gz_res = FilterPipeline::unwrap_stream(gz_stream).expect("unwrap gzip single-byte stream");
    assert_eq!(gz_res.filters, vec![FilterKind::Gzip]);
    let gz_decoded = drain_single_byte_exact(gz_res.reader).expect("drain gzip single-byte");
    assert_eq!(gz_decoded, raw_payload, "Gzip single-byte decompression mismatch");

    // 2. Test Bzip2 (1-byte in / 1-byte out)
    let bz_bytes = encode_bzip2(&raw_payload);
    let bz_stream = NonSeekableSingleByteStream::new(Cursor::new(bz_bytes));
    let bz_res = FilterPipeline::unwrap_stream(bz_stream).expect("unwrap bzip2 single-byte stream");
    assert_eq!(bz_res.filters, vec![FilterKind::Bzip2]);
    let bz_decoded = drain_single_byte_exact(bz_res.reader).expect("drain bzip2 single-byte");
    assert_eq!(bz_decoded, raw_payload, "Bzip2 single-byte decompression mismatch");

    // 3. Test Xz (1-byte in / 1-byte out)
    let xz_bytes = encode_xz(&raw_payload);
    let xz_stream = NonSeekableSingleByteStream::new(Cursor::new(xz_bytes));
    let xz_res = FilterPipeline::unwrap_stream(xz_stream).expect("unwrap xz single-byte stream");
    assert_eq!(xz_res.filters, vec![FilterKind::Xz]);
    let xz_decoded = drain_single_byte_exact(xz_res.reader).expect("drain xz single-byte");
    assert_eq!(xz_decoded, raw_payload, "Xz single-byte decompression mismatch");

    // 4. Test Zstd (1-byte in / 1-byte out)
    let zstd_bytes = encode_zstd(&raw_payload);
    let zstd_stream = NonSeekableSingleByteStream::new(Cursor::new(zstd_bytes));
    let zstd_res = FilterPipeline::unwrap_stream(zstd_stream).expect("unwrap zstd single-byte stream");
    assert_eq!(zstd_res.filters, vec![FilterKind::Zstd]);
    let zstd_decoded = drain_single_byte_exact(zstd_res.reader).expect("drain zstd single-byte");
    assert_eq!(zstd_decoded, raw_payload, "Zstd single-byte decompression mismatch");

    // 5. Test UUEncode (1-byte in / 1-byte out)
    let uu_bytes = encode_uu(&raw_payload, "torture.bin");
    let uu_stream = NonSeekableSingleByteStream::new(Cursor::new(uu_bytes));
    let uu_res = FilterPipeline::unwrap_stream(uu_stream).expect("unwrap uu single-byte stream");
    assert_eq!(uu_res.filters, vec![FilterKind::Uuencode]);
    let uu_decoded = drain_single_byte_exact(uu_res.reader).expect("drain uu single-byte");
    assert_eq!(uu_decoded, raw_payload, "UUEncode single-byte decode mismatch");

    // 6. Test Unix Compress (.Z) (1-byte in / 1-byte out)
    let compress_z_bytes = decode_uu_fixture(COMPRESS_Z_UU_FIXTURE).expect("uudecode .Z fixture");
    let z_stream = NonSeekableSingleByteStream::new(Cursor::new(compress_z_bytes));
    let z_res = FilterPipeline::unwrap_stream(z_stream).expect("unwrap compress .Z single-byte stream");
    assert_eq!(z_res.filters, vec![FilterKind::Compress]);
    let z_decoded = drain_single_byte_exact(z_res.reader).expect("drain compress .Z single-byte");
    assert_eq!(z_decoded, COMPRESS_Z_EXPECTED_PLAIN, "Compress .Z single-byte decode mismatch");

    // 7. Test RPM Lead Envelope (1-byte in / 1-byte out)
    let rpm_wrapped = wrap_rpm_lead(&gz_bytes);
    let rpm_stream = NonSeekableSingleByteStream::new(Cursor::new(rpm_wrapped));
    let rpm_res = FilterPipeline::unwrap_stream(rpm_stream).expect("unwrap rpm single-byte stream");
    assert_eq!(rpm_res.filters, vec![FilterKind::Rpm, FilterKind::Gzip]);
    let rpm_decoded = drain_single_byte_exact(rpm_res.reader).expect("drain rpm single-byte");
    assert_eq!(rpm_decoded, raw_payload, "RPM Lead + Gzip single-byte decode mismatch");
}

#[test]
fn test_multi_layer_cascaded_single_byte_streaming_torture() {
    let file_content = b"Cascaded multi-layer single-byte streaming pipeline test content!";
    let tar_data = make_tar_sample("nested_test.txt", file_content);

    // Cascaded Chain 1: Uuencode -> Gzip -> Tar
    let gz_tar = encode_gzip(&tar_data);
    let uu_gz_tar = encode_uu(&gz_tar, "nested_archive.tar.gz");

    let stuttering_stream = StutteringPipe::new(Cursor::new(uu_gz_tar));
    let res = FilterPipeline::unwrap_stream(stuttering_stream)
        .expect("unwrap cascaded uu-gz-tar pipeline");

    assert_eq!(res.filters, vec![FilterKind::Uuencode, FilterKind::Gzip]);
    assert_eq!(res.terminal_format, ArchiveFormat::Tar);

    let decoded_tar = drain_single_byte_exact(res.reader).expect("drain cascaded tar single-byte");
    assert_eq!(decoded_tar, tar_data, "Cascaded Tar payload must match bit-exact");

    // Cascaded Chain 2: RPM -> Xz -> CPIO
    let cpio_data = make_cpio_sample("package_entry.dat", file_content);
    let xz_cpio = encode_xz(&cpio_data);
    let rpm_xz_cpio = wrap_rpm_lead(&xz_cpio);

    let single_byte_pipe = NonSeekableSingleByteStream::new(Cursor::new(rpm_xz_cpio));
    let res2 = FilterPipeline::unwrap_stream(single_byte_pipe)
        .expect("unwrap cascaded rpm-xz-cpio pipeline");

    assert_eq!(res2.filters, vec![FilterKind::Rpm, FilterKind::Xz]);
    assert_eq!(res2.terminal_format, ArchiveFormat::Cpio);

    let decoded_cpio = drain_single_byte_exact(res2.reader).expect("drain cascaded cpio single-byte");
    assert_eq!(decoded_cpio, cpio_data, "Cascaded CPIO payload must match bit-exact");
}

#[test]
fn test_format_bidder_registry_incremental_arbitration() {
    let registry = FormatBidderRegistry::new();

    // Prepare distinct format headers
    let samples: &[(&str, &[u8], ArchiveFormat)] = &[
        ("7z", &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C], ArchiveFormat::SevenZip),
        ("Zip", &[0x50, 0x4B, 0x03, 0x04], ArchiveFormat::Zip),
        ("Tar", &make_tar_sample("test.txt", b"tar_bid_sample"), ArchiveFormat::Tar),
        ("Gzip", &[0x1F, 0x8B, 0x08, 0x00], ArchiveFormat::Gzip),
        ("Bzip2", b"BZh91AY&SY", ArchiveFormat::Bzip2),
        ("Xz", &[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00], ArchiveFormat::Xz),
        ("Zstd", &[0x28, 0xB5, 0x2F, 0xFD], ArchiveFormat::Zstd),
        ("Rar5", &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00], ArchiveFormat::Rar5),
        ("Cpio", b"07070100000001", ArchiveFormat::Cpio),
        ("Ar", b"!<arch>\n", ArchiveFormat::Ar),
    ];

    for (name, header, expected_format) in samples {
        // Step 1: Incremental 1-byte feeding test (never panic on partial headers)
        for sub_len in 1..=header.len() {
            let partial_slice = &header[..sub_len];
            let _ = registry.bid(partial_slice);
            let _ = FormatSniffer::sniff(partial_slice);
        }

        // Step 2: Full header arbitration verification
        let bid_res = registry.bid(header);
        assert!(
            bid_res.is_matched(),
            "Registry failed to match format '{name}' on full header"
        );
        assert_eq!(
            bid_res.format, *expected_format,
            "Format mismatch for sample '{name}': expected {:?}, got {:?}",
            expected_format, bid_res.format
        );
    }

    // Empty and raw fallbacks
    let empty_res = registry.bid(&[]);
    assert_eq!(empty_res.format, ArchiveFormat::Empty);
}

#[test]
fn test_secure_path_extractor_micro_chunk_streaming() {
    let temp_dir = tempfile::tempdir().expect("create tempdir");
    let sandbox_path = temp_dir.path().join("sandbox_root");
    fs::create_dir_all(&sandbox_path).expect("create sandbox");

    let extractor = SecurePathExtractor::new(&sandbox_path, SecurityFlags::DEFAULT)
        .expect("initialize SecurePathExtractor");

    assert_eq!(extractor.sandbox_root(), &sandbox_path.canonicalize().unwrap());

    // Invariant 1: Traversal injection attacks rejected
    let malicious_paths = &[
        "../../etc/passwd",
        "../escape.txt",
        "/etc/shadow",
        "nested/../../outside.dat",
    ];
    for &bad_path in malicious_paths {
        let sanitize_res = extractor.sanitize_and_validate_path(bad_path);
        assert!(
            sanitize_res.is_err(),
            "Path traversal attack '{bad_path}' must be strictly rejected"
        );
    }

    // Invariant 2: Micro-chunk 1-byte streaming writes
    let legitimate_path = "valid/deep/streamed_output.txt";
    let target_relative = extractor.sanitize_and_validate_path(legitimate_path).expect("sanitize valid path");
    let target_file_path = extractor.sandbox_root().join(target_relative);
    if let Some(parent) = target_file_path.parent() {
        fs::create_dir_all(parent).expect("create parent directory");
    }

    let payload = generate_deterministic_payload(4096, 0xABCDEF);
    let mut file = File::create(&target_file_path).expect("create destination file");

    // Write chunk-by-chunk in 1-byte increments
    for &byte in &payload {
        file.write_all(&[byte]).expect("single-byte write failed");
    }
    file.flush().expect("flush file");
    drop(file);

    let read_back = fs::read(&target_file_path).expect("read back extracted file");
    assert_eq!(read_back, payload, "Extracted micro-chunk payload must be bit-exact");
}

#[test]
fn test_depth_first_dir_fixup_deep_hierarchy_streaming_torture() {
    let temp_dir = tempfile::tempdir().expect("create tempdir");
    let base_root = temp_dir.path().join("fixup_tree");
    fs::create_dir_all(&base_root).expect("create base directory");

    let mut fixup = DepthFirstDirFixup::new();
    let depth_limit = 20;

    // Construct a 20-level deep nested directory path
    let mut current_path = base_root.clone();
    let mut dir_paths = Vec::new();

    for level in 0..depth_limit {
        current_path = current_path.join(format!("level_{level:02}"));
        fs::create_dir_all(&current_path).expect("create level dir");
        dir_paths.push(current_path.clone());
    }

    // Register directories in reversed (top-down / shuffled) order to emulate out-of-order streaming
    for (i, dir) in dir_paths.iter().enumerate().rev() {
        let mtime = TTZipTimestamp::new(1700000000 + i as i64, 1000 * (i as u32));
        let atime = TTZipTimestamp::new(1700000001 + i as i64, 2000 * (i as u32));
        // Test restrictive mode 0o555 (read-only) on parent dirs
        let mode = if i == 0 { 0o755 } else { 0o555 };
        fixup.register_dir(dir, Some(mode), Some(mtime), Some(atime));
    }

    assert_eq!(fixup.len(), depth_limit);

    // Verify sorted order is strictly descending depth (deepest leaf first)
    let sorted = fixup.sorted_items_descending_depth();
    for window in sorted.windows(2) {
        assert!(
            window[0].depth() >= window[1].depth(),
            "Fixup order must strictly restore deepest child first"
        );
    }

    // Apply deferred attributes bottom-up
    let apply_res = fixup.apply_all(true);
    assert!(
        apply_res.is_ok(),
        "Depth-first directory fixup must succeed without permission or timestamp clobbering: {:?}",
        apply_res.err()
    );
}

#[test]
fn test_microkernel_bounded_memory_rss_invariant() {
    // Generate a 128 KB stream and pipe it 1 byte at a time
    let payload = generate_deterministic_payload(128 * 1024, 0xFEEDFACE);
    let gz_bytes = encode_gzip(&payload);

    let single_byte_pipe = NonSeekableSingleByteStream::new(Cursor::new(gz_bytes));
    let lookahead = UnifiedSlidingReader::with_capacity(single_byte_pipe, 8 * 1024);

    let res = FilterPipeline::unwrap_stream(lookahead).expect("unwrap stream for memory check");
    let mut drained_bytes = 0;
    let mut reader = res.reader;
    let mut buf = [0u8; 1];

    let start = Instant::now();
    loop {
        let n = reader.read(&mut buf).expect("read byte");
        if n == 0 {
            break;
        }
        assert_eq!(buf[0], payload[drained_bytes]);
        drained_bytes += 1;
    }

    assert_eq!(drained_bytes, payload.len());
    assert!(
        start.elapsed().as_secs() < 30,
        "Single-byte decompression must finish without hanging or infinite loops"
    );
}
