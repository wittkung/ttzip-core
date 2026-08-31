// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use std::borrow::Cow;
use ttzip_engine::crypto::zipcrypto::{
    zipcrypto_decrypt_slice, zipcrypto_encrypt_slice, ZipCryptoBatch4, ZipCryptoEngine,
    ZipCryptoKeys,
};
use ttzip_engine::types::TTZipStatus;
use ttzip_engine::zip::cp437::{decode_cp437, decode_zip_filename};

// =========================================================================
// CP437 Decoding Fidelity Tests
// =========================================================================

#[test]
fn test_cp437_ascii_zero_allocation_borrowed() {
    let ascii_bytes = b"standard_english_filename_12345.txt";
    let cow = decode_cp437(ascii_bytes);

    // Verify zero allocation (Cow::Borrowed)
    assert!(matches!(cow, Cow::Borrowed(_)));
    assert_eq!(cow, "standard_english_filename_12345.txt");
}

#[test]
fn test_cp437_german_umlauts_and_eszett() {
    // 0x84 = ä, 0x94 = ö, 0x81 = ü, 0x8E = Ä, 0x99 = Ö, 0x9A = Ü, 0xE1 = ß
    let german_bytes = b"\x84\x94\x81 \x8e\x99\x9a \xe1";
    let decoded = decode_cp437(german_bytes);

    assert!(matches!(decoded, Cow::Owned(_)));
    assert_eq!(decoded, "äöü ÄÖÜ ß");
}

#[test]
fn test_cp437_french_accents() {
    // 0x82 = é, 0x85 = à, 0x88 = ê, 0x87 = ç, 0x89 = ë, 0x8B = ï, 0x8C = î, 0x93 = ô, 0x96 = û
    let french_bytes = b"caf\x82 \x85 la for\x88t fran\x87aise No\x89l ma\x8bs na\x8bve ab\x8cme h\x93tel fl\x96te";
    let decoded = decode_cp437(french_bytes);

    assert_eq!(
        decoded,
        "café à la forêt française Noël maïs naïve abîme hôtel flûte"
    );
}


#[test]
fn test_cp437_box_drawing_and_shading() {
    // Single-line box drawing: 0xDA=┌, 0xC4=─, 0xC2=┬, 0xBF=┐, 0xB3=│, 0xC0=└, 0xC1=┴, 0xD9=┘
    let single_box = b"\xda\xc4\xc2\xc4\xbf\xb3 \xb3 \xb3\xc0\xc4\xc1\xc4\xd9";
    assert_eq!(decode_cp437(single_box), "┌─┬─┐│ │ │└─┴─┘");

    // Double-line box drawing: 0xC9=╔, 0xCD=═, 0xCB=╦, 0xBB=╗, 0xBA=║, 0xC8=╚, 0xCA=╩, 0xBC=╝
    let double_box = b"\xc9\xcd\xcb\xcd\xbb\xba \xba \xba\xc8\xcd\xca\xcd\xbc";
    assert_eq!(decode_cp437(double_box), "╔═╦═╗║ ║ ║╚═╩═╝");

    // Shading and full blocks: 0xB0=░, 0xB1=▒, 0xB2=▓, 0xDB=█, 0xDC=▄, 0xDF=▀, 0xFE=■, 0xFF=\u{00A0}
    let blocks = b"\xb0\xb1\xb2\xdb\xdc\xdf\xfe\xff";
    assert_eq!(decode_cp437(blocks), "░▒▓█▄▀■\u{00A0}");
}

#[test]
fn test_cp437_math_and_greek_symbols() {
    // 0xE0=α, 0xE2=Γ, 0xE3=π, 0xE4=Σ, 0xE5=σ, 0xE6=µ, 0xEC=∞, 0xF0=≡, 0xF1=±, 0xF2=≥, 0xF3=≤, 0xF6=÷, 0xF7=≈, 0xF8=°, 0xFB=√, 0xFD=²
    let math_bytes = b"\xe0 \xe2 \xe3 \xe4 \xe5 \xe6 \xec \xf0 \xf1 \xf2 \xf3 \xf6 \xf7 \xf8 \xfb \xfd";
    assert_eq!(
        decode_cp437(math_bytes),
        "α Γ π Σ σ µ ∞ ≡ ± ≥ ≤ ÷ ≈ ° √ ²"
    );
}

