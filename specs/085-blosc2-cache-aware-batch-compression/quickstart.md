# Quickstart Guide: Blosc2 Cache-Aware Batch Compression Pipeline

**Feature**: `085-blosc2-cache-aware-batch-compression`
**Date**: 2026-08-18
**Status**: Ready

---

## Prerequisites
- macOS 14.0+ on Apple Silicon (M1/M2/M3/M4) or Intel x86_64
- Swift 6.0 toolchain (`swift --version` >= 6.0)
- System extraction utilities installed: `/usr/bin/unzip`, `/usr/bin/tar`

---

## Validation Scenarios

### Scenario 1: 500 Small Files High-Throughput Batch Zip Creation

Validates that compressing 500 small files (<64KB, ~2MB total) executes through the cache-aware batch clustering pipeline without errors and achieves high throughput.

- **Command**:
  ```bash
  swift test --filter testZipBatchSmallFiles_XCTestMeasureMetrics
  ```
- **Expected Output**:
  ```text
  Test Case '-[TTZipTests.XCTestPerformanceMeasureTests testZipBatchSmallFiles_XCTestMeasureMetrics]' passed (0.XXX seconds).
  Executed 1 test, with 0 failures (0 unexpected) in 0.XXX (0.XXX) seconds
  ```
- **Failure Diagnostic**:
  - If throughput drops below 50 MB/s (Debug) or 70 MB/s (Release), check if `batchWorkUnit` clustering is enabled and verify that `payload_arena` has 128-byte cache-line alignment.
  - If memory allocation fails, verify `posix_memalign` returned `0` and size calculations do not overflow.

---

### Scenario 2: Bidirectional Differential Verification with System `/usr/bin/unzip`

Validates that archives produced by the cache-aware batch compression engine strictly conform to standard PKWARE ZIP container specifications and extract cleanly via macOS native utilities.

- **Command**:
  ```bash
  swift test --filter ArchiveWriterTests/testZipDifferentialWithSystemUnzip
  ```
- **Expected Output**:
  ```text
  Test Case '-[TTZipTests.ArchiveWriterTests testZipDifferentialWithSystemUnzip]' passed.
  ```
- **Failure Diagnostic**:
  - If `/usr/bin/unzip` reports `checksum mismatch` or `corrupted local header`, verify that the 128-byte aligned offsets in `ttzip_write_zip_archive_disk` correctly record accurate `uncompressed_size`, `compressed_size`, and ARM NEON CRC32 checksums.

---

### Scenario 3: Mixed Hierarchy (Small + Large Files) Tiering Verification

Validates that directories containing both small files (<64KB) and large files (>16MB) smoothly split into Tier 1 batch units and Tier 2/3 direct parallel streams without stalls.

- **Command**:
  ```bash
  swift test --filter ArchiveWriterTests/testMixedDirectoryBatchArchiving
  ```
- **Expected Output**:
  ```text
  Test Case '-[TTZipTests.ArchiveWriterTests testMixedDirectoryBatchArchiving]' passed.
  ```
- **Failure Diagnostic**:
  - If large files are mistakenly routed into small batch units, verify that `uncompressed_size < 64 * 1024` filtering correctly isolates small items from direct parallel streams.

---

### Scenario 4: Hardware Performance Measure Floor Enforcement

Validates that all existing hot-path throughput floors across ZIP, 7Z, TAR.ZST, and LZ4 remain 100% green with zero regression.

- **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests
  ```
- **Expected Output**:
  ```text
  Executed 10 tests, with 0 failures (0 unexpected)
  ```
- **Failure Diagnostic**:
  - If any large-file or decompression test fails, verify that fast-path bypasses for `.sevenZip`, `.store`, and single large files remain untouched in `ArchiveWriter+Dispatch.swift`.
