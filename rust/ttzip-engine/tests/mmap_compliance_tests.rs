// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Official memmap2 cross-platform compliance test suite and TTZip 6-layer defense verification.

use std::fs::{self, File, OpenOptions};
use std::sync::Arc;
use tempfile::tempdir;
use ttzip_engine::security::{
    safe_map_anonymous, safe_map_file, safe_map_file_range, system_page_size,
    MmapResidentMemoryGuard, MmapResourceGuard, PageBoundaryGuard, SafeMmapOptions,
    SafeMmapView, TruncationGuard,
};
use ttzip_engine::types::TTZipStatus;

#[test]
fn test_empty_file_mapping() {
    let dir = tempdir().expect("Failed to create tempdir");
    let path = dir.path().join("empty.dat");
    File::create(&path).expect("Failed to create empty file");

    let file = File::open(&path).expect("Failed to open empty file");
    let view = safe_map_file(&file).expect("Failed to map empty file");

    assert_eq!(view.len(), 0);
    assert!(view.is_empty());
    assert_eq!(view.as_slice(), b"");
    assert_eq!(&*view, b"");
    assert_eq!(view.as_ref(), b"");

    // Slicing empty view
    assert_eq!(view.slice(0, 0).expect("Empty slice from empty view"), b"");
    assert_eq!(
        view.slice(0, 1).unwrap_err(),
        TTZipStatus::ErrInvalidOffset
    );
}

#[test]
fn test_empty_range_mapping() {
    let dir = tempdir().expect("Failed to create tempdir");
    let path = dir.path().join("data.dat");
    let payload = b"Hello, TTZip Safe Mmap Defense!";
    fs::write(&path, payload).expect("Failed to write test file");

    let file = File::open(&path).expect("Failed to open file");
    let view = safe_map_file_range(&file, 5, 0).expect("Failed to map 0-len range");

    assert_eq!(view.len(), 0);
    assert!(view.is_empty());
    assert_eq!(view.as_slice(), b"");
}

#[test]
fn test_full_file_mapping() {
    let dir = tempdir().expect("Failed to create tempdir");
    let path = dir.path().join("full.dat");
    let payload = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    fs::write(&path, payload).expect("Failed to write file");

    let file = File::open(&path).expect("Failed to open file");
    let view = safe_map_file(&file).expect("Failed to map file");

    assert_eq!(view.len(), payload.len());
    assert!(!view.is_empty());
    assert_eq!(view.as_slice(), payload);
    assert_eq!(&*view, payload);
    assert_eq!(view.as_ref(), payload);
}

#[test]
fn test_range_mapping_aligned_and_unaligned() {
    let dir = tempdir().expect("Failed to create tempdir");
    let path = dir.path().join("range.dat");

    // Create 32KB patterned file to test cross-page offsets
    let mut payload = Vec::with_capacity(32 * 1024);
    for i in 0..(32 * 1024) {
        payload.push((i % 251) as u8);
    }
    fs::write(&path, &payload).expect("Failed to write patterned file");

    let file = File::open(&path).expect("Failed to open file");

    // 1. Aligned to 4KB
    let view_aligned = safe_map_file_range(&file, 4096, 2048).expect("Failed aligned map");
    assert_eq!(view_aligned.len(), 2048);
    assert_eq!(view_aligned.as_slice(), &payload[4096..6144]);

    // 2. Unaligned offset (e.g. 17 bytes in)
    let view_unaligned = safe_map_file_range(&file, 17, 100).expect("Failed unaligned map");
    assert_eq!(view_unaligned.len(), 100);
    assert_eq!(view_unaligned.as_slice(), &payload[17..117]);

    // 3. Unaligned offset across page boundary (e.g. 4090 to 4110)
    let view_cross = safe_map_file_range(&file, 4090, 20).expect("Failed cross-page map");
    assert_eq!(view_cross.len(), 20);
    assert_eq!(view_cross.as_slice(), &payload[4090..4110]);
}

