# Quickstart Validation: In-Process 18-Core Parallel Zopfli/Advzip Engine

**Feature**: `specs/101-inprocess-parallel-zopfli-advzip-engine`

## Verification Scenarios

### Scenario 1: Verify Zero External Process Calls in ZipExtremeBlockWriter
- **Command**: `grep -n "Process()" Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`
- **Expected Output**: Empty (zero occurrences).
- **Failure Diagnostic**: If matched, replace with in-process C function call.

### Scenario 2: Verify Pigz All 11-Level Matrix
- **Command**: `swift test --filter ZipMultiCoreParetoFrontierPkTests`
- **Expected Output**: All 11 pigz levels (0..9, 11) tested and charted on the Pareto graph.
