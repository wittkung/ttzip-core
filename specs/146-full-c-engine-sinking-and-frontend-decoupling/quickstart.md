# Quickstart: 146-full-c-engine-sinking-and-frontend-decoupling

## Validation Scenarios

### Scenario 1: Build & Verify Magic Number Sniffing & Natural Sorting
- **Command**: `./build/ttzip-cli --benchmark`
- **Expected Output**: Magic Sniffing & Natural Sorting throughput >= 50M ops/s.
- **Failure Diagnostic**: Check C SIMD / character tables.

### Scenario 2: Build & Verify TAR / 7Z Containers
- **Command**: `./build/ttzip-cli -c test.tar Sources/CTTZipBridge/*.c && ./build/ttzip-cli -t test.tar`
- **Expected Output**: `Archive integrity OK (100% Valid)`
- **Failure Diagnostic**: Check 512-byte block checksums.

### Scenario 3: Full Local CI Verification
- **Command**: `./scripts/local-ci.sh`
- **Expected Output**: `ALL LOCAL CI CHECKS PASSED SUCCESSFULLY (0 Quota)`
- **Failure Diagnostic**: Review test logs.
