// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive corruption injection, chaos mutation, and fuzzing test suite for TTZip MmapSource.
//!
//! Deploys 16 surgical destruction targets:
//! 1. External concurrent file truncation (`ftruncate`) probe & safety bounds enforcement.
//! 2. Sparse file holes cross-page zero-copy reading & zero-fill verification.
//! 3. Extreme offset (`u64::MAX`, `usize::MAX`) overflow and boundary probing.
//! 4. Zero-byte empty file and single-byte edge slice handling.
//! 5. Cross-16KB / 4KB page boundary slice reading with unaligned offsets.
//! 6. 1000+ tasks high-concurrency multi-threaded read-only slice contention.
//! 7. 500+ rounds of pseudo-random slice Seek and read fuzzing.
//! 8. Misaligned offset and misaligned buffer slicing parity.
//! 9. Read-only memory protection invariant and immutability validation.
//! 10. Multi-segment non-uniform slice concatenation parity.
//! 11. Out-of-bounds boundary edge cases (`offset == len`, `offset + len > total_len`).
//! 12. Micro-buffer (1B..16B) tight loop continuous probing.
//! 13. Storage medium detection dispatch (`LocalFastApfs` / `LocalStandard`).
//! 14. Lifecycle & Drop `munmap` idempotence under rapid resource recycling.
//! 15. Chaos random stride multi-threaded concurrent seek pressure test.
//! 16. Chunked slice determinism and cryptographic hash parity.

use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::sync::Arc;
use tempfile::NamedTempFile;
use ttzip_engine::archive::source::{
    detect_storage_medium, open_archive_source, ArchiveSource, MmapSource, StorageMedium,
};
use ttzip_engine::checksum::crc32;

/// High-speed deterministic linear congruential generator for reproducible fuzzing vectors.
#[derive(Clone, Debug)]
struct DeterministicPrng {
    state: u64,
}

impl DeterministicPrng {
    #[inline]
    const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x853c_49e6_748f_ea9b
            } else {
                seed
            },
        }
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
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

    fn fill_bytes(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(4) {
            let bytes = self.next_u32().to_le_bytes();
            let len = chunk.len().min(4);
            chunk[..len].copy_from_slice(&bytes[..len]);
        }
    }
}

/// Helper function to create a temporary test file with deterministic pattern content.
fn create_test_file(size: usize, seed: u64) -> (NamedTempFile, Vec<u8>) {
    let mut temp = NamedTempFile::new().expect("Failed to create temporary file");
    let mut data = vec![0u8; size];
    let mut prng = DeterministicPrng::new(seed);
    prng.fill_bytes(&mut data);
    temp.write_all(&data).expect("Failed to write test data");
    temp.flush().expect("Failed to flush test file");
    (temp, data)
}

// ============================================================================
// Target 1: External Concurrent File Truncation Detection & Bounds Enforcement
// ============================================================================
#[test]
fn test_target_01_external_concurrent_truncation_bounds_enforcement() {
    let (temp_file, _data) = create_test_file(256 * 1024, 0x1122_3344_5566);
    let path = temp_file.path();
    let source = MmapSource::open(path, StorageMedium::LocalStandard)
        .expect("Failed to open MmapSource");
    assert_eq!(source.len(), 256 * 1024);

    // Truncate the backing file on disk to 4KB
    let file = OpenOptions::new()
        .write(true)
        .open(path)
        .expect("Failed to open file for truncation");
    file.set_len(4 * 1024).expect("Failed to truncate file");

    // Read within the new physical length (0..4KB)
    let mut buf = vec![0u8; 4 * 1024];
    let read_bytes = source.read_at(&mut buf, 0).expect("read_at failed");
    assert_eq!(read_bytes, 4 * 1024);

    // Read beyond original length: must safely return Ok(0)
    let mut out_buf = vec![0u8; 1024];
    let oob_read = source
        .read_at(&mut out_buf, 300 * 1024)
        .expect("read_at oob failed");
    assert_eq!(oob_read, 0);

    // Reading at exact original bound
    let bound_read = source
        .read_at(&mut out_buf, 256 * 1024)
        .expect("read_at bound failed");
    assert_eq!(bound_read, 0);
}

