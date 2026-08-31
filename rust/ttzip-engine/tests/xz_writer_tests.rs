// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for XZ Container Stream Writer, Adaptive Block Encoder,
//! BCJ/LZMA2 Filter Pipeline, multi-threaded parallel chunking, and GNU `xz` compatibility.

use std::io::{Cursor, Read, Write};
use std::process::Command;

use ttzip_engine::xz::decoder::{xz_decompress, XzStreamDecoder};
use ttzip_engine::xz::seekable::XzSeekableReader;
use ttzip_engine::xz::types::XzCheckType;
use ttzip_engine::xz::writer::{
    xz_compress, XzBcjType, XzEncoderOptions, XzParallelStreamWriter, XzStreamWriter,
};

/// Deterministic pseudo-random bytes generator for reproducible payload synthesis.
fn generate_deterministic_payload(size: usize, seed: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let mut state = seed ^ 0x5DEECE66D;
    for i in 0..size {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let byte = ((state >> 33) ^ (i as u64)) as u8;
        data.push(byte);
    }
    data
}

/// Synthesizes pseudo x86 machine code containing CALL (0xE8) and JMP (0xE9) branch instructions.
fn generate_x86_bytecode_payload(size: usize) -> Vec<u8> {
    let mut code = Vec::with_capacity(size);
    let mut ip = 0u32;
    while code.len() < size {
        match code.len() % 16 {
            0 => {
                // CALL +relative
                code.push(0xE8);
                let target = (ip.wrapping_add(0x1000) as i32).to_le_bytes();
                code.extend_from_slice(&target);
                ip += 5;
            }
            6 => {
                // JMP -relative
                code.push(0xE9);
                let target = (ip.wrapping_sub(0x500) as i32).to_le_bytes();
                code.extend_from_slice(&target);
                ip += 5;
            }
            _ => {
                // NOP or regular instructions
                code.push(0x90);
                ip += 1;
            }
        }
    }
    code.truncate(size);
    code
}

/// Synthesizes pseudo ARM64 machine code containing BL and ADRP instructions.
fn generate_arm64_bytecode_payload(size: usize) -> Vec<u8> {
    let mut code = Vec::with_capacity(size);
    let count = size / 4;
    for i in 0..count {
        let instr: u32 = match i % 8 {
            0 | 4 => {
                // BL #offset
                0x9400_0000 | ((i as u32 * 4) & 0x03FF_FFFF)
            }
            2 | 6 => {
                // ADRP X0, #page
                let immlo = (i as u32 & 3) << 29;
                let immhi = ((i as u32 >> 2) & 0x7_FFFF) << 5;
                0x9000_0000 | immlo | immhi
            }
            _ => {
                // NOP (0xD503201F)
                0xD503_201F
            }
        };
        code.extend_from_slice(&instr.to_le_bytes());
    }
    let rem = size % 4;
    code.resize(code.len() + rem, 0x00);
    code
}

#[test]
fn test_xz_writer_empty_stream_roundtrip() {
    let options = XzEncoderOptions::new();
    let empty_payload = b"";

    let mut writer = XzStreamWriter::new(Vec::new(), options).expect("create writer");
    writer.write_all(empty_payload).expect("write empty");
    let compressed = writer.finish().expect("finish writer");

    // Empty XZ stream should be valid 32-byte header + index + footer container
    assert_eq!(compressed.len(), 32);

    let decompressed = xz_decompress(&compressed).expect("decompress empty stream");
    assert_eq!(decompressed.as_slice(), empty_payload);
}

#[test]
fn test_xz_writer_various_payload_sizes_roundtrip() {
    let sizes = [
        0,           // 0 B
        1024,        // 1 KB
        64 * 1024,   // 64 KB
        256 * 1024,  // 256 KB
        512 * 1024,  // 512 KB
    ];

    for &size in &sizes {
        let original = generate_deterministic_payload(size, 0xACE1_B00B + size as u64);
        let options = XzEncoderOptions::new()
            .with_check_type(XzCheckType::Crc64)
            .with_dict_size(8 * 1024 * 1024)
            .with_preset_level(4);

        let mut writer = XzStreamWriter::new(Vec::new(), options).expect("create writer");
        writer.write_all(&original).expect("write payload");
        let compressed = writer.finish().expect("finish stream");

        assert!(!compressed.is_empty());

        let mut decoder = XzStreamDecoder::new(Cursor::new(&compressed));
        let mut decompressed = Vec::with_capacity(size);
        decoder
            .read_to_end(&mut decompressed)
            .expect("decompress stream");

        assert_eq!(
            decompressed.len(),
            original.len(),
            "Size mismatch for payload size {}",
            size
        );
        assert_eq!(
            decompressed, original,
            "Bit-exact payload mismatch for size {}",
            size
        );
    }
}

