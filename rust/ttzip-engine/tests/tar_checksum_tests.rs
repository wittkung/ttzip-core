// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use ttzip_engine::tar::checksum::{
    calculate_signed_checksum, calculate_unsigned_checksum, parse_checksum_field,
    verify_header_checksum, verify_header_checksum_slice, write_header_checksum,
    TarChecksumError, CHKSUM_LEN, CHKSUM_OFFSET,
};

/// Helper to build a sample POSIX ustar 512-byte header block.
fn create_sample_ustar_header(name: &str, size: u64, mode: u32) -> [u8; 512] {
    let mut header = [0u8; 512];

    // Name (0..100)
    let name_bytes = name.as_bytes();
    let name_len = name_bytes.len().min(100);
    header[0..name_len].copy_from_slice(&name_bytes[..name_len]);

    // Mode (100..108): 7 octal digits + NUL
    let mode_str = format!("{:07o}\0", mode);
    header[100..108].copy_from_slice(mode_str.as_bytes());

    // UID (108..116) & GID (116..124)
    header[108..116].copy_from_slice(b"0001750\0");
    header[116..124].copy_from_slice(b"0001750\0");

    // Size (124..136): 11 octal digits + NUL
    let size_str = format!("{:011o}\0", size);
    header[124..136].copy_from_slice(size_str.as_bytes());

    // Mtime (136..148): 11 octal digits + NUL
    header[136..148].copy_from_slice(b"14532672341\0");

    // Typeflag (156): '0' for regular file
    header[156] = b'0';

    // Magic (257..263) & Version (263..265)
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");

    // Uname & Gname
    let uname = b"ttzip_user";
    header[265..265 + uname.len()].copy_from_slice(uname);
    let gname = b"ttzip_group";
    header[297..297 + gname.len()].copy_from_slice(gname);

    header
}

#[test]
fn test_ustar_standard_checksum_roundtrip() {
    let mut header = create_sample_ustar_header("docs/architecture/spec.pdf", 1048576, 0o644);

    // Compute and write checksum
    write_header_checksum(&mut header);

    // Validate raw format: 6 octal digits + \0 + ' '
    let raw_chk = &header[CHKSUM_OFFSET..CHKSUM_OFFSET + CHKSUM_LEN];
    assert_eq!(raw_chk[6], 0, "6th byte must be NUL");
    assert_eq!(raw_chk[7], b' ', "7th byte must be space");

    // Verify unsigned calculation matches written field
    let unsigned_val = calculate_unsigned_checksum(&header);
    let verify_res = verify_header_checksum(&header);
    assert!(verify_res.is_ok(), "Header checksum must verify successfully");
    assert_eq!(verify_res.unwrap(), unsigned_val);

    // Verify slice wrapper
    let slice_res = verify_header_checksum_slice(&header[..]);
    assert_eq!(slice_res, Ok(unsigned_val));
}

#[test]
fn test_sunos_signed_char_fallback_tolerance() {
    let mut header = create_sample_ustar_header("archive/résumé_2026.tar", 4096, 0o755);

    // Inject high-bit non-ASCII bytes (e.g. 0x80..0xFF) in filename and prefix
    header[0] = 0xE9; // Latin-1 e-acute
    header[1] = 0x80; // High-bit set
    header[2] = 0xFE; // Signed char would interpret as -2
    header[50] = 0xFF; // Signed char would interpret as -1

    let u_sum = calculate_unsigned_checksum(&header);
    let s_sum = calculate_signed_checksum(&header);

    // Confirm that unsigned and signed sums diverge due to sign extension
    assert_ne!(
        u_sum, s_sum as u32,
        "Checksums must diverge when high-bit bytes are present"
    );

    // Write SunOS historical signed checksum into the header
    let sunos_chk_str = format!("{:06o}\0 ", s_sum);
    header[CHKSUM_OFFSET..CHKSUM_OFFSET + CHKSUM_LEN]
        .copy_from_slice(sunos_chk_str.as_bytes());

    // Verification must succeed by falling back to signed checksum
    let verified = verify_header_checksum(&header);
    assert!(
        verified.is_ok(),
        "SunOS signed-char header must pass verification via fallback"
    );
    assert_eq!(verified.unwrap() as i32, s_sum);
}

#[test]
fn test_arbitrary_1bit_flip_in_header_payload() {
    let mut base_header = create_sample_ustar_header("secure/data.bin", 65536, 0o600);
    write_header_checksum(&mut base_header);

    assert!(verify_header_checksum(&base_header).is_ok());

    // Exhaustively test single-bit flips across ALL 504 non-checksum bytes (4032 mutants)
    let mut tested_bits = 0;
    for byte_idx in 0..512 {
        if (CHKSUM_OFFSET..CHKSUM_OFFSET + CHKSUM_LEN).contains(&byte_idx) {
            continue;
        }
        for bit in 0..8 {
            let mut corrupted = base_header;
            corrupted[byte_idx] ^= 1 << bit;

            let result = verify_header_checksum(&corrupted);
            assert!(
                matches!(result, Err(TarChecksumError::Mismatch { .. })),
                "1-bit corruption in payload at byte {} bit {} MUST return Mismatch",
                byte_idx,
                bit
            );
            tested_bits += 1;
        }
    }

    assert_eq!(tested_bits, 504 * 8);
}

