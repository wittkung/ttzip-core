# Quickstart Validation Guide: Complete Optimization Wiring & Configuration Creep Audit

**Feature**: `specs/093-complete-optimization-wiring-and-configuration-creep-audit`  
**Date**: 2026-08-18  

---

## Scenario 1: Zero-Allocation TAR Streaming Write Audit

### Purpose
Verify that writing 10,000 files into a TAR archive executes with exactly 0 dynamic heap allocations in `write_reg_file_data` and uses 64KB stack buffer fallback.

### Command
```bash
swift test --filter ExhaustiveOptimizationAuditTests/testTarNativeZeroAllocationStreamingWrite
```

### Expected Output
```text
Test Case '-[TTZipTests.ExhaustiveOptimizationAuditTests testTarNativeZeroAllocationStreamingWrite]' passed (0.012 seconds).
```

---

## Scenario 2: 16-Format Full-Stack Exhaustive Dispatch Audit

### Purpose
Verify that all 16 formats (ZIP, 7Z, TAR, TAR.ZST, TAR.GZ, TAR.BZ2, TAR.XZ, WIM, DMG, ISO, LZ4, LZIP, LRZIP, AAR, BROTLI, SNAPPY) execute through dedicated in-process C engines with 100% byte-exact SHA-256 round-trip integrity.

### Command
```bash
swift test --filter ExhaustiveOptimizationAuditTests/testAll16FormatsDirectInProcessExecution
```

### Expected Output
```text
Test Case '-[TTZipTests.ExhaustiveOptimizationAuditTests testAll16FormatsDirectInProcessExecution]' passed (0.450 seconds).
```
