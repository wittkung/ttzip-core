// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Malformed TAR Fault-Injection Fuzzing Harness & RandomReader Jitter Stress Test Suite.
//!
//! Validates 6 critical corruption and resilience dimensions:
//! 1. Truncated 512-Byte Header Sector Injection (1..511 bytes truncation defense).
//! 2. Bad Checksum & Exhaustive Single-Bit Flip Invariant (100% interception & TarChecksumError::Mismatch).
//! 3. Illegal Octal ASCII & Malformed Base-256 Binary Codec Defense (Non-ASCII, non-octal, integer overflow).
//! 4. PAX Length Bomb & Malformed Key-Value Injection (Truncated length, missing delimiters, oversized payloads).
//! 5. PAX Size Smuggling Injection (GHSA-3cv2-h65g-fgmm Size Precedence & Malicious Symlink Isolation).
//! 6. RandomReader Quadratic Biased Micro-Slicing Chaos Jitter Reading (1..10 byte streaming stress).

use std::io::{Cursor, Read};
use std::panic::{catch_unwind, AssertUnwindSafe};

use ttzip_engine::archive::tar::reader::TarArchive;
use ttzip_engine::archive::tar::scanner::TarSeekScanner;
use ttzip_engine::tar::alignment::{
    is_all_zeros, pad_to_512, EofBlockDetector, TarEofStatus, TAR_SECTOR_SIZE,
};
use ttzip_engine::tar::checksum::{
    verify_header_checksum, verify_header_checksum_slice, TarChecksumError, CHKSUM_LEN,
    CHKSUM_OFFSET,
};
use ttzip_engine::tar::codec::{
    base256_from, base256_into, numeric_extended_from, octal_from,
};
use ttzip_engine::tar::header::TarHeader;
use ttzip_engine::tar::pax::{PaxExtensionMap, PaxZeroScanner, TarPaxError};
use ttzip_engine::tar::sparse::{parse_gnu_sparse_1_0_stream, SparseExtent};
use ttzip_engine::tar::types::{TarEntryType, BLOCK_SIZE};

// ============================================================================
// Fixture Construction Helpers
// ============================================================================

/// Helper to build a standard 512-byte POSIX ustar header with valid metadata and checksum.
fn make_ustar_header(name: &str, size: u64, mode: u32, typeflag: u8) -> TarHeader {
    let mut header = TarHeader::new();
    header.set_ustar_magic();
    header.set_name(name);
    header.set_size(size);
    header.set_mode(mode);
    header.set_mtime(1700000000);
    header.set_uid(1000);
    header.set_gid(1000);
    header.set_uname("ttzip");
    header.set_gname("staff");
    header.set_entry_type(TarEntryType::from_byte(typeflag));
    header.update_checksum();
    header
}

// ============================================================================
// Dimension 1: Truncated 512-Byte Header Sector Injection
// ============================================================================

#[test]
fn test_dimension_1_truncated_512b_header_injection() {
    let valid_header = make_ustar_header("fuzz/target.dat", 1024, 0o644, b'0');
    let header_bytes = valid_header.as_bytes();

    // Verify slicing prefixes across 0..512 bytes are safely intercepted
    for len in 0..BLOCK_SIZE {
        let truncated_slice = &header_bytes[..len];

        // 1. TarHeader::from_slice must return None on < 512 bytes
        assert!(
            TarHeader::from_slice(truncated_slice).is_none(),
            "TarHeader::from_slice must reject truncated slice of length {}",
            len
        );

        // 2. verify_header_checksum_slice must return TruncatedHeader error
        let chk_res = verify_header_checksum_slice(truncated_slice);
        assert_eq!(
            chk_res,
            Err(TarChecksumError::TruncatedHeader { found: len }),
            "Expected TruncatedHeader error for length {}",
            len
        );

        // 3. TarSeekScanner & TarArchive must never panic or perform out-of-bounds indexing
        let panic_res = catch_unwind(AssertUnwindSafe(|| {
            let mut scanner = TarSeekScanner::new(truncated_slice);
            let _ = scanner.scan_all();
            let _ = TarArchive::open_slice(truncated_slice);
        }));
        assert!(
            panic_res.is_ok(),
            "Scanner/Archive must not panic on truncated input of length {}",
            len
        );
    }
}

