# Quickstart & Verification Guide: Libdeflate Architecture Integration

**Feature**: [062-libdeflate-architecture-integration](spec.md)

---

## Validation Scenario 1: Libdeflate Swift Adapter & Raw Pointer Roundtrip

### Command
```bash
swift test --filter LibdeflateAcceleratorTests
```

### Expected Output
```text
Test Suite 'LibdeflateAcceleratorTests' passed at ...
Executed 1 test, with 0 failures (0 unexpected) in 0.005 seconds
```

### Failure Diagnostic
- If roundtrip assertion fails, verify that `LibdeflateCAdapter` pointer offsets correctly pass `srcSize` and `dstCapacity` to `ttzip_libdeflate_compress` / `ttzip_libdeflate_decompress`.

---

## Validation Scenario 2: High-Throughput Performance Gate & Zero-Regression Floor

### Command
```bash
swift test --filter XCTestPerformanceMeasureTests
```

### Expected Output
```text
Test Suite 'XCTestPerformanceMeasureTests' passed ...
ZIP Level 1 Compression throughput >= 1500 MB/s
ZIP Decompression throughput >= 7500 MB/s
```

### Failure Diagnostic
- If throughput drops below floor, check thread-local decompressor initialization and ensure no `malloc` is executed in hot decompression loops.

---

## Validation Scenario 3: Large-File Chunked DEFLATE Streaming Integrity

### Command
```bash
swift test --filter ChunkedDeflateStreamWriterTests
```

### Expected Output
```text
Test Suite 'ChunkedDeflateStreamWriterTests' passed ...
Executed all tests with 0 failures
```

### Failure Diagnostic
- If uncompressed size or CRC32 mismatch occurs, inspect `ttzip_zip_chunked_stream_finish` and verify `running_crc32` accumulator.