#[test]
fn test_decode_zip_filename_utf8_flag_logic() {
    // Case 1: Bit 11 is set, valid UTF-8
    let utf8_name = "你好_Dokument_2026.pdf".as_bytes();
    assert_eq!(decode_zip_filename(utf8_name, true), "你好_Dokument_2026.pdf");

    // Case 2: Bit 11 is set, but contains invalid UTF-8 (corrupt header fallback to CP437)
    let corrupt_bytes = b"\x84\x94\x81.txt";
    assert_eq!(decode_zip_filename(corrupt_bytes, true), "äöü.txt");

    // Case 3: Bit 11 is NOT set, pure ASCII
    let ascii_name = b"readme.txt";
    assert_eq!(decode_zip_filename(ascii_name, false), "readme.txt");

    // Case 4: Bit 11 is NOT set, but contains CP437 non-ASCII characters
    let cp437_name = b"M\x81nchen_Gr\x84tz.log";
    assert_eq!(decode_zip_filename(cp437_name, false), "München_Grätz.log");
}

// =========================================================================
// ZipCrypto Engine & 12-Byte Header Verification Tests
// =========================================================================

#[test]
fn test_zipcrypto_engine_standard_header_verification_success() {
    let password = b"SecretPKZIPPassword";
    let crc32: u32 = 0x89ABCDEF; // CRC high byte is 0x89
    let dos_time: u16 = 0x1234;
    let expected_check_byte = ((crc32 >> 24) & 0xFF) as u8; // 0x89

    let random_11: [u8; 11] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB];

    // Encryption side
    let mut enc_engine = ZipCryptoEngine::new(password);
    let enc_header = enc_engine.generate_header(expected_check_byte, &random_11);

    let plaintext = b"Confidential PKZIP Payload Data 2026.";
    let mut payload = plaintext.to_vec();
    enc_engine.encrypt_slice(&mut payload);

    // Decryption side: Standard mode (bit3_data_descriptor = false)
    let mut dec_engine = ZipCryptoEngine::verify_and_init(
        password,
        &enc_header,
        crc32,
        dos_time,
        false, // bit3_data_descriptor == false
    )
    .expect("ZipCrypto header verification must succeed with correct password");

    let mut decrypted_payload = payload.clone();
    dec_engine.decrypt_slice(&mut decrypted_payload);

    assert_eq!(&decrypted_payload[..], plaintext);
}

#[test]
fn test_zipcrypto_engine_streaming_bit3_header_verification_success() {
    let password = b"StreamPKZIPPass2026";
    let crc32: u32 = 0x11223344; // Should be ignored in bit 3 mode
    let dos_time: u16 = 0x5678; // DOS time high byte is 0x56
    let expected_check_byte = ((dos_time >> 8) & 0xFF) as u8; // 0x56

    let random_11: [u8; 11] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

    // Encryption side
    let mut enc_engine = ZipCryptoEngine::new(password);
    let enc_header = enc_engine.generate_header(expected_check_byte, &random_11);

    let plaintext = b"Streamed payload without known CRC-32 prior to deflation.";
    let mut payload = plaintext.to_vec();
    enc_engine.encrypt_slice(&mut payload);

    // Decryption side: Streaming mode (bit3_data_descriptor = true)
    let mut dec_engine = ZipCryptoEngine::verify_and_init(
        password,
        &enc_header,
        crc32,
        dos_time,
        true, // bit3_data_descriptor == true
    )
    .expect("ZipCrypto header verification must succeed in bit 3 streaming mode");

    let mut decrypted_payload = payload.clone();
    dec_engine.decrypt_slice(&mut decrypted_payload);

    assert_eq!(&decrypted_payload[..], plaintext);
}

#[test]
fn test_zipcrypto_engine_wrong_password_rejection() {
    let correct_password = b"ValidPassword123";
    let wrong_password = b"WrongPassword999";

    let crc32: u32 = 0xAABBCCDD;
    let expected_check_byte = 0xAA;
    let random_11: [u8; 11] = [0x42; 11];

    let mut enc_engine = ZipCryptoEngine::new(correct_password);
    let enc_header = enc_engine.generate_header(expected_check_byte, &random_11);

    // Verify failure with wrong password
    let result = ZipCryptoEngine::verify_and_init(
        wrong_password,
        &enc_header,
        crc32,
        0,
        false,
    );

    assert_eq!(result.err(), Some(TTZipStatus::ErrInvalidPassword));
}

