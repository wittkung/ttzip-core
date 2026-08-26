# Quickstart Validation: Genuine Libdeflate DAG Routing & C-Bridge Disconnect Audit

**Feature**: `specs/100-zip-genuine-libdeflate-dag-and-audit`

## Verification Scenarios

### Scenario 1: Verify Zero Magic Degradation in `CTTZipStreamCoder.c`
- **Command**: `grep -n "level == 6" Sources/CTTZipBridge/CTTZipStreamCoder.c`
- **Expected Output**: Empty (zero occurrences).
- **Failure Diagnostic**: If matched, remove the line modifying level 6 to level 4.

### Scenario 2: Verify ZIP64 64-bit Entry Support in `CTTZipExtract.c`
- **Command**: `grep -n "uint64_t total_entries" Sources/CTTZipBridge/CTTZipExtract.c`
- **Expected Output**: Match showing `uint64_t total_entries`.
- **Failure Diagnostic**: If `uint16_t` is found, change it to `uint64_t`.

### Scenario 3: Verify Zip Multi-Core Pareto Frontier
- **Command**: `swift test --filter ZipMultiCoreParetoFrontierPkTests`
- **Expected Output**: Test passed, Pareto chart generated with genuine 7 tiers.
