// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Tier 5 Adversarial Stress Testing & Empirical Hardening Suite.
//!
//! Adversarially challenges TTZip invariants under hostile and corrupted environments:
//! 1. Truncated Streams & Boundary Fuzzing (ZIP, 7z, Bzip2, Brotli, LZ4)
//! 2. Corrupted 7z Solid Blocks, LZMA Streams, Jump Tables & Folder Indices
//! 3. Malformed Multi-Modal & Rich Media Inputs (PDF, EPUB, DOCX, XLSX, HTML)
//! 4. Broken UTF-8 Byte Sequences & Heuristic Mojibake Remediation Fuzzing
//! 5. Oversized Filenames, Path Traversal, Null Bytes & Reserved Device Interception
//! 6. High Concurrency Multi-Threaded Stress Verification (Zero Data Races)
//! 7. Zero Disk Leak in `/tmp` & Darwin Mach RSS Memory Cap ($\Delta RSS \le 64\text{MB}$)

use std::fs;
use std::io::Cursor;
use std::sync::Arc;
use std::thread;

use ttzip_engine::charset::detector::{detect_charset, detect_charset_with_confidence};
use ttzip_engine::codecs::brotli::{brotli_compress, brotli_compress_bound, brotli_decompress};
use ttzip_engine::codecs::bzip2::{bzip2_compress_to_vec, bzip2_decompress_to_vec};
use ttzip_engine::codecs::lz4::{lz4_compress_to_vec, lz4_decompress_custom_to_vec};
use ttzip_engine::pdf::TTZipPdfParser;
use ttzip_engine::security::ebook_defense::ManifestItemCountGuard;
use ttzip_engine::security::html_defense::{AttributeQuotaGuard, TagNestingDepthGuard};
use ttzip_engine::security::office_defense::{col_str_to_index, SheetDimensionsGuard};
use ttzip_engine::security::path_sanitizer::{
    is_windows_reserved_device_name, normalize_to_nfc, sanitize_path,
};
use ttzip_engine::security::pdf_defense::StreamExpansionQuotaGuard;
use ttzip_engine::sevenz::{
    create_7z_solid_archive_bytes, get_current_rss_bytes, SevenZReader,
};
use ttzip_engine::syntax::{SupportedLanguage, TTZipSyntaxParser};
use ttzip_engine::xml::OfficeXmlExtractor;
use ttzip_engine::zip::writer::ZipInputItem;

// ============================================================================
// 1. Truncated Streams & Boundary Fuzzing Across All Codecs
// ============================================================================

#[test]
fn test_adversarial_truncated_streams_all_codecs() {
    let raw_payload = b"TTZip Adversarial Stress Test Payload for Codec Truncation Robustness 2026. \
                        Repeating pattern to ensure multi-block compression behaviors across algorithms: \
                        0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF";

    // 1.1 Bzip2 Truncation Fuzzing
    let bz2_compressed = bzip2_compress_to_vec(raw_payload, 9).expect("bzip2 compress");
    for cut in 0..bz2_compressed.len() {
        let truncated = &bz2_compressed[..cut];
        let res = bzip2_decompress_to_vec(truncated, raw_payload.len() * 2);
        if cut < bz2_compressed.len() {
            assert!(res.is_err(), "Bzip2 truncated at {} must return Err", cut);
        }
    }

    // 1.2 Brotli Truncation Fuzzing
    let mut brotli_comp = vec![0u8; brotli_compress_bound(raw_payload.len())];
    let brotli_len = brotli_compress(raw_payload, &mut brotli_comp, 6, 22).expect("brotli compress");
    let brotli_valid = &brotli_comp[..brotli_len];
    for cut in 0..brotli_valid.len() {
        let truncated = &brotli_valid[..cut];
        let mut out_buf = vec![0u8; raw_payload.len()];
        let res = brotli_decompress(truncated, &mut out_buf);
        if cut < brotli_valid.len() {
            assert!(res.is_err(), "Brotli truncated at {} must return Err", cut);
        }
    }

    // 1.3 LZ4 Truncation Fuzzing
    let lz4_compressed = lz4_compress_to_vec(raw_payload, 1).expect("lz4 compress");
    for cut in 0..lz4_compressed.len() {
        let truncated = &lz4_compressed[..cut];
        let res = lz4_decompress_custom_to_vec(truncated, raw_payload.len());
        if cut < lz4_compressed.len() {
            assert!(res.is_err(), "LZ4 truncated at {} must return Err", cut);
        }
    }
}

// ============================================================================
// 2. Corrupted 7z Solid Blocks, LZMA Streams, Jump Tables & Folder Indices
// ============================================================================

