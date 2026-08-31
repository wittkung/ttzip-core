// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration tests for Multi-Layer Cascaded Filter Pipeline Engine.

use std::io::{Cursor, Read, Write};
use flate2::write::GzEncoder;
use flate2::Compression;

use ttzip_engine::archive::unified::filter_pipeline::{
    FilterKind, FilterPipeline, FilterPipelineError, MAX_FILTER_CHAIN_DEPTH,
};
use ttzip_engine::archive::unified::format_sniffer::ArchiveFormat;

// MARK: - Test Payload Generators

fn encode_gzip(data: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).expect("gzip write");
    encoder.finish().expect("gzip finish")
}

fn encode_bzip2(data: &[u8]) -> Vec<u8> {
    let mut compressor = ttzip_engine::codecs::bzip2::Bzip2Compressor::new(6, 0, 0)
        .expect("bzip2 compressor init");
    let mut out = vec![0u8; data.len() + 1024];
    let (_, written, _) = compressor.compress_chunk(data, &mut out, true).expect("bzip2 compress");
    out.truncate(written);
    out
}

fn encode_zstd(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; data.len() + 1024];
    let written = ttzip_engine::codecs::zstd_compress(data, &mut out, 3).expect("zstd compress");
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
    let mut out = vec![0u8; data.len() + 1024];
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
    let name = b"ttzip-test-suite";
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

// MARK: - 1. Single-Layer Compression Tests

#[test]
fn test_single_layer_gzip_auto_identification_and_decode() {
    let raw_payload = b"Hello TTZip Native High-Performance Pipeline Gzip Single-Layer Test!";
    let gz_data = encode_gzip(raw_payload);

    let res = FilterPipeline::unwrap_stream(Cursor::new(gz_data)).expect("unwrap gzip");
    assert_eq!(res.filters, vec![FilterKind::Gzip]);
    assert_eq!(res.terminal_format, ArchiveFormat::Raw);

    let mut decoded = Vec::new();
    let mut reader = res.reader;
    reader.read_to_end(&mut decoded).expect("read decoded");
    assert_eq!(decoded, raw_payload);
}

#[test]
fn test_single_layer_bzip2_auto_identification_and_decode() {
    let raw_payload = b"Hello TTZip Native High-Performance Pipeline Bzip2 Single-Layer Test!";
    let bz_data = encode_bzip2(raw_payload);

    let res = FilterPipeline::unwrap_stream(Cursor::new(bz_data)).expect("unwrap bzip2");
    assert_eq!(res.filters, vec![FilterKind::Bzip2]);
    assert_eq!(res.terminal_format, ArchiveFormat::Raw);

    let mut decoded = Vec::new();
    let mut reader = res.reader;
    reader.read_to_end(&mut decoded).expect("read decoded");
    assert_eq!(decoded, raw_payload);
}

#[test]
fn test_single_layer_zstd_auto_identification_and_decode() {
    let raw_payload = b"Hello TTZip Native High-Performance Pipeline Zstd Single-Layer Test!";
    let zst_data = encode_zstd(raw_payload);

    let res = FilterPipeline::unwrap_stream(Cursor::new(zst_data)).expect("unwrap zstd");
    assert_eq!(res.filters, vec![FilterKind::Zstd]);
    assert_eq!(res.terminal_format, ArchiveFormat::Raw);

    let mut decoded = Vec::new();
    let mut reader = res.reader;
    reader.read_to_end(&mut decoded).expect("read decoded");
    assert_eq!(decoded, raw_payload);
}

#[test]
fn test_single_layer_xz_auto_identification_and_decode() {
    let raw_payload = b"Hello TTZip Native High-Performance Pipeline XZ Single-Layer Test!";
    let xz_data = encode_xz(raw_payload);

    let res = FilterPipeline::unwrap_stream(Cursor::new(xz_data)).expect("unwrap xz");
    assert_eq!(res.filters, vec![FilterKind::Xz]);
    assert_eq!(res.terminal_format, ArchiveFormat::Raw);

    let mut decoded = Vec::new();
    let mut reader = res.reader;
    reader.read_to_end(&mut decoded).expect("read decoded");
    assert_eq!(decoded, raw_payload);
}

#[test]
fn test_single_layer_uuencode_auto_identification_and_decode() {
    let raw_payload = b"Hello TTZip UUEncode ASCII envelope decoding verification payload 2026!";
    let uu_data = encode_uu(raw_payload, "document.txt");

    let res = FilterPipeline::unwrap_stream(Cursor::new(uu_data)).expect("unwrap uu");
    assert_eq!(res.filters, vec![FilterKind::Uuencode]);
    assert_eq!(res.terminal_format, ArchiveFormat::Raw);

    let mut decoded = Vec::new();
    let mut reader = res.reader;
    reader.read_to_end(&mut decoded).expect("read decoded");
    assert_eq!(decoded, raw_payload);
}

// MARK: - 2. Compound Multi-Layer Cascade Tests

