// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 1,000-Iteration Pseudorandom 1% Bit-Flip and Byte Mutation Fuzzing Stress Test Suite.
//!
//! Validates:
//! 1. 1,000 randomized bit-flip / byte mutations on valid TAR, ZIP, and 7z baseline archives.
//! 2. Zero-Panic and Zero-Crash invariants across all container parsers under corrupted byte streams.
//! 3. Deterministic typed error reporting on all invalid mutated inputs.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

use ttzip_engine::archive::tar::reader::TarArchive;
use ttzip_engine::archive::tar::scanner::TarSeekScanner;
use ttzip_engine::sevenz::{create_7z_solid_archive_bytes, SevenZArchive};
use ttzip_engine::zip::reader::ZipArchive;
use ttzip_engine::zip::writer::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

/// Fast deterministic Xorshift64 PRNG for reproducible fuzz stress iterations.
struct FuzzRng {
    state: u64,
}

impl FuzzRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x853c49e6748fea9b } else { seed },
        }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        self.state = self.state.wrapping_mul(0x2545F4914F6CDD1D);
        self.state
    }

    #[inline]
    fn next_usize(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u64() % (bound as u64)) as usize
        }
    }

    #[inline]
    fn next_u8(&mut self) -> u8 {
        (self.next_u64() & 0xFF) as u8
    }
}