#[test]
fn test_checksum_field_bit_mutation() {
    let mut base_header = create_sample_ustar_header("secure/data.bin", 65536, 0o600);
    write_header_checksum(&mut base_header);

    // Test digits (148..154): value changes and non-octal corruption
    for byte_offset in 0..6 {
        let byte_idx = CHKSUM_OFFSET + byte_offset;
        for bit in [0, 1, 2, 3, 5, 6, 7] {
            let mut corrupted = base_header;
            corrupted[byte_idx] ^= 1 << bit;

            let result = verify_header_checksum(&corrupted);
            assert!(
                result.is_err(),
                "Bit corruption in digit byte {} bit {} must fail validation",
                byte_idx,
                bit
            );
        }
    }

    // Test delimiter bytes (154 = NUL, 155 = Space): non-delimiter corruption
    for byte_idx in [CHKSUM_OFFSET + 6, CHKSUM_OFFSET + 7] {
        for bit in [0, 1, 2, 3, 4, 6, 7] {
            let mut corrupted = base_header;
            corrupted[byte_idx] ^= 1 << bit;

            let result = verify_header_checksum(&corrupted);
            assert!(
                result.is_err(),
                "Bit corruption in delimiter byte {} bit {} must fail validation",
                byte_idx,
                bit
            );
        }
    }
}

#[test]
fn test_all_zeroes_header_rejection() {
    let all_zeroes = [0u8; 512];
    let result = verify_header_checksum(&all_zeroes);

    assert!(result.is_err());
    match result.unwrap_err() {
        TarChecksumError::Mismatch {
            expected,
            actual_unsigned,
            actual_signed,
        } => {
            assert_eq!(expected, 0);
            assert_eq!(actual_unsigned, 256); // 8 * 0x20
            assert_eq!(actual_signed, 256);
        }
        err => panic!("Unexpected error variant: {:?}", err),
    }
}

#[test]
fn test_truncated_header_slice_rejection() {
    let short_slices: [&[u8]; 5] = [
        &[],
        &[0u8; 1],
        &[0u8; 100],
        &[0u8; 148],
        &[0u8; 511],
    ];

    for slice in short_slices {
        let result = verify_header_checksum_slice(slice);
        assert!(
            result.is_err(),
            "Truncated slice of len {} must be rejected",
            slice.len()
        );
        match result.unwrap_err() {
            TarChecksumError::TruncatedHeader { found } => {
                assert_eq!(found, slice.len());
            }
            err => panic!("Expected TruncatedHeader, got {:?}", err),
        }
    }
}

#[test]
fn test_malformed_octal_checksum_fields() {
    let mut header = create_sample_ustar_header("corrupted_chksum.txt", 128, 0o644);

    let malformed_fields: [&[u8; 8]; 4] = [
        b"abcdefgh",
        b"888888\0 ",
        b"999999\0 ",
        b"??!!@@##",
    ];

    for raw in malformed_fields {
        header[CHKSUM_OFFSET..CHKSUM_OFFSET + CHKSUM_LEN].copy_from_slice(raw);
        let result = verify_header_checksum(&header);
        assert!(result.is_err());
        match result.unwrap_err() {
            TarChecksumError::InvalidOctal { raw: actual_raw } => {
                assert_eq!(&actual_raw, raw);
            }
            err => panic!("Expected InvalidOctal, got {:?}", err),
        }
    }
}

#[test]
fn test_various_valid_octal_formats_parsing() {
    // 6 digits + NUL + Space: 0o1234 = 668
    assert_eq!(parse_checksum_field(b"001234\0 "), Ok(0o1234));

    // 6 digits + Space + NUL
    assert_eq!(parse_checksum_field(b"001234 \0"), Ok(0o1234));

    // 7 digits + NUL
    assert_eq!(parse_checksum_field(b"0001234\0"), Ok(0o1234));

    // 7 digits + Space
    assert_eq!(parse_checksum_field(b"0001234 "), Ok(0o1234));

    // Space padded: "  1234\0 "
    assert_eq!(parse_checksum_field(b"  1234\0 "), Ok(0o1234));

    // All spaces: treated as 0
    assert_eq!(parse_checksum_field(b"        "), Ok(0));

    // All zeroes: treated as 0
    assert_eq!(parse_checksum_field(b"\0\0\0\0\0\0\0\0"), Ok(0));
}
