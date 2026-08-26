# Quickstart & Verification Guide: 081-professional-competitor-gap-audit

**Feature**: TTZip 对标顶级专业归档软件全维度差距审计与深度能力补齐  
**Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/081-professional-competitor-gap-audit/spec.md)  
**Date**: 2026-08-18  

---

## 1. Multi-Volume Creation Verification (US1)

### Scenario 1.1: Create 7Z 10MB Split Archive and Verify with 7-Zip / Unarchiver
- **Command**:
  ```bash
  swift test --filter SplitVolumeCreationTests
  ```
- **Expected Output**:
  ```
  Test Case '-[TTZipTests.SplitVolumeCreationTests testCreateAndExtract7zSplitArchive]' passed (0.015 seconds).
  Test Case '-[TTZipTests.SplitVolumeCreationTests testCreateAndExtractZipSplitArchive]' passed (0.012 seconds).
  ```
- **Failure Diagnostic**:
  - Check `MultiVolumeStreamSink.swift` boundary calculation.
  - Verify filename formatting matches `.7z.001` or `.z01`.

---

## 2. Reed-Solomon Recovery Record & Self-Healing Verification (US2)

### Scenario 2.1: Inject 512B Sector Corruption and Auto-Heal Archive
- **Command**:
  ```bash
  swift test --filter ReedSolomonRecoveryRecordTests
  ```
- **Expected Output**:
  ```
  Test Case '-[TTZipTests.ReedSolomonRecoveryRecordTests testRecoveryRecordInjectionAndSelfHealing]' passed (0.025 seconds).
  ```
- **Failure Diagnostic**:
  - Check `ReedSolomonFEC.swift` Galois field $\text{GF}(2^{16})$ matrix inversion.
  - Verify `TTZR` header CRC32 check passes.

---

## 3. Sub-15ms In-Archive Search Verification (US3)

### Scenario 3.1: 100,000 Node Flat Index Benchmark
- **Command**:
  ```bash
  swift test --filter InArchiveSearchEngineTests
  ```
- **Expected Output**:
  ```
  [SEARCH BENCHMARK] Scanned 100,000 items in 2.35 ms (Rate: 42.5M items/s) -> PASS
  ```
- **Failure Diagnostic**:
  - Ensure search is executed on contiguous UTF-8 buffer (`rawNormalizedBuffer`) without per-element `String` allocations.

---

## 4. Touch ID & 7Z Header Encryption Verification (US4)

### Scenario 4.1: 7Z Encrypted Header (-mhe) Parsing
- **Command**:
  ```bash
  swift test --filter StandardsComplianceTests/testSevenZipEncryptedHeaderMetadataMasking
  ```
- **Expected Output**:
  ```
  Test Case '-[TTZipTests.StandardsComplianceTests testSevenZipEncryptedHeaderMetadataMasking]' passed (0.005 seconds).
  ```

---

## 5. GUI Real-Time MIPS Benchmark Engine Verification (US5)

### Scenario 5.1: Multi-Core MIPS Benchmark Suite
- **Command**:
  ```bash
  swift test --filter MIPSBenchmarkEngineTests
  ```
- **Expected Output**:
  ```
  [MIPS BENCHMARK] Compress: 42,500 MIPS | Decompress: 48,200 MIPS | Rating/Usage: 5,420 MIPS/core -> PASS
  ```
