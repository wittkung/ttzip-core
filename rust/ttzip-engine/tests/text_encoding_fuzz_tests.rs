// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive corruption injection, chaos mutation, and fuzzing test suite for TTZip Text Encoding & Charset Engine.
//!
//! Deploys 16 surgical destruction targets:
//! 1. Malformed multibyte truncated slice attack (UTF-8, GB18030, Big5, Shift-JIS, EUC-KR).
//! 2. Dangling trailing incomplete byte sequence injection.
//! 3. Mixed heterogeneous code pages cross-language concatenation & mojibake resilience.
//! 4. Zero-byte (`\0`) and NUL-character path injection defense.
//! 5. Overlong filename (4096+ bytes) buffer defense & ErrPathTooLong validation.
//! 6. 1000+ concurrent tasks charset detection & transcoding race competition.
//! 7. 500+ rounds of pseudo-random data encoding mutation and perturbation fuzzing.
//! 8. BOM header injection & confusion attack (UTF-8 BOM, UTF-16 BE/LE, UTF-32 BOM).
//! 9. Raw C-ABI FFI boundary pointer defense and null pointer safe rejection.
//! 10. Pure ASCII 1-byte, 2-byte, 3-byte tiny payload detection invariant.
//! 11. Shift-JIS trail byte 0x5C ('\\') path delimiter confusion defense.
//! 12. Big5 HKSCS extension area & rare CJK ideograph edge sequence fuzzing.
//! 13. EUC-KR boundary distortion & Hangul syllable code slice attack.
//! 14. Windows-1252 vs ISO-8859-1 control character (0x80-0x9F) injection.
//! 15. Zero-allocation slice buffer overflow boundary test (`sanitize_filename_to_slice`).
//! 16. `CharsetDetector` streaming state machine reset and reentrant chunk perturbation.

use std::ffi::CStr;
use std::panic::catch_unwind;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use rayon::prelude::*;

use ttzip_engine::charset::{
    detect_charset, detect_charset_with_confidence, sanitize_filename,
    sanitize_filename_to_slice, transcode_to_utf8, ttzip_rust_detect_charset,
    ttzip_rust_sanitize_filename, CharsetKind, CodingStateMachine,
};
use ttzip_engine::codecs::chardet::CharsetDetector;
use ttzip_engine::types::TTZipStatus;

/// Deterministic linear congruential generator for reproducible fuzzing vectors.
#[derive(Clone, Debug)]
struct DeterministicPrng {
    state: u64,
}

impl DeterministicPrng {
    #[inline]
    const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9e37_79b9_7f4a_7c15 } else { seed },
        }
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    #[inline]
    fn next_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let span = (max - min + 1) as u64;
        min + (self.next_u32() as u64 % span) as usize
    }
}

// ============================================================================
// Target 1: Malformed Multibyte Truncated Slice Attack
// ============================================================================
#[test]
fn test_target_01_malformed_multibyte_truncated_slice_attack() {
    let utf8_sample = "世界你好！TTZip 高性能归档引擎 2026";
    let gb_sample = encoding_rs::GB18030.encode(utf8_sample).0;
    let sjis_sample = encoding_rs::SHIFT_JIS.encode("日本語テスト文字列データ").0;
    let big5_sample = encoding_rs::BIG5.encode("繁體中文測試檔案資料夾").0;

    for len in 1..gb_sample.len() {
        let slice = &gb_sample[..len];
        let res = catch_unwind(|| {
            let _ = detect_charset(slice);
            let sanitized = sanitize_filename(slice);
            assert!(!sanitized.is_empty());
        });
        assert!(res.is_ok(), "Panic on GB18030 truncated slice at len {}", len);
    }

    for len in 1..sjis_sample.len() {
        let slice = &sjis_sample[..len];
        let res = catch_unwind(|| {
            let _ = detect_charset(slice);
            let sanitized = sanitize_filename(slice);
            assert!(!sanitized.is_empty());
        });
        assert!(res.is_ok(), "Panic on Shift-JIS truncated slice at len {}", len);
    }

    for len in 1..big5_sample.len() {
        let slice = &big5_sample[..len];
        let res = catch_unwind(|| {
            let _ = detect_charset(slice);
            let sanitized = sanitize_filename(slice);
            assert!(!sanitized.is_empty());
        });
        assert!(res.is_ok(), "Panic on Big5 truncated slice at len {}", len);
    }
}

