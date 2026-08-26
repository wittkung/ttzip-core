# Quickstart & Validation Guide: zlib-ng Streaming Fallback Engine

**Feature**: `076-zlib-ng-streaming-fallback`  
**Created**: 2026-08-18  
**Status**: Ready  

---

## Scenario 1: Multi-Format Deflate Streaming Roundtrip Verification

Validate that `DeflateStreamEngine` properly compresses and decompresses data across RFC 1950 (Zlib), RFC 1951 (Raw), and RFC 1952 (GZIP) formats with byte-for-byte fidelity and correct hardware checksums.

### Command
```bash
swift test --filter DeflateStreamCoderTests
```

### Expected Output
```text
Test Suite 'DeflateStreamCoderTests' passed at 2026-08-18 03:16:00.
	 Executed 8 tests, with 0 failures (0 unexpected) in 0.042 seconds
```

### Failure Diagnostic
- **Failure Symptom**: `XCTAssertEqual failed: ("Optional(12345)") is not equal to ("Optional(67890)")` in `testDeflateZlibHeaderRoundtrip`.
- **Root Cause**: Checksum divergence in Adler-32 or CRC-32 calculation.
- **Remedy**: Verify that `ttzip_deflate_stream_process` in `CTTZipStreamCoder.c` correctly updates `state->adler32_checksum` from `strm->adler` and invokes `libdeflate_crc32` / ARMv8 CRC32 on consumed bytes.

---

## Scenario 2: AsyncSequence / AsyncThrowingStream Multi-MegaByte Pipeline

Validate that asynchronous chunk-by-chunk stream compression and decompression handles multi-megabyte streams and concurrent workloads with zero deadlocks and zero memory leaks.

### Command
```bash
swift test --filter DeflateStreamingPipelineTests
```

### Expected Output
```text
Test Suite 'DeflateStreamingPipelineTests' passed at 2026-08-18 03:16:05.
	 Executed 7 tests, with 0 failures (0 unexpected) in 0.128 seconds
```

### Failure Diagnostic
- **Failure Symptom**: Test hangs or triggers a concurrency timeout during `testConcurrentAsyncStreams`.
- **Root Cause**: Mutex lock contention or shared mutable `z_stream` state between tasks.
- **Remedy**: Confirm that each `DeflateStreamCompressor` and `DeflateStreamDecompressor` instance is created independently per async task, without accessing shared mutable state outside its local closure.

---

## Scenario 3: Whole-Buffer Tier 1 Fast-Path Zero-Performance-Regression Gate

Validate that all historical peak performance floors (ZIP compression >= 1,500 MB/s, ZIP decompression >= 7,500 MB/s, 7Z decompression >= 6,600 MB/s) remain 100% compliant.

### Command
```bash
swift test --filter XCTestPerformanceMeasureTests
```

### Expected Output
```text
Test Case '-[TTZipTests.XCTestPerformanceMeasureTests testZipLevel1CompressionPerformance]' passed (average: 1850.42 MB/s).
Test Case '-[TTZipTests.XCTestPerformanceMeasureTests testZipDecompressionPerformance]' passed (average: 11200.15 MB/s).
Test Case '-[TTZipTests.XCTestPerformanceMeasureTests test7zFastDecompressionPerformance]' passed (average: 7450.80 MB/s).
```

### Failure Diagnostic
- **Failure Symptom**: Throughput falls below hard floor (e.g., ZIP compression < 1,500 MB/s).
- **Root Cause**: Fast-path bypass was inadvertently broken or routed to Tier 2 `zlib-ng` instead of `libdeflate`.
- **Remedy**: Check `Sources/TTZipCore/Zip/ZipParallelWriter.swift` and `LibdeflateAccelerator.swift` to ensure `ttzip_libdeflate_compress` TLS pool is invoked directly for whole-buffer operations.