#[test]
fn test_page_boundary_guard_modular_arithmetic() {
    let page_sizes = [4096, 16384, 65536];

    for &page_size in &page_sizes {
        // Offset 0
        let a0 = PageBoundaryGuard::compute_alignment(0, 100, page_size)
            .expect("Alignment failed for 0");
        assert_eq!(a0.aligned_offset, 0);
        assert_eq!(a0.page_offset, 0);
        assert_eq!(a0.aligned_len, 100);

        // Offset 1
        let a1 = PageBoundaryGuard::compute_alignment(1, 100, page_size)
            .expect("Alignment failed for 1");
        assert_eq!(a1.aligned_offset, 0);
        assert_eq!(a1.page_offset, 1);
        assert_eq!(a1.aligned_len, 101);

        // Exact page boundary
        let ap = PageBoundaryGuard::compute_alignment(page_size as u64, 500, page_size)
            .expect("Alignment failed for page_size");
        assert_eq!(ap.aligned_offset, page_size as u64);
        assert_eq!(ap.page_offset, 0);
        assert_eq!(ap.aligned_len, 500);

        // Page boundary + 15
        let ap15 = PageBoundaryGuard::compute_alignment((page_size as u64) + 15, 200, page_size)
            .expect("Alignment failed for page_size + 15");
        assert_eq!(ap15.aligned_offset, page_size as u64);
        assert_eq!(ap15.page_offset, 15);
        assert_eq!(ap15.aligned_len, 215);
    }

    // Invalid page sizes
    assert_eq!(
        PageBoundaryGuard::compute_alignment(0, 10, 0).unwrap_err(),
        TTZipStatus::ErrInvalidParam
    );
    assert_eq!(
        PageBoundaryGuard::compute_alignment(0, 10, 3000).unwrap_err(),
        TTZipStatus::ErrInvalidParam
    );

    // Page alignment check
    assert!(PageBoundaryGuard::is_page_aligned(0, 4096));
    assert!(PageBoundaryGuard::is_page_aligned(4096, 4096));
    assert!(PageBoundaryGuard::is_page_aligned(8192, 4096));
    assert!(!PageBoundaryGuard::is_page_aligned(1, 4096));
    assert!(!PageBoundaryGuard::is_page_aligned(4095, 4096));
}

#[test]
fn test_anonymous_mapping() {
    let view = safe_map_anonymous(4096).expect("Failed anonymous map");
    assert_eq!(view.len(), 4096);
    assert!(!view.is_empty());

    // Anonymous memory is zero-initialized
    for &byte in view.as_slice() {
        assert_eq!(byte, 0);
    }

    // Empty anonymous map
    let empty_view = safe_map_anonymous(0).expect("Failed empty anonymous map");
    assert_eq!(empty_view.len(), 0);
    assert!(empty_view.is_empty());
}