// ============================================================================
// Dimension 2: Bad Checksum & Exhaustive Single-Bit Flip Invariant
// ============================================================================

#[test]
fn test_dimension_2_bad_checksum_and_single_bit_flip_invariant() {
    let header = make_ustar_header("fuzz/checksum_victim.tar", 4096, 0o755, b'0');
    assert!(
        verify_header_checksum(header.as_bytes()).is_ok(),
        "Baseline header must have valid checksum"
    );

    // 1. Single-bit mutations across all header sectors OUTSIDE the checksum field (496 bytes * 8 bits = 3968 mutations)
    // Every single 1-bit change alters the unsigned and signed checksum values by 2^k != 0,
    // guaranteeing 100% mathematical interception with TarChecksumError::Mismatch.
    let mut non_checksum_flips = 0usize;
    for byte_idx in 0..BLOCK_SIZE {
        if (CHKSUM_OFFSET..CHKSUM_OFFSET + CHKSUM_LEN).contains(&byte_idx) {
            continue;
        }

        for bit in 0..8 {
            let mut corrupted = *header.as_bytes();
            corrupted[byte_idx] ^= 1 << bit;

            let verify_res = verify_header_checksum(&corrupted);
            assert!(
                matches!(verify_res, Err(TarChecksumError::Mismatch { .. })),
                "Single-bit flip at byte {} bit {} must fail with TarChecksumError::Mismatch, got {:?}",
                byte_idx,
                bit,
                verify_res
            );

            // Slice verification parity
            assert!(verify_header_checksum_slice(&corrupted).is_err());
            non_checksum_flips += 1;
        }
    }

    assert_eq!(
        non_checksum_flips,
        (BLOCK_SIZE - CHKSUM_LEN) * 8,
        "All 3968 non-checksum single-bit mutations must fail with TarChecksumError::Mismatch"
    );

    // 2. Corrupted and bad checksum injections directly inside the 8-byte checksum field
    let bad_checksum_injections: &[&[u8; 8]] = &[
        b"000000\0 ", // Zero checksum
        b"777777\0 ", // Maximum 6-digit octal checksum
        b"123456\0 ", // Wrong arbitrary checksum
        b"abcdef\0 ", // Alphabetic invalid octal
        b"000080\0 ", // Invalid digit '8'
        b"000099\0 ", // Invalid digit '9'
        b"\x80\x00\x00\x00\x00\x00\x00\x00", // Non-ASCII byte
        b"        ", // All spaces (uninitialized)
    ];

    for bad_chk in bad_checksum_injections {
        let mut corrupted = *header.as_bytes();
        corrupted[CHKSUM_OFFSET..CHKSUM_OFFSET + CHKSUM_LEN].copy_from_slice(*bad_chk);

        let verify_res = verify_header_checksum(&corrupted);
        assert!(
            verify_res.is_err(),
            "Bad checksum {:?} must be intercepted",
            bad_chk
        );

        match verify_res.unwrap_err() {
            TarChecksumError::Mismatch { .. } | TarChecksumError::InvalidOctal { .. } => {
                // Correctly caught as mismatch or invalid octal
            }
            other => panic!("Unexpected error variant: {:?}", other),
        }
    }
}

// ============================================================================
// Dimension 3: Illegal Octal ASCII & Malformed Base-256 Binary Codec Defense
// ============================================================================