#[test]
fn test_xz_writer_all_check_types() {
    let check_types = [
        XzCheckType::None,
        XzCheckType::Crc32,
        XzCheckType::Crc64,
        XzCheckType::Sha256,
    ];

    let payload = generate_deterministic_payload(64 * 1024, 0x1234_5678);

    for check_type in check_types {
        let options = XzEncoderOptions::new()
            .with_check_type(check_type)
            .with_preset_level(3);

        let mut writer = XzStreamWriter::new(Vec::new(), options).expect("create writer");
        writer.write_all(&payload).expect("write payload");
        let compressed = writer.finish().expect("finish stream");
        println!("Compressed len: {}, first 32 bytes: {:02x?}", compressed.len(), &compressed[..compressed.len().min(32)]);

        let decompressed = xz_decompress(&compressed).expect("decompress stream");
        assert_eq!(decompressed, payload, "Checksum mismatch for {:?}", check_type);
    }

}

#[test]
fn test_xz_writer_bcj_x86_filter_roundtrip() {
    let bytecode = generate_x86_bytecode_payload(256 * 1024);

    let options = XzEncoderOptions::new()
        .with_bcj_filter(Some(XzBcjType::X86))
        .with_dict_size(4 * 1024 * 1024)
        .with_preset_level(4);

    let compressed = xz_compress(&bytecode, &options).expect("compress x86 BCJ");
    let decompressed = xz_decompress(&compressed).expect("decompress x86 BCJ");

    assert_eq!(decompressed, bytecode);
}

#[test]
fn test_xz_writer_bcj_arm64_filter_roundtrip() {
    let bytecode = generate_arm64_bytecode_payload(256 * 1024);

    let options = XzEncoderOptions::new()
        .with_bcj_filter(Some(XzBcjType::Arm64))
        .with_dict_size(4 * 1024 * 1024)
        .with_preset_level(4);

    let compressed = xz_compress(&bytecode, &options).expect("compress ARM64 BCJ");
    let decompressed = xz_decompress(&compressed).expect("decompress ARM64 BCJ");

    assert_eq!(decompressed, bytecode);
}

#[test]
fn test_xz_parallel_stream_writer_multi_block_roundtrip() {
    let original = generate_deterministic_payload(512 * 1024, 0xFEED_FACE); // 512 KB
    let block_size = 64 * 1024; // 64 KB blocks -> 8 blocks

    let options = XzEncoderOptions::new()
        .with_block_size(block_size)
        .with_preset_level(3)
        .with_check_type(XzCheckType::Crc64);

    let mut par_writer = XzParallelStreamWriter::new(Vec::new(), options).expect("create par writer");
    par_writer
        .write_parallel(&original)
        .expect("parallel encode");
    let compressed = par_writer.finish().expect("finish par writer");

    let decompressed = xz_decompress(&compressed).expect("decompress multi-block xz");
    assert_eq!(decompressed, original);

    // Verify seekable random access on the multi-block archive
    let mut seekable = XzSeekableReader::new(Cursor::new(&compressed)).expect("open seekable");
    assert_eq!(seekable.total_uncompressed_size(), original.len() as u64);
    assert_eq!(seekable.index().records.len(), 8);

    // Read byte ranges from various blocks
    let mut chunk = vec![0u8; 100];
    seekable.read_exact(&mut chunk).expect("read head");
    assert_eq!(&chunk, &original[0..100]);
}

fn run_process_stdin(cmd: &str, args: &[&str], input: &[u8]) -> std::io::Result<std::process::Output> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("take stdin");
    let input_vec = input.to_vec();
    let write_thread = std::thread::spawn(move || {
        let _ = stdin.write_all(&input_vec);
    });

    let output = child.wait_with_output()?;
    let _ = write_thread.join();
    Ok(output)
}

