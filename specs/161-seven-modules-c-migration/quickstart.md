# Phase 1: Quickstart & Verification Playbook

**Feature**: `161-seven-modules-c-migration`  
**Date**: 2026-08-20  

---

## Verification Scenarios

### Scenario 1: Reed-Solomon Encoding & Self-Healing Decode Verification
- **Command**:
  ```bash
  /Applications/Xcode.app/Contents/Developer/usr/bin/xctest -XCTest ReedSolomonRecoveryRecordTests .build/arm64-apple-macosx/debug/TTZipPackageTests.xctest
  ```
- **Expected Output**:
  ```text
  Test Suite 'ReedSolomonRecoveryRecordTests' passed
  Executed 2 tests, with 0 failures (0 unexpected)
  ```
- **Failure Diagnostic**:
  - Check `ttzip_rs_create_cauchy_matrix` for correct Galois Field inverse computation.
  - Verify that NEON nibble lookup table `vqtbl1q_u8` matches scalar `ttzip_rs_gf_mul`.

---

### Scenario 2: ZIP Extra Field TLV Parsing Verification
- **Command**:
  ```bash
  /Applications/Xcode.app/Contents/Developer/usr/bin/xctest -XCTest ZipExtraFieldParserTests .build/arm64-apple-macosx/debug/TTZipPackageTests.xctest
  ```
- **Expected Output**:
  ```text
  Test Suite 'ZipExtraFieldParserTests' passed
  Executed 9 tests, with 0 failures (0 unexpected)
  ```
- **Failure Diagnostic**:
  - Check unaligned 16/32/64-bit load offsets and buffer bounds checks in `ttzip_zip_parse_extra_fields`.
  - Validate that `0x7075` Unicode Path CRC-32 matches standard filename CRC.

---

### Scenario 3: Flat Columnar In-Archive Search Verification
- **Command**:
  ```bash
  /Applications/Xcode.app/Contents/Developer/usr/bin/xctest -XCTest InArchiveSearchEngineTests .build/arm64-apple-macosx/debug/TTZipPackageTests.xctest
  ```
- **Expected Output**:
  ```text
  Test Suite 'InArchiveSearchEngineTests' passed
  Executed 2 tests, with 0 failures (0 unexpected)
  ```
- **Failure Diagnostic**:
  - Check `ttzip_search_index_add_entry` contiguous buffer capacity reallocation.
  - Ensure NEON string search correctly matches lowercase query bytes.

---

### Scenario 4: N-Dimensional Tensor Hypercube Slicing Verification
- **Command**:
  ```bash
  /Applications/Xcode.app/Contents/Developer/usr/bin/xctest -XCTest NDimTensorHypercubeSlicingTests .build/arm64-apple-macosx/debug/TTZipPackageTests.xctest
  ```
- **Expected Output**:
  ```text
  Test Suite 'NDimTensorHypercubeSlicingTests' passed
  Executed 1 test, with 0 failures (0 unexpected)
  ```
- **Failure Diagnostic**:
  - Check `ttzip_tensor_find_intersecting_blocks` for correct bounding-box coordinate overlap logic.

---

### Scenario 5: Full Regression Suite Verification
- **Command**:
  ```bash
  /Applications/Xcode.app/Contents/Developer/usr/bin/xctest .build/arm64-apple-macosx/debug/TTZipPackageTests.xctest
  ```
- **Expected Output**:
  ```text
  Executed 912 tests, with 0 failures (0 unexpected)
  ```
- **Failure Diagnostic**:
  - Inspect test failure log in `.system_generated/tasks/` for specific assertion failures or stack traces.
