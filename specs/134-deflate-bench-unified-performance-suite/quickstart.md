# Quickstart & Verification Guide: Deflate-Bench Unified Performance & Test Suite Modernization

**Feature Directory**: `specs/134-deflate-bench-unified-performance-suite`  
**Target Subject**: 50 点纯内存基准测试验证与快速单测验证  

---

## 1. Scenario 1: Native In-Memory 50-Point Benchmark Matrix (US1 & US2)

### Command
```bash
swift test --filter TTZipCoreCodecBenchmarkTests
```

### Expected Output
```text
========================================================================================
⚡️ TTZip Deflate-Bench 50-Point In-Memory Unified Matrix
========================================================================================
[1/50] libdeflate  | text        | 128KB | L1  | Comp:  1850.2 MB/s | Decomp:  5210.4 MB/s | OK
[2/50] libdeflate  | text        | 128KB | L6  | Comp:   890.1 MB/s | Decomp:  5180.2 MB/s | OK
[3/50] libdeflate  | text        |   1MB | L1  | Comp:  1920.4 MB/s | Decomp:  5400.1 MB/s | OK
...
[50/50] snappy     | dna         |   1MB | Def | Comp:  2450.0 MB/s | Decomp:  6100.0 MB/s | OK
----------------------------------------------------------------------------------------
Summary: 50/50 Points PASSED | Total Time: 0.382s (< 1.0s) | Median CV: 0.89% (<= 1.50%)
```

### Failure Diagnostic
- **Issue: High CV > 1.5%**: Check for background heavy processes or thermal throttling.
- **Issue: Integrity Mismatch**: Decompressed bytes do not match input corpus; check buffer bounds or dictionary preconditioning.

---

## 2. Scenario 2: High-Speed Clean Test Suite Execution (< 3.5s) (US4)

### Command
```bash
swift test --filter PipeStreamingTests,ShellCompletionTests,ManPageGenerationTests,ArchiveFormatStandardTests,CLIPackagingTests,ArchiveInspectorViewTests,InteractiveTUITests,MediaPreviewAuditTests,QuickLookPreviewTests,GUILocalizationTests,AppStorePackageAuditTests,ArchiveStandardsComplianceTests,DifferentialOracleTests,ArchiveMutationFuzzTests,LibarchiveGoldenCorpusTests
```

### Expected Output
```text
Test Suite 'All tests' passed.
Executed 400+ tests, with 0 failures in 2.85 seconds.
```

### Failure Diagnostic
- **Issue: Test Run > 3.5s**: Inspect if any deleted benchmark file was resurrected or if disk I/O was introduced into regular unit test paths.