#[test]
fn test_dimension_3_illegal_octal_and_malformed_base256_injection() {
    // 1. Octal parser malicious/invalid inputs
    let invalid_octals: &[&[u8]] = &[
        b"1238567\0",                          // Non-octal digit '8'
        b"0000999\0",                          // Non-octal digit '9'
        b"abc777\0",                           // Alphabetic characters
        b"-12345\0",                           // Negative sign
        b"\x80\x00\x00\x00",                   // High-bit non-ASCII
        b"\xFF\xFE\xFD\xFC",                   // High-bit non-ASCII
        b"   \0  9  \0 ",                      // Interleaved invalid digit
        b"777777777777777777777777777777\0",   // Arithmetic overflow (> u64::MAX)
    ];

    for &bad in invalid_octals {
        assert_eq!(
            octal_from(bad),
            None,
            "octal_from must reject invalid input: {:?}",
            bad
        );
    }

    // Valid whitespace and null trimming edge-cases
    assert_eq!(octal_from(b""), Some(0));
    assert_eq!(octal_from(b"   \0\0\0  "), Some(0));
    assert_eq!(octal_from(b" 0000755\0 "), Some(0o755));
    assert_eq!(octal_from(b"0777\0"), Some(0o777));

    // 2. Base-256 binary decoder malicious/invalid inputs (missing 0x80 marker)
    let invalid_base256: &[&[u8]] = &[
        b"\x00\x01\x02\x03", // Missing 0x80 high-bit marker
        b"\x7F\xFF\xFF\xFF", // 0x7F missing marker
        b"\x40\x12\x34\x56", // 0x40 missing marker
    ];

    for &bad in invalid_base256 {
        assert_eq!(
            base256_from(bad),
            None,
            "base256_from must reject invalid input: {:?}",
            bad
        );
    }

    // Base-256 roundtrip encoding and decoding fidelity
    let test_values = [0u64, 1, 0o755, 1_000_000, 0x1234_5678_9ABC_DEF0, u64::MAX / 2];
    for &val in &test_values {
        let mut buf = [0u8; 12];
        base256_into(&mut buf, val);
        assert_eq!(
            base256_from(&buf),
            Some(val),
            "base256 roundtrip failed for {}",
            val
        );
    }

    // 3. numeric_extended_from resilience (zero panics under malformed bytes)
    for &bad in invalid_octals {
        let _ = numeric_extended_from(bad);
    }
    for &bad in invalid_base256 {
        let _ = numeric_extended_from(bad);
    }
    // ASCII invalid octals without high-bit return 0
    assert_eq!(numeric_extended_from(b"1238567\0"), 0);
    assert_eq!(numeric_extended_from(b"bad_octal\0"), 0);
}

// ============================================================================
// Dimension 4: PAX Length Bomb & Malformed Key-Value Injection
// ============================================================================

