# Quickstart: 145-pure-c-container-framing-and-cli-engine

## Validation Scenarios

### Scenario 1: Build & Verify Standalone TTZip CLI
- **Command**: `cmake -B build -DCMAKE_BUILD_TYPE=Release && cmake --build build --config Release`
- **Expected Output**: `Built target ttzip-cli`
- **Failure Diagnostic**: Check C compiler version and CMake include directories.

### Scenario 2: Run Built-In Multi-Core Compression Benchmark
- **Command**: `./build/ttzip-cli --benchmark`
- **Expected Output**: Throughput table for Deflate, Zstd, LZMA2, LZFSE, Snappy, and CRC32/CRC64 hardware benchmarks.
- **Failure Diagnostic**: Verify hardware CPU features.

### Scenario 3: Full Local CI Verification
- **Command**: `./scripts/local-ci.sh`
- **Expected Output**: `ALL LOCAL CI CHECKS PASSED SUCCESSFULLY (0 Quota)`
- **Failure Diagnostic**: Review test failure logs.