// ============================================================================
// Target 2: Sparse File Holes Cross-Page Zero-Copy Reading
// ============================================================================
#[test]
fn test_target_02_sparse_file_holes_zero_copy_reading() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let path = temp_file.path();
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .expect("Failed to open file");

    let sparse_size: u64 = 64 * 1024 * 1024; // 64 MB logical sparse file
    let head_data = b"TTZIP_SPARSE_HEADER_START_BLOCK_V1";
    let tail_data = b"TTZIP_SPARSE_FOOTER_TERMINATOR_END";

    file.write_all(head_data).expect("Failed to write head");
    file.seek(SeekFrom::Start(sparse_size - tail_data.len() as u64))
        .expect("Failed to seek to sparse tail");
    file.write_all(tail_data).expect("Failed to write tail");
    file.flush().expect("Failed to flush sparse file");

    let source = MmapSource::open(path, StorageMedium::LocalFastApfs)
        .expect("Failed to open sparse MmapSource");
    assert_eq!(source.len(), sparse_size);

    // Verify header slice
    let mut head_buf = vec![0u8; head_data.len()];
    let read_head = source.read_at(&mut head_buf, 0).expect("read_at head failed");
    assert_eq!(read_head, head_data.len());
    assert_eq!(&head_buf, head_data);

    // Verify sparse hole (all zeroes across pages)
    let hole_offset = 16 * 1024 * 1024; // 16 MB into the hole
    let mut hole_buf = vec![0xFFu8; 64 * 1024]; // 64 KB chunk
    let read_hole = source
        .read_at(&mut hole_buf, hole_offset)
        .expect("read_at sparse hole failed");
    assert_eq!(read_hole, 64 * 1024);
    assert!(hole_buf.iter().all(|&b| b == 0));

    // Verify footer slice
    let mut tail_buf = vec![0u8; tail_data.len()];
    let tail_offset = sparse_size - tail_data.len() as u64;
    let read_tail = source
        .read_at(&mut tail_buf, tail_offset)
        .expect("read_at tail failed");
    assert_eq!(read_tail, tail_data.len());
    assert_eq!(&tail_buf, tail_data);
}

// ============================================================================
// Target 3: Extreme Offset Overflow & Boundary Probing
// ============================================================================
#[test]
fn test_target_03_extreme_offset_overflow_probing() {
    let (temp_file, _data) = create_test_file(4096, 0xAABB_CCDD_EEFF);
    let source = MmapSource::open(temp_file.path(), StorageMedium::LocalStandard)
        .expect("Failed to open MmapSource");

    let mut buf = [0u8; 64];
    let extreme_offsets = [
        u64::MAX,
        u64::MAX - 1,
        u64::MAX - 4096,
        usize::MAX as u64,
        (u32::MAX as u64) + 1,
        (u32::MAX as u64),
        4096,
        4097,
        10_000_000,
    ];

    for &offset in &extreme_offsets {
        let res = source.read_at(&mut buf, offset);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), 0, "Offset {} must return 0 bytes read", offset);
    }
}

// ============================================================================
// Target 4: Zero-Byte Empty File & Single-Byte Boundary Slice
// ============================================================================
#[test]
fn test_target_04_zero_byte_empty_and_single_byte_boundary() {
    // 1. Zero-byte empty file
    let empty_file = NamedTempFile::new().expect("Failed to create empty temp file");
    let empty_source = MmapSource::open(empty_file.path(), StorageMedium::LocalStandard)
        .expect("Failed to open empty file");
    assert_eq!(empty_source.len(), 0);
    assert!(empty_source.is_empty());
    assert_eq!(empty_source.as_slice(), Some(&[][..]));

    let mut buf = [0u8; 32];
    assert_eq!(empty_source.read_at(&mut buf, 0).unwrap(), 0);
    assert_eq!(empty_source.read_at(&mut buf, 100).unwrap(), 0);

    // 2. Single-byte file
    let mut single_file = NamedTempFile::new().expect("Failed to create single byte file");
    single_file.write_all(&[0x7E]).expect("Failed to write single byte");
    single_file.flush().expect("Failed to flush single byte");

    let single_source = MmapSource::open(single_file.path(), StorageMedium::LocalStandard)
        .expect("Failed to open single byte file");
    assert_eq!(single_source.len(), 1);
    assert!(!single_source.is_empty());
    assert_eq!(single_source.as_slice(), Some(&[0x7Eu8][..]));

    let mut single_buf = [0u8; 4];
    assert_eq!(single_source.read_at(&mut single_buf, 0).unwrap(), 1);
    assert_eq!(single_buf[0], 0x7E);
    assert_eq!(single_source.read_at(&mut single_buf, 1).unwrap(), 0);
}