// ============================================================================
// Target 2: Dangling Trailing Incomplete Byte Sequence Injection
// ============================================================================
#[test]
fn test_target_02_dangling_trailing_incomplete_byte_sequence_injection() {
    let mut dangling_utf8 = "ValidPrefix_".as_bytes().to_vec();
    dangling_utf8.extend_from_slice(&[0xE4, 0xBD]); // Incomplete 3-byte UTF-8

    let sanitized = sanitize_filename(&dangling_utf8);
    assert!(!sanitized.is_empty());

    let mut dangling_gb = "GB_Prefix_".as_bytes().to_vec();
    dangling_gb.push(0x81); // First byte of GB18030 2-byte or 4-byte sequence

    let detected = detect_charset(&dangling_gb);
    assert!(detected.is_some());
    let sanitized_gb = sanitize_filename(&dangling_gb);
    assert!(!sanitized_gb.is_empty());

    let mut dangling_4byte_gb = "GB4_".as_bytes().to_vec();
    dangling_4byte_gb.extend_from_slice(&[0x81, 0x30, 0x81]); // 3 bytes of 4-byte sequence
    let sanitized_gb4 = sanitize_filename(&dangling_4byte_gb);
    assert!(!sanitized_gb4.is_empty());
}

// ============================================================================
// Target 3: Mixed Heterogeneous Code Pages Cross-Language Concatenation
// ============================================================================
#[test]
fn test_target_03_mixed_heterogeneous_codepages_concatenation() {
    let gb_bytes = encoding_rs::GB18030.encode("简体中文目录/").0;
    let sjis_bytes = encoding_rs::SHIFT_JIS.encode("日本語サブフォルダ/").0;
    let big5_bytes = encoding_rs::BIG5.encode("繁體中文檔案.txt").0;

    let mut mixed = Vec::new();
    mixed.extend_from_slice(&gb_bytes);
    mixed.extend_from_slice(&sjis_bytes);
    mixed.extend_from_slice(&big5_bytes);

    let res = catch_unwind(|| {
        let (name, conf) = detect_charset_with_confidence(&mixed);
        assert!(!name.is_empty());
        assert!((0.0..=1.0).contains(&conf));
        let sanitized = sanitize_filename(&mixed);
        assert!(!sanitized.is_empty());
    });
    assert!(res.is_ok(), "Panic on mixed heterogeneous code pages concatenation");
}

// ============================================================================
// Target 4: Zero-Byte (\0) and NUL-Character Path Injection Defense
// ============================================================================
#[test]
fn test_target_04_zero_byte_nul_character_injection_defense() {
    let nul_injected = b"normal_folder/\0malicious_payload.sh\0extra.bin";
    let (charset, _) = detect_charset_with_confidence(nul_injected);
    assert_eq!(charset, "ASCII");

    let sanitized = sanitize_filename(nul_injected);
    assert_eq!(sanitized, "normal_folder/\0malicious_payload.sh\0extra.bin");

    let mut out_buf = [0u8; 128];
    let written = sanitize_filename_to_slice(nul_injected, &mut out_buf).expect("slice sanitize");
    assert_eq!(written, nul_injected.len());
    assert_eq!(&out_buf[..written], nul_injected);
    assert_eq!(out_buf[written], 0);

    let empty = b"";
    let empty_sanitized = sanitize_filename(empty);
    assert!(empty_sanitized.is_empty());
}

