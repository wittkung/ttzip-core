# Quickstart: 149-cross-platform-cmake-and-c-sdk-quickstart

## Validation Scenarios

### Scenario 1: Build and Run C SDK Quickstart Example
- **Command**: `cmake --build build --target ttzip-quickstart && ./build/ttzip-quickstart`
- **Expected Output**:
  ```text
  [TTZip C SDK Quickstart]
  API Version: 1.0.0
  [Demo 1] Hardware CRC32 Checksum: PASS
  [Demo 2] In-Memory Zstd Compression: PASS
  [Demo 3] Constant-Time Magic Sniffing: PASS
  [Demo 4] C11 Natural Numeric Sort: PASS
  All 4 SDK demonstrations completed successfully!
  ```

### Scenario 2: Full Local CI Verification
- **Command**: `./scripts/local-ci.sh`
- **Expected Output**: `ALL LOCAL CI CHECKS PASSED SUCCESSFULLY (0 Quota)`
