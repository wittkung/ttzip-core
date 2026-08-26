# Quickstart & Verification Guide: Deep Code Quality, Memory Safety, and Invariant Hardening

**Feature Branch**: `157-deep-code-quality-and-memory-safety-hardening` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

---

## 1. Prerequisites & Environment Setup

- **Toolchain**: macOS 14.0+, Xcode 16+ with Swift 6.0 and Clang C11.
- **Verification Framework**: `swift test` (XCTest harness).

---

## 2. Validation Scenarios

### Scenario 1: Clean Compilation & Strict Concurrency Build

- **Command**:
  ```bash
  swift build --build-tests
  ```
- **Expected Output**:
  ```text
  Build complete!
  Exit code: 0
  ```
- **Failure Diagnostic**:
  - Inspect any compiler warnings or link errors. Ensure all C bridge signatures match headers in `include/`.

---

### Scenario 2: Memory Safety & Archive Inspection Regression Suite

- **Command**:
  ```bash
  swift test --filter ArchiveInspectionTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'ArchiveInspectionTests' passed.
  Executed all tests with 0 failures.
  ```
- **Failure Diagnostic**:
  - Check for SIGSEGV or double free crashes in Snappy/TAR inspection routines.

---

### Scenario 3: Full Core Regression Suite

- **Command**:
  ```bash
  swift test --filter TTZipTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'TTZipTests' passed.
  Executed 339+ tests with 0 failures.
  ```
- **Failure Diagnostic**:
  - Check XCTest assertion failures for any broken format or engine logic.

---

### Scenario 4: App & View Concurrency Suite

- **Command**:
  ```bash
  swift test --filter TTZipAppTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'TTZipAppTests' passed.
  Executed 195 tests with 0 failures.
  ```
- **Failure Diagnostic**:
  - Check for async timeout or thread isolation assertion errors in password vault or tree tests.