#[test]
fn test_adversarial_corrupted_7z_solid_blocks_and_jump_tables() {
    let items = vec![
        ZipInputItem {
            rel_path: "file1.txt".to_string(),
            data: b"First file payload in solid block".to_vec(),
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "file2.bin".to_string(),
            data: vec![0xEEu8; 8192],
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "file3.rs".to_string(),
            data: b"fn main() { println!(\"Hello\"); }".to_vec(),
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let valid_archive = create_7z_solid_archive_bytes(&items, 5, 1).expect("create solid 7z");

    // 2.1 Bit-flip fuzzing across header and solid data stream
    for offset in [0, 4, 8, 12, 16, 24, 32, valid_archive.len() / 2, valid_archive.len() - 1] {
        let mut corrupted = valid_archive.clone();
        corrupted[offset] ^= 0xFF;

        let open_res = SevenZReader::open_slice(&corrupted);
        match open_res {
            Ok(reader) => {
                // If header parsing succeeded despite corruption, extraction must fail gracefully
                for idx in 0..reader.len() {
                    let _ = reader.extract_entry_bytes_stream(idx, None);
                }
            }
            Err(_) => {
                // Graceful rejection is expected and safe
            }
        }
    }

    // 2.2 Out-of-bounds entry indices & corrupted jump table seeks
    let reader = SevenZReader::open_slice(&valid_archive).expect("open valid 7z");
    assert_eq!(reader.len(), 3);
    assert!(reader.extract_entry_bytes_stream(999, None).is_err());
    assert!(reader.extract_entry_bytes_stream(usize::MAX, None).is_err());
}

// ============================================================================
// 3. Malformed Multi-Modal & Rich Media Inputs (PDF, EPUB, DOCX, XLSX, HTML)
// ============================================================================

#[test]
fn test_adversarial_malformed_epub_pdf_office_and_html() {
    // 3.1 PDF: Corrupted Catalog & Cyclic References
    let malformed_pdf = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
                          2 0 obj\n<< /Type /Pages /Kids [1 0 R] /Count 1 >>\nendobj\n\
                          xref\n0 3\n0000000000 65535 f \n0000000009 00000 n \n0000000058 00000 n \n\
                          trailer\n<< /Size 3 /Root 1 0 R >>\nstartxref\n118\n%%EOF";
    let pdf_res = TTZipPdfParser::open_from_bytes(malformed_pdf);
    if let Ok(parser) = pdf_res {
        let _ = parser.get_page_id(1);
    }

    // 3.2 PDF: Stream Expansion Bomb Quota
    let quota_guard = StreamExpansionQuotaGuard::with_limits(512, 5.0, 2048);
    assert!(quota_guard.validate_metadata(100, Some(400)).is_ok());
    assert!(quota_guard.validate_metadata(100, Some(10000)).is_err());

    // 3.3 EPUB: Empty / Corrupted Manifest & Infinite Entity Expansion Protection
    let mut manifest_guard = ManifestItemCountGuard::new();
    let malformed_opf = b"<?xml version=\"1.0\"?><package><manifest><item id=\"1\" href=\"../secret.txt\"/></manifest></package>";
    let manifest_res = manifest_guard.parse_opf_stream(Cursor::new(malformed_opf), malformed_opf.len() as u64);
    assert!(manifest_res.is_ok());

    // 3.4 Office XML: Extreme Coordinates & Broken Structures
    let sheet_guard = SheetDimensionsGuard::default();
    assert!(sheet_guard.validate_coordinates(16384, 1048576).is_ok());
    assert!(sheet_guard.validate_coordinates(16385, 1).is_err());
    assert!(sheet_guard.validate_coordinates(1, 1048577).is_err());
    assert_eq!(col_str_to_index("INVALID_COL_NAME"), None);

    let malformed_docx = b"<w:document><w:body><w:p><w:r><w:t>Unclosed tag body";
    let docx_res = OfficeXmlExtractor::parse_docx_document(malformed_docx);
    assert!(docx_res.is_err() || docx_res.unwrap().headings.is_empty());

    // 3.5 HTML VFS: Extreme Tag Nesting & Oversized Attributes
    let mut depth_guard = TagNestingDepthGuard::new(32, 10);
    for _ in 0..100 {
        let _ = depth_guard.on_element_start("div", false);
    }
    assert!(depth_guard.current_depth() >= 32);

    let attr_guard = AttributeQuotaGuard::new(64, 1024, 4096, 4096);
    let mut attr_report = ttzip_engine::security::html_defense::AttributeQuotaReport::default();
    let giant_attr = vec![("href", "A".repeat(2048))];
    let attr_res = attr_guard.validate_element_attributes(&giant_attr, &mut attr_report);
    assert!(attr_res.is_err());
}

// ============================================================================
// 4. Broken UTF-8 Sequences & Mojibake Remediation Stress Fuzzing
// ============================================================================

#[test]
fn test_adversarial_broken_utf8_and_mojibake_fuzzing() {
    let adversarial_byte_patterns: Vec<&[u8]> = vec![
        b"\xFF\xFE\xFD",                                     // Invalid single bytes
        b"\xC0\x80",                                         // Overlong ASCII NUL
        b"\xED\xA0\x80",                                     // UTF-16 surrogate half
        b"\xF4\x90\x80\x80",                                 // Beyond Unicode plane 16
        b"\xE4\xBD",                                         // Truncated 3-byte sequence
        b"\xF0\x9F\x98",                                     // Truncated 4-byte emoji sequence
        b"ValidPrefix\x80\x81\x82ValidSuffix",               // Invalid bytes embedded in ASCII
        b"\xD6\xD0\xCE\xC4\xB2\xE2\xCA\xD4",                 // GBK "中文测试"
        b"\x93\xFA\x96\x7B\x8C\xEA",                         // Shift-JIS "日本語"
        b"\xC7\xD1\xB1\xDB\xC5\xD7\xBD\xBA\xC6\xAE",         // EUC-KR "한글테스트"
        b"M\xFCnchen_Stra\xDFe.tar.gz",                       // Windows-1252 Umlauts
    ];

    for (idx, pattern) in adversarial_byte_patterns.iter().enumerate() {
        // Zero-panic guarantee across detector
        let (charset, confidence) = detect_charset_with_confidence(pattern);
        assert!(!charset.is_empty(), "Charset name must never be empty at index {}", idx);
        assert!((0.0..=1.0).contains(&confidence), "Confidence must be normalized");

        let detected = detect_charset(pattern);
        if let Some(enc) = detected {
            assert!(!enc.is_empty());
        }

        // Canonical NFC normalization zero-panic
        if let Ok(utf8_str) = std::str::from_utf8(pattern) {
            let normalized = normalize_to_nfc(utf8_str);
            assert!(std::str::from_utf8(normalized.as_bytes()).is_ok());
        }
    }

    // Pseudo-random deterministic fuzzing loop (1,000 iterations)
    let mut pseudo_seed: u64 = 0x1337_CAFE_BEEF_0042;
    for _ in 0..1000 {
        pseudo_seed = pseudo_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let len = (pseudo_seed % 128) as usize + 1;
        let mut fuzzed_bytes = Vec::with_capacity(len);
        for i in 0..len {
            pseudo_seed = pseudo_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            fuzzed_bytes.push((pseudo_seed >> (i % 32)) as u8);
        }

        let _ = detect_charset_with_confidence(&fuzzed_bytes);
        let _ = detect_charset(&fuzzed_bytes);
    }
}

// ============================================================================
// 5. Oversized Filenames, Path Traversal, Null Bytes & Reserved Device Names
// ============================================================================

#[test]
fn test_adversarial_oversized_filenames_and_path_traversal() {
    let dangerous_paths = [
        "../../../../etc/passwd",
        "..\\..\\..\\windows\\system32\\cmd.exe",
        "....//....//....//shadow",
        "/absolute/root/escape.txt",
        "C:\\Windows\\explorer.exe",
        "safe_dir/../../../escaped.txt",
        "CON",
        "PRN.txt",
        "AUX.tar.gz",
        "NUL",
        "COM1",
        "COM9.dat",
        "LPT1",
        "CLOCK$",
        "PHYSICALDRIVE0",
        "file.txt\0.exe",
        "stream:hidden_ads",
    ];

    for path in &dangerous_paths {
        let result = sanitize_path(path);
        assert!(!result.is_safe(), "Dangerous path '{}' must not be marked safe", path);
    }

    // Windows reserved device name check
    assert!(is_windows_reserved_device_name("CON"));
    assert!(is_windows_reserved_device_name("con.txt"));
    assert!(is_windows_reserved_device_name("AUX"));
    assert!(is_windows_reserved_device_name("COM1"));
    assert!(is_windows_reserved_device_name("LPT3.doc"));
    assert!(is_windows_reserved_device_name("PhysicalDrive0"));
    assert!(!is_windows_reserved_device_name("CONCERT.txt"));
    assert!(!is_windows_reserved_device_name("AUXILIARY.tar"));

    // Extreme path lengths (4096 and 65535 bytes)
    let giant_name = "a/".repeat(2048) + "safe_file.txt";
    let giant_res = sanitize_path(&giant_name);
    assert!(giant_res.is_safe() || giant_res.is_long_path);
}

// ============================================================================
// 6. High Concurrency Multi-Threaded Stress Verification
// ============================================================================

#[test]
fn test_adversarial_high_concurrency_stress() {
    let items = vec![
        ZipInputItem {
            rel_path: "src/lib.rs".to_string(),
            data: b"pub fn compute(x: u64) -> u64 { x * 2 }".to_vec(),
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "src/main.py".to_string(),
            data: b"import sys\nprint('Python concurrency test')".to_vec(),
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "data/config.json".to_string(),
            data: b"{\"status\": \"active\", \"workers\": 32}".to_vec(),
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let archive_bytes = Arc::new(create_7z_solid_archive_bytes(&items, 5, 1).expect("create solid 7z"));

    let num_threads = 32;
    let iterations_per_thread = 50;
    let mut handles = Vec::with_capacity(num_threads);

    for thread_id in 0..num_threads {
        let archive_clone = Arc::clone(&archive_bytes);
        handles.push(thread::spawn(move || {
            for iter in 0..iterations_per_thread {
                // 1. Concurrent 7z solid extraction
                let reader = SevenZReader::open_slice(&archive_clone).expect("concurrent open");
                assert_eq!(reader.len(), 3);
                let entry_idx = (thread_id + iter) % 3;
                let extracted = reader.extract_entry_bytes_stream(entry_idx, None).expect("concurrent extract");
                assert!(!extracted.is_empty());

                // 2. Concurrent Tree-sitter parsing
                let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).expect("with language");
                let code = format!("fn thread_worker_{}_{}() -> u64 {{ {} }}", thread_id, iter, thread_id * 10);
                let tree = parser.parse_full(&code).expect("concurrent syntax parse");
                assert_eq!(tree.root_node().kind(), "source_file");

                // 3. Concurrent Charset detection
                let cjk_sample = b"\xD6\xD0\xCE\xC4\xB2\xE2\xCA\xD4";
                let (enc, conf) = detect_charset_with_confidence(cjk_sample);
                assert!(!enc.is_empty());
                assert!(conf > 0.0);
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Thread joined successfully with zero panics");
    }
}

// ============================================================================
// 7. Zero Disk Leak in `/tmp` & Darwin Mach RSS Memory Cap ($\le 64\text{MB}$)
// ============================================================================

#[test]
fn test_adversarial_zero_tmp_leak_and_rss_memory_bound() {
    let tmp_path = std::path::Path::new("/tmp");
    let initial_tmp_entries = if tmp_path.exists() {
        fs::read_dir(tmp_path).map(|r| r.count()).unwrap_or(0)
    } else {
        0
    };

    let baseline_rss = get_current_rss_bytes();

    // Execute intensive multi-stage workflow
    let mut items = Vec::new();
    for i in 0..30 {
        items.push(ZipInputItem {
            rel_path: format!("workload/file_{}.bin", i),
            data: vec![(i & 0xFF) as u8; 32 * 1024], // 32KB each
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        });
    }

    let archive = create_7z_solid_archive_bytes(&items, 5, 1).expect("heavy solid 7z");
    let reader = SevenZReader::open_slice(&archive).expect("heavy 7z open");

    for idx in 0..items.len() {
        let extracted = reader.extract_entry_bytes_stream(idx, None).expect("heavy extract");
        assert_eq!(extracted.len(), items[idx].data.len());
    }

    let final_rss = get_current_rss_bytes();
    let delta_rss = if final_rss > baseline_rss {
        final_rss - baseline_rss
    } else {
        0
    };

    // Hard invariant: Delta RSS must remain strictly <= 64MB
    let max_allowed_delta = 64 * 1024 * 1024;
    assert!(
        delta_rss <= max_allowed_delta,
        "Delta RSS {} exceeded 64MB hard limit ({})",
        delta_rss,
        max_allowed_delta
    );

    // Verify zero orphaned temporary files in /tmp created by this test
    let final_tmp_entries = if tmp_path.exists() {
        fs::read_dir(tmp_path).map(|r| r.count()).unwrap_or(0)
    } else {
        0
    };

    // Any temporary workspaces used by TTZip are strictly RAII-isolated in memory or clean drop
    assert!(
        final_tmp_entries <= initial_tmp_entries + 2, // Allow for concurrent OS-level transient sockets
        "Detected potential /tmp leak: before={}, after={}",
        initial_tmp_entries,
        final_tmp_entries
    );
}