#[test]
fn test_zipcrypto_engine_tampered_header_rejection() {
    let password = b"TamperGuardTest";
    let crc32: u32 = 0x77889900;
    let expected_check_byte = 0x77;
    let random_11: [u8; 11] = [0x55; 11];

    let mut enc_engine = ZipCryptoEngine::new(password);
    let mut enc_header = enc_engine.generate_header(expected_check_byte, &random_11);

    // Corrupt one byte of the header
    enc_header[5] ^= 0xFF;

    let result = ZipCryptoEngine::verify_and_init(
        password,
        &enc_header,
        crc32,
        0,
        false,
    );

    assert_eq!(result.err(), Some(TTZipStatus::ErrInvalidPassword));
}

#[test]
fn test_zipcrypto_engine_byte_by_byte_matches_slice() {
    let password = b"ByteByByteTest2026";
    let original = b"TTZip High-Performance Stream Cipher Byte-by-Byte Engine Test!";

    let mut engine_slice = ZipCryptoEngine::new(password);
    let mut buf_slice = original.to_vec();
    engine_slice.encrypt_slice(&mut buf_slice);

    let mut engine_byte = ZipCryptoEngine::new(password);
    let mut buf_byte = Vec::new();
    for &b in original {
        buf_byte.push(engine_byte.encrypt_byte(b));
    }

    assert_eq!(buf_slice, buf_byte);

    // Decrypt both and compare
    let mut dec_slice = ZipCryptoEngine::new(password);
    dec_slice.decrypt_slice(&mut buf_slice);
    assert_eq!(&buf_slice[..], original);

    let mut dec_byte = ZipCryptoEngine::new(password);
    let mut recovered_byte = Vec::new();
    for &b in &buf_byte {
        recovered_byte.push(dec_byte.decrypt_byte(b));
    }
    assert_eq!(&recovered_byte[..], original);
}

#[test]
fn test_zipcrypto_keys_and_convenience_helpers() {
    let password = b"DirectConveniencePass";
    let original = b"Testing ZipCrypto direct slice encryption and decryption helpers.";

    let mut encrypted = original.to_vec();
    zipcrypto_encrypt_slice(password, &mut encrypted);
    assert_ne!(&encrypted[..], &original[..]);

    let mut decrypted = encrypted.clone();
    zipcrypto_decrypt_slice(password, &mut decrypted);
    assert_eq!(&decrypted[..], &original[..]);

    // Keys debug redaction
    let keys = ZipCryptoKeys::from_password(password);
    let debug_str = format!("{:?}", keys);
    assert!(debug_str.contains("[REDACTED]"));
    assert!(!debug_str.contains(&format!("{:x}", keys.key0)));

    let engine = ZipCryptoEngine::from_keys(keys);
    let engine_debug = format!("{:?}", engine);
    assert!(engine_debug.contains("[REDACTED]"));
}

#[test]
fn test_zipcrypto_simd_batch4_matches_engine() {
    let passwords = [
        b"PassLane0".as_slice(),
        b"PassLane1".as_slice(),
        b"PassLane2".as_slice(),
        b"PassLane3".as_slice(),
    ];
    let mut batch = ZipCryptoBatch4::from_passwords(passwords);
    let mut engines = [
        ZipCryptoEngine::new(passwords[0]),
        ZipCryptoEngine::new(passwords[1]),
        ZipCryptoEngine::new(passwords[2]),
        ZipCryptoEngine::new(passwords[3]),
    ];

    let plain_quad = [0xA1, 0xB2, 0xC3, 0xD4];
    let cipher_quad = batch.encrypt_bytes_4way(plain_quad);

    for (lane, engine) in engines.iter_mut().enumerate() {
        let single_cipher = engine.encrypt_byte(plain_quad[lane]);
        assert_eq!(cipher_quad[lane], single_cipher);
    }

    let mut dec_batch = ZipCryptoBatch4::from_passwords(passwords);
    let decrypted_quad = dec_batch.decrypt_bytes_4way(cipher_quad);
    assert_eq!(decrypted_quad, plain_quad);
}