#[test]
fn test_dimension_4_pax_length_bomb_and_malformed_kv_injection() {
    // 4.1 Declared length exceeds actual slice (TruncatedRecord)
    let truncated_data = b"100 path=truncated.txt\n";
    let mut scanner = PaxZeroScanner::new(truncated_data);
    assert_eq!(
        scanner.next(),
        Some(Err(TarPaxError::TruncatedRecord {
            expected: 100,
            available: truncated_data.len()
        }))
    );

    // 4.2 Record size exceeds MAX_PAX_RECORD_SIZE security limit (RecordTooLarge)
    let huge_len_data = b"9999999999999 path=huge.txt\n";
    let mut scanner = PaxZeroScanner::new(huge_len_data);
    assert!(matches!(
        scanner.next(),
        Some(Err(TarPaxError::RecordTooLarge { .. }))
    ));

    // 4.3 Missing space delimiter
    let no_space = b"25path=missing_space.txt\n";
    let mut scanner = PaxZeroScanner::new(no_space);
    assert_eq!(scanner.next(), Some(Err(TarPaxError::MissingSpaceDelimiter)));

    // 4.4 Missing equal delimiter (length 22 matches exact payload length)
    let no_equal = b"22 path_missing_equal\n";
    let mut scanner = PaxZeroScanner::new(no_equal);
    assert_eq!(scanner.next(), Some(Err(TarPaxError::MissingEqualDelimiter)));

    // 4.5 Missing newline delimiter (length 18 matches exact payload length)
    let no_newline = b"18 path=no_newline\0";
    let mut scanner = PaxZeroScanner::new(no_newline);
    assert_eq!(
        scanner.next(),
        Some(Err(TarPaxError::MissingNewlineDelimiter))
    );

    // 4.6 Invalid decimal length formatting
    let bad_lens: &[&[u8]] = &[
        b"abc path=bad_len.txt\n",
        b"-25 path=bad_len.txt\n",
        b"0 path=bad_len.txt\n",
    ];
    for &bad in bad_lens {
        let mut s = PaxZeroScanner::new(bad);
        assert!(matches!(s.next(), Some(Err(TarPaxError::InvalidLength { .. }))));
    }

    // 4.7 Declared length smaller than prefix
    let len_too_small = b"4 path=too_small.txt\n";
    let mut s = PaxZeroScanner::new(len_too_small);
    assert!(matches!(
        s.next(),
        Some(Err(TarPaxError::LengthTooSmall { .. }))
    ));

    // 4.8 Invalid UTF-8 in key (25 bytes total: "25 \xFF\xFEkey=some_value_test\n")
    let bad_utf8_key = b"25 \xFF\xFEkey=some_value_test\n";
    let mut s = PaxZeroScanner::new(bad_utf8_key);
    assert!(matches!(
        s.next(),
        Some(Err(TarPaxError::InvalidUtf8Key { .. }))
    ));

    // 4.9 Trailing TAR padding NUL bytes must cleanly terminate scanner
    let trailing_zeros = [0u8; 512];
    let mut s = PaxZeroScanner::new(&trailing_zeros);
    assert_eq!(s.next(), None);
}

// ============================================================================
// Dimension 5: PAX Size Smuggling Injection (GHSA-3cv2-h65g-fgmm)
// ============================================================================