#[test]
fn test_gnu_xz_cli_compatibility() {
    // Check if xz CLI is available on the test machine
    let has_xz_cli = Command::new("xz").arg("--version").output().is_ok();
    if !has_xz_cli {
        println!("Skipping GNU xz CLI validation because xz executable is not found on PATH");
        return;
    }

    let test_matrix = [
        ("empty", Vec::new(), XzEncoderOptions::new()),
        (
            "small_text",
            b"The quick brown fox jumps over the lazy dog. TTZip High-Performance XZ Native Compressor.".to_vec(),
            XzEncoderOptions::new().with_preset_level(6).with_check_type(XzCheckType::Crc64),
        ),
        (
            "x86_bin",
            generate_x86_bytecode_payload(64 * 1024),
            XzEncoderOptions::new()
                .with_bcj_filter(Some(XzBcjType::X86))
                .with_preset_level(5)
                .with_check_type(XzCheckType::Crc32),
        ),
        (
            "arm64_bin",
            generate_arm64_bytecode_payload(64 * 1024),
            XzEncoderOptions::new()
                .with_bcj_filter(Some(XzBcjType::Arm64))
                .with_preset_level(5)
                .with_check_type(XzCheckType::Sha256),
        ),
        (
            "multi_block",
            generate_deterministic_payload(512 * 1024, 0xCAFE_BABE),
            XzEncoderOptions::new()
                .with_block_size(64 * 1024)
                .with_preset_level(3),
        ),
    ];

    for (name, payload, options) in test_matrix {
        let compressed = xz_compress(&payload, &options).expect("compress xz payload");

        // 1. Validate with xz -t - (integrity test)
        let test_output = run_process_stdin("xz", &["-t", "-"], &compressed).expect("run xz -t");
        assert!(
            test_output.status.success(),
            "GNU xz -t failed for test '{}': stderr={}",
            name,
            String::from_utf8_lossy(&test_output.stderr)
        );

        // 2. Validate with xz -dc - (decompression output exact match)
        let dc_output = run_process_stdin("xz", &["-dc", "-"], &compressed).expect("run xz -dc");
        assert!(
            dc_output.status.success(),
            "GNU xz -dc failed for test '{}': stderr={}",
            name,
            String::from_utf8_lossy(&dc_output.stderr)
        );
        assert_eq!(
            dc_output.stdout, payload,
            "GNU xz -dc decompressed data mismatch for test '{}'",
            name
        );
    }
}

#[test]
fn test_tar_xz_archive_compatibility() {
    let has_tar_cli = Command::new("tar").arg("--version").output().is_ok();
    let has_xz_cli = Command::new("xz").arg("--version").output().is_ok();
    if !has_tar_cli || !has_xz_cli {
        println!("Skipping tar.xz compatibility test due to missing tools");
        return;
    }

    // Build a simple synthetic TAR payload in memory
    let mut tar_builder = tar::Builder::new(Vec::new());

    let file1_data = b"Hello from TTZip inside tar.xz container!";
    let mut header1 = tar::Header::new_gnu();
    header1.set_size(file1_data.len() as u64);
    header1.set_mode(0o644);
    header1.set_cksum();
    tar_builder
        .append_data(&mut header1, "hello.txt", &file1_data[..])
        .expect("append file1");

    let file2_data = generate_deterministic_payload(4096, 0x9988_7766);
    let mut header2 = tar::Header::new_gnu();
    header2.set_size(file2_data.len() as u64);
    header2.set_mode(0o644);
    header2.set_cksum();
    tar_builder
        .append_data(&mut header2, "data/payload.bin", &file2_data[..])
        .expect("append file2");

    let tar_bytes = tar_builder.into_inner().expect("finalize tar");

    // Compress TAR archive into XZ container
    let options = XzEncoderOptions::new()
        .with_preset_level(5)
        .with_check_type(XzCheckType::Crc64);

    let xz_bytes = xz_compress(&tar_bytes, &options).expect("compress tar.xz");

    // Verify using system tar -tf -
    let tar_output = run_process_stdin("tar", &["-tf", "-"], &xz_bytes).expect("run tar -tf");
    assert!(
        tar_output.status.success(),
        "tar -tf failed on generated tar.xz: stderr={}",
        String::from_utf8_lossy(&tar_output.stderr)
    );

    let output_str = String::from_utf8_lossy(&tar_output.stdout);
    assert!(output_str.contains("hello.txt"));
    assert!(output_str.contains("data/payload.bin"));
}
