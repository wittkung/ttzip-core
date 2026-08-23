# Quickstart & Verification Guide: Codebase Quality Audit and Optimization

**Feature Branch**: `153-codebase-quality-audit-and-optimization` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

---

## 1. Prerequisites & Environment Setup

- **Operating System**: macOS 14.0 or higher.
- **Toolchain**: Xcode 16+ with Swift 6.0 compiler (`swift-driver` / `swift-tools-version: 6.0`).
- **Dependencies**: Static C frameworks in `Vendor/TTZipVendor.xcframework` and `libTTZipVendor.a`.

---

## 2. Validation Scenarios

### Scenario 1: Clean Compilation Across All Targets

- **Command**:
  ```bash
  swift build --build-tests
  ```
- **Expected Output**:
  ```text
  [14/14] Linking TTZipPackageTests
  Build complete!
  Exit code: 0
  ```
- **Failure Diagnostic**:
  - If `ArchiveError+L10n.swift` fails with missing enum member: Verify `LocaleKey.swift` exports `L10n.Errors` and that `ArchiveError+L10n` references `L10n.Errors.readError` / `L10n.Errors.unsupportedFormat`.
  - If module cache error occurs: Run outside sandbox or clean `.build` directory with `rm -rf .build`.

---

### Scenario 2: Zero Bare Print / Logging Discipline Verification

- **Command**:
  ```bash
  rg -n '(?<!TTLogger\.)(?<!//\s*)print\(' Sources/TTZipCore/ Sources/CTTZipBridge/
  ```
- **Expected Output**:
  - Empty result (0 matching lines in core compression/decompression engines).
- **Failure Diagnostic**:
  - If occurrences are found: Check if the file is in `Sources/TTZipCore/Zip/` or `Sources/CTTZipBridge/`. Replace bare `print(...)` with `TTLogger.shared.debug(...)`, `info(...)`, or `warning(...)`. Note that CLI/TUI formatting tools in `Sources/TTZipCore/CLI/` and `Sources/TTZipBench/` are permitted terminal output writers.

---

### Scenario 3: Localization Catalog Integrity & Parity Test

- **Command**:
  ```bash
  swift test --filter LocalizationIntegrityTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'LocalizationIntegrityTests' passed.
  Executed 3 tests, with 0 failures (0 unexpected) in 0.045 seconds.
  ```
- **Failure Diagnostic**:
  - If missing key errors are reported: Check `LocaleCatalog+*.swift` for the missing string key reported in test assertion and insert the translated key-value pair.

---

### Scenario 4: Full Automated Regression Test Suite

- **Command**:
  ```bash
  swift test --filter "TTZipTests"
  ```
- **Expected Output**:
  ```text
  Test Suite 'All tests' passed.
  Executed 525+ tests, with 0 failures (0 unexpected).
  ```
- **Failure Diagnostic**:
  - Inspect test failure log in XCTest report. Verify that C bridge modifications did not introduce null pointer dereferences or memory corruptions.

---

### Scenario 5: Throughput Floor & Performance Benchmark Verification

- **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'XCTestPerformanceMeasureTests' passed.
  ZIP L1: >= 1500 MB/s
  ZIP L6: >= 800 MB/s
  ZIP Decompress: >= 4500 MB/s
  ```
- **Failure Diagnostic**:
  - Check CPU thermal throttling or background system load. Verify NEON SIMD optimizations in `CTTZipBridge` are active.