#[test]
fn test_dimension_5_pax_size_smuggling_ghsa_3cv2_h65g_fgmm_isolation() {
    // Construct adversarial TAR archive:
    // 1. PAX Header ('x') declaring size=2048 and path="smuggled_file.dat"
    // 2. Base Header ('0') declaring size=8 (the smuggling trap)
    // 3. Payload: 2048 bytes
    //    - Bytes 0..8: legitimate data "PAYLOAD!"
    //    - Bytes 512..1024: forged 512-byte header for malicious symlink "evil_symlink" -> "/etc/shadow"
    //    - Bytes 1024..2048: filler bytes
    // 4. Legitimate next entry: "next_legit_file.txt" (size 100)
    // 5. 2x512B zero blocks EOF.

    let mut archive = Vec::new();

    // 1. PAX header record
    let mut pax_map = PaxExtensionMap::new();
    pax_map.set_size(2048);
    pax_map.set_path("smuggled_file.dat");
    let pax_bytes = pax_map.to_bytes();

    let pax_header = make_ustar_header(
        "PaxHeaders/smuggled_file.dat",
        pax_bytes.len() as u64,
        0o644,
        b'x',
    );
    archive.extend_from_slice(pax_header.as_bytes());
    archive.extend_from_slice(&pax_bytes);
    let pax_pad = pad_to_512(pax_bytes.len() as u64) as usize;
    archive.extend(vec![0u8; pax_pad]);

    // 2. Base header with size=8
    let base_header = make_ustar_header("fallback_name.dat", 8, 0o644, b'0');
    archive.extend_from_slice(base_header.as_bytes());

    // 3. Full 2048-byte payload containing the forged symlink header inside sector 2 (offset 512..1024)
    let mut payload = vec![0x41u8; 2048];
    payload[0..8].copy_from_slice(b"PAYLOAD!");

    let mut evil_symlink_header = make_ustar_header("evil_symlink", 0, 0o777, b'2');
    evil_symlink_header.set_linkname("/etc/shadow");
    evil_symlink_header.update_checksum();

    payload[512..1024].copy_from_slice(evil_symlink_header.as_bytes());
    archive.extend_from_slice(&payload);

    // 4. Next legitimate entry
    let next_header = make_ustar_header("next_legit_file.txt", 100, 0o644, b'0');
    archive.extend_from_slice(next_header.as_bytes());
    let next_data = vec![0x42u8; 100];
    archive.extend_from_slice(&next_data);
    let next_pad = pad_to_512(100) as usize;
    archive.extend(vec![0u8; next_pad]);

    // 5. EOF double zero block (1024 bytes)
    archive.extend(vec![0u8; 1024]);

    // Test PaxExtensionMap precedence
    let parsed_map = PaxExtensionMap::from_slice(&pax_bytes).expect("PAX records must parse");
    assert_eq!(parsed_map.size(), Some(2048));
    assert_eq!(parsed_map.path(), Some("smuggled_file.dat"));

    let resolved_entry = parsed_map.apply_to_entry(&base_header);
    assert_eq!(
        resolved_entry.size, 2048,
        "GHSA-3cv2-h65g-fgmm: PAX size 2048 must strictly override base header size 8"
    );
    assert_eq!(resolved_entry.path, "smuggled_file.dat");

    // In-place modification test
    let mut modified_header = base_header;
    parsed_map.apply_to_header(&mut modified_header);
    assert_eq!(modified_header.size(), 2048);
    assert_eq!(modified_header.name(), "smuggled_file.dat");

    // Test archive scanner: Scanner must skip full 2048 bytes and NOT parse the smuggled symlink!
    let mut scanner = TarSeekScanner::new(&archive);
    let entries = scanner.scan_all().expect("Archive scan must succeed");

    assert_eq!(
        entries.len(),
        2,
        "Archive must only contain 2 entries (smuggled_file and next_legit_file)"
    );
    assert_eq!(entries[0].path, "smuggled_file.dat");
    assert_eq!(entries[0].size, 2048);
    assert_eq!(entries[1].path, "next_legit_file.txt");
    assert_eq!(entries[1].size, 100);

    // Verify none of the entries is the malicious symlink
    for entry in &entries {
        assert_ne!(
            entry.path, "evil_symlink",
            "Malicious symlink in smuggled payload sector was improperly extracted!"
        );
        assert!(!entry.is_symlink);
    }
}

// ============================================================================
// Dimension 6: RandomReader Quadratic Biased Micro-Slicing Chaos Jitter Reading
// ============================================================================

/// Deterministic PRNG and Reader wrapper that yields 1..=10 byte micro-chunks
/// with quadratic bias to simulate hostile network/disk I/O jitter.
struct RandomReader<R: Read> {
    inner: R,
    state: u64,
}

impl<R: Read> RandomReader<R> {
    fn new(inner: R, seed: u64) -> Self {
        Self { inner, state: seed }
    }

    fn next_chunk_len(&mut self, max_cap: usize) -> usize {
        // 64-bit Linear Congruential / Quadratic step
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let jitter = 1 + (self.state % 10) as usize;
        jitter.min(max_cap).max(1)
    }
}

impl<R: Read> Read for RandomReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let chunk_limit = self.next_chunk_len(buf.len());
        self.inner.read(&mut buf[..chunk_limit])
    }
}