#[test]
fn test_compound_tar_gz_cascade_unfolding() {
    let raw_content = b"Content inside tar.gz payload file.";
    let tar_data = make_tar_sample("payload.txt", raw_content);
    let tgz_data = encode_gzip(&tar_data);

    let res = FilterPipeline::unwrap_stream(Cursor::new(tgz_data)).expect("unwrap tar.gz");
    assert_eq!(res.filters, vec![FilterKind::Gzip]);
    assert_eq!(res.terminal_format, ArchiveFormat::Tar);

    let mut decoded = Vec::new();
    let mut reader = res.reader;
    reader.read_to_end(&mut decoded).expect("read tar");
    assert_eq!(decoded, tar_data);
}

#[test]
fn test_compound_tar_bz2_cascade_unfolding() {
    let raw_content = b"Content inside tar.bz2 payload file.";
    let tar_data = make_tar_sample("file.dat", raw_content);
    let tbz_data = encode_bzip2(&tar_data);

    let res = FilterPipeline::unwrap_stream(Cursor::new(tbz_data)).expect("unwrap tar.bz2");
    assert_eq!(res.filters, vec![FilterKind::Bzip2]);
    assert_eq!(res.terminal_format, ArchiveFormat::Tar);

    let mut decoded = Vec::new();
    let mut reader = res.reader;
    reader.read_to_end(&mut decoded).expect("read tar");
    assert_eq!(decoded, tar_data);
}

#[test]
fn test_compound_cpio_gz_cascade_unfolding() {
    let raw_content = b"Content inside cpio.gz payload file.";
    let cpio_data = make_cpio_sample("etc/hosts", raw_content);
    let cpio_gz = encode_gzip(&cpio_data);

    let res = FilterPipeline::unwrap_stream(Cursor::new(cpio_gz)).expect("unwrap cpio.gz");
    assert_eq!(res.filters, vec![FilterKind::Gzip]);
    assert_eq!(res.terminal_format, ArchiveFormat::Cpio);

    let mut decoded = Vec::new();
    let mut reader = res.reader;
    reader.read_to_end(&mut decoded).expect("read cpio");
    assert_eq!(decoded, cpio_data);
}

#[test]
fn test_triple_layer_tar_gz_uu_cascade_unfolding() {
    let raw_content = b"Nested deep payload for triple-layer UU + GZ + TAR validation.";
    let tar_data = make_tar_sample("deep.bin", raw_content);
    let gz_data = encode_gzip(&tar_data);
    let uu_data = encode_uu(&gz_data, "archive.tar.gz");

    let res = FilterPipeline::unwrap_stream(Cursor::new(uu_data)).expect("unwrap tar.gz.uu");
    assert_eq!(res.filters, vec![FilterKind::Uuencode, FilterKind::Gzip]);
    assert_eq!(res.terminal_format, ArchiveFormat::Tar);

    let mut decoded = Vec::new();
    let mut reader = res.reader;
    reader.read_to_end(&mut decoded).expect("read final tar");
    assert_eq!(decoded, tar_data);
}

#[test]
fn test_triple_layer_cpio_xz_rpm_cascade_unfolding() {
    let raw_content = b"Nested RPM payload for triple-layer RPM + XZ + CPIO validation.";
    let cpio_data = make_cpio_sample("usr/bin/ttzip", raw_content);
    let xz_data = encode_xz(&cpio_data);
    let rpm_data = wrap_rpm_lead(&xz_data);

    let res = FilterPipeline::unwrap_stream(Cursor::new(rpm_data)).expect("unwrap cpio.xz.rpm");
    assert_eq!(res.filters, vec![FilterKind::Rpm, FilterKind::Xz]);
    assert_eq!(res.terminal_format, ArchiveFormat::Cpio);

    let mut decoded = Vec::new();
    let mut reader = res.reader;
    reader.read_to_end(&mut decoded).expect("read final cpio");
    assert_eq!(decoded, cpio_data);
}

// MARK: - 3. Anti-DoS Recursion Depth Tests (MAX_FILTER_CHAIN_DEPTH = 25)

#[test]
fn test_max_filter_chain_depth_limit_boundary() {
    let mut payload = b"Deeply nested recursive payload for DoS limit boundary test.".to_vec();

    // 1. Nest exactly 25 levels of Gzip: should succeed
    for _ in 0..MAX_FILTER_CHAIN_DEPTH {
        payload = encode_gzip(&payload);
    }

    let res_25 = FilterPipeline::unwrap_stream(Cursor::new(payload.clone()));
    assert!(res_25.is_ok(), "25 levels of nested filters must succeed within limit");
    let unwrapped_25 = res_25.unwrap();
    assert_eq!(unwrapped_25.filters.len(), MAX_FILTER_CHAIN_DEPTH);

    // 2. Add a 26th level of Gzip: must trigger ErrTooManyFilters
    let payload_26 = encode_gzip(&payload);
    let res_26 = FilterPipeline::unwrap_stream(Cursor::new(payload_26));
    assert!(res_26.is_err(), "26 levels of nested filters must be intercepted");

    match res_26 {
        Err(FilterPipelineError::ErrTooManyFilters { depth, limit }) => {
            assert_eq!(depth, 26);
            assert_eq!(limit, 25);
        }
        other => panic!("expected ErrTooManyFilters, got {:?}", other),
    }
}