#[test]
fn test_multithreaded_concurrent_reads() {
    let dir = tempdir().expect("Failed to create tempdir");
    let path = dir.path().join("concurrent.dat");

    let size = 1024 * 1024; // 1 MB
    let mut payload = Vec::with_capacity(size);
    for i in 0..size {
        payload.push(((i * 17 + 31) % 256) as u8);
    }
    fs::write(&path, &payload).expect("Failed to write concurrent test file");

    let file = File::open(&path).expect("Failed to open file");
    let view = Arc::new(safe_map_file(&file).expect("Failed to map file"));
    let expected = Arc::new(payload);

    let mut handles = Vec::new();
    let thread_count = 16;
    let iterations_per_thread = 200;

    for t in 0..thread_count {
        let v = Arc::clone(&view);
        let exp = Arc::clone(&expected);

        handles.push(std::thread::spawn(move || {
            for i in 0..iterations_per_thread {
                let offset = (t * 1000 + i * 37) % (size - 256);
                let len = 128;
                let slice = v.slice(offset, len).expect("Slice out of bounds");
                assert_eq!(slice, &exp[offset..offset + len]);
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Thread panicked during concurrent read");
    }
}

#[test]
fn test_truncation_guard_out_of_bounds() {
    let dir = tempdir().expect("Failed to create tempdir");
    let path = dir.path().join("small.dat");
    fs::write(&path, b"1234567890").expect("Failed to write file");

    let file = File::open(&path).expect("Failed to open file");

    // 1. Offset beyond EOF
    let err1 = safe_map_file_range(&file, 15, 5).unwrap_err();
    assert_eq!(err1, TTZipStatus::ErrInvalidOffset);

    // 2. Offset + len > EOF
    let err2 = safe_map_file_range(&file, 5, 10).unwrap_err();
    assert_eq!(err2, TTZipStatus::ErrInvalidOffset);

    // 3. TruncationGuard direct bounds check
    assert_eq!(
        TruncationGuard::validate_bounds(5, 10, 10).unwrap_err(),
        TTZipStatus::ErrInvalidOffset
    );
    assert!(TruncationGuard::validate_bounds(5, 5, 10).is_ok());
}

#[test]
fn test_toctou_live_file_shrinkage() {
    let dir = tempdir().expect("Failed to create tempdir");
    let path = dir.path().join("shrinkable.dat");
    fs::write(&path, vec![0xAA; 4096]).expect("Failed to write file");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .expect("Failed to open file");

    // Validate normal size
    let size = TruncationGuard::validate_live_file_size(&file, 4096)
        .expect("Live size check failed");
    assert_eq!(size, 4096);

    // Concurrently truncate file to 1024 bytes
    file.set_len(1024).expect("Failed to truncate file");

    // Live validation must detect shrinkage and reject
    let err = TruncationGuard::validate_live_file_size(&file, 4096).unwrap_err();
    assert_eq!(err, TTZipStatus::ErrSecurityViolation);
}

#[test]
fn test_extreme_negative_and_overflow_offsets() {
    let dir = tempdir().expect("Failed to create tempdir");
    let path = dir.path().join("overflow.dat");
    fs::write(&path, b"Payload").expect("Failed to write file");

    let file = File::open(&path).expect("Failed to open file");

    // Offset u64::MAX
    let err_max = safe_map_file_range(&file, u64::MAX, 10).unwrap_err();
    assert_eq!(err_max, TTZipStatus::ErrInvalidOffset);

    // Offset + len overflow
    let err_overflow = safe_map_file_range(&file, u64::MAX - 5, 20).unwrap_err();
    assert_eq!(err_overflow, TTZipStatus::ErrInvalidOffset);

    // Direct TruncationGuard overflow check
    assert_eq!(
        TruncationGuard::validate_bounds(u64::MAX, 1, 1000).unwrap_err(),
        TTZipStatus::ErrInvalidOffset
    );
}

#[test]
fn test_resident_memory_budget_circuit_breaker() {
    let dir = tempdir().expect("Failed to create tempdir");
    let path = dir.path().join("budget.dat");
    fs::write(&path, vec![0x42; 2048]).expect("Failed to write file");

    let file = File::open(&path).expect("Failed to open file");

    // Budget limit = 1024 bytes, requested = 2048 bytes
    let err = SafeMmapOptions::new()
        .max_resident_limit(1024)
        .map_file(&file)
        .unwrap_err();
    assert_eq!(err, TTZipStatus::ErrOutOfMemory);

    // Budget limit = 4096 bytes, requested = 2048 bytes -> OK
    let ok = SafeMmapOptions::new()
        .max_resident_limit(4096)
        .map_file(&file)
        .expect("Should succeed within budget");
    assert_eq!(ok.len(), 2048);

    // Direct guard validation
    let direct_guard = MmapResidentMemoryGuard::new(1024);
    assert_eq!(
        direct_guard.validate_budget(2048).unwrap_err(),
        TTZipStatus::ErrOutOfMemory
    );
    assert!(direct_guard.validate_budget(512).is_ok());
    assert_eq!(MmapResidentMemoryGuard::DEFAULT_LIMIT, 64 * 1024 * 1024);
}

#[test]
fn test_resource_handle_raii_tracking() {
    let baseline_count = MmapResourceGuard::active_count();
    let baseline_bytes = MmapResourceGuard::allocated_bytes();

    {
        let view1 = safe_map_anonymous(1024).expect("Failed map 1");
        assert!(MmapResourceGuard::active_count() > baseline_count);
        assert!(MmapResourceGuard::allocated_bytes() >= baseline_bytes + 1024);

        {
            let view2 = safe_map_anonymous(2048).expect("Failed map 2");
            assert!(MmapResourceGuard::active_count() >= baseline_count + 2);
            assert!(MmapResourceGuard::allocated_bytes() >= baseline_bytes + 3072);
            assert!(MmapResourceGuard::peak_count() >= baseline_count + 2);
            assert_eq!(view2.len(), 2048);
        }

        // view2 dropped
        assert!(MmapResourceGuard::active_count() > baseline_count);
        assert_eq!(view1.len(), 1024);
    }
}

#[test]
fn test_subslice_bounds_and_errors() {
    let view = safe_map_anonymous(100).expect("Failed map");

    // Valid sub-slices
    assert_eq!(view.slice(0, 50).expect("Slice 0..50").len(), 50);
    assert_eq!(view.slice(50, 50).expect("Slice 50..100").len(), 50);
    assert_eq!(view.slice(100, 0).expect("Slice 100..100").len(), 0);

    // Out of bounds sub-slices
    assert_eq!(
        view.slice(50, 51).unwrap_err(),
        TTZipStatus::ErrInvalidOffset
    );
    assert_eq!(
        view.slice(101, 0).unwrap_err(),
        TTZipStatus::ErrInvalidOffset
    );
    assert_eq!(
        view.slice(usize::MAX, 1).unwrap_err(),
        TTZipStatus::ErrInvalidOffset
    );
}

#[test]
fn test_madvise_hints() {
    let view = safe_map_anonymous(4096).expect("Failed anonymous map");
    assert!(view.advise_sequential().is_ok());
    assert!(view.advise_dontneed().is_ok());

    // Empty view advice is no-op and succeeds
    let empty = SafeMmapView::empty();
    assert!(empty.advise_sequential().is_ok());
    assert!(empty.advise_dontneed().is_ok());
}

#[test]
fn test_system_page_size_resolution() {
    let sz = system_page_size();
    assert!(sz >= 4096);
    assert!(sz.is_power_of_two());
}
