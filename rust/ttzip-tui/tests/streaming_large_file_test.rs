// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use tempfile::NamedTempFile;
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_tui::preview::{
    format_hex_dump, format_text_preview, generate_preview, generate_preview_auto,
    is_text_content, PreviewData, SyntaxHighlighter, MAX_PREVIEW_BYTES,
    MAX_RESIDENT_MEMORY_LIMIT,
};

#[test]
fn test_sparse_large_file_bounded_memory_streaming_and_tui_preview() {
    // 1. Create a 1GB sparse file to verify streaming memory consumption without disk fill
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    let file_handle = file.as_file_mut();

    // Seek to 1GB and write 1 byte
    let target_size: u64 = 1024 * 1024 * 1024; // 1 GB
    file_handle.seek(SeekFrom::Start(target_size - 1)).expect("Failed to seek");
    file_handle.write_all(&[0x42]).expect("Failed to write byte");
    file_handle.flush().expect("Failed to flush");

    // 2. Reopen and stream hash using 128KB buffer
    let read_file = File::open(file.path()).expect("Failed to open file");
    let mut reader = BufReader::with_capacity(128 * 1024, read_file);
    let mut buffer = [0u8; 128 * 1024];
    let mut crc = 0u32;
    let mut total_read = 0u64;
    let mut first_chunk = Vec::new();

    while let Ok(n) = reader.read(&mut buffer) {
        if n == 0 {
            break;
        }
        if first_chunk.is_empty() {
            first_chunk.extend_from_slice(&buffer[..n]);
        }
        crc = crc32_fast(crc, &buffer[..n]);
        total_read += n as u64;
    }

    assert_eq!(total_read, target_size);
    assert_ne!(crc, 0);

    // 3. Test ttzip_tui streaming preview components on large stream chunk
    let highlighter = SyntaxHighlighter::new();
    assert_eq!(MAX_PREVIEW_BYTES, 64 * 1024);
    assert_eq!(MAX_RESIDENT_MEMORY_LIMIT, 16 * 1024 * 1024);

    // HexDump preview on binary/sparse chunk with 64KB truncation clamp
    let binary_preview = generate_preview("sparse_disk.iso", &first_chunk, target_size, &highlighter);
    match binary_preview {
        PreviewData::HexDump {
            offset_hex_pairs,
            total_bytes_displayed,
        } => {
            assert_eq!(total_bytes_displayed, MAX_PREVIEW_BYTES);
            assert_eq!(offset_hex_pairs.len(), MAX_PREVIEW_BYTES / 16);
            assert_eq!(offset_hex_pairs[0].0, "00000000");
        }
        other => panic!("Expected PreviewData::HexDump for sparse binary chunk, got {:?}", other),
    }

    // Text streaming preview on large text file payload
    let sample_line = "fn ttzip_kernel_stream_eval() -> bool { true }\n";
    let large_text = sample_line.repeat(2500).into_bytes(); // ~117 KB text
    assert!(large_text.len() > MAX_PREVIEW_BYTES);
    assert!(is_text_content(&large_text));

    let text_preview = generate_preview("engine_core.rs", &large_text, large_text.len() as u64, &highlighter);
    match text_preview {
        PreviewData::Text {
            lines,
            syntax_language,
            is_truncated,
        } => {
            assert_eq!(syntax_language, "Rust");
            assert!(is_truncated, "117KB payload must be marked truncated at 64KB boundary");
            assert!(!lines.is_empty());
        }
        other => panic!("Expected PreviewData::Text for Rust source, got {:?}", other),
    }

    // Direct auto-highlighter convenience preview
    let auto_preview = generate_preview_auto("config.json", b"{\"kernel\": \"ttzip\", \"threads\": 8}", 33);
    match auto_preview {
        PreviewData::Text {
            lines,
            syntax_language,
            is_truncated,
        } => {
            assert_eq!(syntax_language, "JSON");
            assert!(!is_truncated);
            assert_eq!(lines.len(), 1);
        }
        other => panic!("Expected PreviewData::Text for JSON, got {:?}", other),
    }

    // Direct format_hex_dump and format_text_preview verification
    let direct_hex = format_hex_dump(&first_chunk);
    match direct_hex {
        PreviewData::HexDump { total_bytes_displayed, .. } => {
            assert_eq!(total_bytes_displayed, MAX_PREVIEW_BYTES);
        }
        other => panic!("Expected HexDump from format_hex_dump, got {:?}", other),
    }

    let direct_text = format_text_preview("main.rs", &large_text, target_size, &highlighter);
    match direct_text {
        PreviewData::Text { is_truncated, syntax_language, .. } => {
            assert!(is_truncated);
            assert_eq!(syntax_language, "Rust");
        }
        other => panic!("Expected Text from format_text_preview, got {:?}", other),
    }
}