// ============================================================================
// Target 5: Cross-16KB / 4KB Page Boundary Slicing
// ============================================================================
#[test]
fn test_target_05_cross_page_boundary_slicing() {
    let file_size = 128 * 1024; // 128 KB
    let (temp_file, data) = create_test_file(file_size, 0x1234_5678_9ABC);
    let source = MmapSource::open(temp_file.path(), StorageMedium::LocalStandard)
        .expect("Failed to open MmapSource");

    let page_boundaries = [4096, 8192, 16384, 32768, 65536, 98304];
    let spans = [2, 7, 15, 31, 64, 128, 512, 1024, 4096];

    for &page in &page_boundaries {
        for &span in &spans {
            if span >= page {
                continue;
            }
            let start_offset = (page - (span / 2)) as u64;
            let mut buf = vec![0u8; span];
            let read_bytes = source
                .read_at(&mut buf, start_offset)
                .expect("Cross-page read_at failed");
            assert_eq!(read_bytes, span);
            let expected = &data[start_offset as usize..start_offset as usize + span];
            assert_eq!(&buf, expected, "Mismatch across boundary at {}", page);
        }
    }
}

// ============================================================================
// Target 6: 1000+ Tasks High-Concurrency Multi-Threaded Read Contention
// ============================================================================
#[test]
fn test_target_06_thousand_tasks_high_concurrency_slice_contention() {
    let file_size = 1024 * 1024; // 1 MB
    let (temp_file, data) = create_test_file(file_size, 0xCAFE_BABE_DEAD);
    let source = Arc::new(
        MmapSource::open(temp_file.path(), StorageMedium::LocalFastApfs)
            .expect("Failed to open MmapSource"),
    );
    let reference_data = Arc::new(data);

    let num_tasks = 1000;
    let mut handles = Vec::with_capacity(num_tasks);

    for task_idx in 0..num_tasks {
        let src = Arc::clone(&source);
        let ref_data = Arc::clone(&reference_data);
        let handle = std::thread::spawn(move || {
            let offset = ((task_idx * 1021) % (file_size - 1024)) as u64;
            let len = 512 + (task_idx % 512);
            let mut buf = vec![0u8; len];
            let read_bytes = src.read_at(&mut buf, offset).expect("read_at failed");
            assert_eq!(read_bytes, len);
            assert_eq!(&buf[..len], &ref_data[offset as usize..offset as usize + len]);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Concurrent reader task panicked");
    }
}

// ============================================================================
// Target 7: 500+ Rounds of Pseudo-Random Slice Seek & Read Fuzzing
// ============================================================================
#[test]
fn test_target_07_five_hundred_rounds_random_seek_fuzzing() {
    let file_size = 512 * 1024; // 512 KB
    let (temp_file, data) = create_test_file(file_size, 0xF00D_BA5E_7788);
    let source = MmapSource::open(temp_file.path(), StorageMedium::LocalStandard)
        .expect("Failed to open MmapSource");

    let mut prng = DeterministicPrng::new(0x9988_7766_5544);
    for _ in 0..500 {
        let offset = prng.next_range(0, file_size + 1024) as u64;
        let buf_len = prng.next_range(0, 8192);
        let mut buf = vec![0u8; buf_len];

        let read_bytes = source
            .read_at(&mut buf, offset)
            .expect("Fuzz read_at failed");

        if offset >= file_size as u64 {
            assert_eq!(read_bytes, 0);
        } else {
            let available = (file_size as u64 - offset) as usize;
            let expected_len = buf_len.min(available);
            assert_eq!(read_bytes, expected_len);
            let start = offset as usize;
            assert_eq!(&buf[..read_bytes], &data[start..start + expected_len]);
        }
    }
}

// ============================================================================
// Target 8: Misaligned Offset & Misaligned Buffer Slicing Parity
// ============================================================================
#[test]
fn test_target_08_misaligned_offset_and_buffer_slicing_parity() {
    let file_size = 64 * 1024;
    let (temp_file, data) = create_test_file(file_size, 0x3344_5566_7788);
    let source = MmapSource::open(temp_file.path(), StorageMedium::LocalStandard)
        .expect("Failed to open MmapSource");

    let odd_offsets = [1, 3, 5, 7, 11, 13, 17, 31, 63, 127, 255, 511, 1023, 2047, 4095];
    let odd_lens = [1, 3, 5, 7, 9, 13, 17, 33, 65, 129, 257, 513];

    for &offset in &odd_offsets {
        for &len in &odd_lens {
            let mut buf = vec![0u8; len];
            let read_bytes = source
                .read_at(&mut buf, offset as u64)
                .expect("Misaligned read failed");
            let available = file_size.saturating_sub(offset);
            let expected_len = len.min(available);
            assert_eq!(read_bytes, expected_len);
            assert_eq!(&buf[..read_bytes], &data[offset..offset + expected_len]);
        }
    }
}

// ============================================================================
// Target 9: Read-Only Protection & Memory Immutability Validation
// ============================================================================
#[test]
fn test_target_09_read_only_protection_and_memory_immutability() {
    let file_size = 32 * 1024;
    let (temp_file, data) = create_test_file(file_size, 0x5566_7788_99AA);
    let source = MmapSource::open(temp_file.path(), StorageMedium::LocalFastApfs)
        .expect("Failed to open MmapSource");

    let initial_slice = source.as_slice().expect("Must expose memory slice");
    assert_eq!(initial_slice, &data[..]);

    let initial_crc = crc32(0, initial_slice);

    // Perform multiple read_at calls concurrently
    for offset in (0..file_size).step_by(1024) {
        let mut buf = [0u8; 1024];
        let _ = source.read_at(&mut buf, offset as u64);
    }

    let post_slice = source.as_slice().expect("Must expose memory slice");
    let post_crc = crc32(0, post_slice);
    assert_eq!(initial_crc, post_crc, "Memory slice mutated during read_at!");
}

// ============================================================================
// Target 10: Multi-Segment Non-Uniform Slice Concatenation Parity
// ============================================================================
#[test]
fn test_target_10_multi_segment_slice_concatenation_parity() {
    let file_size = 256 * 1024;
    let (temp_file, data) = create_test_file(file_size, 0x7788_99AA_BBCC);
    let source = MmapSource::open(temp_file.path(), StorageMedium::LocalStandard)
        .expect("Failed to open MmapSource");

    let segment_sizes = [
        17, 33, 127, 256, 1023, 4096, 7777, 16384, 32768, 65535, 12345, 67890,
    ];
    let mut assembled = Vec::with_capacity(file_size);
    let mut cursor: u64 = 0;
    let mut seg_idx = 0;

    while cursor < file_size as u64 {
        let seg_len = segment_sizes[seg_idx % segment_sizes.len()];
        let mut buf = vec![0u8; seg_len];
        let read = source.read_at(&mut buf, cursor).expect("read_at segment failed");
        if read == 0 {
            break;
        }
        assembled.extend_from_slice(&buf[..read]);
        cursor += read as u64;
        seg_idx += 1;
    }

    assert_eq!(assembled.len(), file_size);
    assert_eq!(&assembled, &data);
}

// ============================================================================
// Target 11: Out-of-Bounds Boundary Edge Cases
// ============================================================================
#[test]
fn test_target_11_out_of_bounds_boundary_edge_cases() {
    let file_size = 100;
    let (temp_file, data) = create_test_file(file_size, 0x1111_2222_3333);
    let source = MmapSource::open(temp_file.path(), StorageMedium::LocalStandard)
        .expect("Failed to open MmapSource");

    let mut buf = [0u8; 10];

    // Case 1: offset == len - 1 (1 byte remaining)
    let read1 = source.read_at(&mut buf, 99).expect("read_at 99 failed");
    assert_eq!(read1, 1);
    assert_eq!(buf[0], data[99]);

    // Case 2: offset == len (0 bytes remaining)
    let read2 = source.read_at(&mut buf, 100).expect("read_at 100 failed");
    assert_eq!(read2, 0);

    // Case 3: offset > len
    let read3 = source.read_at(&mut buf, 101).expect("read_at 101 failed");
    assert_eq!(read3, 0);

    // Case 4: zero-length buffer
    let mut empty_buf = [];
    let read4 = source.read_at(&mut empty_buf, 50).expect("read_at empty buf failed");
    assert_eq!(read4, 0);
}

// ============================================================================
// Target 12: Micro-Buffer (1B..16B) Tight Loop Continuous Probing
// ============================================================================
#[test]
fn test_target_12_micro_buffer_tight_loop_continuous_probing() {
    let file_size = 8 * 1024; // 8 KB
    let (temp_file, data) = create_test_file(file_size, 0x2233_4455_6677);
    let source = MmapSource::open(temp_file.path(), StorageMedium::LocalStandard)
        .expect("Failed to open MmapSource");

    for micro_size in [1, 2, 3, 5, 7, 11, 13, 16] {
        let mut reconstructed = Vec::with_capacity(file_size);
        let mut offset = 0u64;
        let mut buf = vec![0u8; micro_size];

        while offset < file_size as u64 {
            let read = source.read_at(&mut buf, offset).expect("read_at failed");
            if read == 0 {
                break;
            }
            reconstructed.extend_from_slice(&buf[..read]);
            offset += read as u64;
        }

        assert_eq!(reconstructed.len(), file_size);
        assert_eq!(&reconstructed, &data, "Failed on micro-size {}", micro_size);
    }
}

// ============================================================================
// Target 13: Storage Medium Detection Dispatch Parity
// ============================================================================
#[test]
fn test_target_13_storage_medium_detection_dispatch() {
    let (temp_file, _data) = create_test_file(1024, 0x4455_6677_8899);
    let path = temp_file.path();

    let detected = detect_storage_medium(path);
    assert!(
        detected == StorageMedium::LocalFastApfs || detected == StorageMedium::LocalStandard,
        "Local temp file must be detected as APFS or Standard filesystem"
    );

    let dynamic_source = open_archive_source(path).expect("open_archive_source failed");
    assert_eq!(dynamic_source.len(), 1024);
    assert_eq!(dynamic_source.medium(), detected);
    assert!(dynamic_source.as_slice().is_some());
}

// ============================================================================
// Target 14: Lifecycle & Drop munmap Idempotence Under Rapid Recycling
// ============================================================================
#[test]
fn test_target_14_lifecycle_and_drop_munmap_idempotence() {
    let (temp_file, _data) = create_test_file(64 * 1024, 0x6677_8899_AABB);
    let path = temp_file.path();

    for _ in 0..200 {
        let source = MmapSource::open(path, StorageMedium::LocalStandard)
            .expect("Failed to open MmapSource in rapid recycling loop");
        assert_eq!(source.len(), 64 * 1024);
        let slice = source.as_slice().expect("Slice must exist");
        assert_eq!(slice.len(), 64 * 1024);
        drop(source);
    }
}

// ============================================================================
// Target 15: Chaos Random Stride Multi-Threaded Concurrent Pressure Test
// ============================================================================
#[test]
fn test_target_15_chaos_random_stride_concurrency() {
    let file_size = 2 * 1024 * 1024; // 2 MB
    let (temp_file, data) = create_test_file(file_size, 0x8899_AABB_CCDD);
    let source = Arc::new(
        MmapSource::open(temp_file.path(), StorageMedium::LocalFastApfs)
            .expect("Failed to open MmapSource"),
    );
    let ref_data = Arc::new(data);

    let num_threads = 8;
    let iterations_per_thread = 100;
    let mut handles = Vec::with_capacity(num_threads);

    for thread_id in 0..num_threads {
        let src = Arc::clone(&source);
        let ref_bytes = Arc::clone(&ref_data);
        let handle = std::thread::spawn(move || {
            let mut prng = DeterministicPrng::new(0x1000 + thread_id as u64);
            for _ in 0..iterations_per_thread {
                let stride = prng.next_range(128, 4096);
                let start_offset = prng.next_range(0, file_size - stride) as u64;
                let mut buf = vec![0u8; stride];
                let read = src.read_at(&mut buf, start_offset).expect("read_at failed");
                assert_eq!(read, stride);
                let expected = &ref_bytes[start_offset as usize..start_offset as usize + stride];
                assert_eq!(&buf, expected);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Worker thread panicked in stride test");
    }
}

// ============================================================================
// Target 16: Chunked Slice Determinism & Cryptographic Hash Parity
// ============================================================================
#[test]
fn test_target_16_chunked_slice_determinism_and_hash_parity() {
    let file_size = 1024 * 1024; // 1 MB
    let (temp_file, _data) = create_test_file(file_size, 0xFEED_FACE_9988);
    let source = MmapSource::open(temp_file.path(), StorageMedium::LocalFastApfs)
        .expect("Failed to open MmapSource");

    let full_slice = source.as_slice().expect("Slice must exist");
    let full_crc = crc32(0, full_slice);

    let mut chunked_crc = 0u32;
    let chunk_size = 32 * 1024; // 32 KB chunks
    let mut offset = 0u64;
    let mut buf = vec![0u8; chunk_size];

    while offset < file_size as u64 {
        let read = source.read_at(&mut buf, offset).expect("read_at failed");
        if read == 0 {
            break;
        }
        chunked_crc = crc32(chunked_crc, &buf[..read]);
        offset += read as u64;
    }

    assert_eq!(full_crc, chunked_crc, "Chunked CRC32 must match whole slice CRC32");
}