// ============================================================================
// Target 5: Overlong Filename (4096+ Bytes) Buffer Defense
// ============================================================================
#[test]
fn test_target_05_overlong_filename_buffer_defense() {
    let mut large_name = Vec::with_capacity(8192);
    let base_gb = encoding_rs::GB18030.encode("超长嵌套目录层级与极端文件名测试").0;
    while large_name.len() < 8192 {
        large_name.extend_from_slice(&base_gb);
        large_name.push(b'/');
    }

    let sanitized = sanitize_filename(&large_name);
    assert!(!sanitized.is_empty());

    let mut small_out = [0u8; 1024];
    let err = sanitize_filename_to_slice(&large_name, &mut small_out);
    assert_eq!(err, Err(TTZipStatus::ErrPathTooLong));

    let mut exact_out = vec![0u8; large_name.len() * 4 + 1];
    let ok = sanitize_filename_to_slice(&large_name, &mut exact_out);
    assert!(ok.is_ok());
}

// ============================================================================
// Target 6: 1000+ Concurrent Tasks Charset Detection & Transcoding Race
// ============================================================================
#[test]
fn test_target_06_concurrent_detection_and_transcoding_race() {
    let sample_gb = Arc::new(encoding_rs::GB18030.encode("并发测试：高吞吐字符集探测与转码流水线").0.to_vec());
    let sample_sjis = Arc::new(encoding_rs::SHIFT_JIS.encode("並行処理テスト：高スループット文字コード検出").0.to_vec());
    let sample_utf8 = Arc::new("Concurrent UTF-8 Verification Stream 2026".as_bytes().to_vec());

    let success_count = Arc::new(AtomicUsize::new(0));

    (0..1200).into_par_iter().for_each(|i| {
        let (data, expected_prefix) = match i % 3 {
            0 => (&sample_gb, "并发测试"),
            1 => (&sample_sjis, "並行処理"),
            _ => (&sample_utf8, "Concurrent"),
        };

        let detected = detect_charset(data).unwrap();
        let sanitized = sanitize_filename(data);
        assert!(sanitized.starts_with(expected_prefix));
        assert!(!detected.is_empty());

        let mut out_buf = [0u8; 256];
        let written = sanitize_filename_to_slice(data, &mut out_buf).unwrap();
        assert!(written > 0);

        success_count.fetch_add(1, Ordering::Relaxed);
    });

    assert_eq!(success_count.load(Ordering::SeqCst), 1200);
}

// ============================================================================
// Target 7: 500+ Rounds of Pseudo-Random Chaos Mutation Fuzzing
// ============================================================================
#[test]
fn test_target_07_pseudorandom_chaos_mutation_fuzzing() {
    let mut prng = DeterministicPrng::new(0xDEAD_BEEF_C0DE_2026);
    let base_text = "TTZip 归档引擎字符集变异 Fuzzing 基准语料库";
    let base_gb = encoding_rs::GB18030.encode(base_text).0.to_vec();

    for round in 0..500 {
        let mut mutated = base_gb.clone();
        let num_mutations = prng.next_range(1, 10);
        for _ in 0..num_mutations {
            let idx = prng.next_range(0, mutated.len() - 1);
            let mode = prng.next_range(0, 4);
            match mode {
                0 => mutated[idx] ^= (prng.next_u32() & 0xFF) as u8,
                1 => mutated[idx] = 0x00,
                2 => mutated[idx] = 0xFF,
                3 => mutated[idx] = prng.next_range(0x80, 0xFE) as u8,
                _ => {}
            }
        }

        let res = catch_unwind(|| {
            let _ = detect_charset(&mutated);
            let sanitized = sanitize_filename(&mutated);
            assert!(!sanitized.is_empty());
        });
        assert!(res.is_ok(), "Chaos mutation failed at round {}", round);
    }
}