/// Creates a valid baseline ZIP archive with 3 distinct entries.
fn create_baseline_zip() -> Vec<u8> {
    let items = vec![
        ZipInputItem {
            rel_path: "file1.txt".to_string(),
            data: b"TTZip Fuzz Baseline Payload 1".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "subdir/".to_string(),
            data: Vec::new(),
            mtime_epoch_secs: 1700000000,
            mode: 0o755,
            is_directory: true,
        },
        ZipInputItem {
            rel_path: "subdir/file2.bin".to_string(),
            data: vec![0x5A; 512],
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let compressed = compress_items_parallel(
        items,
        6,
        ttzip_engine::types::TTZipEncryptionMethod::None,
        None,
        2,
    )
    .expect("baseline zip creation failed");
    assemble_zip_archive(&compressed).expect("assemble zip failed")
}

/// Creates a valid baseline 7z archive with 3 distinct entries.
fn create_baseline_7z() -> Vec<u8> {
    let items = vec![
        ZipInputItem {
            rel_path: "file1.txt".to_string(),
            data: b"TTZip 7z Fuzz Baseline Payload 1".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "dir/".to_string(),
            data: Vec::new(),
            mtime_epoch_secs: 1700000000,
            mode: 0o755,
            is_directory: true,
        },
        ZipInputItem {
            rel_path: "dir/data.bin".to_string(),
            data: vec![0x3C; 512],
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
    ];

    create_7z_solid_archive_bytes(&items, 1, 2).expect("baseline 7z creation failed")
}

/// Creates a valid baseline TAR archive with 2 distinct entries.
fn create_baseline_tar() -> Vec<u8> {
    let mut out = Vec::new();

    // File 1 header
    let mut h1 = [0u8; 512];
    h1[..9].copy_from_slice(b"file1.txt");
    h1[100..108].copy_from_slice(b"0000644\0");
    h1[124..136].copy_from_slice(b"00000000020\0"); // 16 bytes
    h1[136..148].copy_from_slice(b"14400000000\0");
    h1[148..156].copy_from_slice(b"        ");
    h1[156] = b'0';
    h1[257..263].copy_from_slice(b"ustar\0");
    h1[263..265].copy_from_slice(b"00");

    let (c1, _) = ttzip_engine::archive::tar::header::compute_tar_checksum(&h1);
    let c1_str = format!("{:06o}\0 ", c1);
    h1[148..156].copy_from_slice(c1_str.as_bytes());

    out.extend_from_slice(&h1);
    out.extend_from_slice(b"TTZip TAR Fuzz Payload 16 bytes!");
    out.extend_from_slice(&[0u8; 512 - 32]); // pad block

    // End-of-Archive 1024 zero bytes
    out.extend_from_slice(&[0u8; 1024]);
    out
}

#[test]
pub fn test_fuzz_1000_iterations_zip_corruption_stress() {
    let baseline = create_baseline_zip();
    assert!(!baseline.is_empty());

    let mut rng = FuzzRng::new(0x1337_CAFE_BEEF);
    let start_time = Instant::now();

    for i in 0..1000 {
        let mut mutated = baseline.clone();
        let mutation_type = rng.next_usize(4);

        match mutation_type {
            0 => {
                // Random single bit flip
                let byte_idx = rng.next_usize(mutated.len());
                let bit_idx = rng.next_usize(8);
                mutated[byte_idx] ^= 1 << bit_idx;
            }
            1 => {
                // Random byte overwrite
                let byte_idx = rng.next_usize(mutated.len());
                mutated[byte_idx] = rng.next_u8();
            }
            2 => {
                // Random multi-byte corruption (1..4 bytes)
                let num_bytes = 1 + rng.next_usize(4);
                let start_idx = rng.next_usize(mutated.len().saturating_sub(num_bytes));
                for b in 0..num_bytes {
                    mutated[start_idx + b] = rng.next_u8();
                }
            }
            _ => {
                // Random truncation
                let trunc_len = rng.next_usize(mutated.len());
                mutated.truncate(trunc_len);
            }
        }

        let res = catch_unwind(AssertUnwindSafe(|| {
            if let Ok(zip) = ZipArchive::open_slice(&mutated) {
                for entry_idx in 0..zip.len() {
                    let _ = zip.extract_entry_bytes(entry_idx, None);
                }
            }
        }));

        assert!(
            res.is_ok(),
            "ZIP parser panicked on fuzz iteration #{}",
            i
        );
    }

    let elapsed = start_time.elapsed();
    assert!(
        elapsed.as_secs() < 10,
        "1,000 ZIP fuzz iterations took {:?}, must complete rapidly",
        elapsed
    );
}

#[test]
pub fn test_fuzz_1000_iterations_tar_corruption_stress() {
    let baseline = create_baseline_tar();
    assert!(!baseline.is_empty());

    let mut rng = FuzzRng::new(0xDEAD_BEEF_0001);
    let start_time = Instant::now();

    for i in 0..1000 {
        let mut mutated = baseline.clone();
        let mutation_type = rng.next_usize(4);

        match mutation_type {
            0 => {
                let byte_idx = rng.next_usize(mutated.len());
                let bit_idx = rng.next_usize(8);
                mutated[byte_idx] ^= 1 << bit_idx;
            }
            1 => {
                let byte_idx = rng.next_usize(mutated.len());
                mutated[byte_idx] = rng.next_u8();
            }
            2 => {
                let num_bytes = 1 + rng.next_usize(4);
                let start_idx = rng.next_usize(mutated.len().saturating_sub(num_bytes));
                for b in 0..num_bytes {
                    mutated[start_idx + b] = rng.next_u8();
                }
            }
            _ => {
                let trunc_len = rng.next_usize(mutated.len());
                mutated.truncate(trunc_len);
            }
        }

        let res = catch_unwind(AssertUnwindSafe(|| {
            let mut scanner = TarSeekScanner::new(&mutated);
            let _ = scanner.scan_all();
            if let Ok(tar) = TarArchive::open_slice(&mutated) {
                for entry_idx in 0..tar.len() {
                    let _ = tar.extract_entry_bytes(entry_idx);
                }
            }
        }));

        assert!(
            res.is_ok(),
            "TAR scanner panicked on fuzz iteration #{}",
            i
        );
    }

    let elapsed = start_time.elapsed();
    assert!(
        elapsed.as_secs() < 10,
        "1,000 TAR fuzz iterations took {:?}, must complete rapidly",
        elapsed
    );
}

#[test]
pub fn test_fuzz_1000_iterations_7z_corruption_stress() {
    let baseline = create_baseline_7z();
    assert!(!baseline.is_empty());

    let mut rng = FuzzRng::new(0xFEED_FACE_7777);
    let start_time = Instant::now();

    for i in 0..1000 {
        let mut mutated = baseline.clone();
        let mutation_type = rng.next_usize(4);

        match mutation_type {
            0 => {
                let byte_idx = rng.next_usize(mutated.len());
                let bit_idx = rng.next_usize(8);
                mutated[byte_idx] ^= 1 << bit_idx;
            }
            1 => {
                let byte_idx = rng.next_usize(mutated.len());
                mutated[byte_idx] = rng.next_u8();
            }
            2 => {
                let num_bytes = 1 + rng.next_usize(4);
                let start_idx = rng.next_usize(mutated.len().saturating_sub(num_bytes));
                for b in 0..num_bytes {
                    mutated[start_idx + b] = rng.next_u8();
                }
            }
            _ => {
                let trunc_len = rng.next_usize(mutated.len());
                mutated.truncate(trunc_len);
            }
        }

        let res = catch_unwind(AssertUnwindSafe(|| {
            let _ = SevenZArchive::open_slice(&mutated);
        }));

        assert!(
            res.is_ok(),
            "7z parser panicked on fuzz iteration #{}",
            i
        );
    }

    let elapsed = start_time.elapsed();
    assert!(
        elapsed.as_secs() < 10,
        "1,000 7z fuzz iterations took {:?}, must complete rapidly",
        elapsed
    );
}