#[test]
fn test_dimension_6_random_reader_jitter_streaming_robustness() {
    // 6.1 Stream reading GNU Sparse 1.0 map text under 1..10 byte random jitter
    let mut sparse_sector = [0u8; 512];
    let sparse_map_text = b"4\n0\n4096\n65536\n4096\n131072\n8192\n1048576\n0\n";
    sparse_sector[..sparse_map_text.len()].copy_from_slice(sparse_map_text);
    let real_size = 2_000_000u64;

    for seed in [1337u64, 42, 99999, 0xCAFEBABE, 0xDEADBEEF] {
        let cursor = Cursor::new(&sparse_sector[..]);
        let mut jitter_reader = RandomReader::new(cursor, seed);

        let (map, bytes_read) = parse_gnu_sparse_1_0_stream(&mut jitter_reader, real_size)
            .expect("parse_gnu_sparse_1_0_stream must succeed under random chunk jitter");

        assert_eq!(bytes_read, 512);
        assert_eq!(map.real_size, real_size);
        assert_eq!(map.extents.len(), 3);
        assert_eq!(map.extents[0], SparseExtent::new(0, 4096));
        assert_eq!(map.extents[1], SparseExtent::new(65536, 4096));
        assert_eq!(map.extents[2], SparseExtent::new(131072, 8192));
    }

    // 6.2 EofBlockDetector streaming consumption under 1..10 byte random jitter
    for seed in [12345u64, 67890] {
        let mut stream_data = Vec::new();
        // 1 normal block, 1 zero block, 1 normal block, 2 zero blocks (EOF)
        let active_header_1 = make_ustar_header("active_1.txt", 10, 0o644, b'0');
        stream_data.extend_from_slice(active_header_1.as_bytes());
        stream_data.extend(vec![0u8; 512]); // single zero block
        let active_header_2 = make_ustar_header("active_2.txt", 20, 0o644, b'0');
        stream_data.extend_from_slice(active_header_2.as_bytes());
        stream_data.extend(vec![0u8; 1024]); // double zero block EOF

        let cursor = Cursor::new(&stream_data);
        let mut jitter_reader = RandomReader::new(cursor, seed);

        let mut detector = EofBlockDetector::new(false);
        let mut statuses = Vec::new();
        let mut block_buf = [0u8; TAR_SECTOR_SIZE];

        while jitter_reader.read_exact(&mut block_buf).is_ok() {
            let status = detector.feed_block(&block_buf);
            statuses.push(status);
            if status == TarEofStatus::EndOfArchive {
                break;
            }
        }

        assert_eq!(
            statuses,
            vec![
                TarEofStatus::Continue,
                TarEofStatus::Continue, // first zero block waits for next
                TarEofStatus::Continue,
                TarEofStatus::Continue, // first zero block
                TarEofStatus::EndOfArchive // second consecutive zero block
            ]
        );
        assert_eq!(detector.on_stream_end(), TarEofStatus::EndOfArchive);
        assert!(!is_all_zeros(active_header_1.as_bytes()));
    }

    // 6.3 Stepping prefixes through TarSeekScanner and TarArchive with micro-slices
    let mut composite_archive = Vec::new();
    let h1 = make_ustar_header("file1.txt", 50, 0o644, b'0');
    composite_archive.extend_from_slice(h1.as_bytes());
    composite_archive.extend(vec![b'X'; 50]);
    composite_archive.extend(vec![0u8; pad_to_512(50) as usize]);

    let h2 = make_ustar_header("file2.txt", 120, 0o644, b'0');
    composite_archive.extend_from_slice(h2.as_bytes());
    composite_archive.extend(vec![b'Y'; 120]);
    composite_archive.extend(vec![0u8; pad_to_512(120) as usize]);
    composite_archive.extend(vec![0u8; 1024]); // EOF

    // Verify slicing prefixes in 1..7 byte steps causes zero panics
    let panic_res = catch_unwind(AssertUnwindSafe(|| {
        for step in (1..composite_archive.len()).step_by(5) {
            let slice = &composite_archive[..step];
            let mut s = TarSeekScanner::new(slice);
            let _ = s.scan_all();
            let _ = TarArchive::open_slice(slice);
        }
    }));
    assert!(
        panic_res.is_ok(),
        "Stepping micro-slices must never trigger panics"
    );
}