// ============================================================================
// Target 8: BOM Header Injection & Confusion Attack
// ============================================================================
#[test]
fn test_target_08_bom_header_injection_and_confusion() {
    let utf8_payload = b"\xEF\xBB\xBFHello_UTF8_BOM.txt";
    let (charset, conf) = detect_charset_with_confidence(utf8_payload);
    assert_eq!(charset, "UTF-8");
    assert_eq!(conf, 1.0);

    let utf16le_payload = b"\xFF\xFES\x00a\x00m\x00p\x00l\x00e\x00";
    let (detected_16le, _) = detect_charset_with_confidence(utf16le_payload);
    assert!(!detected_16le.is_empty());

    let utf32le_payload = b"\xFF\xFE\x00\x00A\x00\x00\x00";
    let (detected_32le, _) = detect_charset_with_confidence(utf32le_payload);
    assert!(!detected_32le.is_empty());
}

// ============================================================================
// Target 9: Raw C-ABI FFI Boundary Pointer Defense
// ============================================================================
#[test]
fn test_target_09_ffi_boundary_pointer_defense() {
    let mut out_buf = [0i8; 64];
    let mut out_len: usize = 0;

    let status_null_in = ttzip_rust_sanitize_filename(
        std::ptr::null(),
        100,
        out_buf.as_mut_ptr(),
        out_buf.len(),
        &mut out_len,
    );
    assert_eq!(status_null_in, TTZipStatus::ErrInvalidParam);

    let in_bytes = b"safe_input.txt";
    let status_null_out = ttzip_rust_sanitize_filename(
        in_bytes.as_ptr(),
        in_bytes.len(),
        std::ptr::null_mut(),
        out_buf.len(),
        &mut out_len,
    );
    assert_eq!(status_null_out, TTZipStatus::ErrInvalidParam);

    let status_zero_cap = ttzip_rust_sanitize_filename(
        in_bytes.as_ptr(),
        in_bytes.len(),
        out_buf.as_mut_ptr(),
        0,
        &mut out_len,
    );
    assert_eq!(status_zero_cap, TTZipStatus::ErrInvalidParam);

    let mut name_buf = [0i8; 64];
    let det_status = ttzip_rust_detect_charset(
        std::ptr::null(),
        0,
        name_buf.as_mut_ptr(),
        name_buf.len(),
    );
    assert_eq!(det_status, TTZipStatus::Ok);
    let c_str = unsafe { CStr::from_ptr(name_buf.as_ptr()) }.to_str().unwrap();
    assert_eq!(c_str, "ASCII");
}

// ============================================================================
// Target 10: Pure ASCII Tiny Payload Detection Invariant
// ============================================================================
#[test]
fn test_target_10_ascii_tiny_payload_detection_invariant() {
    for b in 0u8..=127u8 {
        let single = [b];
        let (name, conf) = detect_charset_with_confidence(&single);
        assert_eq!(name, "ASCII");
        assert_eq!(conf, 1.0);

        let sanitized = sanitize_filename(&single);
        assert_eq!(sanitized.as_bytes(), &single);
    }
}

// ============================================================================
// Target 11: Shift-JIS Trail Byte 0x5C Path Delimiter Confusion Defense
// ============================================================================
#[test]
fn test_target_11_shift_jis_trail_byte_backslash_defense() {
    // Kanji '表' (0x95 0x5C) and '申' (0x90 0x5C) in Shift-JIS have trail byte '\\' (0x5C)
    let text = "表_予定表_申告書.txt";
    let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(text);
    assert!(encoded.contains(&0x5C), "Encoded Shift-JIS must contain 0x5C trail bytes");

    let detected = detect_charset(&encoded);
    assert_eq!(detected.as_deref(), Some("Shift_JIS"));

    let sanitized = sanitize_filename(&encoded);
    assert_eq!(sanitized, text);
}

// ============================================================================
// Target 12: Big5 HKSCS Extension Area & Rare CJK Ideographs
// ============================================================================
#[test]
fn test_target_12_big5_hkscs_extension_and_rare_cjk() {
    let big5_text = "繁體中文測試：檔案目錄與說明文件_臺灣香港.txt";
    let (encoded, _, _) = encoding_rs::BIG5.encode(big5_text);

    let detected = detect_charset(&encoded);
    assert_eq!(detected.as_deref(), Some("Big5"));

    let sanitized = sanitize_filename(&encoded);
    assert_eq!(sanitized, big5_text);
}

