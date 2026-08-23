# Quickstart & Verification Guide: Multi-Core Optimization Breakdown (Spec 141)

**Feature**: `141-multicore-optimization-breakdown-and-empirical-validation`  
**Date**: 2026-08-20  

---

## Scenario 1: Execute Isolated Multi-Core 8-Point Test Suite

### Command
```bash
swift test --filter MultiCoreOptimizationBreakdownTests
```

### Expected Output
```text
Test Suite 'MultiCoreOptimizationBreakdownTests' started.
Test Case '-[TTZipTests.MultiCoreOptimizationBreakdownTests testOP1_ThreadLocalStorageVsMutexContention]' passed.
Test Case '-[TTZipTests.MultiCoreOptimizationBreakdownTests testOP2_BlockParallel512KBVsSingleThreadedDeflate]' passed.
Test Case '-[TTZipTests.MultiCoreOptimizationBreakdownTests testOP3_MultiTileParallelDecompressionVsSequential]' passed.
Test Case '-[TTZipTests.MultiCoreOptimizationBreakdownTests testOP4_ContainerMultiFilePackagingVsSequential]' passed.
Test Case '-[TTZipTests.MultiCoreOptimizationBreakdownTests testOP5_ContainerMultiFileExtractionVsSequential]' passed.
Test Case '-[TTZipTests.MultiCoreOptimizationBreakdownTests testOP6_ARMv8PMULLVsSoftwareTableCRC32]' passed.
Test Case '-[TTZipTests.MultiCoreOptimizationBreakdownTests testOP7_APFSDirectIOPreallocationVsUnbufferedWrite]' passed.
Test Case '-[TTZipTests.MultiCoreOptimizationBreakdownTests testOP8_TopologyAwareQoSScheduling]' passed.
Executed 8 tests, with 0 failures (0 unexpected).
```

### Failure Diagnostic
- If `testOP1` fails: Check `CTTZipStreamCoder.c` `_Thread_local` compilation support on target toolchain.
- If `testOP2` fails: Ensure system is not under thermal throttling or core-pinned.
- If `testOP6` fails: Verify that ARMv8 Crypto Extensions / `__ARM_FEATURE_CRYPTO` are enabled in compiler flags.

---

## Scenario 2: Full Suite Monotonic Non-Regression Gate

### Command
```bash
swift test
```

### Expected Output
```text
All test targets (TTZipTests, TTZipAppTests) pass with 0 failures, 0 unexpected exits.
```

### Failure Diagnostic
- Check `git status` for untracked duplicate source files or broken module imports.