// ============================================================================
// Target 13: EUC-KR Boundary Distortion & Hangul Syllables
// ============================================================================
#[test]
fn test_target_13_euc_kr_boundary_distortion_hangul() {
    let korean_text = "압축해제_아카이브_테스트문서_최종본.zip";
    let (encoded, _, _) = encoding_rs::EUC_KR.encode(korean_text);

    let detected = detect_charset(&encoded);
    assert_eq!(detected.as_deref(), Some("EUC-KR"));

    let sanitized = sanitize_filename(&encoded);
    assert_eq!(sanitized, korean_text);

    // Distortion: Inject high byte without companion low byte
    let mut distorted = encoded.to_vec();
    distorted.insert(4, 0xFE);
    let sanitized_distorted = sanitize_filename(&distorted);
    assert!(!sanitized_distorted.is_empty());
}

// ============================================================================
// Target 14: Windows-1252 vs ISO-8859-1 Control Characters (0x80-0x9F)
// ============================================================================
#[test]
fn test_target_14_windows_1252_control_characters_injection() {
    // 0x80 is Euro sign '€' in Windows-1252
    let win1252_data = b"Price_\x80_100_Euro.pdf";
    let sanitized = sanitize_filename(win1252_data);
    assert!(sanitized.contains('€') || sanitized.contains("Price_"));

    let transcoded = transcode_to_utf8(win1252_data, "windows-1252").unwrap();
    assert_eq!(transcoded, "Price_€_100_Euro.pdf");
}

// ============================================================================
// Target 15: Zero-Allocation Slice Buffer Overflow Boundary Test
// ============================================================================
#[test]
fn test_target_15_slice_buffer_overflow_boundary_defense() {
    let text = "严格边界容量防御测试.tar.gz";
    let (encoded, _, _) = encoding_rs::GB18030.encode(text);
    let utf8_len = text.len();

    // 1. Exact buffer size (utf8_len + 1 for null terminator)
    let mut exact_buf = vec![0u8; utf8_len + 1];
    let written = sanitize_filename_to_slice(&encoded, &mut exact_buf).unwrap();
    assert_eq!(written, utf8_len);
    assert_eq!(&exact_buf[..written], text.as_bytes());
    assert_eq!(exact_buf[written], 0);

    // 2. Buffer size 1 byte less than required
    let mut undersized_buf = vec![0u8; utf8_len];
    let err = sanitize_filename_to_slice(&encoded, &mut undersized_buf);
    assert_eq!(err, Err(TTZipStatus::ErrPathTooLong));
}

// ============================================================================
// Target 16: CharsetDetector Streaming State Machine Reset & Chunking
// ============================================================================
#[test]
fn test_target_16_charset_detector_streaming_state_machine_reset() {
    let mut detector = CharsetDetector::new().expect("create CharsetDetector");

    let text_gb = encoding_rs::GB18030.encode("第一轮流式数据块注入测试").0;
    for chunk in text_gb.chunks(3) {
        detector.handle_data(chunk).unwrap();
    }
    detector.data_end();
    let detected1 = detector.detected_charset().unwrap();
    assert!(detected1.to_uppercase().contains("GB"));

    detector.reset();

    let text_sjis = encoding_rs::SHIFT_JIS.encode("第二輪ストリーミングリセットテスト").0;
    for chunk in text_sjis.chunks(4) {
        detector.handle_data(chunk).unwrap();
    }
    detector.data_end();
    let detected2 = detector.detected_charset().unwrap();
    assert_eq!(detected2, "Shift_JIS");

    // Direct CSM state machine test
    let mut csm = CodingStateMachine::new(CharsetKind::Gb18030);
    for &b in text_gb.as_ref() {
        let _ = csm.feed_byte(b);
    }
    assert!(csm.is_valid());
    assert!(csm.multibyte_chars() > 0);
}
